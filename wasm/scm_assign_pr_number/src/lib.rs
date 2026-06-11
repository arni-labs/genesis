//! scm_assign_pr_number — per-repo PullRequest number assignment.
//!
//! Declared by `specs/pull_request.ioa.toml` as the trigger integration
//! on `PullRequest.Open`. `Open` is an input action (not a Composite),
//! so this module follows the side-effect contract: it returns an empty
//! callback action (no implicit dispatch back onto the PullRequest) and
//! performs its writes through outbound OData calls.
//!
//! Number assignment: `max(Number) + 1` over all PullRequest rows with
//! the same `RepositoryId`, scanned with bounded paging. The PullRequest
//! spec deliberately has no action that carries `Number` (it is
//! integration-computed bookkeeping, like `Repository.LibsqlDbName`),
//! so the write is an OData field PATCH executed as the system
//! principal — the same pattern the live smokes use for
//! `MarkProvisioned`.
//!
//! Known limitation (documented, not hidden): two `Open` dispatches for
//! the same repository racing through different kernel workers can both
//! compute the same `max+1`. The per-repo Open rate in v1 makes this
//! window acceptable; a uniqueness invariant on (RepositoryId, Number)
//! is the proper fix and is tracked in RFC-0004 follow-ups.

// `deny` instead of the sibling modules' `forbid`: the hand-rolled
// entry point below needs `#[unsafe(no_mangle)]`, which Rust 2024
// counts as unsafe code. The single allow is scoped to that attribute;
// everything else stays denied.
#![deny(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use temper_wasm_sdk::prelude::*;
use temper_wasm_sdk::set_error_result;
use temper_wasm_sdk::set_success_result;

/// Page size for the PullRequest scan. Matches the `$top` ceiling the
/// read plane serves comfortably in one round trip.
const PAGE_SIZE: usize = 200;
/// Hard page cap: 50 pages * 200 rows = 10_000 PRs per repository,
/// far beyond any v1 repo. The loop never runs unbounded.
const MAX_PAGES: usize = 50;
const TEMPER_API: &str = "http://127.0.0.1:3000";
const SYSTEM_PRINCIPAL: &str = "scm-assign-pr-number";

/// Side-effect module entry: success returns an EMPTY callback action.
/// The default `temper_module!` macro returns the literal action
/// "callback", which the kernel would then dispatch against the
/// PullRequest transition table and fail — Open's trigger contract
/// wants data-only completion here.
#[allow(unsafe_code)] // the export attribute alone; no unsafe blocks
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<Value, String> {
        let ctx = Context::from_host().map_err(|e| e.to_string())?;
        assign_number(&ctx)
    })();
    match result {
        Ok(value) => set_success_result("", &value),
        Err(error) => set_error_result(&error),
    }
    0
}

