//! Mutating handlers for the pull-request REST surface: create, close,
//! merge. Reviews live in `reviews.rs`. Every entity action is
//! dispatched with the resolved GitToken principal mirrored in the
//! headers, so Cedar evaluates the real caller.

use alloc::format;
use alloc::string::{String, ToString};

use genesis_git_auth::Principal;
use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;

use crate::gh;
use crate::handlers::{
    find_pull_by_number, full_ref_name, pull_shas, ref_tip, resolve_mutating_principal,
    respond_with_pull,
};
use crate::http;
use crate::odata;
use crate::{Route, fresh_row_id};

struct CreateRequest {
    title: String,
    body_text: String,
    source_ref: String,
    target_ref: String,
}

fn parse_create_request(body: &Value, owner: &str) -> Result<CreateRequest, &'static str> {
    let title = body.get("title").and_then(Value::as_str).unwrap_or("");
    let head = body.get("head").and_then(Value::as_str).unwrap_or("");
    let base = body.get("base").and_then(Value::as_str).unwrap_or("");
    if title.is_empty() || head.is_empty() || base.is_empty() {
        return Err("Validation Failed");
    }
    let source_ref = full_ref_name(head, owner);
    let target_ref = full_ref_name(base, owner);
    if source_ref == target_ref {
        return Err("Validation Failed");
    }
    Ok(CreateRequest {
        title: title.to_string(),
        body_text: body
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        source_ref,
        target_ref,
    })
}

pub(crate) fn create_pull(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
) -> Result<Value, String> {
    let principal = match resolve_mutating_principal(ctx, http)? {
        Ok(p) => p,
        Err(resp) => return Ok(resp),
    };
    let body = match http::read_body_json(http) {
        Ok(v) => v,
        Err(e) => return http::respond_error(http, 400, &format!("Problems parsing JSON: {e}")),
    };
    let request = match parse_create_request(&body, &route.owner) {
        Ok(r) => r,
        Err(message) => return http::respond_error(http, 422, message),
    };

    let api_base = http::api_base_from_headers(&http.headers);
    let repository_id = route.repository_id();
    let caller_headers = principal.outbound_headers();
    for full_ref in [&request.source_ref, &request.target_ref] {
        if ref_tip(ctx, &caller_headers, &api_base, &repository_id, full_ref)?.is_none() {
            return http::respond_error(http, 422, "Validation Failed");
        }
    }

    let seed = format!(
        "{repository_id}|{}|{}|{}",
        request.source_ref, request.target_ref, principal.id
    );
    let pr_id = fresh_row_id("pr", &seed, Context::get_time_millis());
    if let Some(error_response) = dispatch_create_and_open(
        ctx,
        http,
        &principal,
        &api_base,
        &repository_id,
        &pr_id,
        &request,
    )? {
        return Ok(error_response);
    }
    respond_with_pull(ctx, http, route, &api_base, &pr_id, 201)
}

/// `PullRequest.Create` then `Open`, both as the real caller. Open is
/// awaited so scm_assign_pr_number has stamped Number before the
/// response renders. GitHub's draft flag has no spec state (Genesis
/// Draft is pre-Open), so draft:true also opens. Returns
/// `Some(response)` when a step failed and was answered.
fn dispatch_create_and_open(
    ctx: &Context,
    http: &InboundHttp,
    principal: &Principal,
    api_base: &str,
    repository_id: &str,
    pr_id: &str,
    request: &CreateRequest,
) -> Result<Option<Value>, String> {
    let caller_headers = principal.outbound_headers();
    let create = odata::post_action(
        ctx,
        &caller_headers,
        api_base,
        "PullRequests",
        pr_id,
        "Create",
        &json!({
            "RepositoryId": repository_id,
            "SourceRef": request.source_ref,
            "TargetRef": request.target_ref,
            "Title": request.title,
            "Body": request.body_text,
            "OpenedBy": principal.id,
            "ClientRequestId": format!("github-rest-pulls:{pr_id}"),
        }),
        false,
    )?;
    if create.status == 403 {
        return Ok(Some(http::respond_error(http, 403, &create.error_message())?));
    }
    if !create.ok() {
        return Ok(Some(http::respond_error(http, 422, &create.error_message())?));
    }

    let open = odata::post_action(
        ctx,
        &caller_headers,
        api_base,
        "PullRequests",
        pr_id,
        "Open",
        &json!({}),
        true,
    )?;
    if !open.ok() {
        return Ok(Some(http::respond_error(
            http,
            422,
            &format!("Pull request could not be opened: {}", open.error_message()),
        )?));
    }
    Ok(None)
}

