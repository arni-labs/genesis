//! Endpoint handlers for the branch / git-ref REST surface. Routing
//! lives in `lib.rs`; serializers in `gh.rs`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_auth::Principal;
use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;

use crate::gh;
use crate::http;
use crate::odata;
use crate::{Route, auth_env};

/// Anonymous-read gate shared by all GET handlers: the repo row decides
/// visibility (mirrors `git_refs_advertise` — internal reads fall back
/// to the system principal, private repos answer 404 to anonymous).
struct ReadAccess {
    headers: Vec<(String, String)>,
}

fn open_read_access(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    api_base: &str,
) -> Result<Result<ReadAccess, Value>, String> {
    let env = auth_env();
    let resolved = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    let anonymous = resolved.is_anonymous();
    let headers = if anonymous {
        Principal::system(&env).outbound_headers()
    } else {
        resolved.outbound_headers()
    };
    let Some(repo_row) =
        odata::get_entity(ctx, &headers, api_base, "Repositories", &route.repository_id())?
    else {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    };
    let visibility = repo_row
        .fields
        .get("Visibility")
        .and_then(Value::as_str)
        .unwrap_or("private");
    if anonymous && visibility != "public" {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    }
    Ok(Ok(ReadAccess { headers }))
}

/// Active Ref rows for the repository as (full ref name, target sha),
/// sorted by name for stable projections.
fn active_refs(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    repository_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let filter = format!(
        "RepositoryId eq '{}'",
        odata::odata_escape_id(repository_id)
    );
    let rows = odata::list_entities(ctx, headers, api_base, "Refs", &filter)?;
    let mut refs: Vec<(String, String)> = rows
        .into_iter()
        .filter(|row| row.status == "Active")
        .filter_map(|row| {
            let name = row.fields.get("Name").and_then(Value::as_str)?.to_string();
            let sha = row
                .fields
                .get("TargetCommitSha")
                .and_then(Value::as_str)?
                .to_string();
            (!name.is_empty() && !sha.is_empty()).then_some((name, sha))
        })
        .collect();
    refs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(refs)
}

pub(crate) fn list_branches(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
) -> Result<Value, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let access = match open_read_access(ctx, http, route, &api_base)? {
        Ok(access) => access,
        Err(response) => return Ok(response),
    };
    let refs = active_refs(ctx, &access.headers, &api_base, &route.repository_id())?;
    let branches: Vec<Value> = refs
        .iter()
        .filter(|(name, _)| name.starts_with("refs/heads/"))
        .map(|(name, sha)| gh::branch_json(&route.owner, &route.repo, name, sha, &api_base))
        .collect();
    http::respond_json(http, 200, &Value::Array(branches))
}

pub(crate) fn list_refs(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    short_prefix: Option<&str>,
) -> Result<Value, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let access = match open_read_access(ctx, http, route, &api_base)? {
        Ok(access) => access,
        Err(response) => return Ok(response),
    };
    let refs = active_refs(ctx, &access.headers, &api_base, &route.repository_id())?;
    let full_prefix = short_prefix.map(|p| format!("refs/{p}"));
    let items: Vec<Value> = refs
        .iter()
        .filter(|(name, _)| {
            full_prefix
                .as_deref()
                .is_none_or(|prefix| name.starts_with(prefix))
        })
        .map(|(name, sha)| gh::git_ref_json(&route.owner, &route.repo, name, sha, &api_base))
        .collect();
    http::respond_json(http, 200, &Value::Array(items))
}

pub(crate) fn get_single_ref(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    short_name: &str,
) -> Result<Value, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let access = match open_read_access(ctx, http, route, &api_base)? {
        Ok(access) => access,
        Err(response) => return Ok(response),
    };
    let full_name = format!("refs/{short_name}");
    let ref_id = gh::ref_entity_id(&route.repository_id(), &full_name);
    let row = odata::get_entity(ctx, &access.headers, &api_base, "Refs", &ref_id)?;
    let Some(row) = row.filter(|r| r.status == "Active") else {
        return http::respond_error(http, 404, "Not Found");
    };
    let sha = row
        .fields
        .get("TargetCommitSha")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let body = gh::git_ref_json(&route.owner, &route.repo, &full_name, sha, &api_base);
    http::respond_json(http, 200, &body)
}

/// Mutation prologue: resolve the GitToken principal, reject anonymous
/// with the standard challenge, and record token usage.
fn resolve_mutating_principal(
    ctx: &Context,
    http: &InboundHttp,
) -> Result<Result<Principal, Value>, String> {
    let env = auth_env();
    let principal = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    if principal.is_anonymous() {
        return Ok(Err(http::respond_unauthorized(http)?));
    }
    genesis_git_auth::mark_token_used(ctx, &env, &principal);
    Ok(Ok(principal))
}

