//! scm_ingest_pack — Repository.IngestPack parser for Composite sub-writes.
//!
//! This module is intentionally a pure integration-result producer. It reads
//! the spec-triggered `Repository.IngestPack` invocation context, verifies and
//! decomposes the pack bytes, then returns a `sub_writes` JSON envelope. The
//! Temper kernel validates that envelope against the Composite action contract
//! and applies the declared writes. This module does not call Temper actions.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::Value;
use sha2::{Digest, Sha256};
use temper_wasm_sdk::http_stream::streaming_call;
use temper_wasm_sdk::prelude::*;
use tg_wire::pack;

const TEMPER_API: &str = "http://127.0.0.1:3000";
const SYSTEM_TENANT: &str = "default";
const SYSTEM_PRINCIPAL: &str = "scm-ingest-pack";
const FIELD_INLINE_MAX_BYTES: usize = 131_072;
const FIELD_OVERFLOW_BLOB_PREFIX: &str = "field-overflow/sha256/";
const FIELD_OVERFLOW_REF_KEY: &str = "__temper_blob_ref";
const FIELD_OVERFLOW_SIZE_KEY: &str = "__temper_blob_size";
const FIELD_OVERFLOW_ENCODING_KEY: &str = "__temper_blob_encoding";
const HTTP_STREAM_READ_CHUNK_BYTES: usize = 64 * 1024;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let repository_id = ctx.entity_id.clone();
        let api_base = temper_api_base(&ctx);
        let blob_endpoint = blob_endpoint(&ctx, &api_base);
        let ref_updates = parse_ref_updates(&repository_id, &ctx.trigger_params)?;
        let pack_bytes = decode_pack_bytes(&blob_endpoint, &ctx.trigger_params)?;
        let pack_byte_count = pack_bytes.as_ref().map(Vec::len).unwrap_or_default();
        let objects = match pack_bytes {
            Some(pack_bytes) => parse_pack_objects(&ctx, &api_base, &repository_id, &pack_bytes)?,
            None => Vec::new(),
        };

        let pack_parents = pack_commit_parents(&objects);
        let ref_updates =
            classify_force_updates(&ctx, &api_base, &repository_id, &pack_parents, ref_updates);

        let mut sub_writes = Vec::new();
        for obj in objects {
            let (entity_type, row) = object_sub_write(&ctx, &blob_endpoint, &repository_id, obj)?;
            let object_sha = row
                .get("Id")
                .and_then(Value::as_str)
                .ok_or_else(|| "object row missing Id".to_string())?
                .to_string();
            let entity_id = object_entity_id(&repository_id, &object_sha);
            sub_writes.push(json!({
                "entity_type": entity_type,
                "entity_id": entity_id,
                "action": "Create",
                "params": row,
            }));
        }

        let object_count = sub_writes.len();
        let ref_update_count = ref_updates.len();
        let pr_updates = pr_head_updates_for_refs(&ctx, &api_base, &repository_id, &ref_updates)?;
        let pr_update_count = pr_updates.len();
        sub_writes.extend(ref_updates.into_iter().map(RefSubWrite::into_sub_write));
        sub_writes.extend(pr_updates);
        let sub_write_count = sub_writes.len();

        let _ = ctx.log_structured(
            "info",
            "scm_ingest_pack_result",
            &json!({
                "repository_id": repository_id,
                "pack_bytes": pack_byte_count,
                "object_count": object_count,
                "ref_update_count": ref_update_count,
                "pr_update_count": pr_update_count,
                "sub_write_count": sub_write_count,
            }),
        );

        Ok(json!({
            "object_count": object_count,
            "ref_update_count": ref_update_count,
            "pr_update_count": pr_update_count,
            "sub_writes": sub_writes,
        }))
    }
}

