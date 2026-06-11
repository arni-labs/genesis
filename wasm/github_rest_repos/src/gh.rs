//! Pure serializers: Genesis entity rows → GitHub REST v3 response
//! JSON. No I/O, so the shape tests (hard rule 7) run host-side
//! against checked-in github.com fixtures.

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

/// Repository row → GitHub repository object (commonly-consumed
/// subset, structurally faithful to docs.github.com/en/rest/repos).
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
    let description = fields.get("Description").cloned().unwrap_or(Value::Null);
    let default_branch = fields
        .get("DefaultBranch")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("main");
    let created_at = timestamp(fields, "CreatedAt");
    let updated_at = timestamp(fields, "UpdatedAt");
    json!({
        "id": numeric_id(&row_id),
        "node_id": node_id("R", &row_id),
        "name": name,
        "full_name": full_name,
        "private": visibility != "public",
        "owner": user_json(owner, public_base),
        "html_url": format!("{public_base}/{full_name}"),
        "description": description,
        "fork": false,
        "url": format!("{public_base}/api/v3/repos/{full_name}"),
        "git_url": format!("{public_base}/{full_name}.git"),
        "ssh_url": format!("{public_base}/{full_name}.git"),
        "clone_url": format!("{public_base}/{full_name}.git"),
        "default_branch": default_branch,
        "created_at": created_at,
        "updated_at": updated_at.clone(),
        // Genesis has no separate push clock; last update is the
        // closest faithful value with the right type.
        "pushed_at": updated_at,
        "archived": status == "Archived",
        "disabled": false,
        "visibility": visibility,
        "forks_count": 0,
        "stargazers_count": 0,
        "watchers_count": 0,
        "open_issues_count": 0,
        "size": 0,
        "language": Value::Null,
        "topics": [],
    })
}

fn timestamp(fields: &Value, key: &str) -> String {
    fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("1970-01-01T00:00:00Z")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_id_is_stable_and_positive() {
        assert_eq!(numeric_id("rp-octo-hello"), numeric_id("rp-octo-hello"));
        assert!(numeric_id("rp-octo-hello") > 0);
        assert_ne!(numeric_id("rp-octo-hello"), numeric_id("rp-octo-world"));
    }

    #[test]
    fn repository_json_maps_private_visibility() {
        let fields = json!({
            "Visibility": "private",
            "DefaultBranch": "main",
            "Description": "internal",
            "CreatedAt": "2026-06-11T00:00:00Z",
            "UpdatedAt": "2026-06-11T01:00:00Z",
        });
        let repo = repository_json("octo", "hello", &fields, "Active", "https://genesis.test");
        assert_eq!(repo["private"], true);
        assert_eq!(repo["visibility"], "private");
        assert_eq!(repo["full_name"], "octo/hello");
        assert_eq!(repo["clone_url"], "https://genesis.test/octo/hello.git");
        assert_eq!(repo["archived"], false);
    }

    #[test]
    fn repository_json_marks_archived_status() {
        let fields = json!({ "Visibility": "public" });
        let repo = repository_json("octo", "hello", &fields, "Archived", "https://genesis.test");
        assert_eq!(repo["archived"], true);
        assert_eq!(repo["private"], false);
    }
}
