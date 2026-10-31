//! Who may read a repository: its refs, objects and metadata.
//!
//! Anyone when the repository is public. Otherwise its owner, a token with
//! `admin:repos`, or a `repo:read` token whose `Repositories` list names it
//! (GitHub's "only select repositories"; a machine reader gets exactly the
//! repositories it reads and nothing else).
//!
//! The owner and admin arms are the Repository read permit in
//! `policies/repository.cedar`. Cedar cannot apply the rule to the git and REST
//! read surfaces, because the kernel runs an HttpEndpoint guest's internal calls
//! as the guest itself, never as the caller (temper `state/dispatch/wasm.rs`, "A
//! guest's internal calls run as the guest"). So every read module applies it
//! here, against the Repository row, before it serves anything.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use temper_wasm_sdk::prelude::*;

use crate::Principal;

/// What a read module does with a request for one repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadDecision {
    /// Serve it.
    Allow,
    /// No credential was presented and the repository is not public: answer
    /// 401 with a challenge, so git asks its credential helper and retries.
    Challenge,
    /// A credential was presented but it may not read this repository, or the
    /// repository does not exist: answer 404, as GitHub does, so a private
    /// repository's existence is not disclosed.
    NotFound,
}

/// Whether `principal` may read repository `repository_id`, whose row has
/// these fields.
///
/// A repository with no `Visibility` is private: the safe reading of a record
/// that never said it was public.
pub fn may_read_repository(
    principal: &Principal,
    repository_id: &str,
    fields: &serde_json::Value,
) -> bool {
    let visibility = fields.get("Visibility").and_then(|v| v.as_str());
    if visibility == Some("public") {
        return true;
    }
    if principal.is_anonymous() || principal.kind != "customer" {
        return false;
    }
    let owner = fields.get("OwnerAccountId").and_then(|v| v.as_str());
    let owns = owner.is_some_and(|owner| !owner.is_empty() && owner == principal.id);
    let listed = principal.has_scope("repo:read")
        && principal.repositories.iter().any(|r| r == repository_id);
    owns || principal.has_scope("admin:repos") || listed
}

/// The decision for a repository row (`None` when there is no such row).
pub fn decide_read(
    principal: &Principal,
    repository_id: &str,
    fields: Option<&serde_json::Value>,
) -> ReadDecision {
    match fields {
        Some(fields) if may_read_repository(principal, repository_id, fields) => {
            ReadDecision::Allow
        }
        _ if principal.is_anonymous() => ReadDecision::Challenge,
        _ => ReadDecision::NotFound,
    }
}

/// A GitToken's `Repositories` field: repository ids (`rp-owner-repo`) or
/// `owner/repo` names, separated by commas or whitespace.
pub(crate) fn parse_repositories(value: Option<&serde_json::Value>) -> Vec<String> {
    let items: Vec<String> = match value {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|s| s.as_str().map(str::to_string))
            .collect(),
        Some(serde_json::Value::String(s)) => s
            .split(|c: char| c == ',' || c.is_whitespace())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    };
    items
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| match s.split_once('/') {
            Some((owner, repo)) => format!("rp-{owner}-{repo}"),
            None => s.to_string(),
        })
        .collect()
}

