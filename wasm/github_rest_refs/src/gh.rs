//! Pure serializers: Genesis Ref rows → GitHub REST v3 branch and
//! git-ref objects. No I/O, so the shape tests (hard rule 7) run
//! host-side against checked-in github.com fixtures.

use alloc::format;
use alloc::string::String;

use serde_json::Value;
use sha2::Digest;
use temper_wasm_sdk::json;

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

/// Branch list item (GET /repos/{o}/{r}/branches).
pub(crate) fn branch_json(
    owner: &str,
    repo: &str,
    full_ref_name: &str,
    sha: &str,
    public_base: &str,
) -> Value {
    let name = full_ref_name
        .strip_prefix("refs/heads/")
        .unwrap_or(full_ref_name);
    json!({
        "name": name,
        "commit": {
            "sha": sha,
            "url": format!("{public_base}/api/v3/repos/{owner}/{repo}/commits/{sha}"),
        },
        // Branch protection lives in Cedar policy, not on the row; the
        // REST projection reports unprotected until a policy
        // projection exists (RFC-0004 follow-up).
        "protected": false,
    })
}

/// Git reference object (GET/POST/PATCH /repos/{o}/{r}/git/refs...).
pub(crate) fn git_ref_json(
    owner: &str,
    repo: &str,
    full_ref_name: &str,
    sha: &str,
    public_base: &str,
) -> Value {
    let short = full_ref_name.strip_prefix("refs/").unwrap_or(full_ref_name);
    json!({
        "ref": full_ref_name,
        "node_id": node_id("REF", &format!("{owner}/{repo}/{full_ref_name}")),
        "url": format!("{public_base}/api/v3/repos/{owner}/{repo}/git/refs/{short}"),
        "object": {
            "sha": sha,
            "type": "commit",
            "url": format!("{public_base}/api/v3/repos/{owner}/{repo}/git/commits/{sha}"),
        },
    })
}

/// Ref row id convention shared with `scm_ingest_pack`.
pub(crate) fn ref_entity_id(repository_id: &str, full_ref_name: &str) -> String {
    format!("rf-{}-{}", repository_id, full_ref_name.replace('/', "-"))
}

pub(crate) fn ref_kind(full_ref_name: &str) -> &'static str {
    if full_ref_name.starts_with("refs/tags/") {
        "tag"
    } else {
        "branch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branch_json_strips_heads_prefix() {
        let branch = branch_json(
            "octo",
            "hello",
            "refs/heads/feature/x",
            "aa218f56b14c9653891f9e74264a383fa43fefbd",
            "https://genesis.test",
        );
        assert_eq!(branch["name"], "feature/x");
        assert_eq!(
            branch["commit"]["url"],
            "https://genesis.test/api/v3/repos/octo/hello/commits/aa218f56b14c9653891f9e74264a383fa43fefbd"
        );
    }

    #[test]
    fn git_ref_json_keeps_full_ref_name() {
        let r = git_ref_json("octo", "hello", "refs/tags/v1.0.0", "abc123", "https://g.test");
        assert_eq!(r["ref"], "refs/tags/v1.0.0");
        assert_eq!(r["object"]["type"], "commit");
        assert_eq!(
            r["url"],
            "https://g.test/api/v3/repos/octo/hello/git/refs/tags/v1.0.0"
        );
    }

    #[test]
    fn ref_entity_id_matches_ingest_pack_convention() {
        assert_eq!(
            ref_entity_id("rp-octo-hello", "refs/heads/main"),
            "rf-rp-octo-hello-refs-heads-main"
        );
    }

    #[test]
    fn ref_kind_distinguishes_tags() {
        assert_eq!(ref_kind("refs/tags/v1"), "tag");
        assert_eq!(ref_kind("refs/heads/main"), "branch");
    }
}
