//! select_winner — directed-evolution selection-policy orchestrator.
//!
//! Triggered on `Evolution.BeginSelection`. Reads the FitnessSpec's
//! `SelectionPolicy` (v1 supports `lexicographic` and `weighted`), queries
//! genesis OData for this Evolution's Variants whose `Status=Survived`,
//! applies the policy to pick a winner, and RETURNS a `sub_writes` envelope
//! containing exactly one `Evolution.Select{WinnerVariantId}` write (or a
//! `Evolution.Revert` write when no survivors exist). The Temper kernel
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
/// Survivor cap so a degenerate query cannot allocate without bound.
const MAX_SURVIVORS: usize = 1024;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let evolution = EvolutionSnapshot::from_entity_state(&ctx.entity_id, &ctx.entity_state)?;
        let policy = if evolution.fitness_spec_id.is_empty() {
            SelectionPolicy::Lexicographic
        } else {
            let spec = fetch_selection_policy(&ctx, &evolution.fitness_spec_id)?;
            spec.policy
        };
        let survivors = fetch_survivors(&ctx, &evolution.id)?;
        let outcome = pick_winner(&survivors, policy);
        let sub_writes = build_select_winner_sub_writes(&evolution, &outcome)?;

        Ok(json!({
            "evolution_id": evolution.id,
            "policy": policy.as_str(),
            "survivor_count": survivors.len(),
            "outcome": outcome.as_str(),
            "winner_variant_id": match &outcome {
                Outcome::Winner(v) => v.variant_id.clone(),
                Outcome::NoSurvivors => String::new(),
            },
            "sub_write_count": sub_writes.len(),
            "sub_writes": sub_writes,
        }))
    }
}

/// Evolution fields needed for selection: id + (optional) fitness_spec_id.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EvolutionSnapshot {
    id: String,
    fitness_spec_id: String,
}

impl EvolutionSnapshot {
    fn from_entity_state(entity_id: &str, state: &Value) -> Result<Self, String> {
        let id = row_string(state, "Id").unwrap_or_else(|| entity_id.to_string());
        if id.is_empty() {
            return Err("Evolution entity_id is required".to_string());
        }
        let fitness_spec_id = row_string(state, "FitnessSpecId").unwrap_or_default();
        Ok(Self {
            id,
            fitness_spec_id,
        })
    }
}

/// FitnessSpec fields needed for selection (just the policy + optional tie-break).
#[derive(Debug, Clone, PartialEq, Eq)]
struct FitnessSelection {
    policy: SelectionPolicy,
}

impl FitnessSelection {
    fn from_row(row: &Value) -> Result<Self, String> {
        let raw = row_string(row, "SelectionPolicy").unwrap_or_else(|| "lexicographic".to_string());
        Ok(Self {
            policy: SelectionPolicy::parse(&raw),
        })
    }
}

/// Selection policies supported in v1. `lexicographic` = first survivor wins;
/// `weighted` = max(sum(score_value)) across all objective scores (uniform
/// weights). `pareto` is reserved for Phase 2 and falls back to lexicographic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionPolicy {
    Lexicographic,
    Weighted,
    Pareto,
}

impl SelectionPolicy {
    fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "weighted" => Self::Weighted,
            "pareto" => Self::Pareto,
            _ => Self::Lexicographic,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Lexicographic => "lexicographic",
            Self::Weighted => "weighted",
            Self::Pareto => "pareto",
        }
    }
}

/// One survivor candidate, normalized from an OData row.
#[derive(Debug, Clone, PartialEq)]
struct Survivor {
    variant_id: String,
    objective_total: f64,
    arrival_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    Winner(Selected),
    NoSurvivors,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Selected {
    variant_id: String,
}

impl Outcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Winner(_) => "winner",
            Self::NoSurvivors => "no_survivors",
        }
    }
}

/// Public entry-point for unit tests + `run`: produce the sub_writes envelope.
fn build_select_winner_sub_writes(
    evolution: &EvolutionSnapshot,
    outcome: &Outcome,
) -> Result<Vec<Value>, String> {
    match outcome {
        Outcome::Winner(selected) => Ok(alloc::vec![json!({
            "entity_type": "Evolution",
            "entity_id": evolution.id,
            "action": "Select",
            "params": {
                "WinnerVariantId": selected.variant_id
            }
        })]),
        Outcome::NoSurvivors => Err(format!(
            "Evolution {} has no Survived variants; cannot Select (Revert path requires Live state)",
            evolution.id
        )),
    }
}

