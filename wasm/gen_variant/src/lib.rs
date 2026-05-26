//! gen_variant — directed-evolution variant generator.
//!
//! Trigger: `Evolution.StartGenerating`. Reads the Evolution entity, decides
//! how to mutate the target app's specs (pluggable mutagen: HTTP-callable
//! `coding_agent_url` if configured, else a deterministic downvote template
//! for the Phase-1 thin slice), then RETURNS a `sub_writes` envelope: one
//! `Repository.BatchWriteFiles` per variant branch, one `Variant.Create` per
//! variant, plus one `Evolution.VariantProposed` to advance the Evolution
//! FSM. The Temper kernel applies the writes — this module does not dispatch
//! actions (see ADR-0046 + ADR-0011).

#![forbid(unsafe_code)]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

/// Fixed timestamp keeps unit-test outputs and DST replays deterministic.
const CREATED_AT: &str = "1970-01-01T00:00:00Z";
/// Fan-out budget guards against runaway generators; per plan default = 1 for v1.
const DEFAULT_FAN_OUT: usize = 1;
/// Hard upper bound on fan-out so a misconfigured tenant cannot stampede.
const MAX_FAN_OUT: usize = 16;
/// Default deterministic-template ref name; mutated when the agent supplies one.
const DEFAULT_BRANCH_PREFIX: &str = "evo";
/// Phase-1 deterministic target entity for the downvote loop. Matches the
/// CSDL EntityType name (`Answer`) so `Variant.CandidateEntityType` round-
/// trips into `/verify/stage` and `/deploy/tenant` unchanged.
const DEFAULT_MUTATED_ENTITY: &str = "Answer";
/// Phase-1 deterministic target spec path under the genesis repo.
const DEFAULT_MUTATED_PATH: &str = "specs/answer.ioa.toml";

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let evolution = EvolutionSnapshot::from_entity_state(&ctx.entity_id, &ctx.entity_state)?;
        let plan = build_generation_plan(&ctx, &evolution)?;
        let sub_writes = build_gen_variant_sub_writes(&evolution, &plan)?;

        Ok(json!({
            "evolution_id": evolution.id,
            "target_app": evolution.target_app,
            "variant_count": plan.variants.len(),
            "mutagen": plan.source.as_str(),
            "sub_write_count": sub_writes.len(),
            "sub_writes": sub_writes,
        }))
    }
}

/// What the engine needs to know about the Evolution that triggered us.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EvolutionSnapshot {
    id: String,
    target_app: String,
    target_tenant: String,
    intent: String,
    problem_statement: String,
}

impl EvolutionSnapshot {
    fn from_entity_state(entity_id: &str, state: &Value) -> Result<Self, String> {
        let id = row_string(state, "Id").unwrap_or_else(|| entity_id.to_string());
        if id.is_empty() {
            return Err("Evolution entity_id is required".to_string());
        }
        let target_app = row_required(state, "TargetApp", "Evolution state")?;
        let target_tenant = row_string(state, "TargetTenant").unwrap_or_else(|| "default".to_string());
        let intent = row_required(state, "Intent", "Evolution state")?;
        let problem_statement = row_string(state, "ProblemStatement").unwrap_or_default();
        Ok(Self {
            id,
            target_app,
            target_tenant,
            intent,
            problem_statement,
        })
    }
}

/// Origin tag for telemetry; recorded in the Variant row so a reader can tell
/// agent-generated mutations apart from the deterministic v1 template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutagenSource {
    CodingAgent,
    DeterministicTemplate,
}

impl MutagenSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::CodingAgent => "coding_agent",
            Self::DeterministicTemplate => "deterministic_template",
        }
    }
}

/// A single mutated entity file that needs to be written onto a variant branch.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MutationFile {
    path: String,
    entity_type: String,
    content: String,
}

/// One candidate variant the kernel should materialize.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VariantPlan {
    variant_id: String,
    branch_ref: String,
    files: Vec<MutationFile>,
}

/// The full plan returned by either the agent path or the deterministic path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GenerationPlan {
    source: MutagenSource,
    variants: Vec<VariantPlan>,
}

