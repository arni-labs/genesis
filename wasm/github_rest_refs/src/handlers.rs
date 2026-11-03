//! Endpoint handlers for the branch / git-ref REST surface. Routing
//! lives in `lib.rs`; serializers in `gh.rs`.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_auth::{Principal, RepositoryWrite};
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
    let env = auth_env(api_base);
    let resolved = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    let anonymous = resolved.is_anonymous();
    let headers = if anonymous {
        Principal::system(&env).outbound_headers()
    } else {
        resolved.outbound_headers()
    };
    let Some(repo_row) = odata::get_entity(
        ctx,
        &headers,
        api_base,
        "Repositories",
        &route.repository_id(),
    )?
    else {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    };
    if !genesis_git_auth::may_read_repository(&resolved, &route.repository_id(), &repo_row.fields) {
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

/// A token-backed caller and the Repository row it is about to change.
struct Writer {
    principal: Principal,
    repository: Value,
}

/// Mutation prologue: resolve the GitToken principal, reject anonymous
/// with the standard challenge, record token usage, and load the
/// Repository row. A repository the caller may not read answers 404.
fn open_writer(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
) -> Result<Result<Writer, Value>, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let env = auth_env(&api_base);
    let principal = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    if principal.is_anonymous() {
        return Ok(Err(http::respond_unauthorized(http)?));
    }
    genesis_git_auth::mark_token_used(ctx, &env, &principal);
    let repository_id = route.repository_id();
    let system = Principal::system(&env).outbound_headers();
    let Some(row) = odata::get_entity(ctx, &system, &api_base, "Repositories", &repository_id)?
    else {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    };
    let mut repository = row.fields;
    repository["Status"] = Value::String(row.status);
    if !genesis_git_auth::may_read_repository(&principal, &repository_id, &repository) {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    }
    Ok(Ok(Writer {
        principal,
        repository,
    }))
}

/// Decide `write` on the caller's GitToken (crates/git_auth/src/write.rs).
/// `Some(response)` is the 403 to answer. The ref actions then run as this
/// module, which Cedar permits by name: the kernel runs a guest's calls as
/// the guest, so Cedar never sees the token.
fn refuse_unless(
    http: &InboundHttp,
    route: &Route,
    writer: &Writer,
    write: genesis_git_auth::RepositoryWrite,
) -> Result<Option<Value>, String> {
    let repository_id = route.repository_id();
    if genesis_git_auth::may_write_repository(
        &writer.principal,
        &repository_id,
        &writer.repository,
        write,
    ) {
        return Ok(None);
    }
    Ok(Some(http::respond_error(
        http,
        403,
        "Resource not accessible by this token",
    )?))
}

pub(crate) fn create_ref(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
) -> Result<Value, String> {
    let writer = match open_writer(ctx, http, route)? {
        Ok(w) => w,
        Err(response) => return Ok(response),
    };
    if let Some(refused) = refuse_unless(http, route, &writer, RepositoryWrite::Push)? {
        return Ok(refused);
    }
    let principal = writer.principal;
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
    let existing = odata::get_entity(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        "Refs",
        &ref_id,
    )?;
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
    let writer = match open_writer(ctx, http, route)? {
        Ok(w) => w,
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
    let write = if request.force {
        RepositoryWrite::ForcePush
    } else {
        RepositoryWrite::Push
    };
    if let Some(refused) = refuse_unless(http, route, &writer, write)? {
        return Ok(refused);
    }
    let principal = writer.principal;

    let api_base = http::api_base_from_headers(&http.headers);
    let full_name = format!("refs/{short_name}");
    let ref_id = gh::ref_entity_id(&route.repository_id(), &full_name);
    let row = odata::get_entity(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        "Refs",
        &ref_id,
    )?;
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
    // the CAS precondition rejects racing writers; ForceUpdate (decided
    // on the force scope above) skips the precondition entirely.
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
    let body = gh::git_ref_json(
        &route.owner,
        &route.repo,
        &full_name,
        &request.sha,
        &api_base,
    );
    http::respond_json(http, 200, &body)
}

pub(crate) fn delete_ref(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    short_name: &str,
) -> Result<Value, String> {
    let writer = match open_writer(ctx, http, route)? {
        Ok(w) => w,
        Err(response) => return Ok(response),
    };
    let full_name = format!("refs/{short_name}");
    let default_branch = writer
        .repository
        .get("DefaultBranch")
        .and_then(Value::as_str)
        .map(|b| format!("refs/heads/{b}"));
    let write = if default_branch.as_deref() == Some(full_name.as_str()) {
        RepositoryWrite::DeleteDefaultBranch
    } else {
        RepositoryWrite::Push
    };
    if let Some(refused) = refuse_unless(http, route, &writer, write)? {
        return Ok(refused);
    }
    let principal = writer.principal;
    let api_base = http::api_base_from_headers(&http.headers);
    let ref_id = gh::ref_entity_id(&route.repository_id(), &full_name);
    let row = odata::get_entity(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        "Refs",
        &ref_id,
    )?;
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
