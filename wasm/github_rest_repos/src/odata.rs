//! Outbound OData access for the REST handlers. Reads fetch entity
//! rows; mutations dispatch spec-defined actions
//! (`/tdata/<Set>('id')/Temper.Git.<Action>`) with the resolved
//! caller's principal mirrored in the headers so Cedar evaluates the
//! real caller, never a bridge identity.

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

/// One entity row, normalized across the two read shapes the OData
/// plane serves (`{fields: {...}}` envelope vs flat fields).
pub(crate) struct EntityRow {
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
    let fields = value.get("fields").cloned().unwrap_or_else(|| value.clone());
    EntityRow { status, fields }
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

#[allow(dead_code)]
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
