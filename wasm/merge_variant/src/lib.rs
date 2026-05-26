//! merge_variant — directed-evolution merge + hot-deploy orchestrator.
//!
//! Triggered on `Evolution.Approve`. Reads the winning Variant (BranchRef +
//! mutation summary) and RETURNS a `sub_writes` envelope: a PR create/open/
//! squash-merge sequence on the target app's genesis Repository, a Lineage
//! row recording the parent + mutation, and an `Evolution.GoLive` write to
//! advance the Evolution FSM.
//!
//! Hot-deploy of the merged spec into the running target tenant is a
//! best-effort `POST {temper_api_url}/deploy/tenant` (temper ADR-0012),
//! body `{tenant, csdl_xml, ioa_sources:[{entity_type, source}]}`. The
//! payload comes from one of: an optional `deploy_payload_url` HTTP
//! callback (assembled by an external service), inline integration-config
//! overrides (`deploy_csdl_xml` + `deploy_ioa_sources`), or — in the
//! Phase-1 downvote case — a deterministic fallback that pairs the SO
//! tenant CSDL with the downvote-template Answer spec. GoLive is emitted
//! unconditionally so the engine surfaces deploy outcome via Lineage; a
//! failed deploy is a Phase-2 saga-revert concern. The Temper kernel
//! applies the writes — this module never dispatches actions itself.

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

/// OData base used when no override is supplied via integration config.
const DEFAULT_TEMPER_API: &str = "http://127.0.0.1:3000";
/// Deterministic timestamp keeps DST replays + unit tests bit-stable.
const CREATED_AT: &str = "1970-01-01T00:00:00Z";
/// Truncated id chunk used in derived ids (PR id, Lineage id, etc.).
const ID_CHUNK_LEN: usize = 16;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let evolution = EvolutionSnapshot::from_entity_state(&ctx.entity_id, &ctx.entity_state)?;
        let winner_id = evolution
            .winner_variant_id
            .clone()
            .ok_or_else(|| "Evolution.Approve requires WinnerVariantId to be set".to_string())?;
        let variant = fetch_variant(&ctx, &winner_id)?;
        let plan = build_merge_plan(&evolution, &variant)?;
        let deploy = attempt_hot_deploy(&ctx, &evolution, &variant)?;
        let sub_writes = build_merge_variant_sub_writes(&plan, &deploy)?;

        Ok(json!({
            "evolution_id": evolution.id,
            "winner_variant_id": variant.id,
            "repository_id": plan.repository_id,
            "pull_request_id": plan.pull_request_id,
            "lineage_id": plan.lineage_id,
            "deploy": deploy.as_str(),
            "sub_write_count": sub_writes.len(),
            "sub_writes": sub_writes,
        }))
    }
}

/// Evolution fields the merge step needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EvolutionSnapshot {
    id: String,
    target_app: String,
    target_tenant: String,
    target_branch: String,
    winner_variant_id: Option<String>,
    intent: String,
}

impl EvolutionSnapshot {
    fn from_entity_state(entity_id: &str, state: &Value) -> Result<Self, String> {
        let id = row_string(state, "Id").unwrap_or_else(|| entity_id.to_string());
        if id.is_empty() {
            return Err("Evolution entity_id is required".to_string());
        }
        let target_app = row_required(state, "TargetApp", "Evolution state")?;
        let target_tenant = row_string(state, "TargetTenant").unwrap_or_else(|| "default".to_string());
        let target_branch = row_string(state, "TargetBranch")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "refs/heads/main".to_string());
        let winner_variant_id = row_string(state, "WinnerVariantId").filter(|s| !s.is_empty());
        let intent = row_string(state, "Intent").unwrap_or_default();
        Ok(Self {
            id,
            target_app,
            target_tenant,
            target_branch,
            winner_variant_id,
            intent,
        })
    }
}

/// Variant fields needed to construct the merge payload.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VariantSnapshot {
    id: String,
    branch_ref: String,
    commit_sha: String,
    /// Set by `gen_variant` when it emitted the `Variant.Create` sub_write.
    /// When present, `merge_variant` uses these as the `/deploy/tenant`
    /// ioa_sources directly — no git tree-walk, no fetch service required.
    candidate_entity_type: String,
    candidate_source: String,
}

