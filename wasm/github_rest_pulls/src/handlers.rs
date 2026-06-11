//! Read handlers and shared access plumbing for the pull-request REST
//! surface. Mutating handlers live in `mutations.rs`; routing in
//! `lib.rs`; serializers in `gh.rs`.

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
use crate::{Route, auth_env, query_param};

/// Repo row + the headers reads should use, after the anonymous
/// visibility gate (mirrors `git_refs_advertise`: anonymous reads run
/// as the system principal, private repos answer 404 to anonymous).
pub(crate) struct RepoAccess {
    pub headers: Vec<(String, String)>,
    pub repo_fields: Value,
    pub repo_status: String,
}

pub(crate) fn open_repo_access(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    api_base: &str,
) -> Result<Result<RepoAccess, Value>, String> {
    let env = auth_env();
    let resolved = genesis_git_auth::resolve_principal(ctx, &env, &http.headers);
    let anonymous = resolved.is_anonymous();
    let headers = if anonymous {
        Principal::system(&env).outbound_headers()
    } else {
        resolved.outbound_headers()
    };
    let Some(row) =
        odata::get_entity(ctx, &headers, api_base, "Repositories", &route.repository_id())?
    else {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    };
    let visibility = row
        .fields
        .get("Visibility")
        .and_then(Value::as_str)
        .unwrap_or("private");
    if anonymous && visibility != "public" {
        return Ok(Err(http::respond_error(http, 404, "Not Found")?));
    }
    Ok(Ok(RepoAccess {
        headers,
        repo_fields: row.fields,
        repo_status: row.status,
    }))
}

/// Mutation prologue: resolve the GitToken principal, reject anonymous
/// with the standard challenge, and record token usage.
pub(crate) fn resolve_mutating_principal(
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

pub(crate) fn full_ref_name(branch: &str, owner: &str) -> String {
    // GitHub accepts "feature", "owner:feature", and full ref names.
    let name = branch.strip_prefix(&format!("{owner}:")).unwrap_or(branch);
    if name.starts_with("refs/") {
        name.to_string()
    } else {
        format!("refs/heads/{name}")
    }
}

pub(crate) fn ref_tip(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    repository_id: &str,
    full_ref: &str,
) -> Result<Option<String>, String> {
    let ref_id = format!("rf-{}-{}", repository_id, full_ref.replace('/', "-"));
    let row = odata::get_entity(ctx, headers, api_base, "Refs", &ref_id)?;
    Ok(row.filter(|r| r.status == "Active").and_then(|r| {
        r.fields
            .get("TargetCommitSha")
            .and_then(Value::as_str)
            .map(str::to_string)
    }))
}

/// Head/base shas: PR rows learn `HeadCommitSha` from push-time
/// `UpdateHead`; freshly-opened PRs fall back to the live ref tips.
pub(crate) fn pull_shas(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    repository_id: &str,
    pr_fields: &Value,
) -> Result<(String, String), String> {
    let stored_head = pr_fields
        .get("HeadCommitSha")
        .and_then(Value::as_str)
        .unwrap_or("");
    let stored_base = pr_fields
        .get("BaseCommitSha")
        .and_then(Value::as_str)
        .unwrap_or("");
    let source = pr_fields.get("SourceRef").and_then(Value::as_str).unwrap_or("");
    let target = pr_fields.get("TargetRef").and_then(Value::as_str).unwrap_or("");
    let head = if stored_head.is_empty() {
        ref_tip(ctx, headers, api_base, repository_id, source)?.unwrap_or_default()
    } else {
        stored_head.to_string()
    };
    let base = if stored_base.is_empty() {
        ref_tip(ctx, headers, api_base, repository_id, target)?.unwrap_or_default()
    } else {
        stored_base.to_string()
    };
    Ok((head, base))
}

pub(crate) fn render_pull(
    ctx: &Context,
    access: &RepoAccess,
    route: &Route,
    api_base: &str,
    row: &odata::EntityRow,
) -> Result<Value, String> {
    let (head_sha, base_sha) = pull_shas(
        ctx,
        &access.headers,
        api_base,
        &route.repository_id(),
        &row.fields,
    )?;
    let render = gh::PullContext {
        owner: &route.owner,
        repo: &route.repo,
        repo_fields: &access.repo_fields,
        repo_status: &access.repo_status,
        public_base: api_base,
        head_sha: &head_sha,
        base_sha: &base_sha,
    };
    Ok(gh::pull_json(&render, &row.id, &row.status, &row.fields))
}

/// Re-read a PR row and answer it as a rendered pull object. Shared by
/// the mutation handlers, which must reflect post-action state.
pub(crate) fn respond_with_pull(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    api_base: &str,
    pr_id: &str,
    status_code: u16,
) -> Result<Value, String> {
    let access = match open_repo_access(ctx, http, route, api_base)? {
        Ok(a) => a,
        Err(resp) => return Ok(resp),
    };
    let Some(row) = odata::get_entity(ctx, &access.headers, api_base, "PullRequests", pr_id)?
    else {
        return http::respond_error(http, 500, "Pull request row vanished after action");
    };
    let row = odata::EntityRow {
        id: pr_id.to_string(),
        status: row.status,
        fields: row.fields,
    };
    let body = render_pull(ctx, &access, route, api_base, &row)?;
    http::respond_json(http, status_code, &body)
}

pub(crate) fn repo_pulls(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    repository_id: &str,
) -> Result<Vec<odata::EntityRow>, String> {
    let filter = format!(
        "RepositoryId eq '{}'",
        odata::odata_escape_id(repository_id)
    );
    odata::list_entities(ctx, headers, api_base, "PullRequests", &filter)
}

pub(crate) fn find_pull_by_number(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    repository_id: &str,
    number: i64,
) -> Result<Option<odata::EntityRow>, String> {
    let rows = repo_pulls(ctx, headers, api_base, repository_id)?;
    Ok(rows.into_iter().find(|row| {
        row.fields.get("Number").and_then(Value::as_i64) == Some(number)
    }))
}

pub(crate) fn list_pulls(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
) -> Result<Value, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let access = match open_repo_access(ctx, http, route, &api_base)? {
        Ok(a) => a,
        Err(resp) => return Ok(resp),
    };
    let state = query_param(http, "state").unwrap_or_else(|| "open".to_string());
    let mut rows = repo_pulls(ctx, &access.headers, &api_base, &route.repository_id())?;
    rows.retain(|row| match state.as_str() {
        "closed" => matches!(row.status.as_str(), "Merged" | "Closed"),
        "all" => true,
        _ => !matches!(row.status.as_str(), "Merged" | "Closed"),
    });
    // GitHub default sort is newest first; Number is our creation order.
    rows.sort_by_key(|row| {
        core::cmp::Reverse(row.fields.get("Number").and_then(Value::as_i64).unwrap_or(0))
    });
    let mut items = Vec::with_capacity(rows.len());
    for row in &rows {
        items.push(render_pull(ctx, &access, route, &api_base, row)?);
    }
    http::respond_json(http, 200, &Value::Array(items))
}

pub(crate) fn get_pull(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    number: i64,
) -> Result<Value, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let access = match open_repo_access(ctx, http, route, &api_base)? {
        Ok(a) => a,
        Err(resp) => return Ok(resp),
    };
    let row =
        find_pull_by_number(ctx, &access.headers, &api_base, &route.repository_id(), number)?;
    let Some(row) = row else {
        return http::respond_error(http, 404, "Not Found");
    };
    let body = render_pull(ctx, &access, route, &api_base, &row)?;
    http::respond_json(http, 200, &body)
}