/// Apply the policy to the survivor list. Returns `Outcome::NoSurvivors` when
/// the list is empty so the caller can surface that to telemetry; the
/// `build_*` step then turns it into a structured error envelope. Survivors
/// are pre-sorted by `arrival_index` for ties under lexicographic order.
fn pick_winner(survivors: &[Survivor], policy: SelectionPolicy) -> Outcome {
    if survivors.is_empty() {
        return Outcome::NoSurvivors;
    }
    let winner = match policy {
        SelectionPolicy::Lexicographic | SelectionPolicy::Pareto => &survivors[0],
        SelectionPolicy::Weighted => survivors
            .iter()
            .max_by(|a, b| {
                a.objective_total
                    .partial_cmp(&b.objective_total)
                    .unwrap_or(core::cmp::Ordering::Equal)
                    .then_with(|| b.arrival_index.cmp(&a.arrival_index)) // earlier wins on tie
            })
            .expect("non-empty checked above"),
    };
    Outcome::Winner(Selected {
        variant_id: winner.variant_id.clone(),
    })
}

fn fetch_selection_policy(ctx: &Context, fitness_spec_id: &str) -> Result<FitnessSelection, String> {
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
    FitnessSelection::from_row(&row)
}

fn fetch_survivors(ctx: &Context, evolution_id: &str) -> Result<Vec<Survivor>, String> {
    let url = format!(
        "{}/tdata/Variants?$filter=EvolutionId eq '{}' and Status eq 'Survived'&$top={}",
        temper_api_base(ctx),
        odata_key(evolution_id),
        MAX_SURVIVORS
    );
    let resp = ctx
        .http_call("GET", &url, &[], "")
        .map_err(|e| format!("fetch survivors: {e}"))?;
    if !(200..400).contains(&resp.status) {
        return Err(format!("Variants survivors status {}", resp.status));
    }
    parse_survivors(&resp.body)
}

/// Convert an OData collection response into normalized `Survivor`s.
/// Accepts both `{value:[...]}` and a bare JSON array so future shape
/// changes do not destabilize the orchestrator.
fn parse_survivors(body: &str) -> Result<Vec<Survivor>, String> {
    let parsed: Value =
        serde_json::from_str(body).map_err(|e| format!("survivors response not JSON: {e}"))?;
    let rows = match parsed.get("value").and_then(Value::as_array) {
        Some(arr) => arr.clone(),
        None => parsed
            .as_array()
            .ok_or_else(|| "survivors response is neither {value:[...]} nor [...]".to_string())?
            .clone(),
    };
    let mut survivors = Vec::with_capacity(rows.len().min(MAX_SURVIVORS));
    for (idx, row) in rows.iter().take(MAX_SURVIVORS).enumerate() {
        let variant_id = row_string(row, "Id")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("survivor row[{idx}] missing Id"))?;
        let objective_total = parse_objective_total(row_string(row, "ObjectiveTotal").as_deref());
        survivors.push(Survivor {
            variant_id,
            objective_total,
            arrival_index: idx,
        });
    }
    Ok(survivors)
}

