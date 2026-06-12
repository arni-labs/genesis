//! github_rest_repos — GitHub REST v3 repository endpoints
//! (RFC-0004 Slice 2).
//!
//! Streaming direct-response WASM integration (same dispatch shape as
//! `git_refs_advertise`):
//!   * `POST /api/v3/user/repos`          — create + provision a Repository
//!   * `GET  /api/v3/repos/{owner}/{repo}` — repository projection
//!
//! Reads fetch entity rows via outbound OData GET. The create path
//! dispatches the spec-defined `Repository.Create` action AS the
//! resolved GitToken principal (Cedar sees the real caller), then
//! advances Provisioning→Active via `MarkProvisioned` as the system
//! principal — the same provisioning workaround the live smokes use
//! while the `repository_provision` integration remains unimplemented
//! (RFC-0002 Slice C closes here at the API surface).

#![forbid(unsafe_code)]

extern crate alloc;

mod gh;
mod http;
mod odata;
#[cfg(test)]
mod shape;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_auth::{AuthEnv, Principal};
use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;

pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "github-rest-repos";

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let http_value = ctx
            .http_request
            .clone()
            .ok_or_else(|| "github_rest_repos requires HttpEndpoint dispatch".to_string())?;
        let http: InboundHttp = serde_json::from_value(http_value)
            .map_err(|e| format!("http_request parse error: {e}"))?;

        let path = http::strip_query(&http.path);
        match (http.method.as_str(), route_of(path, &http)) {
            ("POST", Route::UserRepos) => create_repository(&ctx, &http),
            ("GET", Route::Repo { owner, repo }) => get_repository(&ctx, &http, &owner, &repo),
            _ => http::respond_error(&http, 404, "Not Found"),
        }
    }
}

enum Route {
    UserRepos,
    Repo { owner: String, repo: String },
    Unknown,
}

fn route_of(path: &str, http: &InboundHttp) -> Route {
    if path == "/api/v3/user/repos" {
        return Route::UserRepos;
    }
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segments.len() == 5 && segments[..3] == ["api", "v3", "repos"] {
        let owner = http
            .params
            .get("owner")
            .cloned()
            .unwrap_or_else(|| segments[3].to_string());
        let repo = http
            .params
            .get("repo")
            .cloned()
            .unwrap_or_else(|| segments[4].to_string());
        return Route::Repo { owner, repo };
    }
    Route::Unknown
}

/// The token lookup must target the server actually handling this
/// request: the host-derived base, not a fixed port (a 3000-only
/// hardcode made token resolution silently degrade to anonymous on
/// any other port).
fn auth_env(api_base: &str) -> AuthEnv<'_> {
    AuthEnv {
        temper_api: api_base,
        tenant: SYSTEM_TENANT,
        system_principal: SYSTEM_PRINCIPAL,
    }
}

/// MarkProvisioned is spec-gated to the system role (Cedar:
/// `principal.agent_type == "system"`); it is the provisioning
/// callback, not a caller-visible action.
fn system_headers(api_base: &str) -> Vec<(String, String)> {
    let mut headers = Principal::system(&auth_env(api_base)).outbound_headers();
    headers.push(("X-Temper-Agent-Type".to_string(), "system".to_string()));
    headers
}

struct CreateRequest {
    name: String,
    description: String,
    visibility: String,
}

fn parse_create_request(body: &Value) -> Result<CreateRequest, &'static str> {
    let Some(name) = body
        .get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return Err("Repository creation failed: name is required");
    };
    let visibility = match body.get("visibility").and_then(Value::as_str) {
        Some(v) if !v.is_empty() => v.to_string(),
        _ if body
            .get("private")
            .and_then(Value::as_bool)
            .unwrap_or(false) =>
        {
            "private".to_string()
        }
        _ => "public".to_string(),
    };
    Ok(CreateRequest {
        name: name.to_string(),
        description: body
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        visibility,
    })
}

fn create_repository(ctx: &Context, http: &InboundHttp) -> Result<Value, String> {
    let api_base_owned = http::api_base_from_headers(&http.headers);
    let env = auth_env(&api_base_owned);
    let principal = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    if principal.is_anonymous() {
        return http::respond_unauthorized(http);
    }
    genesis_git_auth::mark_token_used(ctx, &env, &principal);

    let body = match http::read_body_json(http) {
        Ok(v) => v,
        Err(e) => return http::respond_error(http, 400, &format!("Problems parsing JSON: {e}")),
    };
    let request = match parse_create_request(&body) {
        Ok(r) => r,
        Err(message) => return http::respond_error(http, 422, message),
    };
    let name = &request.name;

    let owner = principal.id.clone();
    let repository_id = format!("rp-{owner}-{name}");
    let api_base = http::api_base_from_headers(&http.headers);

    // GitHub answers 422 when the name is taken on the account; check
    // before dispatching so a duplicate doesn't surface as a confusing
    // state-machine error.
    if odata::get_entity(
        ctx,
        &system_headers(&api_base),
        &api_base,
        "Repositories",
        &repository_id,
    )?
    .is_some()
    {
        return http::respond_error(http, 422, "name already exists on this account");
    }

    if let Some(error_response) =
        dispatch_create_and_provision(ctx, http, &principal, &api_base, &repository_id, &request)?
    {
        return Ok(error_response);
    }

    let Some(row) = odata::get_entity(
        ctx,
        &system_headers(&api_base),
        &api_base,
        "Repositories",
        &repository_id,
    )?
    else {
        return http::respond_error(http, 500, "Repository row vanished after creation");
    };
    let repo_json = gh::repository_json(&owner, name, &row.fields, &row.status, &api_base);
    http::respond_json(http, 201, &repo_json)
}