fn decode_pack_bytes(blob_endpoint: &str, params: &Value) -> Result<Option<Vec<u8>>, String> {
    let Some(raw) = params.get("PackBytes").or_else(|| params.get("pack_bytes")) else {
        return Ok(None);
    };
    if let Some(blob_key) = field_overflow_blob_key(raw)? {
        let serialized = get_overflow_blob(blob_endpoint, &blob_key)?;
        let value: Value = serde_json::from_str(&serialized)
            .map_err(|e| format!("PackBytes blob-ref JSON parse: {e}"))?;
        return decode_pack_bytes_value(&value);
    }

    decode_pack_bytes_value(raw)
}

fn decode_pack_bytes_value(raw: &Value) -> Result<Option<Vec<u8>>, String> {
    if let Some(encoded) = raw.as_str() {
        let bytes = B64
            .decode(encoded)
            .map_err(|e| format!("PackBytes base64 decode: {e}"))?;
        return Ok(if bytes.is_empty() { None } else { Some(bytes) });
    }

    if let Some(values) = raw.as_array() {
        let mut bytes = Vec::with_capacity(values.len());
        for value in values {
            let Some(byte) = value.as_u64().filter(|n| *n <= u8::MAX as u64) else {
                return Err("PackBytes array must contain byte values 0..255".to_string());
            };
            bytes.push(byte as u8);
        }
        return Ok(if bytes.is_empty() { None } else { Some(bytes) });
    }

    if raw.is_null() {
        return Ok(None);
    }

    Err("PackBytes must be a base64 string, byte array, blob ref, null, or omitted".to_string())
}

fn parse_pack_objects(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    pack_bytes: &[u8],
) -> Result<Vec<pack::PackObject>, String> {
    let cursor = std::io::Cursor::new(pack_bytes);
    let mut parser =
        pack::StreamingPackParser::begin(cursor).map_err(|e| format!("pack header: {e}"))?;
    let mut objects = Vec::with_capacity(parser.object_count() as usize);
    while let Some(obj) = parser
        .next_object_with_ref_delta_base(|sha| {
            fetch_existing_delta_base(ctx, api_base, repository_id, sha)
                .map_err(|e| pack::PackError::DeltaBaseMissing(format!("{sha}: {e}")))
        })
        .map_err(|e| format!("pack next: {e}"))?
    {
        objects.push(obj);
    }
    parser.finish().map_err(|e| format!("pack finish: {e}"))?;
    Ok(objects)
}

fn object_sub_write(
    ctx: &Context,
    blob_endpoint: &str,
    repository_id: &str,
    obj: pack::PackObject,
) -> Result<(&'static str, Value), String> {
    let kind_prefix = obj.kind.header_prefix();
    let sha = sha_from_prefix(kind_prefix, &obj.data);
    let mut canonical = format!("{} {}\0", kind_prefix, obj.data.len()).into_bytes();
    canonical.extend_from_slice(&obj.data);

    let entity_type = match obj.kind {
        pack::ObjectKind::Blob => "Blob",
        pack::ObjectKind::Tree => "Tree",
        pack::ObjectKind::Commit => "Commit",
        pack::ObjectKind::Tag => "Tag",
    };
    Ok((
        entity_type,
        build_object_row(
            ctx,
            blob_endpoint,
            obj.kind,
            &sha,
            repository_id,
            &obj.data,
            &canonical,
        )?,
    ))
}

#[derive(Debug, Clone, PartialEq)]
struct RefSubWrite {
    name: String,
    old_sha: String,
    new_sha: String,
    entity_id: String,
    action: &'static str,
    params: Value,
}

impl RefSubWrite {
    fn into_sub_write(self) -> Value {
        json!({
            "entity_type": "Ref",
            "entity_id": self.entity_id,
            "action": self.action,
            "params": self.params,
        })
    }
}

