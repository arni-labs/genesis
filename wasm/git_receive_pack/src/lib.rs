//! git_receive_pack — smart-HTTP receive-pack WASM integration.
//!
//! Handles `POST /{owner}/{repo}.git/git-receive-pack`: pkt-line
//! command list parsing plus pack-byte forwarding into the
//! spec-owned `Repository.IngestPack` action bridge. The preceding
//! `/info/refs` advertisement phase lives in `git_refs_advertise`.
//!
//! For `POST /git-receive-pack`, this module is now only the Git wire
//! adapter. It reads the streamed body, parses the receive-pack command list,
//! buffers the raw pack bytes, and returns typed parameters for the
//! kernel-owned HttpEndpoint action bridge. The bridge invokes the
//! spec-defined `Repository.IngestPack` action; WASM does not dispatch Temper
//! actions or fan out object/ref writes.
//!
//! Repository resolution: v0 convention is `rp-{owner}-{repo}`.
//! Agents must pre-create the Repository row with that Id before
//! pushing (per RFC-0002 Slice C). A subsequent slice adds
//! `/tdata/Repositories?$filter=` lookup.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::io::Read;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use sha2::{Digest, Sha256};
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;
use tg_wire::{CommandKind, commands};

/// Cap on the command-list bytes accumulated before pack parsing
/// begins. The list is pkt-line framed (4-hex length + payload) and
/// in practice tops out at a few KiB even for very large pushes —
/// 1 MiB is generous head-room with a clear failure mode.
const COMMAND_LIST_MAX_BYTES: usize = 1 * 1024 * 1024;
/// BufReader capacity for the request-body stream.
const BUFREAD_CAPACITY: usize = 64 * 1024;
const FIELD_INLINE_MAX_BYTES: usize = 131_072;
const FIELD_OVERFLOW_BLOB_PREFIX: &str = "field-overflow/sha256/";
const FIELD_OVERFLOW_REF_KEY: &str = "__temper_blob_ref";
const FIELD_OVERFLOW_SIZE_KEY: &str = "__temper_blob_size";
const FIELD_OVERFLOW_ENCODING_KEY: &str = "__temper_blob_encoding";
pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "git-receive-pack";

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let http_value = ctx
            .http_request
            .clone()
            .ok_or_else(|| "git_receive_pack requires HttpEndpoint dispatch".to_string())?;
        let http: InboundHttp = serde_json::from_value(http_value)
            .map_err(|e| format!("http_request parse error: {e}"))?;

        let raw = http.path.as_str();
        let path = raw.split('?').next().unwrap_or(raw);

        if http.method == "POST" && path.ends_with("/git-receive-pack") {
            return serve_receive_pack(&ctx, &http);
        }
        respond_text(&http, 404, "text/plain", "no receive-pack route matches")
    }
}

fn serve_receive_pack(ctx: &Context, http: &InboundHttp) -> Result<Value, String> {
    // Convention-based repository id derivation.
    let owner = http.params.get("owner").cloned().unwrap_or_default();
    let repo = http.params.get("repo").cloned().unwrap_or_default();
    let repository_id = format!("rp-{owner}-{repo}");
    let api_base = temper_api_from_headers(&http.headers);

    // Push is a governed mutation: resolve the GitToken principal and
    // reject anonymous callers with the standard smart-HTTP challenge
    // (ADR-0025). The resolved principal rides the action bridge so
    // Cedar evaluates IngestPack sub-writes as the real caller.
    let auth_env = genesis_git_auth::AuthEnv {
        temper_api: TEMPER_API,
        tenant: SYSTEM_TENANT,
        system_principal: SYSTEM_PRINCIPAL,
    };
    let principal = genesis_git_auth::resolve_principal(ctx, &auth_env, &http.headers);
    if principal.is_anonymous() {
        return respond_unauthorized();
    }
    genesis_git_auth::mark_token_used(ctx, &auth_env, &principal);

    // Stream the request body. We read the command list bytes
    // (pkt-line framed; ends at a 0000 flush), then buffer the raw
    // pack bytes as action input. Repository.IngestPack's spec-triggered
    // parser integration owns object verification and sub-write emission.
    let mut reader = std::io::BufReader::with_capacity(
        BUFREAD_CAPACITY,
        WasmRequestReader::new(http.request_body()),
    );
    let command_read = read_command_list(&mut reader)?;
    let cmd_bytes = command_read.bytes;
    let parsed =
        commands::parse_commands(&cmd_bytes).map_err(|e| format!("parse_commands: {e}"))?;

    let mut pack_bytes = command_read.pack_prefix;
    reader
        .read_to_end(&mut pack_bytes)
        .map_err(|e| format!("read pack bytes: {e}"))?;

    let needs_pack = parsed
        .commands
        .iter()
        .any(|cmd| cmd.kind() != CommandKind::Delete);
    if needs_pack && pack_bytes.is_empty() {
        return Err("receive-pack command list requires pack bytes".to_string());
    }

    let ref_updates: Vec<Value> = parsed
        .commands
        .iter()
        .map(|cmd| {
            json!({
                "Name": cmd.refname,
                "PreviousCommitSha": cmd.old_sha,
                "NewCommitSha": cmd.new_sha,
            })
        })
        .collect();
    let refs: Vec<String> = parsed
        .commands
        .iter()
        .map(|cmd| cmd.refname.clone())
        .collect();
    let sideband = parsed.capabilities.iter().any(|c| c == "side-band-64k")
        || command_list_declares_capability(&cmd_bytes, "side-band-64k");

    let mut action_params = json!({
        "RefUpdates": ref_updates,
        "ClientRequestId": receive_pack_client_request_id(&repository_id, &cmd_bytes, &pack_bytes),
    });
    if !pack_bytes.is_empty() {
        let blob_endpoint = blob_endpoint(ctx, &api_base);
        action_params["PackBytes"] =
            maybe_stage_field_value(ctx, &blob_endpoint, B64.encode(&pack_bytes))?;
    }

    Ok(json!({
        "action_params": action_params,
        "bridge_principal": principal.bridge_principal_json(),
        "git_receive_pack": {
            "refs": refs,
            "sideband": sideband,
            "commands": parsed.commands.len(),
            "pack_bytes": pack_bytes.len(),
            "repository_id": repository_id,
        },
    }))
}