/// Public entry-point for unit tests + the wasm `run`: produce the sub_writes.
fn build_gen_variant_sub_writes(
    evolution: &EvolutionSnapshot,
    plan: &GenerationPlan,
) -> Result<Vec<Value>, String> {
    if plan.variants.is_empty() {
        return Err("generation plan must contain at least one variant".to_string());
    }

    let mut writes = Vec::with_capacity(plan.variants.len() * 2 + 1);
    for variant in &plan.variants {
        if variant.files.is_empty() {
            return Err(format!(
                "variant {} must include at least one mutated file",
                variant.variant_id
            ));
        }
        let changes_json = encode_batch_changes(&variant.files)?;
        let commit_message = format!(
            "evolve({}): apply {} mutation",
            short(&variant.variant_id),
            evolution.intent
        );
        // Convention: the first mutated file is the candidate spec — the one
        // run_stage_caller verifies and merge_variant deploys. Additional
        // files are branch-provenance only (e.g. supporting WASM, sibling
        // entities edited in the same generation). The Variant row carries
        // only the candidate so the verify/deploy hops stay one-shot.
        let candidate = &variant.files[0];

        writes.push(json!({
            "entity_type": "Repository",
            "entity_id": repository_id_for(&evolution.target_app),
            "action": "BatchWriteFiles",
            "params": {
                "Ref": variant.branch_ref,
                "Changes": changes_json,
                "Message": commit_message,
                "Author": format!("evolver:{}", plan.source.as_str()),
                "ClientRequestId": format!("evo-{}-{}", evolution.id, variant.variant_id)
            }
        }));

        writes.push(json!({
            "entity_type": "Variant",
            "entity_id": variant.variant_id,
            "action": "Create",
            "params": {
                "EvolutionId": evolution.id,
                "BranchRef": variant.branch_ref,
                "CommitSha": "",
                "CurrentStage": 0,
                "KilledAtStage": "",
                "ObjectiveTotal": "{}",
                "Status": "Proposed",
                "CandidateEntityType": candidate_entity_type(&candidate.entity_type),
                "CandidateSource": candidate.content,
                "CreatedAt": CREATED_AT
            }
        }));
    }

    writes.push(json!({
        "entity_type": "Evolution",
        "entity_id": evolution.id,
        "action": "VariantProposed",
        "params": {}
    }));

    Ok(writes)
}

/// Pluggable variant planner. Tries the configured coding-agent HTTP endpoint
/// first (Phase-2 + autonomous overnight runs); falls back to the deterministic
/// downvote template so Phase-1 closes the loop with zero external services.
fn build_generation_plan(ctx: &Context, evolution: &EvolutionSnapshot) -> Result<GenerationPlan, String> {
    let fan_out = read_fan_out(&ctx.config);
    if let Some(agent_url) = ctx.config.get("coding_agent_url").cloned() {
        if !agent_url.is_empty() {
            return invoke_coding_agent(ctx, evolution, &agent_url, fan_out);
        }
    }
    build_deterministic_plan(evolution, fan_out)
}

/// HTTP-call the headless coding-agent runner (Phase-1 = `claude -p` via
/// temperpaw `coding_agent_runner`). Expects a JSON response shape
/// `{"variants":[{"variant_id"?,"branch_ref"?,"files":[{"path","entity_type","content"}]}]}`.
fn invoke_coding_agent(
    ctx: &Context,
    evolution: &EvolutionSnapshot,
    agent_url: &str,
    fan_out: usize,
) -> Result<GenerationPlan, String> {
    let payload = json!({
        "intent": evolution.intent,
        "problem_statement": evolution.problem_statement,
        "target_app": evolution.target_app,
        "target_tenant": evolution.target_tenant,
        "evolution_id": evolution.id,
        "fan_out": fan_out
    })
    .to_string();

    let resp = ctx
        .http_call("POST", agent_url, &[], &payload)
        .map_err(|e| format!("coding_agent http_call: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("coding_agent status {}", resp.status));
    }
    let body: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("coding_agent response is not JSON: {e}"))?;
    let parsed = parse_agent_variants(evolution, &body, fan_out)?;
    Ok(GenerationPlan {
        source: MutagenSource::CodingAgent,
        variants: parsed,
    })
}