fn parse_ref_updates(repository_id: &str, params: &Value) -> Result<Vec<RefSubWrite>, String> {
    let Some(raw) = params
        .get("RefUpdates")
        .or_else(|| params.get("ref_updates"))
        .or_else(|| params.get("refUpdates"))
    else {
        return Ok(Vec::new());
    };
    let Some(items) = raw.as_array() else {
        return Err("RefUpdates must be an array".to_string());
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let name = read_string_any(item, &["Name", "Ref", "ref", "refname", "name"])
            .ok_or_else(|| "RefUpdates item missing ref name".to_string())?;
        let old_sha = read_string_any(
            item,
            &[
                "PreviousCommitSha",
                "OldCommitSha",
                "old_sha",
                "old",
                "previous",
            ],
        )
        .unwrap_or_default();
        let new_sha = read_string_any(
            item,
            &["NewCommitSha", "NewSha", "new_sha", "new", "target"],
        )
        .unwrap_or_default();
        let entity_id = read_string_any(item, &["Id", "RefId", "entity_id"])
            .unwrap_or_else(|| ref_id_for(repository_id, &name));

        if is_zero_sha(&new_sha) {
            out.push(RefSubWrite {
                name,
                old_sha: old_sha.clone(),
                new_sha,
                entity_id,
                action: "Delete",
                params: json!({
                    "PreviousCommitSha": old_sha,
                }),
            });
        } else if is_zero_sha(&old_sha) {
            out.push(RefSubWrite {
                name: name.clone(),
                old_sha: old_sha.clone(),
                new_sha: new_sha.clone(),
                entity_id,
                action: "Create",
                params: json!({
                    "RepositoryId": repository_id,
                    "Name": name,
                    "PreviousCommitSha": old_sha,
                    "TargetCommitSha": new_sha,
                    "Kind": if name.starts_with("refs/tags/") { "tag" } else { "branch" },
                }),
            });
        } else {
            out.push(RefSubWrite {
                name,
                old_sha: old_sha.clone(),
                new_sha: new_sha.clone(),
                entity_id,
                action: "Update",
                params: json!({
                    "PreviousCommitSha": old_sha,
                    "NewCommitSha": new_sha,
                    "TargetCommitSha": new_sha,
                }),
            });
        }
    }
    Ok(out)
}

/// Upper bound on the ancestry walk that classifies a ref update as
/// fast-forward vs force. Past this budget the update is treated as a
/// force-push: undecidable history rewrites must require the `force`
/// scope rather than slip through (ADR-0025).
const ANCESTRY_WALK_MAX_COMMITS: usize = 4096;

fn pack_commit_parents(
    objects: &[pack::PackObject],
) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut parents = std::collections::BTreeMap::new();
    for obj in objects {
        if obj.kind != pack::ObjectKind::Commit {
            continue;
        }
        let sha = sha_from_prefix(obj.kind.header_prefix(), &obj.data);
        if let Ok(commit) = genesis_git_object::parse_commit(&obj.data) {
            parents.insert(sha, commit.parents);
        }
    }
    parents
}

/// Reclassify plain `Update` sub-writes whose old tip is not an
/// ancestor of the new tip as `ForceUpdate`, so Cedar's `force` scope
/// gate becomes real on the push path (ADR-0025).
fn classify_force_updates(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    pack_parents: &std::collections::BTreeMap<String, Vec<String>>,
    mut ref_updates: Vec<RefSubWrite>,
) -> Vec<RefSubWrite> {
    for update in &mut ref_updates {
        if update.action != "Update" {
            continue;
        }
        let fast_forward =
            ancestry_walk_is_fast_forward(&update.old_sha, &update.new_sha, |sha| {
                pack_parents.get(sha).cloned().unwrap_or_else(|| {
                    fetch_commit_parents(ctx, api_base, repository_id, sha)
                })
            });
        if !fast_forward {
            update.action = "ForceUpdate";
            update.params = json!({ "NewCommitSha": update.new_sha });
            let _ = ctx.log_structured(
                "info",
                "scm_ingest_pack_force_update",
                &json!({
                    "repository_id": repository_id,
                    "ref": update.name,
                    "old_sha": update.old_sha,
                    "new_sha": update.new_sha,
                }),
            );
        }
    }
    ref_updates
}

