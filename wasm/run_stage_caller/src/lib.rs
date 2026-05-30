//! run_stage_caller — directed-evolution per-stage verifier orchestrator.
//!
//! Triggered on `Variant.StartEval` and re-triggered after every
//! `Variant.RecordStageResult`. It walks the parent Evolution's `FitnessSpec`
//! stages one step at a time, posting the variant's mutated IOA source to
//! `temper-platform`'s `POST /verify/stage`, then RETURNS a `sub_writes`
//! envelope: a `StageResult.Create` record plus exactly one of
//! `Variant.RecordStageResult` (advance to the next stage),
//! `Variant.Kill` (failed a gate with `on_fail=kill`), or `Variant.Survive`
//! (passed the final stage). The Temper kernel applies the writes — this
//! module never dispatches actions itself (see ADR-0046 + ADR-0011).
//!
//! `/tdata`, `/verify/stage`, and `/deploy/tenant` are colocated on the
//! same temper-platform process (ADR-0012) — this module reads + writes via
//! a single base URL (`temper_api_url`).

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

/// Single base URL for the whole temper-platform process: serves `/tdata`,
/// `/verify/stage`, and `/deploy/tenant` on the same port (ADR-0012).
/// Phase-1 default matches the runbook (`temper serve --port 3000`).
const DEFAULT_TEMPER_API: &str = "http://127.0.0.1:3000";
/// Deterministic timestamp so DST replays match unit tests bit-for-bit.
const CREATED_AT: &str = "1970-01-01T00:00:00Z";
/// Cheap collision-resistant id length for StageResult ids.
const STAGE_RESULT_ID_CHARS: usize = 16;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let variant = VariantSnapshot::from_entity_state(&ctx.entity_id, &ctx.entity_state)?;
        let evolution = fetch_evolution(&ctx, &variant.evolution_id)?;
        let fitness = fetch_fitness_spec(&ctx, &evolution.fitness_spec_id)?;
        let stage_idx = resolve_stage_idx(&ctx, &fitness)?;
        let plan = stage_plan(&fitness, stage_idx, stage_idx as i64)?;
        let source = fetch_variant_source(&ctx, &variant)?;
        let verdict = invoke_verify_stage(&ctx, &source, &plan, &variant)?;
        let sub_writes = build_run_stage_sub_writes(&variant, &fitness, &plan, &verdict)?;

        Ok(json!({
            "variant_id": variant.id,
            "stage_id": plan.stage_id,
            "passed": verdict.passed,
            "is_last_stage": plan.is_last,
            "sub_write_count": sub_writes.len(),
            "sub_writes": sub_writes,
        }))
    }
}

/// Resolve which stage we are about to evaluate from the trigger context.
/// `StartEval` is unambiguously stage 0. `RecordStageResult` carries the
/// `stage_id` that was just recorded — the next stage is its successor in
/// the FitnessSpec. Anything else falls back to the entity's `CurrentStage`
/// counter so legacy/manual invocations still work.
fn resolve_stage_idx(ctx: &Context, fitness: &FitnessSpecData) -> Result<usize, String> {
    match ctx.trigger_action.as_str() {
        "StartEval" => Ok(0),
        "RecordStageResult" => {
            let prev = ctx
                .trigger_params
                .get("stage_id")
                .and_then(Value::as_str)
                .ok_or_else(|| "RecordStageResult trigger missing stage_id param".to_string())?;
            let idx = fitness
                .stages
                .iter()
                .position(|s| s.stage_id == prev)
                .ok_or_else(|| format!("recorded stage_id '{prev}' is not in FitnessSpec"))?;
            Ok(idx + 1)
        }
        _ => Ok(ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("CurrentStage"))
            .and_then(Value::as_i64)
            .map(|v| v.max(0) as usize)
            .unwrap_or(0)),
    }
}

/// Variant fields the verifier orchestrator needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VariantSnapshot {
    id: String,
    evolution_id: String,
    branch_ref: String,
    current_stage: i64,
    /// Set by `gen_variant` when it emitted the `Variant.Create` sub_write.
    /// Empty string means the row was created some other way (e.g. backfill
    /// or manual seeding); the orchestrator falls through to `spec_fetch_url`
    /// or the deterministic template in that case.
    candidate_entity_type: String,
    /// The full mutated IOA TOML to verify. Empty string ⇒ same fallback path.
    candidate_source: String,
}