/// Read the Repository row's fields as the calling module.
///
/// `Ok(None)` means the row does not exist. Any other failure is an error, so
/// the caller refuses rather than serving a repository it could not check.
pub fn fetch_repository_fields(
    ctx: &Context,
    api_base: &str,
    headers: &[(String, String)],
    repository_id: &str,
) -> Result<Option<serde_json::Value>, String> {
    let key = urlencode(&repository_id.replace('\'', "''"));
    let url = format!("{api_base}/tdata/Repositories('{key}')");
    let resp = ctx
        .http_call("GET", &url, headers, "")
        .map_err(|e| format!("repository read for access check: {e}"))?;
    if resp.status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "repository read for access check returned status {}",
            resp.status
        ));
    }
    let row: serde_json::Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("repository row parse for access check: {e}"))?;
    Ok(Some(row.get("fields").cloned().unwrap_or(row)))
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'\'' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV: crate::AuthEnv<'static> = crate::AuthEnv {
        temper_api: "http://127.0.0.1:3000",
        tenant: "default",
        system_principal: "test-module",
    };
    const RP: &str = "rp-octo-hello";

    fn token(id: &str, scopes: &[&str], repositories: &[&str]) -> Principal {
        Principal {
            kind: "customer".to_string(),
            id: id.to_string(),
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
            repositories: repositories.iter().map(|s| s.to_string()).collect(),
            token_id: Some("tk-1".to_string()),
            tenant: "default".to_string(),
        }
    }

    fn repo(visibility: Option<&str>, owner: &str) -> serde_json::Value {
        let mut fields = serde_json::json!({ "OwnerAccountId": owner });
        if let Some(v) = visibility {
            fields["Visibility"] = serde_json::json!(v);
        }
        fields
    }

    #[test]
    fn anyone_reads_a_public_repository() {
        let public = repo(Some("public"), "octo");
        let anon = Principal::anonymous(&ENV);
        assert_eq!(decide_read(&anon, RP, Some(&public)), ReadDecision::Allow);
        assert_eq!(decide_read(&token("other", &[], &[]), RP, Some(&public)), ReadDecision::Allow);
    }

    #[test]
    fn anonymous_is_challenged_on_private_missing_visibility_and_absent_rows() {
        let anon = Principal::anonymous(&ENV);
        let private = repo(Some("private"), "octo");
        assert_eq!(decide_read(&anon, RP, Some(&private)), ReadDecision::Challenge);
        assert_eq!(decide_read(&anon, RP, Some(&repo(None, ""))), ReadDecision::Challenge);
        assert_eq!(decide_read(&anon, RP, None), ReadDecision::Challenge);
    }

    #[test]
    fn the_owner_and_admin_repos_read_a_private_repository() {
        let private = repo(Some("private"), "octo");
        let owner = token("octo", &["repo:read"], &[]);
        let admin = token("ops", &["admin:repos"], &[]);
        assert_eq!(decide_read(&owner, RP, Some(&private)), ReadDecision::Allow);
        assert_eq!(decide_read(&admin, RP, Some(&private)), ReadDecision::Allow);
    }

    #[test]
    fn a_scoped_read_token_reads_only_the_repositories_it_names() {
        let private = repo(Some("private"), "octo");
        let ci = token("ci", &["repo:read"], &[RP]);
        assert_eq!(decide_read(&ci, RP, Some(&private)), ReadDecision::Allow);
        assert_eq!(decide_read(&ci, "rp-octo-other", Some(&private)), ReadDecision::NotFound);
        let unscoped_list = token("ci", &[], &[RP]);
        assert_eq!(decide_read(&unscoped_list, RP, Some(&private)), ReadDecision::NotFound);
    }

    #[test]
    fn another_token_holder_gets_not_found() {
        let private = repo(Some("private"), "octo");
        let other = token("other", &["repo:read", "repo:write"], &[]);
        assert_eq!(decide_read(&other, RP, Some(&private)), ReadDecision::NotFound);
        assert_eq!(decide_read(&other, RP, None), ReadDecision::NotFound);
    }

    #[test]
    fn an_empty_owner_matches_nobody() {
        assert!(!may_read_repository(&token("", &[], &[]), RP, &repo(None, "")));
    }

    #[test]
    fn the_module_system_identity_is_not_a_reader() {
        let system = Principal::system(&ENV);
        assert!(!may_read_repository(&system, RP, &repo(Some("private"), "test-module")));
    }

    #[test]
    fn repositories_field_accepts_ids_and_owner_repo_names() {
        let parsed = parse_repositories(Some(&serde_json::json!(
            "paw-agent/katagami-control, rp-temperpaw-paw-media\n"
        )));
        assert_eq!(parsed, ["rp-paw-agent-katagami-control", "rp-temperpaw-paw-media"]);
        assert!(parse_repositories(None).is_empty());
    }

    #[test]
    fn repository_key_is_quoted_and_encoded() {
        assert_eq!(urlencode(&"rp-o'hare x".replace('\'', "''")), "rp-o''hare%20x");
    }
}
