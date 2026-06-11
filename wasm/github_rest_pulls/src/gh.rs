//! Pure serializers: Genesis PullRequest / Review rows → GitHub REST
//! v3 response JSON. No I/O, so the shape tests (hard rule 7) run
//! host-side against checked-in github.com fixtures.

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::Value;
use sha2::Digest;
use temper_wasm_sdk::json;

/// GitHub ids are integers; Genesis ids are strings. Derive a stable
/// positive 63-bit integer from the row id so the same row always
/// serializes the same id.
pub(crate) fn numeric_id(seed: &str) -> i64 {
    let digest = sha2::Sha256::digest(seed.as_bytes());
    let mut value: u64 = 0;
    for byte in digest.iter().take(8) {
        value = (value << 8) | u64::from(*byte);
    }
    (value & 0x7fff_ffff_ffff_ffff) as i64
}

/// GitHub node_ids are opaque strings; an opaque stable token keeps
/// the field present with the right type.
pub(crate) fn node_id(prefix: &str, seed: &str) -> String {
    let digest = sha2::Sha256::digest(seed.as_bytes());
    let mut hex = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("{prefix}_{hex}")
}

/// GitHub "simple user" object for an account login.
pub(crate) fn user_json(login: &str, public_base: &str) -> Value {
    json!({
        "login": login,
        "id": numeric_id(&format!("owner:{login}")),
        "node_id": node_id("U", login),
        "avatar_url": format!("{public_base}/avatars/{login}"),
        "gravatar_id": "",
        "url": format!("{public_base}/api/v3/users/{login}"),
        "html_url": format!("{public_base}/{login}"),
        "type": "User",
        "site_admin": false,
    })
}

/// Repository object nested under `head.repo` / `base.repo` —
/// same projection as `github_rest_repos`.
pub(crate) fn repository_json(
    owner: &str,
    name: &str,
    fields: &Value,
    status: &str,
    public_base: &str,
) -> Value {
    let full_name = format!("{owner}/{name}");
    let row_id = format!("rp-{owner}-{name}");
    let visibility = fields
        .get("Visibility")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("private");
    let default_branch = fields
        .get("DefaultBranch")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("main");
    json!({
        "id": numeric_id(&row_id),
        "node_id": node_id("R", &row_id),
        "name": name,
        "full_name": full_name,
        "private": visibility != "public",
        "owner": user_json(owner, public_base),
        "html_url": format!("{public_base}/{full_name}"),
        "description": fields.get("Description").cloned().unwrap_or(Value::Null),
        "fork": false,
        "url": format!("{public_base}/api/v3/repos/{full_name}"),
        "clone_url": format!("{public_base}/{full_name}.git"),
        "default_branch": default_branch,
        "visibility": visibility,
        "archived": status == "Archived",
    })
}

/// Everything a PullRequest projection needs besides the PR row.
pub(crate) struct PullContext<'a> {
    pub owner: &'a str,
    pub repo: &'a str,
    pub repo_fields: &'a Value,
    pub repo_status: &'a str,
    pub public_base: &'a str,
    /// Resolved tips: PR rows only learn `HeadCommitSha` on push
    /// (`UpdateHead`), so freshly-opened PRs resolve from Ref rows.
    pub head_sha: &'a str,
    pub base_sha: &'a str,
}

fn short_branch(full_ref: &str) -> &str {
    full_ref.strip_prefix("refs/heads/").unwrap_or(full_ref)
}

fn optional_string(fields: &Value, key: &str) -> Value {
    match fields.get(key).and_then(Value::as_str) {
        Some(s) if !s.is_empty() => Value::String(s.to_string()),
        _ => Value::Null,
    }
}

fn timestamp(fields: &Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("1970-01-01T00:00:00Z")
        .to_string()
}