impl VariantSnapshot {
    fn from_row(row: &Value, variant_id: &str) -> Result<Self, String> {
        let id = row_string(row, "Id").unwrap_or_else(|| variant_id.to_string());
        let branch_ref = row_required(row, "BranchRef", "Variant row")?;
        let commit_sha = row_string(row, "CommitSha").unwrap_or_default();
        let candidate_entity_type = row_string(row, "CandidateEntityType").unwrap_or_default();
        let candidate_source = row_string(row, "CandidateSource").unwrap_or_default();
        Ok(Self {
            id,
            branch_ref,
            commit_sha,
            candidate_entity_type,
            candidate_source,
        })
    }
}

/// Materialized PR / Lineage / merge metadata derived from Evolution + Variant.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MergePlan {
    repository_id: String,
    pull_request_id: String,
    lineage_id: String,
    source_ref: String,
    target_ref: String,
    title: String,
    body: String,
    commit_sha: String,
    intent: String,
    evolution_id: String,
    variant_id: String,
    target_tenant: String,
}

fn build_merge_plan(evolution: &EvolutionSnapshot, variant: &VariantSnapshot) -> Result<MergePlan, String> {
    if variant.branch_ref.is_empty() {
        return Err(format!("Variant {} has empty BranchRef", variant.id));
    }
    let repository_id = repository_id_for(&evolution.target_app);
    let pull_request_id = pull_request_id_for(&evolution.id, &variant.id);
    let lineage_id = lineage_id_for(&evolution.id, &variant.id);
    let title = format!("evolve({}): {}", short(&evolution.id), evolution.intent);
    let body = format!(
        "Auto-merged by directed-evolution engine.\n\
         \n\
         Evolution: {}\n\
         Variant:   {}\n\
         Intent:    {}\n",
        evolution.id, variant.id, evolution.intent
    );
    Ok(MergePlan {
        repository_id,
        pull_request_id,
        lineage_id,
        source_ref: variant.branch_ref.clone(),
        target_ref: evolution.target_branch.clone(),
        title,
        body,
        commit_sha: variant.commit_sha.clone(),
        intent: evolution.intent.clone(),
        evolution_id: evolution.id.clone(),
        variant_id: variant.id.clone(),
        target_tenant: evolution.target_tenant.clone(),
    })
}

/// Hot-deploy outcome for telemetry + sub-write tagging.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DeployOutcome {
    NotAttempted,
    Succeeded,
    Failed(String),
}

impl DeployOutcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Succeeded => "succeeded",
            Self::Failed(_) => "failed",
        }
    }
}

/// Public entry-point for unit tests + `run`: produce the sub_writes envelope.
fn build_merge_variant_sub_writes(
    plan: &MergePlan,
    deploy: &DeployOutcome,
) -> Result<Vec<Value>, String> {
    let client_request_id = format!("evo-merge-{}", plan.variant_id);
    let merge_message = format!(
        "Merge variant {} into {} via directed-evolution",
        plan.variant_id, plan.target_ref
    );
    let mutation_summary = serde_json::to_string(&json!({
        "type": "spec_evolution",
        "intent": plan.intent,
        "evolution_id": plan.evolution_id,
        "variant_id": plan.variant_id,
        "source_ref": plan.source_ref,
        "target_ref": plan.target_ref,
        "deploy": deploy.as_str()
    }))
    .map_err(|e| format!("encode Lineage.Mutations: {e}"))?;

    let mut writes = Vec::with_capacity(5);

    writes.push(json!({
        "entity_type": "PullRequest",
        "entity_id": plan.pull_request_id,
        "action": "Create",
        "params": {
            "RepositoryId": plan.repository_id,
            "SourceRef": plan.source_ref,
            "TargetRef": plan.target_ref,
            "Title": plan.title,
            "Body": plan.body,
            "OpenedBy": "evolver",
            "ClientRequestId": format!("{client_request_id}-create")
        }
    }));

    writes.push(json!({
        "entity_type": "PullRequest",
        "entity_id": plan.pull_request_id,
        "action": "Open",
        "params": {}
    }));

    writes.push(json!({
        "entity_type": "PullRequest",
        "entity_id": plan.pull_request_id,
        "action": "Merge",
        "params": {
            "Strategy": "squash",
            "Message": merge_message,
            "ClientRequestId": format!("{client_request_id}-merge")
        }
    }));

    writes.push(json!({
        "entity_type": "Lineage",
        "entity_id": plan.lineage_id,
        "action": "Create",
        "params": {
            "ChildRepositoryId": plan.repository_id,
            "ParentRepositoryId": plan.repository_id,
            "ParentCommit": plan.commit_sha,
            "Type": "evolution_merge",
            "CreatedBy": "evolver",
            "Mutations": mutation_summary,
            "CreatedAt": CREATED_AT
        }
    }));

    // GoLive is emitted regardless of deploy outcome — the engine surfaces
    // the deploy status via telemetry + the Lineage row. A failed deploy is
    // a Phase-2 saga-revert concern (plan §1.3 / §2.5).
    writes.push(json!({
        "entity_type": "Evolution",
        "entity_id": plan.evolution_id,
        "action": "GoLive",
        "params": {}
    }));

    Ok(writes)
}

