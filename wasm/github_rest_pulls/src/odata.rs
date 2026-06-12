//! Outbound OData access for the REST handlers. Reads fetch entity
//! rows; mutations dispatch spec-defined actions
//! (`/tdata/<Set>('id')/Temper.Git.<Action>`) with the resolved
//! caller's principal mirrored in the headers so Cedar evaluates the
//! real caller, never a bridge identity.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

/// Page size for list scans.
pub(crate) const LIST_PAGE_SIZE: usize = 500;
/// Hard page cap: 4 pages * 500 rows = 2000 pull requests per
/// repository. A repo beyond that needs the paging follow-up recorded
/// in RFC-0004, not a silent partial answer — callers error past the
/// cap.
pub(crate) const LIST_MAX_PAGES: usize = 4;

/// One entity row, normalized across the two read shapes the OData
/// plane serves (`{fields: {...}}` envelope vs flat fields). The row
/// id rides along because pull requests are addressed by `Number` at
/// the REST surface but by entity id on the action surface.
pub(crate) struct EntityRow {
    pub id: String,
    pub status: String,
    pub fields: Value,
}

pub(crate) fn row_from_value(value: &Value) -> EntityRow {
    let status = value
        .get("status")
        .or_else(|| value.get("Status"))
        .or_else(|| value.get("fields").and_then(|f| f.get("Status")))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let fields = value
        .get("fields")
        .cloned()
        .unwrap_or_else(|| value.clone());
    let id = value
        .get("entity_id")
        .or_else(|| value.get("id"))
        .or_else(|| value.get("Id"))
        .or_else(|| fields.get("Id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    EntityRow { id, status, fields }
}

/// GET a single row. `Ok(None)` is "row does not exist", which the
/// REST layer maps to GitHub's 404 envelope.
pub(crate) fn get_entity(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    set: &str,
    id: &str,
) -> Result<Option<EntityRow>, String> {
    let url = format!(
        "{}/tdata/{set}('{}')",
        api_base.trim_end_matches('/'),
        odata_escape_id(id)
    );
    let resp = ctx
        .http_call("GET", &url, headers, "")
        .map_err(|e| format!("GET {set}('{id}'): {e}"))?;
    if resp.status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&resp.status) {
        return Err(format!("GET {set}('{id}') status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("{set} row parse: {e}"))?;
    Ok(Some(row_from_value(&parsed)))
}

/// Bounded paged list with an OData `$filter`. Errors loudly when the
/// page cap is hit so a truncated projection never masquerades as the
/// full answer.
pub(crate) fn list_entities(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    set: &str,
    filter: &str,
) -> Result<Vec<EntityRow>, String> {
    let mut rows: Vec<EntityRow> = Vec::new();
    for page in 0..LIST_MAX_PAGES {
        let url = format!(
            "{}/tdata/{set}?$filter={}&$top={}&$skip={}",
            api_base.trim_end_matches('/'),
            urlencode(filter),
            LIST_PAGE_SIZE,
            page * LIST_PAGE_SIZE,
        );
        let resp = ctx
            .http_call("GET", &url, headers, "")
            .map_err(|e| format!("list {set}: {e}"))?;
        if !(200..300).contains(&resp.status) {
            return Err(format!("list {set} status {}", resp.status));
        }
        let parsed: Value =
            serde_json::from_str(&resp.body).map_err(|e| format!("{set} list parse: {e}"))?;
        let items = parsed
            .get("value")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let count = items.len();
        for item in &items {
            rows.push(row_from_value(item));
        }
        if count < LIST_PAGE_SIZE {
            return Ok(rows);
        }
    }
    Err(format!(
        "list {set}: more than {} rows match `{filter}`",
        LIST_MAX_PAGES * LIST_PAGE_SIZE
    ))
}

/// Outcome of a bound-action dispatch. Kept as raw status + body so
/// each endpoint maps kernel outcomes (403 denial, 409 ActionFailed)
/// onto its GitHub-documented status code.
pub(crate) struct ActionOutcome {
    pub status: u16,
    pub body: Value,
}

impl ActionOutcome {
    pub(crate) fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Kernel error message, wherever the OData error envelope put it.
    pub(crate) fn error_message(&self) -> String {
        self.body
            .get("error")
            .and_then(|e| e.get("message"))
            .or_else(|| self.body.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("action dispatch failed")
            .to_string()
    }
}

pub(crate) fn post_action(
    ctx: &Context,
    headers: &[(String, String)],
    api_base: &str,
    set: &str,
    id: &str,
    action: &str,
    params: &Value,
    await_integration: bool,
) -> Result<ActionOutcome, String> {
    let suffix = if await_integration {
        "?await_integration=true"
    } else {
        ""
    };
    let url = format!(
        "{}/tdata/{set}('{}')/Temper.Git.{action}{suffix}",
        api_base.trim_end_matches('/'),
        odata_escape_id(id)
    );
    let body = serde_json::to_string(params).map_err(|e| format!("params serialize: {e}"))?;
    let resp = ctx
        .http_call("POST", &url, headers, &body)
        .map_err(|e| format!("POST {set}.{action}: {e}"))?;
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(Value::Null);
    Ok(ActionOutcome {
        status: resp.status,
        body: parsed,
    })
}

pub(crate) fn odata_escape_id(id: &str) -> String {
    id.replace('\'', "''")
}

pub(crate) fn urlencode(value: &str) -> String {
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