/// Bounded BFS from `new_sha` through commit parents looking for
/// `old_sha`. Reachable → fast-forward. Exhausted or over budget →
/// not fast-forward (conservative: requires `force`).
fn ancestry_walk_is_fast_forward(
    old_sha: &str,
    new_sha: &str,
    mut parents_of: impl FnMut(&str) -> Vec<String>,
) -> bool {
    if old_sha == new_sha {
        return true;
    }
    let mut visited = std::collections::BTreeSet::new();
    let mut frontier = alloc::vec![new_sha.to_string()];
    while let Some(sha) = frontier.pop() {
        if sha == old_sha {
            return true;
        }
        if !visited.insert(sha.clone()) {
            continue;
        }
        if visited.len() >= ANCESTRY_WALK_MAX_COMMITS {
            return false;
        }
        frontier.extend(parents_of(&sha));
    }
    false
}

/// Parents of an already-stored commit, read from entity state.
/// Unknown commits (shallow history, missing rows) contribute no
/// parents — the walk simply cannot pass through them.
fn fetch_commit_parents(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    sha: &str,
) -> Vec<String> {
    let entity_id = object_entity_id(repository_id, sha);
    let url = format!("{api_base}/tdata/Commits('{entity_id}')");
    let Ok(resp) = ctx.http_call("GET", &url, &internal_read_headers(), "") else {
        return Vec::new();
    };
    if !(200..400).contains(&resp.status) {
        return Vec::new();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(&resp.body) else {
        return Vec::new();
    };
    parsed
        .get("fields")
        .and_then(|f| f.get("ParentShas"))
        .and_then(Value::as_str)
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn pr_head_updates_for_refs(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    ref_updates: &[RefSubWrite],
) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for update in ref_updates {
        if update.action == "Delete" || is_zero_sha(&update.new_sha) {
            continue;
        }
        for pr in
            fetch_open_pull_requests_for_source_ref(ctx, api_base, repository_id, &update.name)?
        {
            out.push(json!({
                "entity_type": "PullRequest",
                "entity_id": pr.entity_id,
                "action": "UpdateHead",
                "params": {
                    "NewHeadCommitSha": update.new_sha,
                    "HeadCommitSha": update.new_sha,
                },
            }));
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PullRequestTarget {
    entity_id: String,
}

fn fetch_open_pull_requests_for_source_ref(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    source_ref: &str,
) -> Result<Vec<PullRequestTarget>, String> {
    let filter = format!(
        "RepositoryId eq {} and SourceRef eq {}",
        odata_string_literal(repository_id),
        odata_string_literal(source_ref)
    );
    let url = format!(
        "{api_base}/tdata/PullRequests?$filter={}&$top=1000",
        urlencode(&filter)
    );
    let resp = ctx
        .http_call("GET", &url, &internal_read_headers(), "")
        .map_err(|e| format!("fetch PullRequests: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("PullRequests status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("pull requests json: {e}"))?;
    let items = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for row in items {
        let fields = row.get("fields").cloned().unwrap_or(Value::Null);
        let repo = fields
            .get("RepositoryId")
            .and_then(Value::as_str)
            .unwrap_or("");
        let source = fields
            .get("SourceRef")
            .and_then(Value::as_str)
            .unwrap_or("");
        if repo != repository_id || source != source_ref {
            continue;
        }

        let status = row
            .get("status")
            .and_then(Value::as_str)
            .or_else(|| fields.get("Status").and_then(Value::as_str))
            .or_else(|| fields.get("State").and_then(Value::as_str))
            .unwrap_or("");
        if !is_open_pull_request_status(status) {
            continue;
        }

        let entity_id = row
            .get("entity_id")
            .and_then(Value::as_str)
            .or_else(|| fields.get("Id").and_then(Value::as_str))
            .unwrap_or("");
        if entity_id.is_empty() {
            continue;
        }
        out.push(PullRequestTarget {
            entity_id: entity_id.to_string(),
        });
    }

    Ok(out)
}

fn is_open_pull_request_status(status: &str) -> bool {
    matches!(
        status,
        "Open" | "UnderReview" | "Approved" | "ChangesRequested"
    )
}

fn fetch_existing_delta_base(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    sha: &str,
) -> Result<Option<pack::PackObject>, String> {
    for (kind, set) in [
        (pack::ObjectKind::Commit, "Commits"),
        (pack::ObjectKind::Tree, "Trees"),
        (pack::ObjectKind::Blob, "Blobs"),
        (pack::ObjectKind::Tag, "Tags"),
    ] {
        if let Some(data) = fetch_existing_object_body(ctx, api_base, repository_id, set, sha)? {
            return Ok(Some(pack::PackObject { kind, data }));
        }
    }
    Ok(None)
}

fn fetch_existing_object_body(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    set: &str,
    sha: &str,
) -> Result<Option<Vec<u8>>, String> {
    let filter = format!(
        "Id eq {} and RepositoryId eq {}",
        odata_string_literal(sha),
        odata_string_literal(repository_id)
    );
    let url = format!("{api_base}/tdata/{set}?$filter={}", urlencode(&filter));
    let resp = ctx
        .http_call("GET", &url, &internal_read_headers(), "")
        .map_err(|e| format!("fetch {set}({sha}): {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("{set}({sha}) status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("object json: {e}"))?;
    let row = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .cloned();
    let Some(row) = row else {
        return Ok(None);
    };
    let fields = row
        .get("fields")
        .ok_or_else(|| format!("{set}({sha}): row has no fields"))?;
    let canonical_b64 = fields
        .get("CanonicalBytes")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{set}({sha}): no CanonicalBytes"))?;
    let canonical = B64
        .decode(canonical_b64)
        .map_err(|e| format!("base64 decode: {e}"))?;
    let nul = canonical
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("{set}({sha}): no NUL in canonical"))?;
    Ok(Some(canonical[nul + 1..].to_vec()))
}

fn temper_api_base(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TEMPER_API.to_string())
}

/// Headers for this integration's internal OData reads (PR-head lookup,
/// delta-base, ancestry). The parent Repository.IngestPack action was
/// already Cedar-gated on the push path; these reads run under the
/// module's trusted server-side identity rather than anonymous, which
/// would be denied and fail the whole composite (the module is not an
/// InboundHttp handler, so it has no caller headers to forward).
fn internal_read_headers() -> Vec<(String, String)> {
    alloc::vec![
        ("X-Tenant-Id".to_string(), SYSTEM_TENANT.to_string()),
        ("X-Temper-Principal-Kind".to_string(), "admin".to_string()),
        ("X-Temper-Principal-Id".to_string(), SYSTEM_PRINCIPAL.to_string()),
        (
            "X-Temper-Principal-Scopes".to_string(),
            "admin:repos,repo:read,repo:write,pr:write".to_string(),
        ),
        ("X-Temper-Agent-Type".to_string(), "system".to_string()),
    ]
}

fn blob_endpoint(ctx: &Context, api_base: &str) -> String {
    ctx.get_secret("blob_endpoint")
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{api_base}/_internal/blobs"))
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

fn read_string_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn is_zero_sha(value: &str) -> bool {
    value.is_empty() || value.chars().all(|c| c == '0')
}

fn ref_id_for(repository_id: &str, refname: &str) -> String {
    format!("rf-{}-{}", repository_id, refname.replace('/', "-"))
}

fn object_entity_id(repository_id: &str, sha: &str) -> String {
    let mut repo = String::with_capacity(repository_id.len());
    let mut last_dash = false;
    for ch in repository_id.chars() {
        if ch.is_ascii_alphanumeric() {
            repo.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            repo.push('-');
            last_dash = true;
        }
    }
    let repo = repo.trim_matches('-');
    if repo.is_empty() {
        format!("obj-{sha}")
    } else {
        format!("{repo}-{sha}")
    }
}

fn build_object_row(
    ctx: &Context,
    blob_endpoint: &str,
    kind: pack::ObjectKind,
    sha: &str,
    repository_id: &str,
    raw: &[u8],
    canonical: &[u8],
) -> Result<Value, String> {
    if let Err(error) = put_raw_git_object_cache(ctx, blob_endpoint, repository_id, sha, raw) {
        let _ = ctx.log_structured(
            "warn",
            "git_object_cache_write_failed",
            &json!({
                "repository_id": repository_id,
                "sha": sha,
                "error": error,
            }),
        );
    }

    let canonical_b64 = B64.encode(canonical);
    let created_at = "1970-01-01T00:00:00Z";
    Ok(match kind {
        pack::ObjectKind::Blob => json!({
            "Id": sha,
            "RepositoryId": repository_id,
            "Size": raw.len(),
            "Content": always_stage_field_value(ctx, blob_endpoint, B64.encode(raw))?,
            "CanonicalBytes": always_stage_field_value(ctx, blob_endpoint, canonical_b64)?,
            "Status": "Durable",
            "CreatedAt": created_at,
        }),
        pack::ObjectKind::Tree => json!({
            "Id": sha,
            "RepositoryId": repository_id,
            "CanonicalBytes": always_stage_field_value(ctx, blob_endpoint, canonical_b64)?,
            "Status": "Durable",
            "CreatedAt": created_at,
        }),
        pack::ObjectKind::Commit => {
            let parsed = genesis_git_object::parse_commit(raw).ok();
            let (tree, parents, author, committer, message, gpg) = match &parsed {
                Some(c) => (
                    c.tree.clone(),
                    c.parents.join(","),
                    c.author.clone(),
                    c.committer.clone(),
                    c.message.clone(),
                    c.gpg_signature.clone(),
                ),
                None => Default::default(),
            };
            let mut row = json!({
                "Id": sha,
                "RepositoryId": repository_id,
                "TreeSha": tree,
                "ParentShas": parents,
                "Author": author,
                "Committer": committer,
                "Message": message,
                "CanonicalBytes": always_stage_field_value(ctx, blob_endpoint, canonical_b64)?,
                "Status": "Durable",
                "CreatedAt": created_at,
            });
            if let Some(sig) = gpg {
                row["PgpSignature"] = Value::String(sig);
            }
            row
        }
        pack::ObjectKind::Tag => {
            let parsed = genesis_git_object::parse_tag(raw).ok();
            let (target, ttype, name, tagger, message, gpg) = match &parsed {
                Some(t) => (
                    t.object.clone(),
                    t.target_type.clone(),
                    t.tag.clone(),
                    t.tagger.clone(),
                    t.message.clone(),
                    t.gpg_signature.clone(),
                ),
                None => Default::default(),
            };
            let mut row = json!({
                "Id": sha,
                "RepositoryId": repository_id,
                "TargetSha": target,
                "TargetType": ttype,
                "TagName": name,
                "Tagger": tagger,
                "Message": message,
                "CanonicalBytes": always_stage_field_value(ctx, blob_endpoint, canonical_b64)?,
                "Status": "Durable",
                "CreatedAt": created_at,
            });
            if let Some(sig) = gpg {
                row["PgpSignature"] = Value::String(sig);
            }
            row
        }
    })
}

fn put_raw_git_object_cache(
    ctx: &Context,
    blob_endpoint: &str,
    repository_id: &str,
    sha: &str,
    raw: &[u8],
) -> Result<(), String> {
    let blob_key = format!("git-objects/{repository_id}/{sha}.b64");
    let url = format!("{}/{blob_key}", blob_endpoint.trim_end_matches('/'));
    let response = ctx
        .http_call("PUT", &url, &[], &B64.encode(raw))
        .map_err(|e| format!("raw-object cache PUT {sha}: {e}"))?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        let _ = ctx.log_structured(
            "warn",
            "git_object_cache_put_rejected",
            &json!({
                "repository_id": repository_id,
                "sha": sha,
                "status": response.status,
            }),
        );
        Err(format!(
            "raw-object cache PUT returned HTTP {}",
            response.status
        ))
    }
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

fn field_overflow_blob_key(value: &Value) -> Result<Option<String>, String> {
    let Some(obj) = value.as_object() else {
        return Ok(None);
    };
    let Some(blob_key) = obj.get(FIELD_OVERFLOW_REF_KEY).and_then(Value::as_str) else {
        return Ok(None);
    };
    let encoding = obj
        .get(FIELD_OVERFLOW_ENCODING_KEY)
        .and_then(Value::as_str)
        .unwrap_or("json");
    if encoding != "json" {
        return Err(format!(
            "PackBytes blob-ref encoding {encoding:?} is not supported"
        ));
    }
    Ok(Some(blob_key.to_string()))
}

/// Stage a value to the object store unconditionally (ADR-0027): git
/// object content never sits inline on entity rows; the row keeps a
/// content-addressed reference. Readers already resolve these refs and
/// legacy inline rows stay readable.
fn always_stage_field_value(
    ctx: &Context,
    blob_endpoint: &str,
    value: String,
) -> Result<Value, String> {
    let json_value = Value::String(value);
    let serialized =
        serde_json::to_vec(&json_value).map_err(|e| format!("object-content serialize: {e}"))?;
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

fn get_overflow_blob(blob_endpoint: &str, blob_key: &str) -> Result<String, String> {
    let url = format!("{}/{blob_key}", blob_endpoint.trim_end_matches('/'));
    let (request_body, mut response_body, response_head) = streaming_call("GET", &url, &[])
        .map_err(|e| format!("field-overflow GET {blob_key} stream begin: {e}"))?;
    request_body
        .finish()
        .map_err(|e| format!("field-overflow GET {blob_key} request close: {e}"))?;
    let head =
        response_head().map_err(|e| format!("field-overflow GET {blob_key} response head: {e}"))?;
    if !(200..300).contains(&head.status) {
        let _ = response_body.close();
        return Err(format!(
            "field-overflow GET {blob_key} returned HTTP {}",
            head.status
        ));
    }

    let mut out = Vec::new();
    let mut buf = alloc::vec![0u8; HTTP_STREAM_READ_CHUNK_BYTES];
    loop {
        let Some(n) = response_body
            .read_next_chunk(&mut buf)
            .map_err(|e| format!("field-overflow GET {blob_key} response body: {e}"))?
        else {
            break;
        };
        out.extend_from_slice(&buf[..n]);
    }
    let _ = response_body.close();
    String::from_utf8(out).map_err(|e| format!("field-overflow GET {blob_key} utf8: {e}"))
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

fn sha_from_prefix(prefix: &str, body: &[u8]) -> String {
    let header = format!("{} {}\0", prefix, body.len());
    let mut hasher = genesis_git_object::Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(body);
    hasher.hex()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_update_projects_target_commit_sha() {
        let updates = parse_ref_updates(
            "rp-acme-demo",
            &json!({
                "RefUpdates": [{
                    "Name": "refs/heads/main",
                    "PreviousCommitSha": "1111111111111111111111111111111111111111",
                    "NewCommitSha": "2222222222222222222222222222222222222222"
                }]
            }),
        )
        .unwrap();

        assert_eq!(updates.len(), 1);
        let update = &updates[0];
        assert_eq!(update.action, "Update");
        assert_eq!(
            update.params["NewCommitSha"],
            "2222222222222222222222222222222222222222"
        );
        assert_eq!(
            update.params["TargetCommitSha"],
            "2222222222222222222222222222222222222222"
        );
    }

    #[test]
    fn ref_create_carries_previous_sha_for_kernel_cas() {
        let update = parse_ref_updates(
            "rp-acme-demo",
            &json!({
                "RefUpdates": [{
                    "Name": "refs/heads/main",
                    "PreviousCommitSha": "0000000000000000000000000000000000000000",
                    "NewCommitSha": "2222222222222222222222222222222222222222"
                }]
            }),
        )
        .unwrap()
        .remove(0);

        assert_eq!(update.action, "Create");
        assert_eq!(
            update.params["PreviousCommitSha"],
            "0000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn ref_update_carries_previous_sha_for_kernel_cas() {
        let update = parse_ref_updates(
            "rp-acme-demo",
            &json!({
                "RefUpdates": [{
                    "Name": "refs/heads/main",
                    "PreviousCommitSha": "1111111111111111111111111111111111111111",
                    "NewCommitSha": "2222222222222222222222222222222222222222"
                }]
            }),
        )
        .unwrap()
        .remove(0);

        assert_eq!(update.action, "Update");
        assert_eq!(
            update.params["PreviousCommitSha"],
            "1111111111111111111111111111111111111111"
        );
    }

    #[test]
    fn pr_update_status_filter_matches_open_states_only() {
        for status in ["Open", "UnderReview", "Approved", "ChangesRequested"] {
            assert!(is_open_pull_request_status(status), "{status} should match");
        }
        for status in ["Draft", "Merged", "Closed", ""] {
            assert!(
                !is_open_pull_request_status(status),
                "{status} should not match"
            );
        }
    }

    #[test]
    fn odata_literals_escape_quotes_before_url_encoding() {
        let filter = format!(
            "RepositoryId eq {} and SourceRef eq {}",
            odata_string_literal("repo ' one"),
            odata_string_literal("refs/heads/feature/a b")
        );

        assert_eq!(
            filter,
            "RepositoryId eq 'repo '' one' and SourceRef eq 'refs/heads/feature/a b'"
        );
        assert_eq!(
            urlencode(&filter),
            "RepositoryId%20eq%20%27repo%20%27%27%20one%27%20and%20SourceRef%20eq%20%27refs%2Fheads%2Ffeature%2Fa%20b%27"
        );
    }

    #[test]
    fn overflow_blob_ref_matches_temper_field_contract() {
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

    fn chain_parents(chain: &[(&str, &[&str])]) -> std::collections::BTreeMap<String, Vec<String>> {
        chain
            .iter()
            .map(|(sha, parents)| {
                (
                    sha.to_string(),
                    parents.iter().map(|p| p.to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn fast_forward_when_old_tip_is_ancestor() {
        let graph = chain_parents(&[("c3", &["c2"]), ("c2", &["c1"]), ("c1", &[])]);
        assert!(ancestry_walk_is_fast_forward("c1", "c3", |sha| {
            graph.get(sha).cloned().unwrap_or_default()
        }));
    }

    #[test]
    fn force_when_history_diverged() {
        // c3 rewrites history: its line never reaches c9.
        let graph = chain_parents(&[("c3", &["c2"]), ("c2", &["c1"]), ("c1", &[])]);
        assert!(!ancestry_walk_is_fast_forward("c9", "c3", |sha| {
            graph.get(sha).cloned().unwrap_or_default()
        }));
    }

    #[test]
    fn fast_forward_through_merge_parent() {
        let graph = chain_parents(&[
            ("m1", &["c2", "f1"]),
            ("f1", &["c1"]),
            ("c2", &["c1"]),
            ("c1", &[]),
        ]);
        assert!(ancestry_walk_is_fast_forward("f1", "m1", |sha| {
            graph.get(sha).cloned().unwrap_or_default()
        }));
    }

    #[test]
    fn same_tip_is_fast_forward() {
        assert!(ancestry_walk_is_fast_forward("c1", "c1", |_| Vec::new()));
    }

    #[test]
    fn budget_exhaustion_classifies_as_force() {
        // A synthetic endless chain: every sha has one fabricated parent.
        let mut calls = 0usize;
        let result = ancestry_walk_is_fast_forward("never-found", "c0", |sha| {
            calls += 1;
            alloc::vec![format!("{sha}x")]
        });
        assert!(!result);
        assert!(calls <= ANCESTRY_WALK_MAX_COMMITS);
    }
}