impl VariantSnapshot {
    fn from_entity_state(entity_id: &str, state: &Value) -> Result<Self, String> {
        let id = row_string(state, "Id").unwrap_or_else(|| entity_id.to_string());
        if id.is_empty() {
            return Err("Variant entity_id is required".to_string());
        }
        let evolution_id = row_required(state, "EvolutionId", "Variant state")?;
        let branch_ref = row_string(state, "BranchRef").unwrap_or_default();
        let current_stage = row_i64(state, "CurrentStage").unwrap_or(0);
        if current_stage < 0 {
            return Err(format!("Variant.CurrentStage must be non-negative, got {current_stage}"));
        }
        let candidate_entity_type = row_string(state, "CandidateEntityType").unwrap_or_default();
        let candidate_source = row_string(state, "CandidateSource").unwrap_or_default();
        Ok(Self {
            id,
            evolution_id,
            branch_ref,
            current_stage,
            candidate_entity_type,
            candidate_source,
        })
    }
}

/// Evolution fields needed to look up the FitnessSpec for this episode.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EvolutionLookup {
    fitness_spec_id: String,
    target_app: String,
}

impl EvolutionLookup {
    fn from_row(row: &Value) -> Result<Self, String> {
        Ok(Self {
            fitness_spec_id: row_required(row, "FitnessSpecId", "Evolution row")?,
            target_app: row_string(row, "TargetApp").unwrap_or_default(),
        })
    }
}

/// FitnessSpec stages JSON + selection policy as the orchestrator needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FitnessSpecData {
    stages: Vec<StageDef>,
}

impl FitnessSpecData {
    fn from_row(row: &Value) -> Result<Self, String> {
        let stages_field = row_string(row, "Stages").unwrap_or_else(|| "[]".to_string());
        let parsed: Value = serde_json::from_str(&stages_field)
            .map_err(|e| format!("FitnessSpec.Stages is not JSON: {e}"))?;
        let arr = parsed
            .as_array()
            .ok_or_else(|| "FitnessSpec.Stages must be a JSON array".to_string())?;
        if arr.is_empty() {
            return Err("FitnessSpec.Stages must contain at least one stage".to_string());
        }
        let mut stages = Vec::with_capacity(arr.len());
        for (idx, raw) in arr.iter().enumerate() {
            stages.push(StageDef::parse(idx, raw)?);
        }
        Ok(Self { stages })
    }
}

/// One row of the FitnessSpec stages array.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StageDef {
    stage_id: String,
    evaluator: String,
    on_fail: OnFail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnFail {
    Kill,
    Continue,
    Warn,
}

impl OnFail {
    fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("kill") {
            "continue" => Self::Continue,
            "warn" => Self::Warn,
            _ => Self::Kill,
        }
    }
}

impl StageDef {
    fn parse(idx: usize, raw: &Value) -> Result<Self, String> {
        let stage_id = raw
            .get("stage_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| format!("FitnessSpec.Stages[{idx}].stage_id missing"))?;
        let evaluator = raw
            .get("evaluator")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| stage_id.clone());
        let on_fail = OnFail::parse(raw.get("on_fail").and_then(Value::as_str));
        Ok(Self {
            stage_id,
            evaluator,
            on_fail,
        })
    }
}

/// The stage the variant is currently being judged at (after lookup).
#[derive(Debug, Clone, PartialEq, Eq)]
struct StagePlan {
    stage_id: String,
    evaluator: String,
    on_fail: OnFail,
    is_last: bool,
}

fn stage_plan(spec: &FitnessSpecData, idx: usize, raw_stage: i64) -> Result<StagePlan, String> {
    let stage = spec
        .stages
        .get(idx)
        .ok_or_else(|| format!("CurrentStage {raw_stage} is past the FitnessSpec stage count {}", spec.stages.len()))?;
    Ok(StagePlan {
        stage_id: stage.stage_id.clone(),
        evaluator: stage.evaluator.clone(),
        on_fail: stage.on_fail,
        is_last: idx + 1 == spec.stages.len(),
    })
}