/// Bridge short-circuit 401 (temper ADR-0138): on action-bridge routes
/// the kernel formats responses, so the challenge must travel through
/// `bridge_response` rather than a guest-submitted head.
fn respond_unauthorized() -> Result<Value, String> {
    Ok(json!({
        "bridge_response": {
            "status": 401,
            "headers": { "WWW-Authenticate": "Basic realm=\"Genesis\"" },
            "body": "authentication required: supply a GitToken as Basic username or Bearer token\n",
        }
    }))
}

fn receive_pack_client_request_id(
    repository_id: &str,
    command_bytes: &[u8],
    pack_bytes: &[u8],
) -> String {
    let mut hasher = genesis_git_object::Sha1::new();
    hasher.update(repository_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(command_bytes);
    hasher.update(b"\0");
    hasher.update(pack_bytes);
    format!("git-receive-pack:{}", hasher.hex())
}

fn command_list_declares_capability(command_bytes: &[u8], needle: &str) -> bool {
    if command_bytes.len() < needle.len() {
        return false;
    }
    command_bytes
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn temper_api_from_headers(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| format!("http://{v}"))
        .unwrap_or_else(|| TEMPER_API.to_string())
}

fn blob_endpoint(ctx: &Context, api_base: &str) -> String {
    ctx.get_secret("blob_endpoint")
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{api_base}/_internal/blobs"))
}

fn maybe_stage_field_value(
    ctx: &Context,
    blob_endpoint: &str,
    value: String,
) -> Result<Value, String> {
    let json_value = Value::String(value);
    let serialized =
        serde_json::to_vec(&json_value).map_err(|e| format!("field-overflow serialize: {e}"))?;
    if serialized.len() <= FIELD_INLINE_MAX_BYTES {
        return Ok(json_value);
    }

    let (blob_key, blob_ref) = overflow_blob_ref_for_serialized(&serialized);
    put_overflow_blob(ctx, blob_endpoint, &blob_key, &serialized)?;
    Ok(blob_ref)
}

fn overflow_blob_ref_for_serialized(serialized: &[u8]) -> (String, Value) {
    let digest = Sha256::digest(serialized);
    let blob_key = format!("{FIELD_OVERFLOW_BLOB_PREFIX}{digest:x}.json");
    (
        blob_key.clone(),
        json!({
            FIELD_OVERFLOW_REF_KEY: blob_key,
            FIELD_OVERFLOW_SIZE_KEY: serialized.len(),
            FIELD_OVERFLOW_ENCODING_KEY: "json",
        }),
    )
}

fn put_overflow_blob(
    ctx: &Context,
    blob_endpoint: &str,
    blob_key: &str,
    serialized: &[u8],
) -> Result<(), String> {
    let body = core::str::from_utf8(serialized)
        .map_err(|e| format!("field-overflow body was not utf-8: {e}"))?;
    let url = format!("{}/{blob_key}", blob_endpoint.trim_end_matches('/'));
    let response = ctx
        .http_call("PUT", &url, &[], body)
        .map_err(|e| format!("field-overflow PUT {blob_key}: {e}"))?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!(
            "field-overflow PUT {blob_key} returned HTTP {}",
            response.status
        ))
    }
}