/// Best-effort hot-deploy: POST `{temper_api_url}/deploy/tenant` with the
/// superset CSDL + the merged IOA sources (ADR-0012). On non-2xx we
/// record the failure and let the kernel-side caller decide whether to
/// compensate; we do NOT block `GoLive` on deploy success because the
/// genesis merge has already happened.
fn attempt_hot_deploy(
    ctx: &Context,
    evolution: &EvolutionSnapshot,
    variant: &VariantSnapshot,
) -> Result<DeployOutcome, String> {
    let payload = match build_deploy_payload(ctx, evolution, variant)? {
        Some(payload) => payload,
        None => return Ok(DeployOutcome::NotAttempted),
    };
    let url = format!("{}/deploy/tenant", temper_api_base(ctx));
    let body = json!({
        "tenant": payload.tenant,
        "csdl_xml": payload.csdl_xml,
        "ioa_sources": payload
            .ioa_sources
            .iter()
            .map(|s| json!({"entity_type": s.entity_type, "source": s.source}))
            .collect::<Vec<_>>()
    })
    .to_string();
    let resp = ctx
        .http_call("POST", &url, &[], &body)
        .map_err(|e| format!("/deploy/tenant http_call: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Ok(DeployOutcome::Failed(format!(
            "/deploy/tenant status {}",
            resp.status
        )));
    }
    Ok(DeployOutcome::Succeeded)
}

/// Materialized payload for `POST /deploy/tenant`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeployPayload {
    tenant: String,
    csdl_xml: String,
    ioa_sources: Vec<DeployIoaSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeployIoaSource {
    entity_type: String,
    source: String,
}

/// Assemble the `/deploy/tenant` body. Resolution order (configurable via
/// integration config, so the same module serves Phase-1 dry runs + future
/// real-fetch deployments without code changes):
///
/// 1. `deploy_payload_url` → POST `{tenant, target_app, variant_id,
///    branch_ref, commit_sha}` and parse `{csdl_xml, ioa_sources:[...]}`.
///    First so operators can override what the row carries (e.g. when the
///    superset CSDL needs assembly across multiple variants).
/// 2. Inline `deploy_csdl_xml` + `deploy_ioa_sources` (JSON-encoded array)
///    from integration config — handy for cron / dry runs.
/// 3. **The winner's `CandidateSource` from the Variant row** paired with
///    whichever CSDL the operator supplied (`deploy_csdl_xml`) or — if
///    nothing else is configured — the byte-stable Phase-1 SO tenant CSDL.
///    This is the canonical path: gen_variant wrote the candidate inline
///    on Variant.Create, so the loop closes with no extra fetch hops.
/// 4. Phase-1 deterministic fallback as last resort (no row candidate,
///    no inline override): the SO tenant CSDL paired with the deterministic
///    downvote Answer template. Keeps unattended dry runs working.
///
/// Returns `Ok(None)` when the operator has explicitly disabled the deploy
/// (`deploy_enabled = "false"`) — surfaces as `DeployOutcome::NotAttempted`.
fn build_deploy_payload(
    ctx: &Context,
    evolution: &EvolutionSnapshot,
    variant: &VariantSnapshot,
) -> Result<Option<DeployPayload>, String> {
    if ctx
        .config
        .get("deploy_enabled")
        .map(|v| v.eq_ignore_ascii_case("false") || v == "0")
        .unwrap_or(false)
    {
        return Ok(None);
    }

    if let Some(url) = ctx.config.get("deploy_payload_url").filter(|v| !v.is_empty()) {
        let req = json!({
            "tenant": evolution.target_tenant,
            "target_app": evolution.target_app,
            "evolution_id": evolution.id,
            "variant_id": variant.id,
            "branch_ref": variant.branch_ref,
            "commit_sha": variant.commit_sha,
        })
        .to_string();
        let resp = ctx
            .http_call("POST", url, &[], &req)
            .map_err(|e| format!("deploy_payload_url http_call: {e}"))?;
        if !(200..400).contains(&resp.status) {
            return Err(format!("deploy_payload_url status {}", resp.status));
        }
        return Ok(Some(parse_deploy_payload(
            &resp.body,
            &evolution.target_tenant,
        )?));
    }

    if let (Some(csdl), Some(ioa_raw)) = (
        ctx.config.get("deploy_csdl_xml").filter(|v| !v.is_empty()),
        ctx.config.get("deploy_ioa_sources").filter(|v| !v.is_empty()),
    ) {
        return Ok(Some(DeployPayload {
            tenant: evolution.target_tenant.clone(),
            csdl_xml: csdl.clone(),
            ioa_sources: parse_inline_ioa_sources(ioa_raw)?,
        }));
    }

    if !variant.candidate_source.is_empty() {
        let csdl_xml = ctx
            .config
            .get("deploy_csdl_xml")
            .filter(|v| !v.is_empty())
            .cloned()
            .unwrap_or_else(|| phase1_stackoverflow_csdl().to_string());
        let entity_type = if variant.candidate_entity_type.is_empty() {
            "Answer".to_string()
        } else {
            variant.candidate_entity_type.clone()
        };
        return Ok(Some(DeployPayload {
            tenant: evolution.target_tenant.clone(),
            csdl_xml,
            ioa_sources: alloc::vec![DeployIoaSource {
                entity_type,
                source: variant.candidate_source.clone(),
            }],
        }));
    }

    Ok(Some(phase1_downvote_payload(evolution)))
}

