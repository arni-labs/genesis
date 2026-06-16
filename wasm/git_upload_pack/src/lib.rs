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
use genesis_git_auth::Principal;
use std::time::Instant;
use temper_wasm_sdk::http_stream::{HttpRequestBodyWriter, InboundHttp, streaming_call};
use temper_wasm_sdk::prelude::*;
use tg_wire::{ObjectKind, PackEmitter, SidebandWriter, encode_into, flush};

pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "git-upload-pack";
const FIELD_OVERFLOW_REF_KEY: &str = "__temper_blob_ref";
const FIELD_OVERFLOW_ENCODING_KEY: &str = "__temper_blob_encoding";

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
            return match serve_upload_pack(&ctx, &http) {
                Ok(value) => Ok(value),
                Err(error) => {
                    let _ = ctx.log_structured(
                        "error",
                        "Genesis git upload-pack failed",
                        &json!({
                            "error": error,
                            "owner": http.params.get("owner").cloned().unwrap_or_default(),
                            "repo": http.params.get("repo").cloned().unwrap_or_default(),
                        }),
                    );
                    respond_upload_pack_error(&http, &error)
                }
            };
        }
        respond_text(&http, 404, "text/plain", "no upload-pack route matches")
    }
}

/// Resolve the inbound caller and fall back to the system principal
/// if none is presented. Production deployments lock down via Cedar
/// to require a real GitToken; dev quickstarts work without one.
fn effective_principal(ctx: &Context, headers: &[(String, String)]) -> Principal {
    let api_base = temper_api_from_headers(headers);
    let auth_env = genesis_git_auth::AuthEnv {
        temper_api: &api_base,
        tenant: SYSTEM_TENANT,
        system_principal: SYSTEM_PRINCIPAL,
    };
    let resolved = genesis_git_auth::resolve_principal(ctx, &auth_env, headers);
    if resolved.is_anonymous() {
        Principal::system(&auth_env)
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
const MAX_CACHED_OBJECT_BYTES: usize = 128 * 1024 * 1024;
const PREFETCH_PAGE_SIZE: usize = 20;
const HTTP_STREAM_READ_CHUNK_BYTES: usize = 64 * 1024;

fn respond_upload_pack_error(http: &InboundHttp, error: &str) -> Result<Value, String> {
    http.submit_response_head(
        200,
        &[
            ("content-type", "application/x-git-upload-pack-result"),
            ("cache-control", "no-cache"),
        ],
    )
    .map_err(|e| format!("error response head: {e}"))?;

    let clean = error.replace(['\r', '\n'], " ");
    let mut payload = Vec::new();
    encode_into(&mut payload, format!("ERR {clean}\n").as_bytes())
        .map_err(|e| format!("error pkt-line: {e}"))?;
    let mut writer = http.response_body();
    writer
        .write_all_chunk(&payload)
        .map_err(|e| format!("error response body: {e}"))?;
    writer
        .finish()
        .map_err(|e| format!("error response close: {e}"))?;

    Ok(json!({
        "status": 500,
        "error": clean,
    }))
}

fn serve_upload_pack(ctx: &Context, http: &InboundHttp) -> Result<Value, String> {
    let total_started = Instant::now();
    let principal = effective_principal(ctx, &http.headers);
    let api_base = temper_api_from_headers(&http.headers);
    let blob_endpoint = blob_endpoint(ctx, &api_base);
    // 1. Read the request body. Bounded: want/have negotiation
    //    payloads are tiny (a few KB even for huge repos), so we
    //    cap at 16 MiB and buffer.
    let mut body = Vec::new();
    let mut scratch = alloc::vec![0u8; READ_CHUNK];
    let mut reader = http.request_body();
    let body_started = Instant::now();
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
    log_upload_pack_phase(
        ctx,
        "read_request_body",
        body_started,
        0,
        body.len(),
        &http.params,
    );

    // 2. Parse want/have/done.
    let parse_started = Instant::now();
    let parsed = parse_upload_request(&body)?;
    log_upload_pack_phase(
        ctx,
        "parse_request",
        parse_started,
        parsed.wants.len() + parsed.haves.len(),
        body.len(),
        &http.params,
    );
    let owner = http.params.get("owner").cloned().unwrap_or_default();
    let repo = http.params.get("repo").cloned().unwrap_or_default();
    let repository_id = format!("rp-{owner}-{repo}");
    let prefetch_started = Instant::now();
    let prefetched_objects =
        match prefetch_repository_object_bodies(ctx, &principal, &api_base, &repository_id) {
            Ok(objects) => {
                let count = objects.len();
                let bytes = objects.bytes();
                log_upload_pack_phase(
                    ctx,
                    "prefetch_repository_objects",
                    prefetch_started,
                    count,
                    bytes,
                    &http.params,
                );
                Some(objects)
            }
            Err(error) => {
                let _ = ctx.log_structured(
                    "warn",
                    "Genesis git upload-pack object prefetch failed",
                    &json!({
                        "repository_id": repository_id,
                        "error": error,
                    }),
                );
                None
            }
        };

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
    let walk_started = Instant::now();
    while let Some((sha, kind)) = queue.pop_front() {
        if !visited.insert(sha.clone()) {
            continue;
        }
        match kind {
            ObjectKind::Commit | ObjectKind::Tree => {
                let raw_body = fetch_object_body(
                    ctx,
                    &principal,
                    kind,
                    &sha,
                    &repository_id,
                    &api_base,
                    &blob_endpoint,
                    prefetched_objects.as_ref(),
                )?;
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
    log_upload_pack_phase(
        ctx,
        "walk_reachable_objects",
        walk_started,
        walk_order.len(),
        graph_cache.values().map(|bytes| bytes.len()).sum(),
        &http.params,
    );

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
    let emit_started = Instant::now();
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
            &blob_endpoint,
            prefetched_objects.as_ref(),
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
            &blob_endpoint,
            prefetched_objects.as_ref(),
            ctx,
        )?;
        pack_byte_count
    };
    log_upload_pack_phase(
        ctx,
        "emit_pack",
        emit_started,
        object_count as usize,
        pack_byte_count,
        &http.params,
    );

    // Trailing pkt-line flush ends the response.
    let mut tail = Vec::new();
    flush(&mut tail);
    writer.write_all(&tail).map_err(|e| format!("tail: {e}"))?;
    writer
        .into_inner()
        .finish()
        .map_err(|e| format!("body close: {e}"))?;
    log_upload_pack_phase(
        ctx,
        "total",
        total_started,
        object_count as usize,
        pack_byte_count,
        &http.params,
    );

    Ok(json!({
        "wants": parsed.wants.len(),
        "objects": object_count,
        "pack_bytes": pack_byte_count,
    }))
}

fn log_upload_pack_phase(
    ctx: &Context,
    phase: &str,
    started: Instant,
    count: usize,
    bytes: usize,
    params: &BTreeMap<String, String>,
) {
    let _ = ctx.log_structured(
        "info",
        "Genesis git upload-pack phase complete",
        &json!({
            "phase": phase,
            "duration_ms": started.elapsed().as_millis() as u64,
            "count": count,
            "bytes": bytes,
            "owner": params.get("owner").cloned().unwrap_or_default(),
            "repo": params.get("repo").cloned().unwrap_or_default(),
        }),
    );
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
    blob_endpoint: &str,
    prefetched_objects: Option<&RepositoryObjectBodies>,
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
            ObjectKind::Blob | ObjectKind::Tag => fetch_object_body(
                ctx,
                principal,
                kind,
                &sha,
                repository_id,
                api_base,
                blob_endpoint,
                prefetched_objects,
            )?,
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

#[derive(Default)]
struct RepositoryObjectBodies {
    commits: BTreeMap<String, Vec<u8>>,
    trees: BTreeMap<String, Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
    tags: BTreeMap<String, Vec<u8>>,
}

impl RepositoryObjectBodies {
    fn get(&self, kind: ObjectKind, sha: &str) -> Option<&Vec<u8>> {
        match kind {
            ObjectKind::Commit => self.commits.get(sha),
            ObjectKind::Tree => self.trees.get(sha),
            ObjectKind::Blob => self.blobs.get(sha),
            ObjectKind::Tag => self.tags.get(sha),
        }
    }

    fn insert(&mut self, kind: ObjectKind, sha: String, body: Vec<u8>) {
        match kind {
            ObjectKind::Commit => self.commits.insert(sha, body),
            ObjectKind::Tree => self.trees.insert(sha, body),
            ObjectKind::Blob => self.blobs.insert(sha, body),
            ObjectKind::Tag => self.tags.insert(sha, body),
        };
    }

    fn len(&self) -> usize {
        self.commits.len() + self.trees.len() + self.blobs.len() + self.tags.len()
    }

    fn bytes(&self) -> usize {
        self.commits.values().map(Vec::len).sum::<usize>()
            + self.trees.values().map(Vec::len).sum::<usize>()
            + self.blobs.values().map(Vec::len).sum::<usize>()
            + self.tags.values().map(Vec::len).sum::<usize>()
    }
}

fn prefetch_repository_object_bodies(
    ctx: &Context,
    principal: &Principal,
    api_base: &str,
    repo_id: &str,
) -> Result<RepositoryObjectBodies, String> {
    let mut bodies = RepositoryObjectBodies::default();
    for (kind, set) in [
        (ObjectKind::Commit, "Commits"),
        (ObjectKind::Tree, "Trees"),
        (ObjectKind::Blob, "Blobs"),
        (ObjectKind::Tag, "Tags"),
    ] {
        let filter = format!("RepositoryId eq {}", odata_string_literal(repo_id));
        let mut skip = 0usize;
        loop {
            let url = format!(
                "{api_base}/tdata/{set}?$filter={}&$select=Id,RepositoryId,CanonicalBytes&$top={PREFETCH_PAGE_SIZE}&$skip={skip}",
                urlencode(&filter)
            );
            let response = ctx
                .http_call("GET", &url, &principal.outbound_headers(), "")
                .map_err(|e| format!("prefetch {set}: {e}"))?;
            if !(200..400).contains(&response.status) {
                return Err(format!("prefetch {set} status {}", response.status));
            }
            let parsed: serde_json::Value = serde_json::from_str(&response.body)
                .map_err(|e| format!("prefetch {set} json: {e}"))?;
            let rows = parsed
                .get("value")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            let row_count = rows.len();
            for row in rows {
                let fields = row.get("fields").unwrap_or(&row);
                let Some(sha) = string_field(fields, "Id") else {
                    continue;
                };
                let Some(canonical_value) = fields
                    .get("CanonicalBytes")
                    .or_else(|| fields.get("canonical_bytes"))
                else {
                    continue;
                };
                let body =
                    canonical_body_from_field_value(set, &sha, canonical_value, |blob_key| {
                        get_overflow_blob(api_base, principal, blob_key)
                    })
                    .map_err(|e| format!("prefetch {set}({sha}): {e}"))?;
                bodies.insert(kind, sha, body);
            }
            if row_count < PREFETCH_PAGE_SIZE {
                break;
            }
            skip += row_count;
        }
    }
    Ok(bodies)
}

fn fetch_object_body(
    ctx: &Context,
    principal: &Principal,
    kind: ObjectKind,
    sha: &str,
    repo_id: &str,
    api_base: &str,
    blob_endpoint: &str,
    prefetched_objects: Option<&RepositoryObjectBodies>,
) -> Result<Vec<u8>, String> {
    if let Some(body) = prefetched_objects.and_then(|objects| objects.get(kind, sha)) {
        return Ok(body.clone());
    }

    if let Some(cached) = fetch_cached_object_body(blob_endpoint, repo_id, sha)? {
        return Ok(cached);
    }

    let set = match kind {
        ObjectKind::Commit => "Commits",
        ObjectKind::Tree => "Trees",
        ObjectKind::Blob => "Blobs",
        ObjectKind::Tag => "Tags",
    };
    let url = existing_object_lookup_url(api_base, set, repo_id, sha);
    let outbound_headers = principal.outbound_headers();
    let header_refs = header_refs(&outbound_headers);
    let (status, body) = streaming_get(&url, &header_refs, &format!("fetch {set}({sha})"))?;
    if !(200..300).contains(&status) {
        return Err(format!("{set}({sha}) status {status}"));
    }
    let parsed: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| format!("object json: {e}"))?;
    let items = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let row = items
        .into_iter()
        .next()
        .ok_or_else(|| format!("{set}({sha}): no row matched"))?;
    let fields = row.get("fields").unwrap_or(&row);
    let canonical_value = fields
        .get("CanonicalBytes")
        .or_else(|| fields.get("canonical_bytes"))
        .ok_or_else(|| format!("{set}({sha}): no CanonicalBytes"))?;
    let body = canonical_body_from_field_value(set, sha, canonical_value, |blob_key| {
        get_overflow_blob(api_base, principal, blob_key)
    })?;
    if let Err(error) = store_cached_object_body(ctx, blob_endpoint, repo_id, sha, &body) {
        let _ = ctx.log_structured(
            "warn",
            "Genesis git upload-pack object cache fill failed",
            &json!({
                "repo_id": repo_id,
                "sha": sha,
                "error": error,
            }),
        );
    }
    Ok(body)
}

fn existing_object_lookup_url(api_base: &str, set: &str, repo_id: &str, sha: &str) -> String {
    let filter = format!(
        "Id eq {} and RepositoryId eq {}",
        odata_string_literal(sha),
        odata_string_literal(repo_id)
    );
    format!(
        "{}/tdata/{set}?$filter={}&$select=CanonicalBytes&$top=1",
        api_base.trim_end_matches('/'),
        urlencode(&filter)
    )
}

fn canonical_body_from_field_value<F>(
    set: &str,
    sha: &str,
    value: &serde_json::Value,
    mut resolve_blob: F,
) -> Result<Vec<u8>, String>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let canonical_b64 = string_from_field_value(set, sha, "CanonicalBytes", value, |blob_key| {
        resolve_blob(blob_key)
    })?;
    let canonical = B64
        .decode(canonical_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;
    let nul = canonical
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("{set}({sha}): no NUL in canonical"))?;
    Ok(canonical[nul + 1..].to_vec())
}

fn string_from_field_value<F>(
    set: &str,
    sha: &str,
    field: &str,
    value: &serde_json::Value,
    mut resolve_blob: F,
) -> Result<String, String>
where
    F: FnMut(&str) -> Result<String, String>,
{
    if let Some(text) = value.as_str() {
        return Ok(text.to_string());
    }
    if let Some(blob_key) = field_overflow_blob_key(value)? {
        let serialized = resolve_blob(&blob_key)?;
        let resolved: serde_json::Value = serde_json::from_str(&serialized)
            .map_err(|e| format!("{set}({sha}) {field} blob-ref JSON parse: {e}"))?;
        return resolved
            .as_str()
            .map(ToString::to_string)
            .ok_or_else(|| format!("{set}({sha}) {field} blob-ref did not contain a string"));
    }
    Err(format!(
        "{set}({sha}) {field} must be a string or field-overflow ref"
    ))
}

fn fetch_cached_object_body(
    blob_endpoint: &str,
    repo_id: &str,
    sha: &str,
) -> Result<Option<Vec<u8>>, String> {
    let url = format!(
        "{}/git-objects/{repo_id}/{sha}.b64",
        blob_endpoint.trim_end_matches('/')
    );
    let (status, body) = streaming_get(&url, &[], &format!("object-cache GET {sha}"))?;
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(format!("object-cache GET {sha} returned HTTP {status}"));
    }
    let body = B64
        .decode(String::from_utf8_lossy(&body).trim())
        .map_err(|e| format!("object-cache GET {sha} base64 decode: {e}"))?;
    if body.len() > MAX_CACHED_OBJECT_BYTES {
        return Err(format!(
            "object-cache GET {sha} exceeds {MAX_CACHED_OBJECT_BYTES} bytes"
        ));
    }
    Ok(Some(body))
}

fn store_cached_object_body(
    ctx: &Context,
    blob_endpoint: &str,
    repo_id: &str,
    sha: &str,
    body: &[u8],
) -> Result<(), String> {
    let url = format!(
        "{}/git-objects/{repo_id}/{sha}.b64",
        blob_endpoint.trim_end_matches('/')
    );
    let response = ctx
        .http_call("PUT", &url, &[], &B64.encode(body))
        .map_err(|e| format!("object-cache PUT {sha}: {e}"))?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!(
            "object-cache PUT {sha} returned HTTP {}",
            response.status
        ))
    }
}