pub(crate) fn create_ref(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
) -> Result<Value, String> {
    let principal = match resolve_mutating_principal(ctx, http)? {
        Ok(p) => p,
        Err(response) => return Ok(response),
    };
    let body = match http::read_body_json(http) {
        Ok(v) => v,
        Err(e) => return http::respond_error(http, 400, &format!("Problems parsing JSON: {e}")),
    };
    let full_name = body
        .get("ref")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let sha = body
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    // GitHub validates the fully-qualified two-slash form before
    // touching the repo.
    if !full_name.starts_with("refs/") || full_name.matches('/').count() < 2 || sha.is_empty() {
        return http::respond_error(http, 422, "Validation Failed");
    }

    let api_base = http::api_base_from_headers(&http.headers);
    let repository_id = route.repository_id();
    let ref_id = gh::ref_entity_id(&repository_id, &full_name);
    let existing =
        odata::get_entity(ctx, &principal.outbound_headers(), &api_base, "Refs", &ref_id)?;
    if existing.is_some_and(|row| row.status == "Active") {
        return http::respond_error(http, 422, "Reference already exists");
    }

    let outcome = odata::post_action(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        "Refs",
        &ref_id,
        "Create",
        &json!({
            "RepositoryId": repository_id,
            "Name": full_name,
            "TargetCommitSha": sha,
            "Kind": gh::ref_kind(&full_name),
        }),
        false,
    )?;
    if outcome.status == 403 {
        return http::respond_error(http, 403, &outcome.error_message());
    }
    if !outcome.ok() {
        return http::respond_error(http, 422, &outcome.error_message());
    }
    let body = gh::git_ref_json(&route.owner, &route.repo, &full_name, &sha, &api_base);
    http::respond_json(http, 201, &body)
}

struct UpdateRequest {
    sha: String,
    force: bool,
}

fn parse_update_request(body: &Value) -> Result<UpdateRequest, &'static str> {
    let sha = body
        .get("sha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if sha.is_empty() {
        return Err("Validation Failed");
    }
    Ok(UpdateRequest {
        sha,
        force: body.get("force").and_then(Value::as_bool).unwrap_or(false),
    })
}

pub(crate) fn update_ref(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    short_name: &str,
) -> Result<Value, String> {
    let principal = match resolve_mutating_principal(ctx, http)? {
        Ok(p) => p,
        Err(response) => return Ok(response),
    };
    let body = match http::read_body_json(http) {
        Ok(v) => v,
        Err(e) => return http::respond_error(http, 400, &format!("Problems parsing JSON: {e}")),
    };
    let request = match parse_update_request(&body) {
        Ok(r) => r,
        Err(message) => return http::respond_error(http, 422, message),
    };

    let api_base = http::api_base_from_headers(&http.headers);
    let full_name = format!("refs/{short_name}");
    let ref_id = gh::ref_entity_id(&route.repository_id(), &full_name);
    let row = odata::get_entity(ctx, &principal.outbound_headers(), &api_base, "Refs", &ref_id)?;
    let Some(row) = row.filter(|r| r.status == "Active") else {
        return http::respond_error(http, 422, "Reference does not exist");
    };
    let current_sha = row
        .fields
        .get("TargetCommitSha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    // See lib.rs module docs: Update carries the freshly-read tip so
    // the CAS precondition rejects racing writers; ForceUpdate (force
    // scope, Cedar-gated) skips the precondition entirely.
    let (action, params) = if request.force {
        ("ForceUpdate", json!({ "NewCommitSha": request.sha }))
    } else {
        (
            "Update",
            json!({ "PreviousCommitSha": current_sha, "NewCommitSha": request.sha }),
        )
    };
    let outcome = odata::post_action(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        "Refs",
        &ref_id,
        action,
        &params,
        false,
    )?;
    if outcome.status == 403 {
        return http::respond_error(http, 403, &outcome.error_message());
    }
    if !outcome.ok() {
        return http::respond_error(
            http,
            422,
            &format!("Update is not a fast forward: {}", outcome.error_message()),
        );
    }
    let body = gh::git_ref_json(&route.owner, &route.repo, &full_name, &request.sha, &api_base);
    http::respond_json(http, 200, &body)
}

pub(crate) fn delete_ref(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    short_name: &str,
) -> Result<Value, String> {
    let principal = match resolve_mutating_principal(ctx, http)? {
        Ok(p) => p,
        Err(response) => return Ok(response),
    };
    let api_base = http::api_base_from_headers(&http.headers);
    let full_name = format!("refs/{short_name}");
    let ref_id = gh::ref_entity_id(&route.repository_id(), &full_name);
    let row = odata::get_entity(ctx, &principal.outbound_headers(), &api_base, "Refs", &ref_id)?;
    if row.filter(|r| r.status == "Active").is_none() {
        return http::respond_error(http, 422, "Reference does not exist");
    }
    let outcome = odata::post_action(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        "Refs",
        &ref_id,
        "Delete",
        &json!({}),
        false,
    )?;
    if outcome.status == 403 {
        return http::respond_error(http, 403, &outcome.error_message());
    }
    if !outcome.ok() {
        return http::respond_error(http, 422, &outcome.error_message());
    }
    http::respond_no_content(http)
}