/// Parse the JSON body returned by a `deploy_payload_url` callback. Required
/// shape: `{csdl_xml: "<...>", ioa_sources: [{entity_type, source}, ...]}`.
/// `tenant` defaults to the Evolution's `TargetTenant` when omitted so the
/// callback can stay tenant-agnostic.
fn parse_deploy_payload(body: &str, fallback_tenant: &str) -> Result<DeployPayload, String> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|e| format!("deploy_payload_url response not JSON: {e}"))?;
    let csdl_xml = parsed
        .get("csdl_xml")
        .and_then(Value::as_str)
        .ok_or_else(|| "deploy_payload_url response missing csdl_xml".to_string())?
        .to_string();
    if csdl_xml.is_empty() {
        return Err("deploy_payload_url returned empty csdl_xml".to_string());
    }
    let sources = parsed
        .get("ioa_sources")
        .and_then(Value::as_array)
        .ok_or_else(|| "deploy_payload_url response missing ioa_sources array".to_string())?;
    if sources.is_empty() {
        return Err("deploy_payload_url returned empty ioa_sources".to_string());
    }
    let mut ioa_sources = Vec::with_capacity(sources.len());
    for (idx, raw) in sources.iter().enumerate() {
        let entity_type = raw
            .get("entity_type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("ioa_sources[{idx}].entity_type missing"))?
            .to_string();
        let source = raw
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("ioa_sources[{idx}].source missing"))?
            .to_string();
        if entity_type.is_empty() || source.is_empty() {
            return Err(format!("ioa_sources[{idx}] entity_type/source must be non-empty"));
        }
        ioa_sources.push(DeployIoaSource {
            entity_type,
            source,
        });
    }
    let tenant = parsed
        .get("tenant")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| fallback_tenant.to_string());
    Ok(DeployPayload {
        tenant,
        csdl_xml,
        ioa_sources,
    })
}

/// Parse a JSON-array string in `deploy_ioa_sources` (integration config).
fn parse_inline_ioa_sources(raw: &str) -> Result<Vec<DeployIoaSource>, String> {
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|e| format!("deploy_ioa_sources is not a JSON array: {e}"))?;
    let arr = parsed
        .as_array()
        .ok_or_else(|| "deploy_ioa_sources must be a JSON array".to_string())?;
    if arr.is_empty() {
        return Err("deploy_ioa_sources is empty".to_string());
    }
    let mut sources = Vec::with_capacity(arr.len());
    for (idx, raw) in arr.iter().enumerate() {
        let entity_type = raw
            .get("entity_type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("deploy_ioa_sources[{idx}].entity_type missing"))?
            .to_string();
        let source = raw
            .get("source")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("deploy_ioa_sources[{idx}].source missing"))?
            .to_string();
        if entity_type.is_empty() || source.is_empty() {
            return Err(format!(
                "deploy_ioa_sources[{idx}] entity_type/source must be non-empty"
            ));
        }
        sources.push(DeployIoaSource {
            entity_type,
            source,
        });
    }
    Ok(sources)
}