pub(crate) fn patch_pull(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    number: i64,
) -> Result<Value, String> {
    let principal = match resolve_mutating_principal(ctx, http)? {
        Ok(p) => p,
        Err(resp) => return Ok(resp),
    };
    let body = match http::read_body_json(http) {
        Ok(v) => v,
        Err(e) => return http::respond_error(http, 400, &format!("Problems parsing JSON: {e}")),
    };
    // The PullRequest spec has no title/body edit action (reported as
    // a spec gap in RFC-0004 follow-ups); refusing loudly beats
    // pretending the edit happened.
    if body.get("title").is_some() || body.get("body").is_some() {
        return http::respond_error(
            http,
            422,
            "Validation Failed: Genesis does not support pull request title/body edits yet",
        );
    }
    if body.get("state").and_then(Value::as_str) != Some("closed") {
        // Closed is terminal in the spec — reopen has no transition.
        return http::respond_error(
            http,
            422,
            "Validation Failed: only {\"state\":\"closed\"} is supported",
        );
    }

    let api_base = http::api_base_from_headers(&http.headers);
    let caller_headers = principal.outbound_headers();
    let row =
        find_pull_by_number(ctx, &caller_headers, &api_base, &route.repository_id(), number)?;
    let Some(row) = row else {
        return http::respond_error(http, 404, "Not Found");
    };
    let close = odata::post_action(
        ctx,
        &caller_headers,
        &api_base,
        "PullRequests",
        &row.id,
        "Close",
        &json!({ "Reason": "closed via REST" }),
        false,
    )?;
    if close.status == 403 {
        return http::respond_error(http, 403, &close.error_message());
    }
    if !close.ok() {
        return http::respond_error(http, 422, &close.error_message());
    }
    respond_with_pull(ctx, http, route, &api_base, &row.id, 200)
}

/// Map a failed `PullRequest.Merge` dispatch onto GitHub's documented
/// status codes: `merge-conflict:` (scm_merge_pr, ADR-0024) → 409;
/// everything else (not Approved, already merged, ...) → 405.
fn respond_merge_failure(
    http: &InboundHttp,
    outcome: &odata::ActionOutcome,
) -> Result<Value, String> {
    if outcome.status == 403 {
        return http::respond_error(http, 403, &outcome.error_message());
    }
    let error = outcome.error_message();
    if let Some(idx) = error.find("merge-conflict:") {
        let detail = error[idx + "merge-conflict:".len()..].trim();
        let message = if detail.is_empty() {
            "Merge conflict".to_string()
        } else {
            format!("Merge conflict: {detail}")
        };
        return http::respond_error(http, 409, &message);
    }
    http::respond_error(http, 405, &format!("Pull Request is not mergeable: {error}"))
}

pub(crate) fn merge_pull(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    number: i64,
) -> Result<Value, String> {
    let principal = match resolve_mutating_principal(ctx, http)? {
        Ok(p) => p,
        Err(resp) => return Ok(resp),
    };
    let body = match http::read_body_json(http) {
        Ok(v) => v,
        Err(e) => return http::respond_error(http, 400, &format!("Problems parsing JSON: {e}")),
    };
    let strategy = body
        .get("merge_method")
        .and_then(Value::as_str)
        .unwrap_or("merge")
        .to_string();
    if !matches!(strategy.as_str(), "merge" | "squash" | "rebase") {
        return http::respond_error(http, 422, "Validation Failed");
    }

    let api_base = http::api_base_from_headers(&http.headers);
    let caller_headers = principal.outbound_headers();
    let repository_id = route.repository_id();
    let row = find_pull_by_number(ctx, &caller_headers, &api_base, &repository_id, number)?;
    let Some(row) = row else {
        return http::respond_error(http, 404, "Not Found");
    };
    // GitHub's optimistic-concurrency guard: a caller-supplied sha
    // must still be the head.
    if let Some(expected) = body.get("sha").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        let (head_sha, _) =
            pull_shas(ctx, &caller_headers, &api_base, &repository_id, &row.fields)?;
        if expected != head_sha {
            return http::respond_error(
                http,
                409,
                "Head branch was modified. Review and try the merge again.",
            );
        }
    }

    let message = body
        .get("commit_message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let outcome = odata::post_action(
        ctx,
        &caller_headers,
        &api_base,
        "PullRequests",
        &row.id,
        "Merge",
        &json!({
            "Strategy": strategy,
            "Message": message,
            "ClientRequestId": format!("github-rest-pulls:merge:{}", row.id),
        }),
        true,
    )?;
    if !outcome.ok() {
        return respond_merge_failure(http, &outcome);
    }
    let sha = merged_result_sha(ctx, &caller_headers, &api_base, &repository_id, &row)?;
    http::respond_json(http, 200, &gh::merge_json(&sha))
}

/// Merge-result sha: the row's `MergedCommitSha` when the engine
/// minted a commit; otherwise (fast-forward advances the target ref
/// without a new commit) the target tip is the merge result.
fn merged_result_sha(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    repository_id: &str,
    row: &odata::EntityRow,
) -> Result<String, String> {
    let merged_sha = odata::get_entity(ctx, headers, api_base, "PullRequests", &row.id)?
        .and_then(|r| {
            r.fields
                .get("MergedCommitSha")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
    match merged_sha {
        Some(sha) => Ok(sha),
        None => {
            let target = row.fields.get("TargetRef").and_then(Value::as_str).unwrap_or("");
            Ok(ref_tip(ctx, headers, api_base, repository_id, target)?.unwrap_or_default())
        }
    }
}