fn assign_number(ctx: &Context) -> Result<Value, String> {
    if ctx.trigger_action != "Open" {
        return Err(format!(
            "scm_assign_pr_number: unsupported trigger action {}",
            ctx.trigger_action
        ));
    }
    let pull_request_id = ctx.entity_id.clone();
    let fields = entity_fields(&ctx.entity_state);

    // Idempotency: a retried Open dispatch must not renumber a PR that
    // already carries a positive Number.
    let existing = fields.get("Number").and_then(Value::as_i64).unwrap_or(0);
    if existing > 0 {
        return Ok(json!({
            "pull_request_id": pull_request_id,
            "number": existing,
            "skipped": "already-assigned",
        }));
    }

    let repository_id = fields
        .get("RepositoryId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "scm_assign_pr_number: PullRequest row has no RepositoryId".to_string())?
        .to_string();

    let api_base = api_base(ctx);
    let headers = system_headers(ctx);
    let (max_number, scanned) =
        max_pr_number_for_repo(ctx, &api_base, &headers, &repository_id)?;
    let next = max_number + 1;

    patch_number(ctx, &api_base, &headers, &pull_request_id, next)?;

    let _ = ctx.log_structured(
        "info",
        "scm_assign_pr_number_result",
        &json!({
            "pull_request_id": pull_request_id,
            "repository_id": repository_id,
            "number": next,
            "rows_scanned": scanned,
        }),
    );
    Ok(json!({
        "pull_request_id": pull_request_id,
        "number": next,
        "rows_scanned": scanned,
    }))
}

/// Bounded paged scan over /tdata/PullRequests for the repository,
/// returning (max Number seen, rows scanned).
fn max_pr_number_for_repo(
    ctx: &Context,
    api_base: &str,
    headers: &[(String, String)],
    repository_id: &str,
) -> Result<(i64, usize), String> {
    let filter = format!(
        "RepositoryId eq '{}'",
        repository_id.replace('\'', "''")
    );
    let mut max_number: i64 = 0;
    let mut scanned: usize = 0;
    for page in 0..MAX_PAGES {
        let url = format!(
            "{}/tdata/PullRequests?$filter={}&$top={}&$skip={}",
            api_base.trim_end_matches('/'),
            urlencode(&filter),
            PAGE_SIZE,
            page * PAGE_SIZE,
        );
        let resp = ctx
            .http_call("GET", &url, headers, "")
            .map_err(|e| format!("list pull requests: {e}"))?;
        if !(200..300).contains(&resp.status) {
            return Err(format!("list pull requests status {}", resp.status));
        }
        let parsed: Value = serde_json::from_str(&resp.body)
            .map_err(|e| format!("pull request list parse: {e}"))?;
        let rows = parsed
            .get("value")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let row_count = rows.len();
        for row in rows {
            let fields = row.get("fields").unwrap_or(&row);
            if let Some(number) = fields.get("Number").and_then(Value::as_i64) {
                max_number = max_number.max(number);
            }
        }
        scanned += row_count;
        if row_count < PAGE_SIZE {
            return Ok((max_number, scanned));
        }
    }
    // A repo at the page cap means our bound is wrong, not the data —
    // fail loudly instead of assigning a possibly-duplicate number.
    Err(format!(
        "scm_assign_pr_number: more than {} pull requests for {repository_id}",
        MAX_PAGES * PAGE_SIZE
    ))
}

fn patch_number(
    ctx: &Context,
    api_base: &str,
    headers: &[(String, String)],
    pull_request_id: &str,
    number: i64,
) -> Result<(), String> {
    // Dispatch the governed AssignNumber action (not a raw field
    // PATCH, which does not persist on spec-governed entities).
    let url = format!(
        "{}/tdata/PullRequests('{}')/Temper.Git.AssignNumber",
        api_base.trim_end_matches('/'),
        pull_request_id.replace('\'', "''")
    );
    let body = json!({ "Number": number }).to_string();
    let resp = ctx
        .http_call("POST", &url, headers, &body)
        .map_err(|e| format!("AssignNumber: {e}"))?;
    if (200..300).contains(&resp.status) {
        Ok(())
    } else {
        Err(format!(
            "patch Number on {pull_request_id} returned HTTP {}",
            resp.status
        ))
    }
}

fn entity_fields(entity_state: &Value) -> Value {
    entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn api_base(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| TEMPER_API.to_string())
}

/// Number bookkeeping runs as the system principal: the PullRequest
/// spec exposes no caller action for it, and the originating caller
/// (the PR author) must not need a scope that lets them renumber PRs.
fn system_headers(ctx: &Context) -> Vec<(String, String)> {
    alloc::vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-Tenant-Id".to_string(), ctx.tenant.clone()),
        ("X-Temper-Principal-Kind".to_string(), "admin".to_string()),
        (
            "X-Temper-Principal-Id".to_string(),
            SYSTEM_PRINCIPAL.to_string(),
        ),
        (
            "X-Temper-Principal-Scopes".to_string(),
            "admin:repos,pr:write".to_string(),
        ),
        ("X-Temper-Agent-Type".to_string(), "system".to_string()),
    ]
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencode_escapes_filter_expression() {
        assert_eq!(
            urlencode("RepositoryId eq 'rp-octo-hello'"),
            "RepositoryId%20eq%20%27rp-octo-hello%27"
        );
    }

    #[test]
    fn entity_fields_unwraps_fields_envelope() {
        let state = json!({ "status": "Open", "fields": { "Number": 3 } });
        assert_eq!(entity_fields(&state)["Number"], 3);
    }

    #[test]
    fn entity_fields_defaults_to_empty_object() {
        assert!(entity_fields(&json!("not-an-object")).as_object().is_some_and(|o| o.is_empty()));
    }
}