fn blob_endpoint(ctx: &Context, api_base: &str) -> String {
    ctx.get_secret("blob_endpoint")
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{api_base}/_internal/blobs"))
}

fn get_overflow_blob(
    api_base: &str,
    principal: &Principal,
    blob_key: &str,
) -> Result<String, String> {
    let url = format!(
        "{}/_internal/blobs/{blob_key}",
        api_base.trim_end_matches('/')
    );
    let outbound_headers = principal.outbound_headers();
    let header_refs = header_refs(&outbound_headers);
    let (status, body) = streaming_get(
        &url,
        &header_refs,
        &format!("field-overflow GET {blob_key}"),
    )?;
    if !(200..300).contains(&status) {
        return Err(format!(
            "field-overflow GET {blob_key} returned HTTP {status}"
        ));
    }
    let body =
        String::from_utf8(body).map_err(|e| format!("field-overflow GET {blob_key} utf8: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("field-overflow {blob_key} json: {e}"))
}

fn header_refs(headers: &[(String, String)]) -> Vec<(&str, &str)> {
    headers
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
}

fn streaming_get(
    url: &str,
    headers: &[(&str, &str)],
    label: &str,
) -> Result<(u16, Vec<u8>), String> {
    let (request_body, mut response_body, response_head) =
        streaming_call("GET", url, headers).map_err(|e| format!("{label} stream begin: {e}"))?;
    request_body
        .finish()
        .map_err(|e| format!("{label} request close: {e}"))?;
    let head = response_head().map_err(|e| format!("{label} response head: {e}"))?;
    if !(200..300).contains(&head.status) {
        let _ = response_body.close();
        return Ok((head.status, Vec::new()));
    }

    let mut out = Vec::new();
    let mut buf = alloc::vec![0u8; HTTP_STREAM_READ_CHUNK_BYTES];
    loop {
        let Some(n) = response_body
            .read_next_chunk(&mut buf)
            .map_err(|e| format!("{label} response body: {e}"))?
        else {
            break;
        };
        out.extend_from_slice(&buf[..n]);
    }
    let _ = response_body.close();
    Ok((head.status, out))
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

fn odata_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn string_field(fields: &serde_json::Value, name: &str) -> Option<String> {
    fields.get(name).and_then(|value| match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
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

    #[test]
    fn existing_object_lookup_selects_only_canonical_bytes() {
        let url =
            existing_object_lookup_url("https://temper.example/", "Blobs", "repo ' one", "abc123");

        assert_eq!(
            url,
            "https://temper.example/tdata/Blobs?$filter=Id%20eq%20%27abc123%27%20and%20RepositoryId%20eq%20%27repo%20%27%27%20one%27&$select=CanonicalBytes&$top=1"
        );
    }

    #[test]
    fn canonical_body_resolves_field_overflow_ref() {
        let canonical_b64 = B64.encode(b"blob 5\0hello");
        let value = json!({
            FIELD_OVERFLOW_REF_KEY: "field-overflow/sha256/canonical.json",
            FIELD_OVERFLOW_ENCODING_KEY: "json",
        });

        let body = canonical_body_from_field_value("Blobs", "abc123", &value, |blob_key| {
            assert_eq!(blob_key, "field-overflow/sha256/canonical.json");
            Ok(serde_json::to_string(&canonical_b64).unwrap())
        })
        .unwrap();

        assert_eq!(body, b"hello");
    }
}
