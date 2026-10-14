//! github_rest_repos — GitHub REST v3 repository endpoints
//! (RFC-0004 Slice 2).
//!
//! Streaming direct-response WASM integration (same dispatch shape as
//! `git_refs_advertise`):
//!   * `POST /api/v3/user/repos`          — create + provision a Repository
//!     under the caller's own account
//!   * `POST /api/v3/orgs/{org}/repos`    — the same under another owner
//!     (`admin:repos`, or the caller's own account)
//!   * `GET  /api/v3/repos/{owner}/{repo}` — repository projection
//!
//! Reads fetch entity rows via outbound OData GET. The create path is
//! authorized HERE, against the resolved GitToken's scopes: Temper runs
//! an HttpEndpoint guest's internal calls as the guest itself and strips
//! any header that claims a caller's identity or scopes (ARN-208/255), so
//! Cedar never sees the token principal on those calls. The module then
//! dispatches `Repository.Create` and `MarkProvisioned` under its own
//! module identity, which `policies/repository.cedar` permits for this
//! module alone (the provisioning workaround while the
//! `repository_provision` integration remains unimplemented; RFC-0002
//! Slice C closes here at the API surface).

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
            ("POST", Route::UserRepos) => create_repository(&ctx, &http, None),
            ("POST", Route::OrgRepos { org }) => create_repository(&ctx, &http, Some(org)),
            ("GET", Route::Repo { owner, repo }) => get_repository(&ctx, &http, &owner, &repo),
            _ => http::respond_error(&http, 404, "Not Found"),
        }
    }
}

enum Route {
    UserRepos,
    OrgRepos { org: String },
    Repo { owner: String, repo: String },
    Unknown,
}

fn route_of(path: &str, http: &InboundHttp) -> Route {
    if path == "/api/v3/user/repos" {
        return Route::UserRepos;
    }
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segments.len() == 5 && segments[..3] == ["api", "v3", "orgs"] && segments[4] == "repos" {
        let org = http
            .params
            .get("org")
            .cloned()
            .unwrap_or_else(|| segments[3].to_string());
        return Route::OrgRepos { org };
    }
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

/// GitHub-safe account and repository names: they become part of the
/// row id (`rp-{owner}-{name}`) and of git URLs.
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// Who may create a repository under `owner`. The kernel cannot see the
/// token's scopes on this module's internal calls, so this is the check:
/// `admin:repos` creates under any owner; `repo:write` only under the
/// caller's own account. Mirrors `policies/repository.cedar`.
fn authorize_create(principal: &Principal, owner: &str) -> Result<(), &'static str> {
    if principal.has_scope("admin:repos") {
        return Ok(());
    }
    if owner == principal.id && principal.has_scope("repo:write") {
        return Ok(());
    }
    Err("Resource not accessible by personal access token")
}

fn create_repository(
    ctx: &Context,
    http: &InboundHttp,
    org: Option<String>,
) -> Result<Value, String> {
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

    let owner = org.unwrap_or_else(|| principal.id.clone());
    if !is_valid_name(name) || !is_valid_name(&owner) {
        return http::respond_error(http, 422, "Repository creation failed: invalid name");
    }
    if let Err(message) = authorize_create(&principal, &owner) {
        return http::respond_error(http, 403, message);
    }
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
        dispatch_create_and_provision(ctx, http, &owner, &api_base, &repository_id, &request)?
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

/// `Repository.Create`, then `MarkProvisioned`, both under this module's
/// own identity once `authorize_create` has passed (see the module doc).
/// Returns `Some(response)` when a step failed and was answered.
fn dispatch_create_and_provision(
    ctx: &Context,
    http: &InboundHttp,
    owner: &str,
    api_base: &str,
    repository_id: &str,
    request: &CreateRequest,
) -> Result<Option<Value>, String> {
    let create = odata::post_action(
        ctx,
        &system_headers(api_base),
        api_base,
        "Repositories",
        repository_id,
        "Create",
        &json!({
            "OwnerAccountId": owner,
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
    fn routes_org_repos_collection() {
        let http = inbound("POST", "/api/v3/orgs/katagami/repos");
        match route_of(http::strip_query(&http.path), &http) {
            Route::OrgRepos { org } => assert_eq!(org, "katagami"),
            _ => panic!("expected org repos route"),
        }
    }

    fn token(id: &str, scopes: &[&str]) -> Principal {
        let env = auth_env("http://127.0.0.1:3000");
        let mut p = Principal::anonymous(&env);
        p.kind = "customer".to_string();
        p.id = id.to_string();
        p.scopes = scopes.iter().map(|s| s.to_string()).collect();
        p
    }

    #[test]
    fn admin_repos_creates_under_any_owner() {
        let admin = token("paw-agent", &["repo:read", "repo:write", "admin:repos"]);
        assert!(authorize_create(&admin, "katagami").is_ok());
        assert!(authorize_create(&admin, "paw-agent").is_ok());
    }

    #[test]
    fn repo_write_creates_only_under_its_own_account() {
        let writer = token("alice", &["repo:read", "repo:write"]);
        assert!(authorize_create(&writer, "alice").is_ok());
        assert!(authorize_create(&writer, "katagami").is_err());
    }

    #[test]
    fn read_only_token_creates_nothing() {
        let reader = token("alice", &["repo:read"]);
        assert!(authorize_create(&reader, "alice").is_err());
        assert!(authorize_create(&token("alice", &[]), "alice").is_err());
    }

    #[test]
    fn names_that_would_bend_the_row_id_are_refused() {
        for bad in ["", ".hidden", "a/b", "a'b", "a b", "a%2Fb", "..", "x\\y"] {
            assert!(!is_valid_name(bad), "{bad:?}");
        }
        for good in ["katagami-control", "katagami_billing", "app.v2", "A1"] {
            assert!(is_valid_name(good), "{good:?}");
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
