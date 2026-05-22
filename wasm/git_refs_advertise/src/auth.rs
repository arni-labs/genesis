//! GitToken-backed principal resolution for inbound git wire calls.
//!
//! A real `git push` or `git clone` arrives with an `Authorization`
//! header. The resolver hashes the token, looks up a GitToken row via
//! OData, and mirrors the originating caller onto downstream internal
//! calls. Anonymous is returned as a distinct value; callers decide
//! whether development fallback is allowed.

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use sha2::Digest;
use temper_wasm_sdk::prelude::*;

use crate::{SYSTEM_PRINCIPAL, SYSTEM_TENANT, TEMPER_API};

#[derive(Debug, Clone)]
pub struct Principal {
    pub kind: String,
    pub id: String,
    #[allow(dead_code)]
    pub scopes: Vec<String>,
}

impl Principal {
    pub fn anonymous() -> Self {
        Self {
            kind: "anonymous".to_string(),
            id: "anonymous".to_string(),
            scopes: Vec::new(),
        }
    }

    pub fn system() -> Self {
        Self {
            kind: "admin".to_string(),
            id: SYSTEM_PRINCIPAL.to_string(),
            scopes: Vec::new(),
        }
    }

    pub fn is_anonymous(&self) -> bool {
        self.kind == "anonymous"
    }

    pub fn outbound_headers(&self) -> Vec<(String, String)> {
        let mut headers = alloc::vec![
            ("X-Tenant-Id".to_string(), SYSTEM_TENANT.to_string()),
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
}

pub fn resolve_principal(ctx: &Context, headers: &[(String, String)]) -> Principal {
    let Some(token) = extract_token(headers) else {
        return Principal::anonymous();
    };
    let hash = sha256_hex(token.as_bytes());
    let url = format!("{TEMPER_API}/tdata/GitTokens?$filter=HashedSecret%20eq%20'{hash}'&$top=1");
    let lookup_headers = Principal::system().outbound_headers();
    let resp = match ctx.http_call("GET", &url, &lookup_headers, "") {
        Ok(r) => r,
        Err(_) => return Principal::anonymous(),
    };
    if !(200..400).contains(&resp.status) {
        return Principal::anonymous();
    }
    let parsed: serde_json::Value = match serde_json::from_str(&resp.body) {
        Ok(v) => v,
        Err(_) => return Principal::anonymous(),
    };
    let row = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first());
    let Some(row) = row else {
        return Principal::anonymous();
    };
    let fields = row.get("fields").unwrap_or(row);
    if fields.get("Status").and_then(|v| v.as_str()) != Some("Active") {
        return Principal::anonymous();
    }
    let id = fields
        .get("PrincipalId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let scopes = parse_scopes(fields.get("Scopes"));
    Principal {
        kind: "customer".to_string(),
        id,
        scopes,
    }
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

    #[test]
    fn extracts_bearer() {
        let headers = alloc::vec![("authorization".to_string(), "Bearer abc123".to_string())];
        assert_eq!(extract_token(&headers).unwrap(), "abc123");
    }

    #[test]
    fn extracts_basic_username() {
        let basic = B64.encode("ghp_secret:x-oauth-basic");
        let headers = alloc::vec![("Authorization".to_string(), format!("Basic {basic}"))];
        assert_eq!(extract_token(&headers).unwrap(), "ghp_secret");
    }
}
