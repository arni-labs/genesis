//! git_refs_advertise — smart-HTTP refs advertisement WASM integration.
//!
//! Handles `GET /{owner}/{repo}.git/info/refs?service=...` and emits
//! the pkt-line wrapped advertisement Git clients need before fetch
//! or push negotiation. Object graph walking and pack emission live in
//! later phase modules.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;
use tg_wire::{AdvertisedRef, Service, advertise_info_refs};

pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "git-refs-advertise";

mod auth;
pub(crate) use auth::Principal;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let http_value = ctx
            .http_request
            .clone()
            .ok_or_else(|| "git_refs_advertise requires HttpEndpoint dispatch (http_request missing)".to_string())?;
        let http: InboundHttp = serde_json::from_value(http_value)
            .map_err(|e| format!("http_request parse error: {e}"))?;

        let raw = http.path.as_str();
        let path = raw.split('?').next().unwrap_or(raw);
        if http.method == "GET" && path.ends_with("/info/refs") {
            return serve_info_refs(&ctx, &http);
        }
        respond_text(&http, 404, "text/plain", "no refs-advertise route matches")
    }
}

fn serve_info_refs(ctx: &Context, http: &InboundHttp) -> Result<Value, String> {
    let service = match query_param(http, "service").as_deref() {
        Some("git-upload-pack") | None => Service::UploadPack,
        Some("git-receive-pack") => Service::ReceivePack,
        Some(other) => {
            return respond_text(
                http,
                400,
                "text/plain",
                &format!("unknown service '{other}' on /info/refs"),
            );
        }
    };

    let (owner, repo) = repo_parts_from_http(http);
    let repository_id = format!("rp-{owner}-{repo}");

    let principal = effective_principal(ctx, &http.headers);
    let api_base = temper_api_from_headers(&http.headers);
    let refs_rows = fetch_refs_for_repo(ctx, &principal, &repository_id, &api_base)?;

    let mut owned: Vec<(String, String)> = refs_rows
        .into_iter()
        .filter(|row| row.status == "Active")
        .map(|row| (row.target_sha, row.name))
        .collect();
    let default_branch =
        fetch_repository_default_branch(ctx, &principal, &repository_id, &api_base)
            .unwrap_or_else(|_| "main".to_string());
    add_symbolic_head_advertisement(&mut owned, &default_branch);
    let refs: Vec<AdvertisedRef<'_>> = owned
        .iter()
        .map(|(sha, name)| AdvertisedRef {
            sha: sha.as_str(),
            name: name.as_str(),
        })
        .collect();

    let body =
        advertise_info_refs(service, &refs).map_err(|e| format!("advertise_info_refs: {e}"))?;

    http.submit_response_head(
        200,
        &[
            ("content-type", service.content_type()),
            ("cache-control", "no-cache"),
        ],
    )
    .map_err(|e| format!("submit_response_head: {e}"))?;

    let mut writer = http.response_body();
    writer
        .write_all_chunk(&body)
        .map_err(|e| format!("response_body write: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("response_body close: {e}"))?;

    Ok(json!({
        "bytes_written": body.len(),
        "ref_count": refs.len(),
        "repository_id": repository_id,
        "default_branch": default_branch,
    }))
}

struct RefRow {
    name: String,
    target_sha: String,
    status: String,
}

fn fetch_refs_for_repo(
    ctx: &Context,
    principal: &Principal,
    repository_id: &str,
    api_base: &str,
) -> Result<Vec<RefRow>, String> {
    let url = format!("{api_base}/tdata/Refs");
    let resp = ctx
        .http_call("GET", &url, &principal.outbound_headers(), "")
        .map_err(|e| format!("fetch refs: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("fetch refs status {}", resp.status));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("refs parse: {e}"))?;
    let items = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(items.len());
    for row in items {
        let fields = row
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let repo = fields
            .get("RepositoryId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if repo != repository_id {
            continue;
        }
        let name = fields
            .get("Name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let target_sha = fields
            .get("TargetCommitSha")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = fields
            .get("Status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() || target_sha.is_empty() {
            continue;
        }
        rows.push(RefRow {
            name,
            target_sha,
            status,
        });
    }
    rows.sort_by(|a, b| {
        let a_is_head = a.name == "HEAD";
        let b_is_head = b.name == "HEAD";
        match (a_is_head, b_is_head) {
            (true, false) => core::cmp::Ordering::Less,
            (false, true) => core::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });
    Ok(rows)
}

fn fetch_repository_default_branch(
    ctx: &Context,
    principal: &Principal,
    repository_id: &str,
    api_base: &str,
) -> Result<String, String> {
    let url = format!("{api_base}/tdata/Repositories('{repository_id}')");
    let resp = ctx
        .http_call("GET", &url, &principal.outbound_headers(), "")
        .map_err(|e| format!("fetch repository: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("fetch repository status {}", resp.status));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("repository parse: {e}"))?;
    let default_branch = parsed
        .get("fields")
        .and_then(|v| v.get("DefaultBranch"))
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .trim();
    if default_branch.is_empty() {
        Ok("main".to_string())
    } else {
        Ok(default_branch.to_string())
    }
}

fn add_symbolic_head_advertisement(refs: &mut Vec<(String, String)>, default_branch: &str) {
    if refs.iter().any(|(_, name)| name == "HEAD") {
        return;
    }
    let default_ref = if default_branch.starts_with("refs/") {
        default_branch.to_string()
    } else {
        format!("refs/heads/{default_branch}")
    };
    if let Some((sha, _)) = refs.iter().find(|(_, name)| name == &default_ref) {
        refs.insert(0, (sha.clone(), "HEAD".to_string()));
    }
}

fn effective_principal(ctx: &Context, headers: &[(String, String)]) -> Principal {
    let resolved = auth::resolve_principal(ctx, headers);
    if resolved.is_anonymous() {
        Principal::system()
    } else {
        resolved
    }
}

fn temper_api_from_headers(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| {
            let host = v.trim();
            let scheme = if host.starts_with("localhost")
                || host.starts_with("127.0.0.1")
                || host.starts_with("0.0.0.0")
                || host.starts_with("[::1]")
            {
                "http"
            } else {
                "https"
            };
            format!("{scheme}://{host}")
        })
        .unwrap_or_else(|| TEMPER_API.to_string())
}

fn respond_text(
    http: &InboundHttp,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<Value, String> {
    http.submit_response_head(status, &[("content-type", content_type)])
        .map_err(|e| format!("submit_response_head: {e}"))?;
    let mut writer = http.response_body();
    writer
        .write_all_chunk(body.as_bytes())
        .map_err(|e| format!("response_body write: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("response_body close: {e}"))?;
    Ok(json!({ "status": status }))
}

fn query_param(http: &InboundHttp, key: &str) -> Option<String> {
    let qs = http.path.split_once('?')?.1;
    for pair in qs.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        let v = it.next().unwrap_or("");
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

fn repo_parts_from_http(http: &InboundHttp) -> (String, String) {
    let owner = http.params.get("owner").cloned().unwrap_or_default();
    let repo = http.params.get("repo").cloned().unwrap_or_default();
    if !owner.is_empty() && !repo.is_empty() {
        return (owner, repo);
    }

    let raw = http.path.split('?').next().unwrap_or(http.path.as_str());
    let trimmed = raw.trim_start_matches('/');
    let Some((owner, rest)) = trimmed.split_once('/') else {
        return (owner, repo);
    };
    let Some(repo) = rest.strip_suffix(".git/info/refs") else {
        return (owner.to_string(), repo);
    };
    (owner.to_string(), repo.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    #[test]
    fn repo_parts_fall_back_to_literal_git_path() {
        let http = InboundHttp {
            method: "GET".to_string(),
            path: "/octo/hello.git/info/refs?service=git-upload-pack".to_string(),
            headers: Vec::new(),
            params: BTreeMap::new(),
            principal_id: None,
            request_body_handle: 0,
            response_body_handle: 0,
        };

        assert_eq!(
            repo_parts_from_http(&http),
            ("octo".to_string(), "hello".to_string())
        );
    }
}