/// Validate + normalize the coding-agent JSON into `VariantPlan` rows.
fn parse_agent_variants(
    evolution: &EvolutionSnapshot,
    body: &Value,
    fan_out: usize,
) -> Result<Vec<VariantPlan>, String> {
    let variants = body
        .get("variants")
        .and_then(Value::as_array)
        .ok_or_else(|| "coding_agent response missing 'variants' array".to_string())?;
    if variants.is_empty() {
        return Err("coding_agent returned zero variants".to_string());
    }
    let take = variants.len().min(fan_out);
    let mut planned = Vec::with_capacity(take);
    for (idx, variant) in variants.iter().take(take).enumerate() {
        let supplied_id = variant
            .get("variant_id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let variant_id = supplied_id
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| variant_id_for(&evolution.id, idx));
        let branch_ref = variant
            .get("branch_ref")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| branch_ref_for(&evolution.id, idx));
        let files_value = variant
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("variant {} missing 'files' array", variant_id))?;
        let mut files = Vec::with_capacity(files_value.len());
        for (file_idx, file) in files_value.iter().enumerate() {
            let path = read_str_field(file, "path")
                .ok_or_else(|| format!("variant {} file[{}] missing 'path'", variant_id, file_idx))?;
            let content = read_str_field(file, "content").ok_or_else(|| {
                format!("variant {} file[{}] missing 'content'", variant_id, file_idx)
            })?;
            let entity_type = read_str_field(file, "entity_type")
                .unwrap_or_else(|| infer_entity_type_from_path(&path));
            files.push(MutationFile {
                path,
                entity_type,
                content,
            });
        }
        if files.is_empty() {
            return Err(format!("variant {} produced zero mutated files", variant_id));
        }
        planned.push(VariantPlan {
            variant_id,
            branch_ref,
            files,
        });
    }
    Ok(planned)
}

/// Phase-1 deterministic mutation: take the target app's Answer spec and add
/// a `Downvote` action driving a new `downvotes` counter on Active/Accepted.
/// The mutated file is the whole spec text — it is byte-stable so DST and
/// unit tests can compare against a known reference.
fn build_deterministic_plan(
    evolution: &EvolutionSnapshot,
    fan_out: usize,
) -> Result<GenerationPlan, String> {
    let take = fan_out.max(1).min(MAX_FAN_OUT);
    let mut variants = Vec::with_capacity(take);
    for idx in 0..take {
        let variant_id = variant_id_for(&evolution.id, idx);
        let branch_ref = branch_ref_for(&evolution.id, idx);
        let content = downvote_template_spec();
        variants.push(VariantPlan {
            variant_id,
            branch_ref,
            files: alloc::vec![MutationFile {
                path: DEFAULT_MUTATED_PATH.to_string(),
                entity_type: DEFAULT_MUTATED_ENTITY.to_string(),
                content,
            }],
        });
    }
    Ok(GenerationPlan {
        source: MutagenSource::DeterministicTemplate,
        variants,
    })
}

