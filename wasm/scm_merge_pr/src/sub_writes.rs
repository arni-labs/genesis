//! Composite sub-write rows for the merge engine.
//!
//! Row shapes are byte-compatible with `scm_ingest_pack::build_object_row`
//! (same field names, same overflow staging, same ADR-0011 raw-object
//! cache writes) so a commit written by a merge is indistinguishable
//! from one written by a push.

use alloc::format;
use alloc::string::{String, ToString};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde_json::Value;
use sha2::{Digest, Sha256};
use temper_wasm_sdk::prelude::*;

use crate::commits::BuiltCommit;
use crate::tree_merge::NewTree;

const FIELD_OVERFLOW_BLOB_PREFIX: &str = "field-overflow/sha256/";
const FIELD_OVERFLOW_REF_KEY: &str = "__temper_blob_ref";
const FIELD_OVERFLOW_SIZE_KEY: &str = "__temper_blob_size";
const FIELD_OVERFLOW_ENCODING_KEY: &str = "__temper_blob_encoding";

/// Deterministic placeholder timestamp used by all object rows the
/// SCM modules write (matches `scm_ingest_pack`): row metadata must
/// not depend on wall clock.
const OBJECT_ROW_CREATED_AT: &str = "1970-01-01T00:00:00Z";

/// Build the `Tree.Create` sub-write for a synthesized tree and write
/// the raw object body into the ADR-0011 cache.
pub fn tree_create_sub_write(
    ctx: &Context,
    blob_endpoint: &str,
    repository_id: &str,
    tree: &NewTree,
) -> Result<Value, String> {
    assert_eq!(tree.sha.len(), 40, "tree sha must be 40 hex chars");
    let body = object_body(&tree.canonical)?;
    cache_raw_object(ctx, blob_endpoint, repository_id, &tree.sha, body);
    let row = json!({
        "Id": tree.sha,
        "RepositoryId": repository_id,
        "CanonicalBytes": always_stage_field_value(ctx, blob_endpoint, B64.encode(&tree.canonical))?,
        "Status": "Durable",
        "CreatedAt": OBJECT_ROW_CREATED_AT,
    });
    Ok(sub_write(
        "Tree",
        &crate::odata::object_entity_id(repository_id, &tree.sha),
        "Create",
        row,
    ))
}

/// Build the `Commit.Create` sub-write for the merge/squash commit
/// and write the raw object body into the ADR-0011 cache.
pub fn commit_create_sub_write(
    ctx: &Context,
    blob_endpoint: &str,
    repository_id: &str,
    commit: &BuiltCommit,
) -> Result<Value, String> {
    assert_eq!(commit.sha.len(), 40, "commit sha must be 40 hex chars");
    let body = object_body(&commit.canonical)?;
    cache_raw_object(ctx, blob_endpoint, repository_id, &commit.sha, body);
    let row = json!({
        "Id": commit.sha,
        "RepositoryId": repository_id,
        "TreeSha": commit.tree_sha,
        "ParentShas": commit.parent_shas.join(","),
        "Author": commit.author,
        "Committer": commit.committer,
        "Message": commit.message,
        "CanonicalBytes": always_stage_field_value(ctx, blob_endpoint, B64.encode(&commit.canonical))?,
        "Status": "Durable",
        "CreatedAt": OBJECT_ROW_CREATED_AT,
    });
    Ok(sub_write(
        "Commit",
        &crate::odata::object_entity_id(repository_id, &commit.sha),
        "Create",
        row,
    ))
}

/// Compare-and-set advance of the target ref: the kernel rejects the
/// whole envelope if the ref moved since `previous_sha` was read.
pub fn ref_update_sub_write(ref_entity_id: &str, previous_sha: &str, new_sha: &str) -> Value {
    assert!(!ref_entity_id.is_empty(), "ref entity id must be non-empty");
    assert_ne!(previous_sha, new_sha, "ref CAS advance must change the tip");
    sub_write(
        "Ref",
        ref_entity_id,
        "Update",
        json!({
            "PreviousCommitSha": previous_sha,
            "NewCommitSha": new_sha,
            "TargetCommitSha": new_sha,
        }),
    )
}

/// The PullRequest state transition, emitted only on the
/// `Repository.MergePullRequest` trigger path (on the
/// `PullRequest.Merge` path the parent action itself is the
/// transition and a self-targeted sub-write would re-enter it).
pub fn pull_request_merge_sub_write(
    pr_entity_id: &str,
    strategy: &str,
    message: &str,
    client_request_id: &str,
    merge_commit_sha: &str,
) -> Value {
    assert!(!pr_entity_id.is_empty(), "pr entity id must be non-empty");
    assert_eq!(merge_commit_sha.len(), 40, "merge sha must be 40 hex chars");
    sub_write(
        "PullRequest",
        pr_entity_id,
        "Merge",
        json!({
            "Strategy": strategy,
            "Message": message,
            "ClientRequestId": client_request_id,
            // Not declared in pull_request.ioa.toml's Merge params today
            // (spec gap reported with this module); carried so the row
            // records which commit the merge produced.
            "MergeCommitSha": merge_commit_sha,
        }),
    )
}

fn sub_write(entity_type: &str, entity_id: &str, action: &str, params: Value) -> Value {
    json!({
        "entity_type": entity_type,
        "entity_id": entity_id,
        "action": action,
        "params": params,
    })
}

/// Strip the `<kind> <len>\0` header from canonical bytes.
fn object_body(canonical: &[u8]) -> Result<&[u8], String> {
    let nul = canonical
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| "canonical object bytes have no header NUL".to_string())?;
    Ok(&canonical[nul + 1..])
}

/// ADR-0011 raw-object cache write. Best-effort: a cache miss only
/// costs the next clone a row read, so failures are logged, not fatal.
fn cache_raw_object(
    ctx: &Context,
    blob_endpoint: &str,
    repository_id: &str,
    sha: &str,
    body: &[u8],
) {
    let blob_key = format!("git-objects/{repository_id}/{sha}.b64");
    let url = format!("{}/{blob_key}", blob_endpoint.trim_end_matches('/'));
    let outcome = ctx.http_call("PUT", &url, &[], &B64.encode(body));
    let failure = match outcome {
        Ok(resp) if (200..300).contains(&resp.status) => None,
        Ok(resp) => Some(format!("HTTP {}", resp.status)),
        Err(e) => Some(e),
    };
    if let Some(error) = failure {
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
}

/// Stage git object content to the object store unconditionally
/// (ADR-0027): rows carry metadata plus a content-addressed reference,
/// matching `scm_ingest_pack::always_stage_field_value`.
pub fn always_stage_field_value(
    ctx: &Context,
    blob_endpoint: &str,
    value: String,
) -> Result<Value, String> {
    let json_value = Value::String(value);
    let serialized =
        serde_json::to_vec(&json_value).map_err(|e| format!("object-content serialize: {e}"))?;

    let digest = Sha256::digest(&serialized);
    let blob_key = format!("{FIELD_OVERFLOW_BLOB_PREFIX}{digest:x}.json");
    put_overflow_blob(ctx, blob_endpoint, &blob_key, &serialized)?;
    Ok(json!({
        FIELD_OVERFLOW_REF_KEY: blob_key,
        FIELD_OVERFLOW_SIZE_KEY: serialized.len(),
        FIELD_OVERFLOW_ENCODING_KEY: "json",
    }))
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
