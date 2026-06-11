//! GitToken-backed principal resolution shared by genesis protocol
//! WASM integrations (refs-advertise, upload-pack, receive-pack, and
//! the GitHub REST shims).
//!
//! A real `git push`/`git clone`/`gh api` call arrives with an
//! `Authorization` header — either `Bearer <token>` or HTTP Basic with
//! the token as the username. The resolver SHA-256s the token, looks
//! the GitToken row up via OData, and builds a `Principal` whose
//! downstream headers mirror the originating caller so internal OData
//! calls execute AS that principal and Cedar gates evaluate against
//! the real customer/agent rather than a system bridge.
//!
//! Anonymous (no `Authorization` header, or token not found / not
//! `Active`) is returned as a distinct value. The caller decides
//! whether to reject, challenge, or allow a development fallback.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use sha2::Digest;
use temper_wasm_sdk::prelude::*;

/// Deployment-specific identifiers each consuming module supplies.
#[derive(Debug, Clone, Copy)]
pub struct AuthEnv<'a> {
    /// Base URL for internal OData calls (kernel-local).
    pub temper_api: &'a str,
    /// Tenant the git surface operates in.
    pub tenant: &'a str,
    /// System principal id used for the token-lookup call itself.
    pub system_principal: &'a str,
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub kind: String,
    pub id: String,
    /// Scopes from the resolved GitToken row, enforced by Cedar gates
    /// downstream (e.g. `force` on `Ref.ForceUpdate`).
    pub scopes: Vec<String>,
    /// GitToken row id when this principal came from a token —
    /// callers fire `GitToken.MarkUsed` (best-effort) with it.
    pub token_id: Option<String>,
    tenant: String,
}

impl Principal {
    pub fn anonymous(env: &AuthEnv<'_>) -> Self {
        Self {
            kind: "anonymous".to_string(),
            id: "anonymous".to_string(),
            scopes: Vec::new(),
            token_id: None,
            tenant: env.tenant.to_string(),
        }
    }

    pub fn system(env: &AuthEnv<'_>) -> Self {
        Self {
            kind: "admin".to_string(),
            id: env.system_principal.to_string(),
            scopes: Vec::new(),
            token_id: None,
            tenant: env.tenant.to_string(),
        }
    }

    pub fn is_anonymous(&self) -> bool {
        self.kind == "anonymous"
    }

    pub fn has_scope(&self, want: &str) -> bool {
        self.scopes.iter().any(|s| s == want)
    }

    /// Headers attached to outbound internal OData calls so they
    /// execute AS this principal.
    pub fn outbound_headers(&self) -> Vec<(String, String)> {
        let mut headers = alloc::vec![
            ("X-Tenant-Id".to_string(), self.tenant.clone()),
            ("X-Temper-Principal-Kind".to_string(), self.kind.clone()),
            ("X-Temper-Principal-Id".to_string(), self.id.clone()),
            ("Content-Type".to_string(), "application/json".to_string()),
        ];
        if !self.scopes.is_empty() {
            headers.push((
                "X-Temper-Principal-Scopes".to_string(),
                self.scopes.join(","),
            ));
        }
        headers
    }

    /// `bridge_principal` payload for the kernel HttpEndpoint action
    /// bridge (temper ADR-0138): the dispatched entity action is
    /// Cedar-evaluated as this principal.
    pub fn bridge_principal_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": self.kind,
            "id": self.id,
            "scopes": self.scopes,
        })
    }
}