/// `Repository.Create` as the real caller (Cedar sees the token
/// principal), then `MarkProvisioned` as the system role — the same
/// provisioning workaround the live smokes use while the
/// `repository_provision` integration remains unimplemented.
/// Returns `Some(response)` when a step failed and was answered.
fn dispatch_create_and_provision(
    ctx: &Context,
    http: &InboundHttp,
    principal: &Principal,
    api_base: &str,
    repository_id: &str,
    request: &CreateRequest,
) -> Result<Option<Value>, String> {
    let create = odata::post_action(
        ctx,
        &principal.outbound_headers(),
        api_base,
        "Repositories",
        repository_id,
        "Create",
        &json!({
            "OwnerAccountId": principal.id,
            "Name": request.name,
            "Description": request.description,
            "DefaultBranch": "main",
            "Visibility": request.visibility,
        }),
        false,
    )?;
    if create.status == 403 {
        return Ok(Some(http::respond_error(
            http,
            403,
            &create.error_message(),
        )?));
    }
    if !create.ok() {
        return Ok(Some(http::respond_error(
            http,
            422,
            &format!("Repository creation failed: {}", create.error_message()),
        )?));
    }

    let provision = odata::post_action(
        ctx,
        &system_headers(api_base),
        api_base,
        "Repositories",
        repository_id,
        "MarkProvisioned",
        &json!({ "LibsqlDbName": format!("{repository_id}.db") }),
        false,
    )?;
    if !provision.ok() {
        return Ok(Some(http::respond_error(
            http,
            500,
            &format!(
                "Repository provisioning failed: {}",
                provision.error_message()
            ),
        )?));
    }
    Ok(None)
}

fn get_repository(
    ctx: &Context,
    http: &InboundHttp,
    owner: &str,
    repo: &str,
) -> Result<Value, String> {
    let api_base_owned = http::api_base_from_headers(&http.headers);
    let env = auth_env(&api_base_owned);
    let resolved = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    let anonymous = resolved.is_anonymous();
    // Anonymous reads mirror git_refs_advertise: the internal row read
    // runs as the system principal, then visibility gates the result.
    let read_headers = if anonymous {
        system_headers(&http::api_base_from_headers(&http.headers))
    } else {
        resolved.outbound_headers()
    };

    let repository_id = format!("rp-{owner}-{repo}");
    let api_base = http::api_base_from_headers(&http.headers);
    let row = match odata::get_entity(
        ctx,
        &read_headers,
        &api_base,
        "Repositories",
        &repository_id,
    ) {
        Ok(Some(row)) => row,
        // GitHub hides private repos and denials behind 404, never 403.
        Ok(None) => return http::respond_error(http, 404, "Not Found"),
        Err(_) if !anonymous => return http::respond_error(http, 404, "Not Found"),
        Err(e) => return Err(e),
    };
    let visibility = row
        .fields
        .get("Visibility")
        .and_then(Value::as_str)
        .unwrap_or("private");
    if anonymous && visibility != "public" {
        return http::respond_error(http, 404, "Not Found");
    }
    let repo_json = gh::repository_json(owner, repo, &row.fields, &row.status, &api_base);
    http::respond_json(http, 200, &repo_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    fn inbound(method: &str, path: &str) -> InboundHttp {
        InboundHttp {
            method: method.to_string(),
            path: path.to_string(),
            headers: Vec::new(),
            params: BTreeMap::new(),
            principal_id: None,
            request_body_handle: 0,
            response_body_handle: 0,
        }
    }

    #[test]
    fn routes_user_repos_collection() {
        let http = inbound("POST", "/api/v3/user/repos");
        assert!(matches!(
            route_of(http::strip_query(&http.path), &http),
            Route::UserRepos
        ));
    }

    #[test]
    fn routes_single_repo_from_path_segments() {
        let http = inbound("GET", "/api/v3/repos/octo/hello?per_page=1");
        match route_of(http::strip_query(&http.path), &http) {
            Route::Repo { owner, repo } => {
                assert_eq!(owner, "octo");
                assert_eq!(repo, "hello");
            }
            _ => panic!("expected repo route"),
        }
    }

    #[test]
    fn rejects_deeper_paths() {
        let http = inbound("GET", "/api/v3/repos/octo/hello/branches");
        assert!(matches!(
            route_of(http::strip_query(&http.path), &http),
            Route::Unknown
        ));
    }
}