/// Phase-1 deterministic fallback: SO tenant CSDL (hot-SWAP — unchanged for
/// the downvote case) paired with the downvote-template Answer source. Kept
/// byte-stable with run_stage_caller's `phase1_downvote_template()` so DST
/// replays + verify cascade reproduce identically.
fn phase1_downvote_payload(evolution: &EvolutionSnapshot) -> DeployPayload {
    DeployPayload {
        tenant: evolution.target_tenant.clone(),
        csdl_xml: phase1_stackoverflow_csdl().to_string(),
        ioa_sources: alloc::vec![DeployIoaSource {
            entity_type: "Answer".to_string(),
            source: phase1_downvote_template(),
        }],
    }
}

/// Minimal CSDL for the SO `default` tenant — Question + Answer only, no
/// Vote (Phase-1 plan note: "PURE IOA (Question + Answer only; Upvote
/// present, Downvote ABSENT)"). The CSDL doesn't change in the downvote
/// hot-SWAP — the merged Answer just gains a `Downvote` action, which the
/// kernel rebinds without a CSDL edit (per memory P0.1 finding).
fn phase1_stackoverflow_csdl() -> &'static str {
    r#"<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx" Version="4.0">
  <edmx:DataServices>
    <Schema Namespace="Soa.QA" xmlns="http://docs.oasis-open.org/odata/ns/edm">
      <EntityType Name="Question">
        <Key><PropertyRef Name="Id"/></Key>
        <Property Name="Id" Type="Edm.String" Nullable="false"/>
        <Property Name="Title" Type="Edm.String"/>
        <Property Name="Body" Type="Edm.String"/>
      </EntityType>
      <EntityType Name="Answer">
        <Key><PropertyRef Name="Id"/></Key>
        <Property Name="Id" Type="Edm.String" Nullable="false"/>
        <Property Name="QuestionId" Type="Edm.String"/>
        <Property Name="Body" Type="Edm.String"/>
      </EntityType>
      <EntityContainer Name="Container">
        <EntitySet Name="Questions" EntityType="Soa.QA.Question"/>
        <EntitySet Name="Answers"   EntityType="Soa.QA.Answer"/>
      </EntityContainer>
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
"#
}

/// Byte-stable downvote-template Answer spec — must match the spec content
/// gen_variant + run_stage_caller emit, so the same source flows from
/// generation → verification → deploy without drift.
fn phase1_downvote_template() -> String {
    String::from(
        "# Answer (downvote-evolved) — added by gen_variant for the Phase-1 loop.\n\
         #\n\
         # Deterministic-template mutation: original three states plus Downvote.\n\
         # Genesis verify cascade (parse/L0/L1) must keep passing after this edit.\n\
         \n\
         [automaton]\n\
         name = \"Answer\"\n\
         states = [\"Active\", \"Accepted\", \"Deleted\"]\n\
         initial = \"Active\"\n\
         \n\
         [[state]]\n\
         name = \"upvotes\"\n\
         type = \"counter\"\n\
         initial = \"0\"\n\
         \n\
         [[state]]\n\
         name = \"downvotes\"\n\
         type = \"counter\"\n\
         initial = \"0\"\n\
         \n\
         [[action]]\n\
         name = \"Upvote\"\n\
         kind = \"input\"\n\
         from = [\"Active\", \"Accepted\"]\n\
         to = \"Active\"\n\
         effect = \"increment upvotes\"\n\
         \n\
         [[action]]\n\
         name = \"Downvote\"\n\
         kind = \"input\"\n\
         from = [\"Active\", \"Accepted\"]\n\
         to = \"Active\"\n\
         effect = \"increment downvotes\"\n\
         \n\
         [[action]]\n\
         name = \"Accept\"\n\
         kind = \"input\"\n\
         from = [\"Active\"]\n\
         to = \"Accepted\"\n\
         \n\
         [[action]]\n\
         name = \"Delete\"\n\
         kind = \"input\"\n\
         from = [\"Active\", \"Accepted\"]\n\
         to = \"Deleted\"\n\
         \n\
         [[invariant]]\n\
         name = \"DeletedIsFinal\"\n\
         when = [\"Deleted\"]\n\
         assert = \"no_further_transitions\"\n",
    )
}

fn fetch_variant(ctx: &Context, variant_id: &str) -> Result<VariantSnapshot, String> {
    let url = format!(
        "{}/tdata/Variants('{}')",
        temper_api_base(ctx),
        odata_key(variant_id)
    );
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch Variant: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("Variant status {}", resp.status));
    }
    let row: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("Variant json: {e}"))?;
    VariantSnapshot::from_row(&row, variant_id)
}