/// `std::io::Read` adapter over the SDK's inbound body reader. Used
/// to read the streamed receive-pack body.
struct WasmRequestReader {
    inner: temper_wasm_sdk::http_stream::HttpResponseBodyReader,
}

impl WasmRequestReader {
    fn new(inner: temper_wasm_sdk::http_stream::HttpResponseBodyReader) -> Self {
        Self { inner }
    }
}

impl std::io::Read for WasmRequestReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read_next_chunk(out) {
            Ok(None) => Ok(0),
            Ok(Some(n)) => Ok(n),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{e}"),
            )),
        }
    }
}

struct CommandRead {
    bytes: Vec<u8>,
    pack_prefix: Vec<u8>,
}

/// Read pkt-line packets from `reader` until either the `0000` flush
/// that ends the receive-pack command list or the first `PACK` bytes.
/// Git clients may put the pack header directly after the last command.
/// If this consumes the `PACK` header while probing for the next pkt-line,
/// it returns that prefix so the caller can prepend it to the pack body.
fn read_command_list<R: std::io::BufRead>(reader: &mut R) -> Result<CommandRead, String> {
    let mut out = Vec::new();
    loop {
        if out.len() >= COMMAND_LIST_MAX_BYTES {
            return Err(format!(
                "command list exceeds {COMMAND_LIST_MAX_BYTES} bytes"
            ));
        }
        let mut len_buf = [0u8; 4];
        reader
            .read_exact(&mut len_buf)
            .map_err(|e| format!("read pkt length: {e}"))?;
        if &len_buf == b"PACK" {
            return Ok(CommandRead {
                bytes: out,
                pack_prefix: len_buf.to_vec(),
            });
        }
        out.extend_from_slice(&len_buf);
        let len_str =
            core::str::from_utf8(&len_buf).map_err(|e| format!("pkt length not ASCII: {e}"))?;
        let pkt_len =
            usize::from_str_radix(len_str, 16).map_err(|e| format!("pkt length not hex: {e}"))?;
        if pkt_len == 0 {
            // Flush — end of command list.
            return Ok(CommandRead {
                bytes: out,
                pack_prefix: Vec::new(),
            });
        }
        if pkt_len < 4 {
            return Err(format!("pkt length {pkt_len} below 4-byte header"));
        }
        let payload_len = pkt_len - 4;
        let prev = out.len();
        out.resize(prev + payload_len, 0);
        reader
            .read_exact(&mut out[prev..])
            .map_err(|e| format!("read pkt payload: {e}"))?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_command_list_capability_detection_finds_sideband() {
        let command = b"00800000000000000000000000000000000000000000 1111111111111111111111111111111111111111 refs/heads/main\0report-status side-band-64k agent=git/2.50\n0000";

        assert!(command_list_declares_capability(command, "side-band-64k"));
        assert!(!command_list_declares_capability(command, "push-options"));
    }

    #[test]
    fn read_command_list_stops_before_pack_header_without_flush() {
        let mut body = Vec::new();
        let command = b"0000000000000000000000000000000000000000 1111111111111111111111111111111111111111 refs/heads/main\0 report-status side-band-64k agent=git/2.43.0\n";
        body.extend_from_slice(format!("{:04x}", command.len() + 4).as_bytes());
        body.extend_from_slice(command);
        body.extend_from_slice(b"PACKpack-bytes");
        let mut reader = std::io::BufReader::new(std::io::Cursor::new(body));

        let command_read = read_command_list(&mut reader).unwrap();
        let command_bytes = command_read.bytes;
        assert!(command_bytes.starts_with(format!("{:04x}", command.len() + 4).as_bytes()));
        assert!(command_bytes.ends_with(b"git/2.43.0\n"));
        assert_eq!(command_read.pack_prefix, b"PACK");

        let mut pack_rest = Vec::new();
        reader.read_to_end(&mut pack_rest).unwrap();
        assert_eq!(pack_rest, b"pack-bytes");
    }

    #[test]
    fn staged_pack_bytes_use_field_overflow_contract() {
        let serialized = serde_json::to_vec(&Value::String("x".repeat(FIELD_INLINE_MAX_BYTES)))
            .expect("serialize");
        let (key, value) = overflow_blob_ref_for_serialized(&serialized);

        assert!(key.starts_with(FIELD_OVERFLOW_BLOB_PREFIX));
        assert_eq!(value[FIELD_OVERFLOW_REF_KEY].as_str(), Some(key.as_str()));
        assert_eq!(
            value[FIELD_OVERFLOW_SIZE_KEY].as_u64(),
            Some(serialized.len() as u64)
        );
        assert_eq!(value[FIELD_OVERFLOW_ENCODING_KEY].as_str(), Some("json"));
    }
}
