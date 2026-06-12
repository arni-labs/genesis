//! Inbound/outbound HTTP plumbing for the GitHub REST handlers:
//! request-body reading, GitHub-shaped JSON responses, and base-URL
//! derivation from the Host header.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::json;

/// REST request bodies are small JSON documents; anything above this
/// is a malformed or hostile request, not a legitimate payload.
pub(crate) const BODY_MAX_BYTES: usize = 1024 * 1024;
const BODY_CHUNK_BYTES: usize = 16 * 1024;

pub(crate) const DOCS_URL: &str = "https://docs.github.com/rest";

pub(crate) fn strip_query(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

/// Internal OData base. Mirrors `git_refs_advertise`: localhost hosts
/// speak plain http, anything else is fronted by TLS.
pub(crate) fn api_base_from_headers(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| {
            let host = v.trim();
            let scheme = if host.starts_with("localhost")
                || host.starts_with("127.0.0.1")
                || host.starts_with("0.0.0.0")
                || host.starts_with("[::1]")
            {
                "http"
            } else {
                "https"
            };
            format!("{scheme}://{host}")
        })
        .unwrap_or_else(|| crate::TEMPER_API.to_string())
}

/// Read the streamed request body to the end, bounded by
/// [`BODY_MAX_BYTES`], and parse it as JSON. An empty body parses as
/// an empty object so optional-body endpoints stay uniform.
pub(crate) fn read_body_json(http: &InboundHttp) -> Result<Value, String> {
    let mut reader = http.request_body();
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; BODY_CHUNK_BYTES];
    // Bounded: at most BODY_MAX_BYTES / chunk + 1 iterations.
    loop {
        if buf.len() > BODY_MAX_BYTES {
            return Err(format!("request body exceeds {BODY_MAX_BYTES} bytes"));
        }
        match reader.read_next_chunk(&mut chunk) {
            Ok(None) => break,
            Ok(Some(n)) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(format!("request body read: {e}")),
        }
    }
    if buf.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&buf).map_err(|e| format!("request body JSON parse: {e}"))
}

pub(crate) fn respond_json(http: &InboundHttp, status: u16, body: &Value) -> Result<Value, String> {
    let bytes = serde_json::to_vec(body).map_err(|e| format!("response serialize: {e}"))?;
    http.submit_response_head(
        status,
        &[("content-type", "application/json; charset=utf-8")],
    )
    .map_err(|e| format!("submit_response_head: {e}"))?;
    let mut writer = http.response_body();
    writer
        .write_all_chunk(&bytes)
        .map_err(|e| format!("response_body write: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("response_body close: {e}"))?;
    Ok(json!({ "status": status, "bytes_written": bytes.len() }))
}

/// GitHub error envelope: `{"message": ..., "documentation_url": ...}`.
pub(crate) fn respond_error(
    http: &InboundHttp,
    status: u16,
    message: &str,
) -> Result<Value, String> {
    respond_json(
        http,
        status,
        &json!({ "message": message, "documentation_url": DOCS_URL }),
    )
}

/// 401 challenge: same `WWW-Authenticate` contract as the git wire
/// handlers (ADR-0025) so `gh`/`git credential` flows can retry with a
/// GitToken, plus GitHub's documented error body.
pub(crate) fn respond_unauthorized(http: &InboundHttp) -> Result<Value, String> {
    let body = json!({
        "message": "Requires authentication",
        "documentation_url": DOCS_URL,
    });
    let bytes = serde_json::to_vec(&body).map_err(|e| format!("response serialize: {e}"))?;
    http.submit_response_head(
        401,
        &[
            ("content-type", "application/json; charset=utf-8"),
            ("WWW-Authenticate", "Basic realm=\"Genesis\""),
        ],
    )
    .map_err(|e| format!("submit_response_head: {e}"))?;
    let mut writer = http.response_body();
    writer
        .write_all_chunk(&bytes)
        .map_err(|e| format!("response_body write: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("response_body close: {e}"))?;
    Ok(json!({ "status": 401 }))
}