/// Sum every numeric value in `ObjectiveTotal` (a serialized JSON object).
/// Uniform weights in v1; weighted-by-name lives behind a future config knob.
fn parse_objective_total(raw: Option<&str>) -> f64 {
    let s = match raw {
        Some(text) if !text.is_empty() => text,
        _ => return 0.0,
    };
    let parsed: Value = match serde_json::from_str(s) {
        Ok(v) => v,
        Err(_) => return 0.0,
    };
    let mut total = 0.0;
    if let Some(map) = parsed.as_object() {
        for (_, value) in map.iter() {
            if let Some(n) = value.as_f64() {
                total += n;
            }
        }
    } else if let Some(arr) = parsed.as_array() {
        for value in arr {
            if let Some(n) = value.as_f64() {
                total += n;
            }
        }
    } else if let Some(n) = parsed.as_f64() {
        total += n;
    }
    total
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

fn odata_key(input: &str) -> String {
    input.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evolution() -> EvolutionSnapshot {
        EvolutionSnapshot {
            id: "ev-001".to_string(),
            fitness_spec_id: "fs-001".to_string(),
        }
    }

    fn survivor(id: &str, total: f64, idx: usize) -> Survivor {
        Survivor {
            variant_id: id.to_string(),
            objective_total: total,
            arrival_index: idx,
        }
    }

    #[test]
    fn lexicographic_picks_first_arrived_survivor() {
        let s = alloc::vec![
            survivor("var-A", 1.0, 0),
            survivor("var-B", 99.0, 1),
        ];
        let outcome = pick_winner(&s, SelectionPolicy::Lexicographic);
        let Outcome::Winner(sel) = outcome else { panic!("expected Winner") };
        assert_eq!(sel.variant_id, "var-A");
    }

    #[test]
    fn weighted_picks_max_objective_total_with_earlier_tiebreak() {
        let s = alloc::vec![
            survivor("var-A", 1.0, 0),
            survivor("var-B", 5.0, 1),
            survivor("var-C", 5.0, 2),
            survivor("var-D", 3.5, 3),
        ];
        let outcome = pick_winner(&s, SelectionPolicy::Weighted);
        let Outcome::Winner(sel) = outcome else { panic!("expected Winner") };
        assert_eq!(sel.variant_id, "var-B", "earlier arrival wins on tie");
    }

    #[test]
    fn pareto_falls_back_to_lexicographic_in_v1() {
        let s = alloc::vec![survivor("var-A", 0.0, 0), survivor("var-B", 99.0, 1)];
        let outcome = pick_winner(&s, SelectionPolicy::Pareto);
        let Outcome::Winner(sel) = outcome else { panic!("expected Winner") };
        assert_eq!(sel.variant_id, "var-A");
    }

    #[test]
    fn empty_survivors_returns_no_survivors_outcome() {
        let outcome = pick_winner(&[], SelectionPolicy::Lexicographic);
        assert_eq!(outcome, Outcome::NoSurvivors);
    }

    #[test]
    fn sub_writes_emit_evolution_select_with_winner() {
        let outcome = Outcome::Winner(Selected {
            variant_id: "var-A".to_string(),
        });
        let writes = build_select_winner_sub_writes(&evolution(), &outcome).unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0]["entity_type"], "Evolution");
        assert_eq!(writes[0]["entity_id"], "ev-001");
        assert_eq!(writes[0]["action"], "Select");
        assert_eq!(writes[0]["params"]["WinnerVariantId"], "var-A");
    }

    #[test]
    fn sub_writes_error_when_no_survivors() {
        let err = build_select_winner_sub_writes(&evolution(), &Outcome::NoSurvivors).unwrap_err();
        assert!(err.contains("no Survived variants"));
    }

    #[test]
    fn parse_objective_total_handles_object_and_array_shapes() {
        assert_eq!(parse_objective_total(Some("{\"a\":1.0,\"b\":2.0}")), 3.0);
        assert_eq!(parse_objective_total(Some("[1.0, 2.5]")), 3.5);
        assert_eq!(parse_objective_total(Some("4.0")), 4.0);
        assert_eq!(parse_objective_total(Some("")), 0.0);
        assert_eq!(parse_objective_total(None), 0.0);
        assert_eq!(parse_objective_total(Some("garbage")), 0.0);
    }

    #[test]
    fn parse_survivors_accepts_value_envelope_and_bare_array() {
        let body_value = json!({
            "value": [
                {"fields": {"Id": "var-A", "ObjectiveTotal": "{\"x\":1.0}"}},
                {"fields": {"Id": "var-B", "ObjectiveTotal": "{\"x\":2.0}"}}
            ]
        })
        .to_string();
        let survivors = parse_survivors(&body_value).unwrap();
        assert_eq!(survivors.len(), 2);
        assert_eq!(survivors[0].variant_id, "var-A");
        assert_eq!(survivors[0].arrival_index, 0);

        let body_array = json!([
            {"fields": {"Id": "var-X", "ObjectiveTotal": "{}"}}
        ])
        .to_string();
        let survivors_arr = parse_survivors(&body_array).unwrap();
        assert_eq!(survivors_arr.len(), 1);
        assert_eq!(survivors_arr[0].variant_id, "var-X");
    }

    #[test]
    fn parse_survivors_rejects_missing_id() {
        let body = json!({"value": [{"fields": {"ObjectiveTotal": "{}"}}]}).to_string();
        let err = parse_survivors(&body).unwrap_err();
        assert!(err.contains("missing Id"));
    }

    #[test]
    fn parse_survivors_caps_at_max() {
        let mut rows = Vec::new();
        for i in 0..(MAX_SURVIVORS + 5) {
            rows.push(json!({"fields": {"Id": format!("var-{i:04}"), "ObjectiveTotal": "{}"}}));
        }
        let body = json!({"value": rows}).to_string();
        let survivors = parse_survivors(&body).unwrap();
        assert_eq!(survivors.len(), MAX_SURVIVORS);
    }

    #[test]
    fn selection_policy_parses_known_names_case_insensitively() {
        assert_eq!(SelectionPolicy::parse("Lexicographic"), SelectionPolicy::Lexicographic);
        assert_eq!(SelectionPolicy::parse("WEIGHTED"), SelectionPolicy::Weighted);
        assert_eq!(SelectionPolicy::parse("pareto"), SelectionPolicy::Pareto);
        assert_eq!(SelectionPolicy::parse("garbage"), SelectionPolicy::Lexicographic);
    }

    #[test]
    fn fitness_selection_defaults_to_lexicographic_when_field_missing() {
        let spec = FitnessSelection::from_row(&json!({"fields": {}})).unwrap();
        assert_eq!(spec.policy, SelectionPolicy::Lexicographic);
    }

    #[test]
    fn evolution_snapshot_accepts_missing_fitness_spec_id() {
        let snap = EvolutionSnapshot::from_entity_state(
            "ev-002",
            &json!({"fields": {"TargetApp": "x"}}),
        )
        .unwrap();
        assert_eq!(snap.fitness_spec_id, "");
    }

    #[test]
    fn weighted_outcome_is_deterministic_for_identical_input() {
        let s = alloc::vec![survivor("var-A", 1.0, 0), survivor("var-B", 2.0, 1)];
        let one = pick_winner(&s, SelectionPolicy::Weighted);
        let two = pick_winner(&s, SelectionPolicy::Weighted);
        assert_eq!(one, two);
    }
}
