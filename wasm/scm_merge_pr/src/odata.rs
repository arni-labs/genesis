//! OData reads for the merge engine.
//!
//! Read shapes mirror `scm_ingest_pack` / `git_upload_pack`: object
//! rows live at `/tdata/{Set}('{entity_id}')` with the repository-
//! scoped entity id composed by [`object_entity_id`]; large fields
//! may be staged as ADR field-overflow blob refs and are resolved
//! transparently by [`resolve_field_value`].

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use genesis_git_object::ParsedTreeEntry;
use serde_json::Value;
use temper_wasm_sdk::prelude::*;

const TEMPER_API: &str = "http://127.0.0.1:3000";
const FIELD_OVERFLOW_REF_KEY: &str = "__temper_blob_ref";
const FIELD_OVERFLOW_ENCODING_KEY: &str = "__temper_blob_encoding";

/// Cap on PullRequest rows scanned when resolving a PR by number.
const PR_LOOKUP_PAGE_MAX: usize = 1000;

pub fn temper_api_base(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TEMPER_API.to_string())
}

pub fn blob_endpoint(ctx: &Context, api_base: &str) -> String {
    ctx.get_secret("blob_endpoint")
        .ok()
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{api_base}/_internal/blobs"))
}

/// Repository-scoped entity id for a git object row — identical
/// composition to `scm_ingest_pack::object_entity_id` so the rows it
/// wrote are addressable here.
pub fn object_entity_id(repository_id: &str, sha: &str) -> String {
    debug_assert_eq!(sha.len(), 40, "object sha must be 40 hex chars");
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

pub fn urlencode(s: &str) -> String {
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

pub fn odata_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// A PullRequest row, reduced to what the merge engine needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestRow {
    pub entity_id: String,
    pub repository_id: String,
    pub source_ref: String,
    pub target_ref: String,
    pub number: Option<u64>,
    pub title: String,
    pub status: String,
}

/// A Ref row: the entity id (for the CAS sub-write) and current tip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefRow {
    pub entity_id: String,
    pub target_commit_sha: String,
}

/// A Commit row, reduced to merge-engine needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRow {
    pub tree_sha: String,
    pub parent_shas: Vec<String>,
    pub committer: String,
}

pub fn fetch_pull_request(
    ctx: &Context,
    api_base: &str,
    pr_id: &str,
) -> Result<PullRequestRow, String> {
    assert!(!pr_id.is_empty(), "pull request id must be non-empty");
    let url = format!("{api_base}/tdata/PullRequests('{}')", urlencode(pr_id));
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch PullRequest({pr_id}): {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("PullRequest({pr_id}) status {}", resp.status));
    }
    let row: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("PullRequest({pr_id}) json: {e}"))?;
    pull_request_from_row(&row, pr_id)
}

/// Resolve a PR by repository + number. The number match happens
/// client-side so it works whether `Number` was stored as a JSON
/// number or string.
pub fn fetch_pull_request_by_number(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    number: u64,
) -> Result<PullRequestRow, String> {
    let filter = format!("RepositoryId eq {}", odata_string_literal(repository_id));
    let url = format!(
        "{api_base}/tdata/PullRequests?$filter={}&$top={PR_LOOKUP_PAGE_MAX}",
        urlencode(&filter)
    );
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("list PullRequests: {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("PullRequests list status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("PullRequests json: {e}"))?;
    let items = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for row in &items {
        let entity_id = row_entity_id(row);
        let Ok(pr) = pull_request_from_row(row, &entity_id) else {
            continue;
        };
        if pr.repository_id == repository_id && pr.number == Some(number) {
            return Ok(pr);
        }
    }
    Err(format!(
        "no pull request #{number} found on repository {repository_id}"
    ))
}

fn pull_request_from_row(row: &Value, pr_id: &str) -> Result<PullRequestRow, String> {
    let fields = row.get("fields").cloned().unwrap_or(Value::Null);
    let field = |name: &str| -> String {
        fields
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let repository_id = field("RepositoryId");
    let source_ref = field("SourceRef");
    let target_ref = field("TargetRef");
    if repository_id.is_empty() || source_ref.is_empty() || target_ref.is_empty() {
        return Err(format!(
            "PullRequest({pr_id}) row is missing RepositoryId/SourceRef/TargetRef"
        ));
    }
    let number = fields.get("Number").and_then(value_as_u64);
    let status = row
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| fields.get("Status").and_then(Value::as_str))
        .unwrap_or_default()
        .to_string();
    let entity_id = row
        .get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or(pr_id)
        .to_string();
    Ok(PullRequestRow {
        entity_id,
        repository_id,
        source_ref,
        target_ref,
        number,
        title: field("Title"),
        status,
    })
}

