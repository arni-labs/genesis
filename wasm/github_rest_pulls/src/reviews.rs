//! POST .../pulls/{n}/reviews — `Review.Create` plus the matching
//! PullRequest verdict transition (`Approve` / `RequestChanges`).

use alloc::format;
use alloc::string::{String, ToString};

use genesis_git_auth::Principal;
use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;

use crate::gh;
use crate::handlers::{find_pull_by_number, pull_shas, resolve_mutating_principal};
use crate::http;
use crate::odata;
use crate::{Route, fresh_row_id, system_headers};

struct ReviewRequest {
    event: String,
    decision: &'static str,
    body_text: String,
}

fn parse_review_request(body: &Value) -> Result<ReviewRequest, &'static str> {
    // GitHub's missing-event default is a PENDING review; Genesis has
    // no pending state, so a bare body submits as COMMENT.
    let event = body
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("COMMENT")
        .to_string();
    let decision = match event.as_str() {
        "APPROVE" => "approved",
        "REQUEST_CHANGES" => "changes_requested",
        "COMMENT" => "commented",
        _ => return Err("Validation Failed"),
    };
    Ok(ReviewRequest {
        event,
        decision,
        body_text: body
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    })
}

pub(crate) fn create_review(
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
    let request = match parse_review_request(&body) {
        Ok(r) => r,
        Err(message) => return http::respond_error(http, 422, message),
    };

    let api_base = http::api_base_from_headers(&http.headers);
    let caller_headers = principal.outbound_headers();
    let row = find_pull_by_number(
        ctx,
        &caller_headers,
        &api_base,
        &route.repository_id(),
        number,
    )?;
    let Some(row) = row else {
        return http::respond_error(http, 404, "Not Found");
    };
    let opened_by = row
        .fields
        .get("OpenedBy")
        .and_then(Value::as_str)
        .unwrap_or("");
    if request.event != "COMMENT" && opened_by == principal.id {
        // Cedar forbids it too; answering in GitHub's words first.
        return http::respond_error(http, 422, "Can not approve your own pull request");
    }

    let review_id = fresh_row_id(
        "rv",
        &format!("{}|{}", row.id, principal.id),
        Context::get_time_millis(),
    );
    if let Some(response) = submit_review_row(
        ctx, http, &principal, &api_base, &review_id, &row.id, &request,
    )? {
        return Ok(response);
    }
    if let Some(response) =
        apply_review_transition(ctx, http, &api_base, &row, &principal, &request)?
    {
        return Ok(response);
    }
    respond_with_review(
        ctx, http, route, number, &row, &principal, &review_id, &request,
    )
}

/// `Review.Create` as the real caller. Returns `Some(response)` when
/// the dispatch failed and was answered.
fn submit_review_row(
    ctx: &Context,
    http: &InboundHttp,
    principal: &Principal,
    api_base: &str,
    review_id: &str,
    pull_request_id: &str,
    request: &ReviewRequest,
) -> Result<Option<Value>, String> {
    let create = odata::post_action(
        ctx,
        &principal.outbound_headers(),
        api_base,
        "Reviews",
        review_id,
        "Create",
        &json!({
            "PullRequestId": pull_request_id,
            "ReviewerPrincipal": principal.id,
            "Decision": request.decision,
            "Body": request.body_text,
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
            &create.error_message(),
        )?));
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn respond_with_review(
    ctx: &Context,
    http: &InboundHttp,
    route: &Route,
    number: i64,
    row: &odata::EntityRow,
    principal: &Principal,
    review_id: &str,
    request: &ReviewRequest,
) -> Result<Value, String> {
    let api_base = http::api_base_from_headers(&http.headers);
    let (head_sha, _) = pull_shas(
        ctx,
        &principal.outbound_headers(),
        &api_base,
        &route.repository_id(),
        &row.fields,
    )?;
    let fields = json!({
        "ReviewerPrincipal": principal.id,
        "Decision": request.decision,
        "Body": request.body_text,
    });
    let body = gh::review_json(
        &route.owner,
        &route.repo,
        number,
        review_id,
        &fields,
        &head_sha,
        &api_base,
    );
    http::respond_json(http, 200, &body)
}

/// Drive the PullRequest state machine to match the review verdict.
/// The spec interposes `UnderReview` between `Open` and any verdict,
/// and Cedar scopes `RequestReview` to the PR author — a reviewer
/// walking up with a verdict is exactly GitHub's flow, so the hop runs
/// as the system principal (the verdict itself stays caller-mirrored).
fn apply_review_transition(
    ctx: &Context,
    http: &InboundHttp,
    api_base: &str,
    row: &odata::EntityRow,
    principal: &Principal,
    request: &ReviewRequest,
) -> Result<Option<Value>, String> {
    let verdict_action = match request.event.as_str() {
        "APPROVE" => "Approve",
        "REQUEST_CHANGES" => "RequestChanges",
        _ => return Ok(None),
    };
    // Already at the verdict state: the Review row is recorded; the
    // state machine has nothing to move.
    if (request.event == "APPROVE" && row.status == "Approved")
        || (request.event == "REQUEST_CHANGES" && row.status == "ChangesRequested")
    {
        return Ok(None);
    }
    if row.status == "Open" {
        let hop = odata::post_action(
            ctx,
            &system_headers(api_base),
            api_base,
            "PullRequests",
            &row.id,
            "RequestReview",
            &json!({ "ReviewerPrincipal": principal.id }),
            false,
        )?;
        if !hop.ok() {
            return Ok(Some(http::respond_error(
                http,
                422,
                &format!("Review transition failed: {}", hop.error_message()),
            )?));
        }
    }
    let verdict = odata::post_action(
        ctx,
        &principal.outbound_headers(),
        api_base,
        "PullRequests",
        &row.id,
        verdict_action,
        &json!({ "ReviewerPrincipal": principal.id, "Body": request.body_text }),
        false,
    )?;
    if verdict.status == 403 {
        return Ok(Some(http::respond_error(
            http,
            403,
            &verdict.error_message(),
        )?));
    }
    if !verdict.ok() {
        return Ok(Some(http::respond_error(
            http,
            422,
            &format!("Review transition failed: {}", verdict.error_message()),
        )?));
    }
    Ok(None)
}