/// PullRequest row → GitHub pull-request object (commonly-consumed
/// subset of docs.github.com/en/rest/pulls).
pub(crate) fn pull_json(ctx: &PullContext<'_>, pr_id: &str, pr_status: &str, pr: &Value) -> Value {
    let number = pr.get("Number").and_then(Value::as_i64).unwrap_or(0);
    let state = match pr_status {
        "Merged" | "Closed" => "closed",
        _ => "open",
    };
    let source_ref = pr.get("SourceRef").and_then(Value::as_str).unwrap_or("");
    let target_ref = pr.get("TargetRef").and_then(Value::as_str).unwrap_or("");
    let opened_by = pr.get("OpenedBy").and_then(Value::as_str).unwrap_or("");
    let repo_json = repository_json(
        ctx.owner,
        ctx.repo,
        ctx.repo_fields,
        ctx.repo_status,
        ctx.public_base,
    );
    let full_name = format!("{}/{}", ctx.owner, ctx.repo);
    let api_url = format!("{}/api/v3/repos/{full_name}/pulls/{number}", ctx.public_base);
    let html_url = format!("{}/{full_name}/pull/{number}", ctx.public_base);
    let merged_by = match pr.get("MergedBy").and_then(Value::as_str) {
        Some(login) if !login.is_empty() => user_json(login, ctx.public_base),
        _ => Value::Null,
    };
    json!({
        "url": api_url,
        "id": numeric_id(pr_id),
        "node_id": node_id("PR", pr_id),
        "html_url": html_url.clone(),
        "diff_url": format!("{html_url}.diff"),
        "patch_url": format!("{html_url}.patch"),
        "issue_url": format!("{}/api/v3/repos/{full_name}/issues/{number}", ctx.public_base),
        "number": number,
        "state": state,
        "locked": false,
        "title": pr.get("Title").and_then(Value::as_str).unwrap_or(""),
        "user": user_json(opened_by, ctx.public_base),
        "body": optional_string(pr, "Body"),
        "created_at": timestamp(pr, "OpenedAt"),
        "updated_at": timestamp(pr, "UpdatedAt"),
        "closed_at": optional_string(pr, "ClosedAt"),
        "merged_at": optional_string(pr, "MergedAt"),
        "merge_commit_sha": optional_string(pr, "MergedCommitSha"),
        // Genesis Draft is a pre-Open spec state, not GitHub's draft
        // flag; REST-created PRs are opened immediately (see lib.rs).
        "draft": pr_status == "Draft",
        "head": {
            "label": format!("{}:{}", ctx.owner, short_branch(source_ref)),
            "ref": short_branch(source_ref),
            "sha": ctx.head_sha,
            "user": user_json(ctx.owner, ctx.public_base),
            "repo": repo_json.clone(),
        },
        "base": {
            "label": format!("{}:{}", ctx.owner, short_branch(target_ref)),
            "ref": short_branch(target_ref),
            "sha": ctx.base_sha,
            "user": user_json(ctx.owner, ctx.public_base),
            "repo": repo_json,
        },
        "merged": pr_status == "Merged",
        // Mergeability is computed by the merge engine at merge time;
        // GitHub also serves null while unevaluated.
        "mergeable": Value::Null,
        "merged_by": merged_by,
    })
}

/// Review row → GitHub review object (POST .../reviews response).
pub(crate) fn review_json(
    owner: &str,
    repo: &str,
    pr_number: i64,
    review_id: &str,
    fields: &Value,
    commit_sha: &str,
    public_base: &str,
) -> Value {
    let state = match fields.get("Decision").and_then(Value::as_str) {
        Some("approved") => "APPROVED",
        Some("changes_requested") => "CHANGES_REQUESTED",
        _ => "COMMENTED",
    };
    let reviewer = fields
        .get("ReviewerPrincipal")
        .and_then(Value::as_str)
        .unwrap_or("");
    json!({
        "id": numeric_id(review_id),
        "node_id": node_id("PRR", review_id),
        "user": user_json(reviewer, public_base),
        "body": optional_string(fields, "Body"),
        "state": state,
        "html_url": format!(
            "{public_base}/{owner}/{repo}/pull/{pr_number}#pullrequestreview-{}",
            numeric_id(review_id)
        ),
        "pull_request_url": format!("{public_base}/api/v3/repos/{owner}/{repo}/pulls/{pr_number}"),
        "commit_id": commit_sha,
        "submitted_at": timestamp(fields, "SubmittedAt"),
        "author_association": "NONE",
    })
}

/// PUT .../merge success body.
pub(crate) fn merge_json(sha: &str) -> Value {
    json!({
        "sha": sha,
        "merged": true,
        "message": "Pull Request successfully merged",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context<'a>(repo_fields: &'a Value) -> PullContext<'a> {
        PullContext {
            owner: "octo",
            repo: "hello",
            repo_fields,
            repo_status: "Active",
            public_base: "https://genesis.test",
            head_sha: "6dcb09b5b57875f334f61aebed695e2e4193db5e",
            base_sha: "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc",
        }
    }

    #[test]
    fn pull_json_maps_open_state() {
        let repo_fields = json!({ "Visibility": "public", "DefaultBranch": "main" });
        let pr = json!({
            "Number": 7,
            "SourceRef": "refs/heads/feature",
            "TargetRef": "refs/heads/main",
            "Title": "Add feature",
            "OpenedBy": "octo",
        });
        let value = pull_json(&sample_context(&repo_fields), "pr-1", "Open", &pr);
        assert_eq!(value["state"], "open");
        assert_eq!(value["merged"], false);
        assert_eq!(value["number"], 7);
        assert_eq!(value["head"]["ref"], "feature");
        assert_eq!(value["base"]["label"], "octo:main");
    }

    #[test]
    fn pull_json_maps_merged_state() {
        let repo_fields = json!({ "Visibility": "public" });
        let pr = json!({
            "Number": 7,
            "SourceRef": "refs/heads/feature",
            "TargetRef": "refs/heads/main",
            "OpenedBy": "octo",
            "MergedCommitSha": "abc123",
            "MergedBy": "rita",
        });
        let value = pull_json(&sample_context(&repo_fields), "pr-1", "Merged", &pr);
        assert_eq!(value["state"], "closed");
        assert_eq!(value["merged"], true);
        assert_eq!(value["merge_commit_sha"], "abc123");
        assert_eq!(value["merged_by"]["login"], "rita");
    }

    #[test]
    fn review_json_maps_decisions_to_github_states() {
        let fields = json!({
            "ReviewerPrincipal": "rita",
            "Decision": "changes_requested",
            "Body": "needs work",
            "SubmittedAt": "2026-06-11T00:00:00Z",
        });
        let value = review_json("octo", "hello", 7, "rv-1", &fields, "abc", "https://g.test");
        assert_eq!(value["state"], "CHANGES_REQUESTED");
        assert_eq!(value["user"]["login"], "rita");
    }
}