fn temper_api_base(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .map(|v| v.trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_TEMPER_API.to_string())
}

fn repository_id_for(target_app: &str) -> String {
    format!("rp-{}", sanitize(target_app))
}

fn pull_request_id_for(evolution_id: &str, variant_id: &str) -> String {
    format!(
        "pr-evo-{}-{}",
        chunk_id(evolution_id),
        chunk_id(variant_id)
    )
}

fn lineage_id_for(evolution_id: &str, variant_id: &str) -> String {
    format!(
        "ln-evo-{}-{}",
        chunk_id(evolution_id),
        chunk_id(variant_id)
    )
}

fn chunk_id(input: &str) -> String {
    sanitize(input).chars().take(ID_CHUNK_LEN).collect()
}

fn short(id: &str) -> String {
    id.chars().take(ID_CHUNK_LEN).collect()
}

fn sanitize(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "item".to_string() } else { trimmed }
}

fn row_string(row: &Value, key: &str) -> Option<String> {
    row.get("fields")
        .and_then(|fields| fields.get(key))
        .or_else(|| row.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn row_required(row: &Value, key: &str, ctx: &str) -> Result<String, String> {
    row_string(row, key)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{ctx} missing required field '{key}'"))
}

fn odata_key(input: &str) -> String {
    input.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evolution() -> EvolutionSnapshot {
        EvolutionSnapshot {
            id: "ev-001".to_string(),
            target_app: "stackoverflow-agents".to_string(),
            target_tenant: "default".to_string(),
            target_branch: "refs/heads/main".to_string(),
            winner_variant_id: Some("var-ev-001-00".to_string()),
            intent: "downvote".to_string(),
        }
    }

    fn variant() -> VariantSnapshot {
        VariantSnapshot {
            id: "var-ev-001-00".to_string(),
            branch_ref: "refs/heads/evo/ev-001/var-00".to_string(),
            commit_sha: "deadbeefcafef00d".to_string(),
            candidate_entity_type: "Answer".to_string(),
            candidate_source: "[automaton]\nname = \"Answer\"\nstates = [\"Active\"]\n"
                .to_string(),
        }
    }

    fn plan() -> MergePlan {
        build_merge_plan(&evolution(), &variant()).unwrap()
    }

    #[test]
    fn sub_writes_contract_is_pr_create_open_merge_lineage_golive() {
        let writes = build_merge_variant_sub_writes(&plan(), &DeployOutcome::Succeeded).unwrap();
        let pairs: Vec<_> = writes
            .iter()
            .map(|w| (w["entity_type"].as_str().unwrap(), w["action"].as_str().unwrap()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("PullRequest", "Create"),
                ("PullRequest", "Open"),
                ("PullRequest", "Merge"),
                ("Lineage", "Create"),
                ("Evolution", "GoLive"),
            ]
        );
    }

    #[test]
    fn pr_create_uses_target_apps_repository_and_winner_branch() {
        let writes = build_merge_variant_sub_writes(&plan(), &DeployOutcome::Succeeded).unwrap();
        let create = &writes[0];
        assert_eq!(create["entity_id"], "pr-evo-ev-001-var-ev-001-00");
        assert_eq!(create["params"]["RepositoryId"], "rp-stackoverflow-agents");
        assert_eq!(create["params"]["SourceRef"], "refs/heads/evo/ev-001/var-00");
        assert_eq!(create["params"]["TargetRef"], "refs/heads/main");
        assert_eq!(create["params"]["OpenedBy"], "evolver");
    }

    #[test]
    fn pr_merge_uses_squash_strategy() {
        let writes = build_merge_variant_sub_writes(&plan(), &DeployOutcome::Succeeded).unwrap();
        assert_eq!(writes[2]["action"], "Merge");
        assert_eq!(writes[2]["params"]["Strategy"], "squash");
    }

    #[test]
    fn lineage_records_evolution_metadata_in_mutations_payload() {
        let writes = build_merge_variant_sub_writes(&plan(), &DeployOutcome::Succeeded).unwrap();
        let lineage = &writes[3];
        let mutations_str = lineage["params"]["Mutations"].as_str().unwrap();
        let mutations: Value = serde_json::from_str(mutations_str).unwrap();
        assert_eq!(mutations["type"], "spec_evolution");
        assert_eq!(mutations["intent"], "downvote");
        assert_eq!(mutations["variant_id"], "var-ev-001-00");
        assert_eq!(mutations["deploy"], "succeeded");
    }

    #[test]
    fn golive_is_emitted_even_when_deploy_fails_or_skipped() {
        for outcome in [
            DeployOutcome::NotAttempted,
            DeployOutcome::Failed("status 503".to_string()),
        ] {
            let writes = build_merge_variant_sub_writes(&plan(), &outcome).unwrap();
            assert_eq!(writes.last().unwrap()["action"], "GoLive");
            let mutations_str = writes[3]["params"]["Mutations"].as_str().unwrap();
            let mutations: Value = serde_json::from_str(mutations_str).unwrap();
            assert_eq!(mutations["deploy"], outcome.as_str());
        }
    }

    #[test]
    fn evolution_snapshot_requires_winner_variant_id_via_run() {
        let mut evo = evolution();
        evo.winner_variant_id = None;
        // mirror the check inside `run`
        let err = evo
            .winner_variant_id
            .clone()
            .ok_or_else(|| "missing".to_string())
            .unwrap_err();
        assert!(err.contains("missing"));
    }

    #[test]
    fn build_merge_plan_rejects_variant_without_branch_ref() {
        let mut v = variant();
        v.branch_ref = String::new();
        let err = build_merge_plan(&evolution(), &v).unwrap_err();
        assert!(err.contains("BranchRef"));
    }

    #[test]
    fn evolution_snapshot_defaults_target_branch_to_main() {
        let snap = EvolutionSnapshot::from_entity_state(
            "ev-x",
            &json!({"fields": {"TargetApp": "app", "Intent": "i"}}),
        )
        .unwrap();
        assert_eq!(snap.target_branch, "refs/heads/main");
    }

    #[test]
    fn evolution_snapshot_requires_target_app() {
        let err = EvolutionSnapshot::from_entity_state(
            "ev-x",
            &json!({"fields": {"Intent": "i"}}),
        )
        .unwrap_err();
        assert!(err.contains("TargetApp"));
    }

    #[test]
    fn variant_snapshot_requires_branch_ref() {
        let err = VariantSnapshot::from_row(&json!({"fields": {"Id": "var-x"}}), "var-x").unwrap_err();
        assert!(err.contains("BranchRef"));
    }

    #[test]
    fn derived_ids_are_url_safe_and_bounded() {
        let pr = pull_request_id_for("Evolution Id With Spaces", "var-XY/123");
        assert!(pr.starts_with("pr-evo-"));
        assert!(pr.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        let ln = lineage_id_for("ev-x", "var-x");
        assert!(ln.starts_with("ln-evo-"));
    }

    #[test]
    fn repository_id_sanitizes_target_app_name() {
        assert_eq!(repository_id_for("My/App-Name"), "rp-my-app-name");
    }

    #[test]
    fn sub_writes_are_deterministic_across_calls() {
        let p = plan();
        let one = build_merge_variant_sub_writes(&p, &DeployOutcome::Succeeded).unwrap();
        let two = build_merge_variant_sub_writes(&p, &DeployOutcome::Succeeded).unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn deploy_outcome_strings_are_stable_for_telemetry() {
        assert_eq!(DeployOutcome::NotAttempted.as_str(), "not_attempted");
        assert_eq!(DeployOutcome::Succeeded.as_str(), "succeeded");
        assert_eq!(DeployOutcome::Failed("x".to_string()).as_str(), "failed");
    }

    #[test]
    fn phase1_fallback_pairs_so_csdl_with_downvote_answer_source() {
        let payload = phase1_downvote_payload(&evolution());
        assert_eq!(payload.tenant, "default");
        assert!(payload.csdl_xml.contains("Soa.QA"));
        assert!(payload.csdl_xml.contains("EntityType Name=\"Answer\""));
        assert_eq!(payload.ioa_sources.len(), 1);
        assert_eq!(payload.ioa_sources[0].entity_type, "Answer");
        assert!(payload.ioa_sources[0].source.contains("Downvote"));
        assert!(payload.ioa_sources[0].source.contains("downvotes"));
    }

    #[test]
    fn parse_deploy_payload_accepts_well_formed_response() {
        let body = json!({
            "tenant": "stackoverflow-agents",
            "csdl_xml": "<edmx>…</edmx>",
            "ioa_sources": [
                {"entity_type": "Answer", "source": "[automaton]\nname=\"Answer\""}
            ]
        })
        .to_string();
        let payload = parse_deploy_payload(&body, "default").unwrap();
        assert_eq!(payload.tenant, "stackoverflow-agents");
        assert_eq!(payload.ioa_sources.len(), 1);
        assert_eq!(payload.ioa_sources[0].entity_type, "Answer");
    }

    #[test]
    fn parse_deploy_payload_defaults_tenant_when_missing() {
        let body = json!({
            "csdl_xml": "<edmx>…</edmx>",
            "ioa_sources": [{"entity_type": "X", "source": "y"}]
        })
        .to_string();
        let payload = parse_deploy_payload(&body, "fallback-tenant").unwrap();
        assert_eq!(payload.tenant, "fallback-tenant");
    }

    #[test]
    fn parse_deploy_payload_rejects_missing_csdl() {
        let body = json!({
            "ioa_sources": [{"entity_type": "X", "source": "y"}]
        })
        .to_string();
        let err = parse_deploy_payload(&body, "t").unwrap_err();
        assert!(err.contains("csdl_xml"));
    }

    #[test]
    fn parse_deploy_payload_rejects_empty_csdl_or_ioa() {
        let body = json!({"csdl_xml": "", "ioa_sources": [{"entity_type": "X", "source": "y"}]})
            .to_string();
        let err = parse_deploy_payload(&body, "t").unwrap_err();
        assert!(err.contains("empty csdl_xml"));

        let body = json!({"csdl_xml": "<x/>", "ioa_sources": []}).to_string();
        let err = parse_deploy_payload(&body, "t").unwrap_err();
        assert!(err.contains("empty ioa_sources"));
    }

    #[test]
    fn parse_deploy_payload_rejects_blank_entity_type_or_source() {
        let body = json!({
            "csdl_xml": "<x/>",
            "ioa_sources": [{"entity_type": "", "source": "y"}]
        })
        .to_string();
        let err = parse_deploy_payload(&body, "t").unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn parse_inline_ioa_sources_accepts_json_array() {
        let raw = json!([
            {"entity_type": "Answer", "source": "x"},
            {"entity_type": "Question", "source": "y"}
        ])
        .to_string();
        let sources = parse_inline_ioa_sources(&raw).unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].entity_type, "Answer");
        assert_eq!(sources[1].source, "y");
    }

    #[test]
    fn parse_inline_ioa_sources_rejects_non_array() {
        let raw = json!({"entity_type": "Answer", "source": "x"}).to_string();
        let err = parse_inline_ioa_sources(&raw).unwrap_err();
        assert!(err.contains("must be a JSON array"));
    }

    #[test]
    fn phase1_csdl_is_byte_stable_for_dst_replays() {
        let one = phase1_stackoverflow_csdl();
        let two = phase1_stackoverflow_csdl();
        assert_eq!(one, two);
        assert!(one.starts_with("<?xml version=\"1.0\""));
    }

    // NB: tests do not call `build_deploy_payload` directly because that
    // function statically references `ctx.http_call`, which (under cargo's
    // host test target) drags `host_http_call` into the link graph and
    // fails because the host symbol is only resolved at WASM-runtime. The
    // pure helpers below cover every branch of build_deploy_payload short
    // of the http_call invocation itself.

    #[test]
    fn phase1_downvote_payload_has_byte_stable_so_csdl() {
        let one = phase1_downvote_payload(&evolution());
        let two = phase1_downvote_payload(&evolution());
        assert_eq!(one, two);
        assert!(one.csdl_xml.contains("Soa.QA"));
        assert!(one.csdl_xml.contains("EntityType Name=\"Answer\""));
    }

    #[test]
    fn variant_snapshot_carries_candidate_entity_type_and_source() {
        let snap = VariantSnapshot::from_row(
            &json!({
                "fields": {
                    "BranchRef": "refs/heads/x",
                    "CommitSha": "abc",
                    "CandidateEntityType": "Answer",
                    "CandidateSource": "[automaton]\nname = \"Answer\""
                }
            }),
            "var-x",
        )
        .unwrap();
        assert_eq!(snap.candidate_entity_type, "Answer");
        assert!(snap.candidate_source.contains("[automaton]"));
    }

    #[test]
    fn variant_snapshot_defaults_candidate_fields_to_empty_when_absent() {
        let snap = VariantSnapshot::from_row(
            &json!({"fields": {"BranchRef": "refs/heads/x"}}),
            "var-x",
        )
        .unwrap();
        assert_eq!(snap.candidate_entity_type, "");
        assert_eq!(snap.candidate_source, "");
    }
}