/// Resolve the inbound caller. Falls through to anonymous on missing
/// header, malformed Basic auth, OData lookup failure, expired or
/// revoked token. The caller decides what to do with anonymous.
pub fn resolve_principal(
    ctx: &Context,
    env: &AuthEnv<'_>,
    headers: &[(String, String)],
) -> Principal {
    let Some(token) = extract_token(headers) else {
        return Principal::anonymous(env);
    };
    let hash = sha256_hex(token.as_bytes());
    let url = format!(
        "{}/tdata/GitTokens?$filter=HashedSecret%20eq%20'{hash}'&$top=1",
        env.temper_api
    );
    let lookup_headers = Principal::system(env).outbound_headers();
    let resp = match ctx.http_call("GET", &url, &lookup_headers, "") {
        Ok(r) => r,
        Err(_) => return Principal::anonymous(env),
    };
    if !(200..400).contains(&resp.status) {
        return Principal::anonymous(env);
    }
    let parsed: serde_json::Value = match serde_json::from_str(&resp.body) {
        Ok(v) => v,
        Err(_) => return Principal::anonymous(env),
    };
    let row = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first());
    let Some(row) = row else {
        return Principal::anonymous(env);
    };
    let fields = row.get("fields").unwrap_or(row);
    if fields.get("Status").and_then(|v| v.as_str()) != Some("Active") {
        return Principal::anonymous(env);
    }
    let id = fields
        .get("PrincipalId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let token_id = row
        .get("entity_id")
        .or_else(|| row.get("id"))
        .or_else(|| fields.get("Id"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let scopes = parse_scopes(fields.get("Scopes"));
    Principal {
        kind: "customer".to_string(),
        id,
        scopes,
        token_id,
        tenant: env.tenant.to_string(),
    }
}

/// Fire `GitToken.MarkUsed` for a token-backed principal. Best-effort:
/// usage telemetry must never block the protocol operation.
pub fn mark_token_used(ctx: &Context, env: &AuthEnv<'_>, principal: &Principal) {
    let Some(token_id) = principal.token_id.as_deref() else {
        return;
    };
    let url = format!(
        "{}/tdata/GitTokens('{token_id}')/Temper.Git.MarkUsed",
        env.temper_api
    );
    let headers = Principal::system(env).outbound_headers();
    let _ = ctx.http_call("POST", &url, &headers, "{}");
}

fn parse_scopes(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|s| s.as_str().map(String::from))
            .collect(),
        Some(serde_json::Value::String(s)) => s
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

fn extract_token(headers: &[(String, String)]) -> Option<String> {
    for (k, v) in headers {
        if !k.eq_ignore_ascii_case("authorization") {
            continue;
        }
        if let Some(rest) = v.strip_prefix("Bearer ") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = v.strip_prefix("token ") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = v.strip_prefix("Basic ") {
            let decoded = B64.decode(rest.trim()).ok()?;
            let s = core::str::from_utf8(&decoded).ok()?;
            if let Some((user, _)) = s.split_once(':') {
                if !user.is_empty() {
                    return Some(user.to_string());
                }
            }
        }
    }
    None
}

fn sha256_hex(data: &[u8]) -> String {
    let digest = sha2::Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in digest.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV: AuthEnv<'static> = AuthEnv {
        temper_api: "http://127.0.0.1:3000",
        tenant: "default",
        system_principal: "test-module",
    };

    #[test]
    fn extracts_bearer() {
        let headers = alloc::vec![("authorization".to_string(), "Bearer abc123".to_string())];
        assert_eq!(extract_token(&headers).unwrap(), "abc123");
    }

    #[test]
    fn extracts_gh_token_scheme() {
        let headers = alloc::vec![("Authorization".to_string(), "token ghp_abc".to_string())];
        assert_eq!(extract_token(&headers).unwrap(), "ghp_abc");
    }

    #[test]
    fn extracts_basic_username() {
        // base64("ghp_secret:x-oauth-basic")
        let basic = B64.encode("ghp_secret:x-oauth-basic");
        let headers = alloc::vec![("Authorization".to_string(), format!("Basic {basic}"))];
        assert_eq!(extract_token(&headers).unwrap(), "ghp_secret");
    }

    #[test]
    fn missing_header_returns_none() {
        let headers = alloc::vec![("X-Other".to_string(), "x".to_string())];
        assert!(extract_token(&headers).is_none());
    }

    #[test]
    fn sha256_hex_is_64_chars() {
        assert_eq!(sha256_hex(b"hello").len(), 64);
        assert!(sha256_hex(b"hello").chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn parses_scope_string() {
        let scopes = parse_scopes(Some(&serde_json::json!("repo:read,repo:write force")));
        assert_eq!(scopes, ["repo:read", "repo:write", "force"]);
    }

    #[test]
    fn anonymous_has_no_token_and_no_scopes() {
        let p = Principal::anonymous(&ENV);
        assert!(p.is_anonymous());
        assert!(p.token_id.is_none());
        assert!(!p.has_scope("repo:push"));
    }

    #[test]
    fn bridge_principal_json_carries_kind_id_scopes() {
        let p = Principal {
            kind: "customer".to_string(),
            id: "user-1".to_string(),
            scopes: alloc::vec!["repo:write".to_string(), "force".to_string()],
            token_id: Some("tk-1".to_string()),
            tenant: "default".to_string(),
        };
        let v = p.bridge_principal_json();
        assert_eq!(v["kind"], "customer");
        assert_eq!(v["id"], "user-1");
        assert_eq!(v["scopes"][1], "force");
    }

    #[test]
    fn outbound_headers_mirror_principal() {
        let p = Principal {
            kind: "customer".to_string(),
            id: "user-1".to_string(),
            scopes: alloc::vec!["repo:read".to_string()],
            token_id: None,
            tenant: "default".to_string(),
        };
        let headers = p.outbound_headers();
        assert!(headers.contains(&("X-Temper-Principal-Id".to_string(), "user-1".to_string())));
        assert!(
            headers.contains(&(
                "X-Temper-Principal-Scopes".to_string(),
                "repo:read".to_string()
            ))
        );
    }
}