fn row_entity_id(row: &Value) -> String {
    row.get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

/// Fetch a Ref row by repository + full ref name.
pub fn fetch_ref(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    name: &str,
) -> Result<RefRow, String> {
    assert!(!name.is_empty(), "ref name must be non-empty");
    let filter = format!(
        "RepositoryId eq {} and Name eq {}",
        odata_string_literal(repository_id),
        odata_string_literal(name)
    );
    let url = format!("{api_base}/tdata/Refs?$filter={}&$top=10", urlencode(&filter));
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch Ref({name}): {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("Ref({name}) status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("Ref({name}) json: {e}"))?;
    let items = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for row in &items {
        let fields = row.get("fields").cloned().unwrap_or(Value::Null);
        let row_repo = fields
            .get("RepositoryId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let row_name = fields.get("Name").and_then(Value::as_str).unwrap_or_default();
        if row_repo != repository_id || row_name != name {
            continue;
        }
        let target = fields
            .get("TargetCommitSha")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if target.is_empty() {
            return Err(format!("Ref({name}) row has no TargetCommitSha"));
        }
        let mut entity_id = row_entity_id(row);
        if entity_id.is_empty() {
            // Same id composition `scm_ingest_pack::ref_id_for` uses
            // when creating refs — the canonical Ref row layout.
            entity_id = format!("rf-{}-{}", repository_id, name.replace('/', "-"));
        }
        return Ok(RefRow {
            entity_id,
            target_commit_sha: target,
        });
    }
    Err(format!(
        "ref '{name}' not found on repository {repository_id}"
    ))
}

/// Parents of a stored commit, for the merge-base walk. A missing row
/// (404 / empty) yields no parents — the walk cannot pass through it.
/// Transport-level failures are real errors and abort the merge.
pub fn fetch_commit_parents(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    sha: &str,
) -> Result<Vec<String>, String> {
    let row = fetch_commit_row(ctx, api_base, repository_id, sha)?;
    Ok(row.map(|c| c.parent_shas).unwrap_or_default())
}

/// Fetch a Commit row by SHA. `Ok(None)` = no such row.
pub fn fetch_commit_row(
    ctx: &Context,
    api_base: &str,
    repository_id: &str,
    sha: &str,
) -> Result<Option<CommitRow>, String> {
    assert_eq!(sha.len(), 40, "commit sha must be 40 hex chars");
    let entity_id = object_entity_id(repository_id, sha);
    let url = format!("{api_base}/tdata/Commits('{}')", urlencode(&entity_id));
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch Commit({sha}): {e}"))?;
    if resp.status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&resp.status) {
        return Err(format!("Commit({sha}) status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("Commit({sha}) json: {e}"))?;
    let Some(fields) = parsed.get("fields") else {
        return Ok(None);
    };
    let tree_sha = fields
        .get("TreeSha")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let parent_shas = fields
        .get("ParentShas")
        .and_then(Value::as_str)
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let committer = fields
        .get("Committer")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(Some(CommitRow {
        tree_sha,
        parent_shas,
        committer,
    }))
}

/// Fetch and parse a Tree object's entries from its row.
pub fn fetch_tree_entries(
    ctx: &Context,
    api_base: &str,
    blob_endpoint: &str,
    repository_id: &str,
    sha: &str,
) -> Result<Vec<ParsedTreeEntry>, String> {
    assert_eq!(sha.len(), 40, "tree sha must be 40 hex chars");
    let entity_id = object_entity_id(repository_id, sha);
    let url = format!("{api_base}/tdata/Trees('{}')", urlencode(&entity_id));
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch Tree({sha}): {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("Tree({sha}) status {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("Tree({sha}) json: {e}"))?;
    let canonical_value = parsed
        .get("fields")
        .and_then(|f| f.get("CanonicalBytes"))
        .ok_or_else(|| format!("Tree({sha}): no CanonicalBytes"))?;
    let canonical_b64 = resolve_field_value(ctx, blob_endpoint, canonical_value)?;
    let canonical = B64
        .decode(canonical_b64.trim())
        .map_err(|e| format!("Tree({sha}) base64 decode: {e}"))?;
    let nul = canonical
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| format!("Tree({sha}): no NUL in canonical bytes"))?;
    if !canonical.starts_with(b"tree ") {
        return Err(format!("Tree({sha}): canonical bytes are not a tree object"));
    }
    genesis_git_object::parse_tree(&canonical[nul + 1..])
        .map_err(|e| format!("Tree({sha}) parse: {e}"))
}

/// True if an object row already exists in the given set.
pub fn object_exists(
    ctx: &Context,
    api_base: &str,
    set: &str,
    repository_id: &str,
    sha: &str,
) -> Result<bool, String> {
    let entity_id = object_entity_id(repository_id, sha);
    let url = format!("{api_base}/tdata/{set}('{}')", urlencode(&entity_id));
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("probe {set}({sha}): {e}"))?;
    if resp.status == 404 {
        return Ok(false);
    }
    if !(200..300).contains(&resp.status) {
        return Err(format!("probe {set}({sha}) status {}", resp.status));
    }
    Ok(true)
}

/// Resolve a field value that may be inline or an ADR field-overflow
/// blob ref (`{"__temper_blob_ref": ..., "__temper_blob_encoding": "json"}`).
pub fn resolve_field_value(
    ctx: &Context,
    blob_endpoint: &str,
    value: &Value,
) -> Result<String, String> {
    if let Some(s) = value.as_str() {
        return Ok(s.to_string());
    }
    let Some(obj) = value.as_object() else {
        return Err("field value is neither string nor blob ref".to_string());
    };
    let Some(blob_key) = obj.get(FIELD_OVERFLOW_REF_KEY).and_then(Value::as_str) else {
        return Err("field value object has no blob ref key".to_string());
    };
    let encoding = obj
        .get(FIELD_OVERFLOW_ENCODING_KEY)
        .and_then(Value::as_str)
        .unwrap_or("json");
    if encoding != "json" {
        return Err(format!(
            "field-overflow encoding {encoding:?} is not supported"
        ));
    }
    let url = format!("{}/{blob_key}", blob_endpoint.trim_end_matches('/'));
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("field-overflow GET {blob_key}: {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "field-overflow GET {blob_key} returned HTTP {}",
            resp.status
        ));
    }
    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("field-overflow {blob_key} json: {e}"))?;
    parsed
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| format!("field-overflow {blob_key} is not a JSON string"))
}