/// The Phase-1 reference mutation: Answer (Active|Accepted) gains Downvote +
/// a `downvotes` counter. The text mirrors the seed Answer spec authored in
/// P1.1 (`temperpaw/os-apps/stackoverflow-agents/specs/answer.ioa.toml`) so
/// it round-trips through the verify cascade unchanged outside the additions.
fn downvote_template_spec() -> String {
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

/// Encode the BatchWriteFiles `Changes` parameter: a JSON-array string of
/// `{Op,Path,Content}` rows. Matches genesis Repository.BatchWriteFiles.
fn encode_batch_changes(files: &[MutationFile]) -> Result<String, String> {
    let rows: Vec<Value> = files
        .iter()
        .map(|file| {
            json!({
                "Op": "write",
                "Path": file.path,
                "Content": file.content,
                "Mode": "100644"
            })
        })
        .collect();
    serde_json::to_string(&rows).map_err(|e| format!("encode BatchWriteFiles changes: {e}"))
}

/// Stable, URL-safe variant id derived from the evolution id + fan-out index.
fn variant_id_for(evolution_id: &str, idx: usize) -> String {
    format!("var-{}-{:02}", sanitize(evolution_id), idx)
}

/// Stable branch name — matches `evo/<evolution>/var-NN` for grep-friendliness.
fn branch_ref_for(evolution_id: &str, idx: usize) -> String {
    format!(
        "refs/heads/{}/{}/var-{:02}",
        DEFAULT_BRANCH_PREFIX,
        sanitize(evolution_id),
        idx
    )
}

/// Target-app id → genesis Repository id. Genesis pre-seeds the app's repo
/// (see plan P1.6); the variant branches live under that repo.
fn repository_id_for(target_app: &str) -> String {
    format!("rp-{}", sanitize(target_app))
}

/// Fan-out budget read from integration config; bounded by `MAX_FAN_OUT`.
fn read_fan_out(config: &alloc::collections::BTreeMap<String, String>) -> usize {
    let raw = config
        .get("fan_out")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_FAN_OUT);
    raw.max(1).min(MAX_FAN_OUT)
}

fn read_str_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
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

fn infer_entity_type_from_path(path: &str) -> String {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .strip_suffix(".ioa.toml")
        .unwrap_or(path);
    if stem.is_empty() { "entity".to_string() } else { stem.to_string() }
}

/// Normalize an entity-type label into the TitleCase form CSDL expects.
/// If the input already starts with an ASCII uppercase letter we leave it
/// alone (handles compound names like "PullRequest" the agent might emit);
/// otherwise we capitalize the first ASCII letter. Empty input maps to
/// "Entity" to keep the deploy payload non-blank.
fn candidate_entity_type(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Entity".to_string();
    }
    let mut chars = trimmed.chars();
    let head = chars.next().unwrap();
    if head.is_ascii_uppercase() {
        return trimmed.to_string();
    }
    let mut out = String::with_capacity(trimmed.len());
    out.push(head.to_ascii_uppercase());
    out.extend(chars);
    out
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

fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn evolution() -> EvolutionSnapshot {
        EvolutionSnapshot {
            id: "ev-001".to_string(),
            target_app: "stackoverflow-agents".to_string(),
            target_tenant: "default".to_string(),
            intent: "downvote".to_string(),
            problem_statement: "Agents want to downvote bad answers.".to_string(),
        }
    }

    fn deterministic_plan() -> GenerationPlan {
        build_deterministic_plan(&evolution(), 1).unwrap()
    }

    #[test]
    fn deterministic_plan_produces_one_variant_with_downvote_spec() {
        let plan = deterministic_plan();
        assert_eq!(plan.source, MutagenSource::DeterministicTemplate);
        assert_eq!(plan.variants.len(), 1);
        let variant = &plan.variants[0];
        assert_eq!(variant.variant_id, "var-ev-001-00");
        assert_eq!(variant.branch_ref, "refs/heads/evo/ev-001/var-00");
        assert_eq!(variant.files.len(), 1);
        assert!(variant.files[0].content.contains("Downvote"));
        assert!(variant.files[0].content.contains("downvotes"));
        assert_eq!(variant.files[0].path, "specs/answer.ioa.toml");
        // TitleCase to match the CSDL EntityType name so the row's
        // CandidateEntityType round-trips into /verify/stage and /deploy/tenant.
        assert_eq!(variant.files[0].entity_type, "Answer");
    }

    #[test]
    fn fan_out_budget_caps_template_at_max_and_floors_at_one() {
        let mut config = BTreeMap::new();
        config.insert("fan_out".to_string(), "999".to_string());
        assert_eq!(read_fan_out(&config), MAX_FAN_OUT);

        config.insert("fan_out".to_string(), "0".to_string());
        assert_eq!(read_fan_out(&config), 1);

        config.remove("fan_out");
        assert_eq!(read_fan_out(&config), DEFAULT_FAN_OUT);
    }

    #[test]
    fn deterministic_plan_respects_fan_out_with_unique_ids() {
        let plan = build_deterministic_plan(&evolution(), 3).unwrap();
        assert_eq!(plan.variants.len(), 3);
        let ids: Vec<_> = plan.variants.iter().map(|v| v.variant_id.clone()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "variant ids must be unique");
    }

    #[test]
    fn sub_writes_contract_is_batchwrite_then_variantcreate_then_proposed() {
        let writes = build_gen_variant_sub_writes(&evolution(), &deterministic_plan()).unwrap();
        let pairs: Vec<_> = writes
            .iter()
            .map(|w| {
                (
                    w["entity_type"].as_str().unwrap(),
                    w["action"].as_str().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("Repository", "BatchWriteFiles"),
                ("Variant", "Create"),
                ("Evolution", "VariantProposed"),
            ]
        );
        assert_eq!(writes[0]["entity_id"], "rp-stackoverflow-agents");
        assert_eq!(writes[1]["params"]["EvolutionId"], "ev-001");
        assert_eq!(writes[1]["params"]["Status"], "Proposed");
        assert_eq!(writes[2]["entity_id"], "ev-001");
    }

    #[test]
    fn variant_create_carries_candidate_entity_type_and_source() {
        let writes = build_gen_variant_sub_writes(&evolution(), &deterministic_plan()).unwrap();
        let create = &writes[1];
        assert_eq!(create["entity_type"], "Variant");
        assert_eq!(create["action"], "Create");
        assert_eq!(create["params"]["CandidateEntityType"], "Answer");
        let source = create["params"]["CandidateSource"].as_str().unwrap();
        assert!(source.contains("Downvote"));
        assert!(source.contains("downvotes"));
        assert!(source.contains("[automaton]"));
    }

    #[test]
    fn variant_create_uses_first_file_as_candidate_when_agent_emits_many() {
        let body = json!({
            "variants": [
                {
                    "variant_id": "var-multi",
                    "branch_ref": "refs/heads/multi",
                    "files": [
                        {"path": "specs/answer.ioa.toml", "entity_type": "Answer", "content": "PRIMARY"},
                        {"path": "specs/question.ioa.toml", "entity_type": "Question", "content": "SECONDARY"}
                    ]
                }
            ]
        });
        let parsed = parse_agent_variants(&evolution(), &body, 1).unwrap();
        let plan = GenerationPlan {
            source: MutagenSource::CodingAgent,
            variants: parsed,
        };
        let writes = build_gen_variant_sub_writes(&evolution(), &plan).unwrap();
        assert_eq!(writes[1]["params"]["CandidateEntityType"], "Answer");
        assert_eq!(writes[1]["params"]["CandidateSource"], "PRIMARY");
        // BatchWriteFiles still carries both for branch provenance.
        let changes: Value =
            serde_json::from_str(writes[0]["params"]["Changes"].as_str().unwrap()).unwrap();
        assert_eq!(changes.as_array().unwrap().len(), 2);
    }

    #[test]
    fn candidate_entity_type_title_cases_lowercase_input_and_preserves_already_upper() {
        assert_eq!(candidate_entity_type("answer"), "Answer");
        assert_eq!(candidate_entity_type("Answer"), "Answer");
        assert_eq!(candidate_entity_type("PullRequest"), "PullRequest");
        assert_eq!(candidate_entity_type("  question  "), "Question");
        assert_eq!(candidate_entity_type(""), "Entity");
    }

    #[test]
    fn batch_write_changes_payload_is_json_array_of_write_ops() {
        let writes = build_gen_variant_sub_writes(&evolution(), &deterministic_plan()).unwrap();
        let changes_str = writes[0]["params"]["Changes"].as_str().unwrap();
        let changes: Value = serde_json::from_str(changes_str).unwrap();
        let arr = changes.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["Op"], "write");
        assert_eq!(arr[0]["Path"], "specs/answer.ioa.toml");
        assert!(arr[0]["Content"].as_str().unwrap().contains("Downvote"));
    }

    #[test]
    fn build_rejects_empty_plan() {
        let plan = GenerationPlan {
            source: MutagenSource::DeterministicTemplate,
            variants: Vec::new(),
        };
        assert!(build_gen_variant_sub_writes(&evolution(), &plan).is_err());
    }

    #[test]
    fn build_rejects_variant_with_no_files() {
        let plan = GenerationPlan {
            source: MutagenSource::DeterministicTemplate,
            variants: alloc::vec![VariantPlan {
                variant_id: "var-x".to_string(),
                branch_ref: "refs/heads/x".to_string(),
                files: Vec::new(),
            }],
        };
        let err = build_gen_variant_sub_writes(&evolution(), &plan).unwrap_err();
        assert!(err.contains("mutated file"));
    }

    #[test]
    fn parse_agent_variants_respects_fan_out_and_defaults_branch_and_id() {
        let body = json!({
            "variants": [
                {"files": [
                    {"path": "specs/answer.ioa.toml", "content": "[automaton]\nname=\"Answer\""}
                ]},
                {"variant_id": "var-custom", "branch_ref": "refs/heads/custom", "files": [
                    {"path": "specs/answer.ioa.toml", "entity_type": "answer", "content": "spec"}
                ]},
                {"files": [{"path": "specs/x.ioa.toml", "content": "spec3"}]}
            ]
        });
        let parsed = parse_agent_variants(&evolution(), &body, 2).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].variant_id, "var-ev-001-00");
        assert_eq!(parsed[0].branch_ref, "refs/heads/evo/ev-001/var-00");
        assert_eq!(parsed[0].files[0].entity_type, "answer");
        assert_eq!(parsed[1].variant_id, "var-custom");
        assert_eq!(parsed[1].branch_ref, "refs/heads/custom");
    }

    #[test]
    fn parse_agent_variants_rejects_missing_files() {
        let body = json!({"variants": [{}]});
        let err = parse_agent_variants(&evolution(), &body, 1).unwrap_err();
        assert!(err.contains("'files'"));
    }

    #[test]
    fn parse_agent_variants_rejects_missing_content() {
        let body = json!({"variants": [{"files": [{"path": "x.ioa.toml"}]}]});
        let err = parse_agent_variants(&evolution(), &body, 1).unwrap_err();
        assert!(err.contains("'content'"));
    }

    #[test]
    fn parse_agent_variants_rejects_empty_array() {
        let body = json!({"variants": []});
        let err = parse_agent_variants(&evolution(), &body, 1).unwrap_err();
        assert!(err.contains("zero variants"));
    }

    #[test]
    fn evolution_snapshot_parses_entity_state_with_fields_envelope() {
        let snap = EvolutionSnapshot::from_entity_state(
            "ev-002",
            &json!({
                "fields": {
                    "TargetApp": "stackoverflow-agents",
                    "TargetTenant": "tenant-a",
                    "Intent": "downvote",
                    "ProblemStatement": "..."
                }
            }),
        )
        .unwrap();
        assert_eq!(snap.id, "ev-002");
        assert_eq!(snap.target_tenant, "tenant-a");
        assert_eq!(snap.intent, "downvote");
    }

    #[test]
    fn evolution_snapshot_rejects_missing_required_intent() {
        let err = EvolutionSnapshot::from_entity_state(
            "ev-003",
            &json!({"fields": {"TargetApp": "x"}}),
        )
        .unwrap_err();
        assert!(err.contains("Intent"));
    }

    #[test]
    fn repository_id_sanitizes_target_app() {
        assert_eq!(repository_id_for("My/App"), "rp-my-app");
    }

    #[test]
    fn infer_entity_type_strips_suffix_and_directory() {
        assert_eq!(
            infer_entity_type_from_path("specs/question.ioa.toml"),
            "question"
        );
        assert_eq!(infer_entity_type_from_path("notes.txt"), "notes.txt");
    }

    #[test]
    fn build_sub_writes_deterministic_under_repeated_calls() {
        let plan = deterministic_plan();
        let first = build_gen_variant_sub_writes(&evolution(), &plan).unwrap();
        let second = build_gen_variant_sub_writes(&evolution(), &plan).unwrap();
        assert_eq!(first, second);
    }
}