/// Verdict returned by `POST /verify/stage`. Mirrors temper-verify::StageVerdict.
#[derive(Debug, Clone, PartialEq)]
struct StageVerdict {
    stage: String,
    passed: bool,
    summary: String,
    counterexample: Option<Value>,
    objective_scores: Value,
}

impl StageVerdict {
    fn parse(body: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(body)
            .map_err(|e| format!("/verify/stage response is not JSON: {e}"))?;
        let stage = v
            .get("stage")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_default();
        let passed = v.get("passed").and_then(Value::as_bool).unwrap_or(false);
        let summary = v
            .get("summary")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_default();
        let counterexample = v.get("counterexample").cloned().filter(|c| !c.is_null());
        let objective_scores = v
            .get("objective_scores")
            .cloned()
            .unwrap_or_else(|| json!({}));
        Ok(Self {
            stage,
            passed,
            summary,
            counterexample,
            objective_scores,
        })
    }
}

/// Public entry-point for unit tests + `run`: produce the sub_writes envelope.
fn build_run_stage_sub_writes(
    variant: &VariantSnapshot,
    spec: &FitnessSpecData,
    plan: &StagePlan,
    verdict: &StageVerdict,
) -> Result<Vec<Value>, String> {
    let _ = spec; // The spec is captured by `plan` already; the parameter signals intent.
    let stage_result_id = stage_result_id_for(&variant.id, &plan.stage_id);
    let scores_str = serde_json::to_string(&verdict.objective_scores)
        .map_err(|e| format!("encode objective_scores: {e}"))?;
    let evidence_str = match &verdict.counterexample {
        Some(ce) => serde_json::to_string(ce).map_err(|e| format!("encode counterexample: {e}"))?,
        None => serde_json::to_string(&json!({"summary": verdict.summary}))
            .map_err(|e| format!("encode summary evidence: {e}"))?,
    };
    let verdict_str = if verdict.passed { "passed" } else { "failed" };

    let mut writes = Vec::with_capacity(2);
    writes.push(json!({
        "entity_type": "StageResult",
        "entity_id": stage_result_id.clone(),
        "action": "Create",
        "params": {
            "VariantId": variant.id,
            "StageId": plan.stage_id,
            "Evaluator": plan.evaluator,
            "Verdict": verdict_str,
            "ObjectiveScores": scores_str,
            "Evidence": evidence_str,
            "CreatedAt": CREATED_AT
        }
    }));

    let counterexample_str = verdict
        .counterexample
        .as_ref()
        .map(|ce| serde_json::to_string(ce).unwrap_or_default())
        .unwrap_or_default();

    if !verdict.passed && matches!(plan.on_fail, OnFail::Kill) {
        writes.push(json!({
            "entity_type": "Variant",
            "entity_id": variant.id,
            "action": "Kill",
            "params": {
                "stage_id": plan.stage_id,
                "counterexample": counterexample_str
            }
        }));
        return Ok(writes);
    }

    if verdict.passed && plan.is_last {
        writes.push(json!({
            "entity_type": "Variant",
            "entity_id": variant.id,
            "action": "Survive",
            "params": {}
        }));
        return Ok(writes);
    }

    writes.push(json!({
        "entity_type": "Variant",
        "entity_id": variant.id,
        "action": "RecordStageResult",
        "params": {
            "stage_id": plan.stage_id,
            "verdict": verdict_str,
            "objective_scores": scores_str,
            "evidence": evidence_str
        }
    }));
    Ok(writes)
}

fn fetch_evolution(ctx: &Context, evolution_id: &str) -> Result<EvolutionLookup, String> {
    let url = format!(
        "{}/tdata/Evolutions('{}')",
        temper_api_base(ctx),
        odata_key(evolution_id)
    );
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch Evolution: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("Evolution status {}", resp.status));
    }
    let row: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("Evolution json: {e}"))?;
    EvolutionLookup::from_row(&row)
}

fn fetch_fitness_spec(ctx: &Context, fitness_spec_id: &str) -> Result<FitnessSpecData, String> {
    let url = format!(
        "{}/tdata/FitnessSpecs('{}')",
        temper_api_base(ctx),
        odata_key(fitness_spec_id)
    );
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch FitnessSpec: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("FitnessSpec status {}", resp.status));
    }
    let row: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("FitnessSpec json: {e}"))?;
    FitnessSpecData::from_row(&row)
}

