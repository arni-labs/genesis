//! git_upload_pack — smart-HTTP upload-pack POST WASM integration.
//!
//! Handles `POST /{owner}/{repo}.git/git-upload-pack`: want/have
//! negotiation and pack-v2 emission. The preceding `/info/refs`
//! advertisement phase lives in `git_refs_advertise`.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use temper_wasm_sdk::http_stream::{HttpRequestBodyWriter, InboundHttp, streaming_call};
use temper_wasm_sdk::prelude::*;
use tg_wire::{ObjectKind, PackEmitter, SidebandWriter, encode_into, flush};

pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "git-upload-pack";
const FIELD_OVERFLOW_REF_KEY: &str = "__temper_blob_ref";
const FIELD_OVERFLOW_ENCODING_KEY: &str = "__temper_blob_encoding";

mod auth;
pub(crate) use auth::Principal;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let http_value = ctx
            .http_request
            .clone()
            .ok_or_else(|| "git_upload_pack requires HttpEndpoint dispatch (http_request missing)".to_string())?;
        let http: InboundHttp = serde_json::from_value(http_value)
            .map_err(|e| format!("http_request parse error: {e}"))?;

        let raw = http.path.as_str();
        let path = raw.split('?').next().unwrap_or(raw);

        if http.method == "POST" && path.ends_with("/git-upload-pack") {
            return serve_upload_pack(&ctx, &http);
        }
        respond_text(&http, 404, "text/plain", "no upload-pack route matches")
    }
}

/// Resolve the inbound caller and fall back to the system principal
/// if none is presented. Production deployments lock down via Cedar
/// to require a real GitToken; dev quickstarts work without one.
fn effective_principal(ctx: &Context, headers: &[(String, String)]) -> Principal {
    let resolved = auth::resolve_principal(ctx, headers);
    if resolved.is_anonymous() {
        Principal::system()
    } else {
        resolved
    }
}

fn temper_api_from_headers(headers: &[(String, String)]) -> String {
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
        .unwrap_or_else(|| TEMPER_API.to_string())
}