/// Resolve the candidate IOA source to verify. Resolution order:
///
/// 1. The Variant row itself — `CandidateEntityType` + `CandidateSource`.
///    Canonical path: `gen_variant` populates both on Create, so verification
///    reads inline state instead of walking the git tree (avoids the heavy
///    blob/tree/commit dance in v1).
/// 2. `spec_fetch_url` (configurable) → POST `{variant_id, evolution_id,
///    branch_ref}` and parse the response. Reserved for rows that lack
///    inline candidates (manual creation, future hot-fix flows).
/// 3. Phase-1 deterministic downvote template as a last-resort fallback,
///    so dry-runs without state still close the loop.
fn fetch_variant_source(ctx: &Context, variant: &VariantSnapshot) -> Result<IoaSource, String> {
    if !variant.candidate_source.is_empty() {
        let entity_type = if variant.candidate_entity_type.is_empty() {
            "Variant".to_string()
        } else {
            variant.candidate_entity_type.clone()
        };
        return Ok(IoaSource {
            entity_type,
            source: variant.candidate_source.clone(),
        });
    }
    if let Some(url) = ctx.config.get("spec_fetch_url").filter(|s| !s.is_empty()) {
        let body = json!({
            "variant_id": variant.id,
            "evolution_id": variant.evolution_id,
            "branch_ref": variant.branch_ref,
        })
        .to_string();
        let resp = ctx
            .http_call("POST", url, &[], &body)
            .map_err(|e| format!("spec_fetch_url http_call: {e}"))?;
        if !(200..400).contains(&resp.status) {
            return Err(format!("spec_fetch_url status {}", resp.status));
        }
        return parse_spec_response(&resp.body);
    }
    Ok(IoaSource {
        entity_type: "Answer".to_string(),
        source: phase1_downvote_template(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IoaSource {
    entity_type: String,
    source: String,
}

/// `spec_fetch_url` may return either a raw IOA body or JSON
/// `{entity_type, source}` (or `{ioa_sources:[...]}`). All three shapes are
/// accepted so the fetch service can evolve without breaking this orchestrator.
fn parse_spec_response(body: &str) -> Result<IoaSource, String> {
    if let Ok(parsed) = serde_json::from_str::<Value>(body) {
        if let Some(arr) = parsed.get("ioa_sources").and_then(Value::as_array) {
            let first = arr
                .first()
                .ok_or_else(|| "spec_fetch returned empty ioa_sources".to_string())?;
            return Ok(IoaSource {
                entity_type: first
                    .get("entity_type")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "Variant".to_string()),
                source: first
                    .get("source")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .ok_or_else(|| "spec_fetch ioa_sources[0].source missing".to_string())?,
            });
        }
        if let (Some(entity_type), Some(source)) = (
            parsed.get("entity_type").and_then(Value::as_str),
            parsed.get("source").and_then(Value::as_str),
        ) {
            return Ok(IoaSource {
                entity_type: entity_type.to_string(),
                source: source.to_string(),
            });
        }
    }
    Ok(IoaSource {
        entity_type: "Variant".to_string(),
        source: body.to_string(),
    })
}

fn invoke_verify_stage(
    ctx: &Context,
    source: &IoaSource,
    plan: &StagePlan,
    variant: &VariantSnapshot,
) -> Result<StageVerdict, String> {
    // ADR-0012: `/verify/stage` is colocated with `/tdata` on the same
    // temper-platform process; no separate URL.
    let url = format!("{}/verify/stage", temper_api_base(ctx));
    let body = json!({
        "ioa_sources": [{
            "entity_type": source.entity_type,
            "source": source.source,
        }],
        "stage": plan.evaluator,
        "client_request_id": format!("{}-{}", variant.id, plan.stage_id),
    })
    .to_string();
    let resp = ctx
        .http_call("POST", &url, &[], &body)
        .map_err(|e| format!("verify/stage http_call: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("/verify/stage status {}", resp.status));
    }
    StageVerdict::parse(&resp.body)
}

fn stage_result_id_for(variant_id: &str, stage_id: &str) -> String {
    let sanitized_variant: String = sanitize(variant_id).chars().take(STAGE_RESULT_ID_CHARS).collect();
    format!("sr-{}-{}", sanitized_variant, sanitize(stage_id))
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

fn temper_api_base(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .map(|v| v.trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_TEMPER_API.to_string())
}

fn row_string(row: &Value, key: &str) -> Option<String> {
    row.get("fields")
        .and_then(|fields| fields.get(key))
        .or_else(|| row.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn row_i64(row: &Value, key: &str) -> Option<i64> {
    row.get("fields")
        .and_then(|fields| fields.get(key))
        .or_else(|| row.get(key))
        .and_then(Value::as_i64)
}

fn row_required(row: &Value, key: &str, context: &str) -> Result<String, String> {
    row_string(row, key)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{context} missing required field '{key}'"))
}

fn odata_key(input: &str) -> String {
    input.replace('\'', "''")
}

/// Phase-1 deterministic downvote template — must match gen_variant's output
/// byte-for-byte so the verify cascade reproduces independently of the
/// generator path. Kept in this module to avoid a cross-crate dep.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(stage: i64) -> VariantSnapshot {
        VariantSnapshot {
            id: "var-ev-001-00".to_string(),
            evolution_id: "ev-001".to_string(),
            branch_ref: "refs/heads/evo/ev-001/var-00".to_string(),
            current_stage: stage,
            candidate_entity_type: "Answer".to_string(),
            candidate_source: "[automaton]\nname = \"Answer\"\n".to_string(),
        }
    }

    fn fitness_v1() -> FitnessSpecData {
        FitnessSpecData {
            stages: alloc::vec![
                StageDef { stage_id: "parse".to_string(), evaluator: "parse".to_string(), on_fail: OnFail::Kill },
                StageDef { stage_id: "l0".to_string(), evaluator: "l0".to_string(), on_fail: OnFail::Kill },
                StageDef { stage_id: "l1".to_string(), evaluator: "l1".to_string(), on_fail: OnFail::Kill },
                StageDef { stage_id: "budget".to_string(), evaluator: "budget".to_string(), on_fail: OnFail::Kill },
            ],
        }
    }

    fn pass_verdict(stage: &str) -> StageVerdict {
        StageVerdict {
            stage: stage.to_string(),
            passed: true,
            summary: "ok".to_string(),
            counterexample: None,
            objective_scores: json!({"state_space": 7.0}),
        }
    }

    fn fail_verdict(stage: &str) -> StageVerdict {
        StageVerdict {
            stage: stage.to_string(),
            passed: false,
            summary: "guard unreachable".to_string(),
            counterexample: Some(json!({"guard": "downvotes >= 0"})),
            objective_scores: json!({}),
        }
    }

    #[test]
    fn passed_intermediate_stage_records_and_advances() {
        let variant = variant(0);
        let spec = fitness_v1();
        let plan = stage_plan(&spec, 0, 0).unwrap();
        let writes = build_run_stage_sub_writes(&variant, &spec, &plan, &pass_verdict("parse")).unwrap();
        let pairs: Vec<_> = writes
            .iter()
            .map(|w| (w["entity_type"].as_str().unwrap(), w["action"].as_str().unwrap()))
            .collect();
        assert_eq!(
            pairs,
            vec![("StageResult", "Create"), ("Variant", "RecordStageResult")]
        );
        assert_eq!(writes[0]["params"]["StageId"], "parse");
        assert_eq!(writes[0]["params"]["Verdict"], "passed");
        assert_eq!(writes[1]["params"]["stage_id"], "parse");
    }

    #[test]
    fn passed_final_stage_survives() {
        let variant = variant(3);
        let spec = fitness_v1();
        let plan = stage_plan(&spec, 3, 3).unwrap();
        assert!(plan.is_last);
        let writes = build_run_stage_sub_writes(&variant, &spec, &plan, &pass_verdict("budget")).unwrap();
        let actions: Vec<_> = writes.iter().map(|w| w["action"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["Create", "Survive"]);
    }

    #[test]
    fn failed_gate_with_kill_on_fail_kills_variant() {
        let variant = variant(1);
        let spec = fitness_v1();
        let plan = stage_plan(&spec, 1, 1).unwrap();
        let verdict = fail_verdict("l0");
        let writes = build_run_stage_sub_writes(&variant, &spec, &plan, &verdict).unwrap();
        let actions: Vec<_> = writes.iter().map(|w| w["action"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["Create", "Kill"]);
        assert_eq!(writes[1]["params"]["stage_id"], "l0");
        let ce = writes[1]["params"]["counterexample"].as_str().unwrap();
        assert!(ce.contains("downvotes"));
    }

    #[test]
    fn failed_gate_with_continue_advances_anyway() {
        let mut spec = fitness_v1();
        spec.stages[0].on_fail = OnFail::Continue;
        let plan = stage_plan(&spec, 0, 0).unwrap();
        let writes =
            build_run_stage_sub_writes(&variant(0), &spec, &plan, &fail_verdict("parse")).unwrap();
        let actions: Vec<_> = writes.iter().map(|w| w["action"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["Create", "RecordStageResult"]);
    }

    #[test]
    fn stage_plan_rejects_index_past_stage_count() {
        let spec = fitness_v1();
        let err = stage_plan(&spec, 99, 99).unwrap_err();
        assert!(err.contains("past the FitnessSpec stage count"));
    }

    #[test]
    fn fitness_spec_from_row_parses_v1_lex_stages() {
        let row = json!({
            "fields": {
                "Stages": "[{\"stage_id\":\"parse\",\"evaluator\":\"parse\",\"on_fail\":\"kill\"},\
                           {\"stage_id\":\"l1\",\"evaluator\":\"l1\",\"on_fail\":\"continue\"}]"
            }
        });
        let spec = FitnessSpecData::from_row(&row).unwrap();
        assert_eq!(spec.stages.len(), 2);
        assert_eq!(spec.stages[0].on_fail, OnFail::Kill);
        assert_eq!(spec.stages[1].on_fail, OnFail::Continue);
    }

    #[test]
    fn fitness_spec_from_row_rejects_empty_stages() {
        let row = json!({"fields": {"Stages": "[]"}});
        let err = FitnessSpecData::from_row(&row).unwrap_err();
        assert!(err.contains("at least one stage"));
    }

    #[test]
    fn variant_snapshot_rejects_missing_evolution_id() {
        let err = VariantSnapshot::from_entity_state(
            "var-x",
            &json!({"fields": {"BranchRef": "refs/heads/x"}}),
        )
        .unwrap_err();
        assert!(err.contains("EvolutionId"));
    }

    #[test]
    fn variant_snapshot_rejects_negative_current_stage() {
        let err = VariantSnapshot::from_entity_state(
            "var-x",
            &json!({
                "fields": {
                    "EvolutionId": "ev-001",
                    "BranchRef": "refs/heads/x",
                    "CurrentStage": -1
                }
            }),
        )
        .unwrap_err();
        assert!(err.contains("non-negative"));
    }

    #[test]
    fn stage_verdict_parse_accepts_platform_response_shape() {
        let body = json!({
            "stage": "l0",
            "passed": true,
            "summary": "L0 OK",
            "counterexample": null,
            "objective_scores": {"guards": 4.0}
        })
        .to_string();
        let verdict = StageVerdict::parse(&body).unwrap();
        assert!(verdict.passed);
        assert!(verdict.counterexample.is_none());
        assert_eq!(verdict.objective_scores["guards"], 4.0);
    }

    #[test]
    fn stage_verdict_parse_captures_counterexample_when_failing() {
        let body = json!({
            "stage": "l0",
            "passed": false,
            "summary": "guard unreachable",
            "counterexample": {"trace": ["A","B"]},
            "objective_scores": {}
        })
        .to_string();
        let verdict = StageVerdict::parse(&body).unwrap();
        assert!(!verdict.passed);
        assert!(verdict.counterexample.is_some());
    }

    #[test]
    fn parse_spec_response_accepts_json_envelope() {
        let body = json!({"entity_type": "Answer", "source": "..."}).to_string();
        let parsed = parse_spec_response(&body).unwrap();
        assert_eq!(parsed.entity_type, "Answer");
        assert_eq!(parsed.source, "...");
    }

    #[test]
    fn parse_spec_response_accepts_ioa_sources_envelope() {
        let body = json!({"ioa_sources": [{"entity_type": "Question", "source": "x"}]}).to_string();
        let parsed = parse_spec_response(&body).unwrap();
        assert_eq!(parsed.entity_type, "Question");
        assert_eq!(parsed.source, "x");
    }

    #[test]
    fn parse_spec_response_falls_through_to_raw_body() {
        let raw = "[automaton]\nname = \"Bare\"";
        let parsed = parse_spec_response(raw).unwrap();
        assert_eq!(parsed.entity_type, "Variant");
        assert_eq!(parsed.source, raw);
    }

    #[test]
    fn stage_result_id_is_url_safe_and_bounded() {
        let id = stage_result_id_for("var-ev-001-00", "parse");
        assert!(id.starts_with("sr-"));
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        let long = stage_result_id_for("var-ev-001-00-very-long-suffix", "l1");
        assert!(long.len() <= 3 + STAGE_RESULT_ID_CHARS + 1 + "l1".len() + 4);
    }

    #[test]
    fn sub_writes_are_deterministic_across_calls() {
        let plan = stage_plan(&fitness_v1(), 0, 0).unwrap();
        let first = build_run_stage_sub_writes(&variant(0), &fitness_v1(), &plan, &pass_verdict("parse")).unwrap();
        let second = build_run_stage_sub_writes(&variant(0), &fitness_v1(), &plan, &pass_verdict("parse")).unwrap();
        assert_eq!(first, second);
    }

    fn ctx_with(trigger_action: &str, trigger_params: Value, entity_state: Value) -> Context {
        Context {
            config: std::collections::BTreeMap::new(),
            trigger_params,
            entity_state,
            tenant: "test".to_string(),
            entity_type: "Variant".to_string(),
            entity_id: "var-ev-001-00".to_string(),
            trigger_action: trigger_action.to_string(),
            wasm_module: "run_stage_caller".to_string(),
            http_request: None,
        }
    }

    #[test]
    fn resolve_stage_idx_zero_for_start_eval() {
        let ctx = ctx_with("StartEval", json!({}), json!({}));
        assert_eq!(resolve_stage_idx(&ctx, &fitness_v1()).unwrap(), 0);
    }

    #[test]
    fn resolve_stage_idx_advances_from_recorded_stage_id() {
        let ctx = ctx_with("RecordStageResult", json!({"stage_id": "l0"}), json!({}));
        assert_eq!(resolve_stage_idx(&ctx, &fitness_v1()).unwrap(), 2);
    }

    #[test]
    fn resolve_stage_idx_rejects_unknown_recorded_stage() {
        let ctx = ctx_with("RecordStageResult", json!({"stage_id": "missing"}), json!({}));
        let err = resolve_stage_idx(&ctx, &fitness_v1()).unwrap_err();
        assert!(err.contains("not in FitnessSpec"));
    }

    #[test]
    fn resolve_stage_idx_falls_back_to_current_stage_for_other_triggers() {
        let ctx = ctx_with(
            "ManualReplay",
            json!({}),
            json!({"fields": {"CurrentStage": 2}}),
        );
        assert_eq!(resolve_stage_idx(&ctx, &fitness_v1()).unwrap(), 2);
    }

    #[test]
    fn variant_snapshot_carries_candidate_entity_type_and_source_when_present() {
        let snap = VariantSnapshot::from_entity_state(
            "var-x",
            &json!({
                "fields": {
                    "EvolutionId": "ev-001",
                    "BranchRef": "refs/heads/evo/x",
                    "CurrentStage": 0,
                    "CandidateEntityType": "Answer",
                    "CandidateSource": "[automaton]\nname = \"Answer\""
                }
            }),
        )
        .unwrap();
        assert_eq!(snap.candidate_entity_type, "Answer");
        assert!(snap.candidate_source.contains("[automaton]"));
    }

    #[test]
    fn variant_snapshot_defaults_candidate_fields_to_empty_when_absent() {
        let snap = VariantSnapshot::from_entity_state(
            "var-x",
            &json!({
                "fields": {
                    "EvolutionId": "ev-001",
                    "BranchRef": "refs/heads/evo/x",
                    "CurrentStage": 0
                }
            }),
        )
        .unwrap();
        assert_eq!(snap.candidate_entity_type, "");
        assert_eq!(snap.candidate_source, "");
    }
}