fn respond_text(
    http: &InboundHttp,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<Value, String> {
    http.submit_response_head(status, &[("content-type", content_type)])
        .map_err(|e| format!("submit_response_head: {e}"))?;
    let mut writer = http.response_body();
    writer
        .write_all_chunk(body.as_bytes())
        .map_err(|e| format!("response_body write: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("response_body close: {e}"))?;
    Ok(json!({ "status": status }))
}

const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const READ_CHUNK: usize = 16 * 1024;
const OUTBOUND_READ_CHUNK: usize = 64 * 1024;
const MAX_CACHED_OBJECT_BYTES: usize = 128 * 1024 * 1024;

fn serve_upload_pack(ctx: &Context, http: &InboundHttp) -> Result<Value, String> {
    let principal = effective_principal(ctx, &http.headers);
    let api_base = temper_api_from_headers(&http.headers);
    // 1. Read the request body. Bounded: want/have negotiation
    //    payloads are tiny (a few KB even for huge repos), so we
    //    cap at 16 MiB and buffer.
    let mut body = Vec::new();
    let mut scratch = alloc::vec![0u8; READ_CHUNK];
    let mut reader = http.request_body();
    loop {
        match reader.read_next_chunk(&mut scratch) {
            Ok(None) => break,
            Ok(Some(n)) => {
                if body.len() + n > MAX_BODY_BYTES {
                    return Err("request body too large".into());
                }
                body.extend_from_slice(&scratch[..n]);
            }
            Err(e) => return Err(format!("read body: {e}")),
        }
    }

    // 2. Parse want/have/done.
    let parsed = parse_upload_request(&body)?;
    let owner = http.params.get("owner").cloned().unwrap_or_default();
    let repo = http.params.get("repo").cloned().unwrap_or_default();
    let repository_id = format!("rp-{owner}-{repo}");

    // 3. Pass 1 — walk the DAG. We need the object count for the
    //    pack header before we can stream a single byte, so this
    //    pass enumerates SHAs and caches commit/tree bytes (small,
    //    needed for parsing). Blob and Tag bytes are NOT fetched
    //    here; they're streamed in pass 2 and dropped between
    //    objects, so peak memory stays at O(largest blob).
    let have_set: BTreeSet<String> = parsed.haves.iter().cloned().collect();
    let mut visited: BTreeSet<String> = have_set.clone();
    let mut queue: VecDeque<(String, ObjectKind)> = VecDeque::new();
    for want in &parsed.wants {
        queue.push_back((want.clone(), ObjectKind::Commit));
    }

    let mut walk_order: Vec<(String, ObjectKind)> = Vec::new();
    let mut graph_cache: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    while let Some((sha, kind)) = queue.pop_front() {
        if !visited.insert(sha.clone()) {
            continue;
        }
        match kind {
            ObjectKind::Commit | ObjectKind::Tree => {
                let raw_body =
                    fetch_object_body(ctx, &principal, kind, &sha, &repository_id, &api_base)?;
                if matches!(kind, ObjectKind::Commit) {
                    let refs = genesis_git_object::parse_commit_refs(&raw_body)
                        .map_err(|e| format!("commit {sha}: {e}"))?;
                    queue.push_back((refs.tree, ObjectKind::Tree));
                    for p in refs.parents {
                        queue.push_back((p, ObjectKind::Commit));
                    }
                } else {
                    let entries = genesis_git_object::parse_tree(&raw_body)
                        .map_err(|e| format!("tree {sha}: {e}"))?;
                    for entry in entries {
                        let k = if entry.is_tree {
                            ObjectKind::Tree
                        } else {
                            ObjectKind::Blob
                        };
                        queue.push_back((entry.sha, k));
                    }
                }
                graph_cache.insert(sha.clone(), raw_body);
            }
            ObjectKind::Blob | ObjectKind::Tag => {
                // Defer to pass 2 — body is fetched, deflated, and
                // dropped during emission.
            }
        }
        walk_order.push((sha, kind));
    }

    // 4. Pass 2 — stream the response. Order:
    //      pkt-line "NAK\n"   (no negotiation in v0)
    //      pack header + objects + SHA-1 trailer (sidebanded if
    //      negotiated)
    //      pkt-line flush
    //
    // The pack flows through PackEmitter → SidebandWriter (if
    // negotiated) → WasmBodyWriter, so we never hold the assembled
    // pack or the framed response in memory.
    http.submit_response_head(
        200,
        &[
            ("content-type", "application/x-git-upload-pack-result"),
            ("cache-control", "no-cache"),
        ],
    )
    .map_err(|e| format!("head: {e}"))?;

    let mut writer = WasmBodyWriter::new(http.response_body());

    // NAK pkt-line. Tiny — no need to stream.
    let mut nak = Vec::new();
    encode_into(&mut nak, b"NAK\n").map_err(|e| format!("nak: {e}"))?;
    use std::io::Write;
    writer
        .write_all(&nak)
        .map_err(|e| format!("nak write: {e}"))?;

    let sideband = parsed.capabilities.iter().any(|c| c == "side-band-64k");
    let object_count = walk_order.len() as u32;
    let pack_byte_count = if sideband {
        let sb = SidebandWriter::new(&mut writer);
        let (pack_byte_count, sb) = emit_pack_streaming(
            sb,
            object_count,
            walk_order,
            graph_cache,
            &repository_id,
            &principal,
            &api_base,
            ctx,
        )?;
        sb.finish().map_err(|e| format!("sideband finish: {e}"))?;
        pack_byte_count
    } else {
        let (pack_byte_count, _) = emit_pack_streaming(
            &mut writer,
            object_count,
            walk_order,
            graph_cache,
            &repository_id,
            &principal,
            &api_base,
            ctx,
        )?;
        pack_byte_count
    };

    // Trailing pkt-line flush ends the response.
    let mut tail = Vec::new();
    flush(&mut tail);
    writer.write_all(&tail).map_err(|e| format!("tail: {e}"))?;
    writer
        .into_inner()
        .finish()
        .map_err(|e| format!("body close: {e}"))?;

    Ok(json!({
        "wants": parsed.wants.len(),
        "objects": object_count,
        "pack_bytes": pack_byte_count,
    }))
}

/// Drives the PackEmitter. Returns the number of pack bytes written
/// (header + objects + trailer) for the response envelope.
fn emit_pack_streaming<W: std::io::Write>(
    sink: W,
    object_count: u32,
    walk_order: Vec<(String, ObjectKind)>,
    mut graph_cache: BTreeMap<String, Vec<u8>>,
    repository_id: &str,
    principal: &Principal,
    api_base: &str,
    ctx: &Context,
) -> Result<(usize, W), String> {
    // Wrap the sink in a counting writer so we can report bytes
    // written without the caller having to track them.
    let counting = CountingWriter::new(sink);
    let mut emitter =
        PackEmitter::begin(counting, object_count).map_err(|e| format!("pack header: {e}"))?;

    for (sha, kind) in walk_order {
        let body = match kind {
            ObjectKind::Commit | ObjectKind::Tree => graph_cache
                .remove(&sha)
                .ok_or_else(|| format!("walk-cache miss for {sha}"))?,
            ObjectKind::Blob | ObjectKind::Tag => {
                fetch_object_body(ctx, principal, kind, &sha, repository_id, api_base)?
            }
        };
        emitter
            .write_object(kind, &body)
            .map_err(|e| format!("emit {sha}: {e}"))?;
    }

    let counting = emitter.finish().map_err(|e| format!("pack trailer: {e}"))?;
    let pack_bytes = counting.bytes_written();

    Ok((pack_bytes, counting.into_inner()))
}

/// `std::io::Write` adapter over `HttpRequestBodyWriter`. The SDK
/// only exposes `write_all_chunk` / `finish`; this lets the pack
/// emitter and sideband framer write through it with a normal
/// `Write` impl.
struct WasmBodyWriter {
    inner: HttpRequestBodyWriter,
}

impl WasmBodyWriter {
    fn new(inner: HttpRequestBodyWriter) -> Self {
        Self { inner }
    }

    fn into_inner(self) -> HttpRequestBodyWriter {
        self.inner
    }
}

impl std::io::Write for WasmBodyWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner
            .write_all_chunk(buf)
            .map(|_| buf.len())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e}")))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Pass-through `Write` that counts the bytes that go through it.
struct CountingWriter<W: std::io::Write> {
    inner: W,
    n: usize,
}

impl<W: std::io::Write> CountingWriter<W> {
    fn new(inner: W) -> Self {
        Self { inner, n: 0 }
    }

    fn bytes_written(&self) -> usize {
        self.n
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: std::io::Write> std::io::Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.n += written;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

struct UploadRequest {
    wants: Vec<String>,
    haves: Vec<String>,
    capabilities: Vec<String>,
}

fn parse_upload_request(buf: &[u8]) -> Result<UploadRequest, String> {
    let mut wants = Vec::new();
    let mut haves = Vec::new();
    let mut capabilities: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i + 4 <= buf.len() {
        let len_str = core::str::from_utf8(&buf[i..i + 4]).map_err(|_| "pkt-line len non-utf8")?;
        let declared = usize::from_str_radix(len_str, 16).map_err(|_| "pkt-line len non-hex")?;
        if declared == 0 {
            i += 4;
            continue; // flush between wants and haves/done
        }
        if declared < 4 || i + declared > buf.len() {
            break;
        }
        let payload = &buf[i + 4..i + declared];
        i += declared;
        let line = core::str::from_utf8(payload).map_err(|_| "pkt-line non-utf8")?;
        let line = line.trim_end_matches('\n');
        if let Some(rest) = line.strip_prefix("want ") {
            // First want carries capabilities after a space.
            let mut parts = rest.splitn(2, ' ');
            let sha = parts.next().unwrap_or("").to_string();
            if !sha.is_empty() {
                wants.push(sha);
            }
            if capabilities.is_empty()
                && let Some(caps) = parts.next()
            {
                capabilities = caps.split_whitespace().map(|s| s.to_string()).collect();
            }
        } else if let Some(sha) = line.strip_prefix("have ") {
            haves.push(sha.to_string());
        } else if line == "done" {
            break;
        }
    }
    if wants.is_empty() {
        return Err("no wants in upload-pack request".into());
    }
    Ok(UploadRequest {
        wants,
        haves,
        capabilities,
    })
}

fn fetch_object_body(
    ctx: &Context,
    principal: &Principal,
    kind: ObjectKind,
    sha: &str,
    repo_id: &str,
    api_base: &str,
) -> Result<Vec<u8>, String> {
    if let Some(cached) = fetch_cached_object_body(api_base, repo_id, sha)? {
        return Ok(cached);
    }

    let set = match kind {
        ObjectKind::Commit => "Commits",
        ObjectKind::Tree => "Trees",
        ObjectKind::Blob => "Blobs",
        ObjectKind::Tag => "Tags",
    };
    // Stored entity ids are repository-scoped so shared Git objects can appear
    // in multiple repos. The Git SHA stays in the Id field.
    let filter = format!(
        "Id eq '{}' and RepositoryId eq '{}'",
        sha.replace('\'', "''"),
        repo_id.replace('\'', "''")
    );
    let url = format!("{api_base}/tdata/{set}?$filter={}", urlencode(&filter));
    let response = ctx
        .http_call("GET", &url, &principal.outbound_headers(), "")
        .map_err(|e| format!("fetch {set}({sha}): {e}"))?;
    if !(200..400).contains(&response.status) {
        return Err(format!("{set}({sha}) status {}", response.status));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(&response.body).map_err(|e| format!("object json: {e}"))?;
    let items = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let row = items
        .into_iter()
        .next()
        .ok_or_else(|| format!("{set}({sha}): no row matched"))?;
    let fields = row
        .get("fields")
        .ok_or_else(|| format!("{set}({sha}): row has no fields"))?;
    let canonical_value = fields
        .get("CanonicalBytes")
        .or_else(|| fields.get("canonical_bytes"))
        .ok_or_else(|| format!("{set}({sha}): no CanonicalBytes"))?;
    let canonical_b64 = resolve_field_value(ctx, principal, api_base, canonical_value)?;
    let canonical = B64
        .decode(&canonical_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;
    let nul = canonical
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("{set}({sha}): no NUL in canonical"))?;
    Ok(canonical[nul + 1..].to_vec())
}

fn fetch_cached_object_body(
    api_base: &str,
    repo_id: &str,
    sha: &str,
) -> Result<Option<Vec<u8>>, String> {
    let url = format!(
        "{}/_internal/blobs/git-objects/{repo_id}/{sha}.raw",
        api_base.trim_end_matches('/')
    );
    let (request, mut response, head) =
        streaming_call("GET", &url, &[]).map_err(|e| format!("object-cache GET {sha}: {e}"))?;
    request
        .finish()
        .map_err(|e| format!("object-cache GET {sha} request close: {e}"))?;
    let head = head().map_err(|e| format!("object-cache GET {sha} response head: {e}"))?;
    if head.status == 404 {
        let _ = response.close();
        return Ok(None);
    }
    if !(200..300).contains(&head.status) {
        let _ = response.close();
        return Err(format!(
            "object-cache GET {sha} returned HTTP {}",
            head.status
        ));
    }

    let mut body = Vec::new();
    let mut scratch = alloc::vec![0u8; OUTBOUND_READ_CHUNK];
    loop {
        match response.read_next_chunk(&mut scratch) {
            Ok(None) => break,
            Ok(Some(n)) => {
                if body.len() + n > MAX_CACHED_OBJECT_BYTES {
                    let _ = response.close();
                    return Err(format!(
                        "object-cache GET {sha} exceeds {MAX_CACHED_OBJECT_BYTES} bytes"
                    ));
                }
                body.extend_from_slice(&scratch[..n]);
            }
            Err(e) => {
                let _ = response.close();
                return Err(format!("object-cache GET {sha} response body: {e}"));
            }
        }
    }
    let _ = response.close();
    Ok(Some(body))
}

fn resolve_field_value(
    ctx: &Context,
    principal: &Principal,
    api_base: &str,
    value: &serde_json::Value,
) -> Result<String, String> {
    if let Some(value) = value.as_str() {
        return Ok(value.to_string());
    }

    let Some(blob_key) = field_overflow_blob_key(value)? else {
        return Err("field value is neither string nor supported blob ref".to_string());
    };
    let url = format!(
        "{}/_internal/blobs/{blob_key}",
        api_base.trim_end_matches('/')
    );
    let response = ctx
        .http_call("GET", &url, &principal.outbound_headers(), "")
        .map_err(|e| format!("field-overflow GET {blob_key}: {e}"))?;
    if !(200..300).contains(&response.status) {
        return Err(format!(
            "field-overflow GET {blob_key} returned HTTP {}",
            response.status
        ));
    }
    serde_json::from_str(&response.body).map_err(|e| format!("field-overflow {blob_key} json: {e}"))
}

fn field_overflow_blob_key(value: &serde_json::Value) -> Result<Option<String>, String> {
    let Some(obj) = value.as_object() else {
        return Ok(None);
    };
    let Some(blob_key) = obj.get(FIELD_OVERFLOW_REF_KEY).and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let encoding = obj
        .get(FIELD_OVERFLOW_ENCODING_KEY)
        .and_then(|v| v.as_str())
        .unwrap_or("json");
    if encoding != "json" {
        return Err(format!(
            "field-overflow encoding {encoding:?} is not supported"
        ));
    }
    Ok(Some(blob_key.to_string()))
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_overflow_blob_key_accepts_json_refs() {
        let value = json!({
            FIELD_OVERFLOW_REF_KEY: "field-overflow/sha256/abc.json",
            FIELD_OVERFLOW_ENCODING_KEY: "json",
        });

        assert_eq!(
            field_overflow_blob_key(&value).unwrap().as_deref(),
            Some("field-overflow/sha256/abc.json")
        );
    }

    #[test]
    fn field_overflow_blob_key_rejects_unknown_encoding() {
        let value = json!({
            FIELD_OVERFLOW_REF_KEY: "field-overflow/sha256/abc.json",
            FIELD_OVERFLOW_ENCODING_KEY: "raw",
        });

        let err = field_overflow_blob_key(&value).unwrap_err();
        assert!(err.contains("not supported"));
    }
}
