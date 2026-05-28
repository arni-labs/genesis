use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> std::result::Result<(String, Value), String> {
        let ctx = Context::from_host().map_err(|error| error.to_string())?;
        if ctx.trigger_action != "Configure" {
            return Err(format!(
                "agent_answers_state_verifier only supports ValidatorRun.Configure, got {}",
                ctx.trigger_action
            ));
        }

        let trial_suite_id = param_str(&ctx.trigger_params, "trial_suite_id");
        let candidate_id = param_str(&ctx.trigger_params, "candidate_id");
        let scenario_id = param_str(&ctx.trigger_params, "scenario_id");
        let validator_kind = param_str(&ctx.trigger_params, "validator_kind");
        let evaluator_ref = ctx
            .config
            .get("evaluator_ref")
            .cloned()
            .unwrap_or_else(|| "genesis://nerdsane/agent-answers-evaluation@0.1.2".to_string());

        let mut findings = Vec::new();
        if trial_suite_id.trim().is_empty() {
            findings.push("trial_suite_id is required".to_string());
        }
        if candidate_id.trim().is_empty() {
            findings.push("candidate_id is required".to_string());
        }
        if scenario_id.trim().is_empty() {
            findings.push("scenario_id is required".to_string());
        }
        if validator_kind.trim().is_empty() {
            findings.push("validator_kind is required".to_string());
        }
        let lower_candidate = candidate_id.to_ascii_lowercase();
        if lower_candidate.contains("evaluator")
            || lower_candidate.contains("invalid")
            || lower_candidate.contains("regression")
        {
            findings.push(
                "candidate_id indicates an evaluator mutation, invalid candidate, or regression"
                    .to_string(),
            );
        }

        let passed = findings.is_empty();
        let summary = if passed {
            format!(
                "Agent Answers state verifier accepted candidate {candidate_id} for scenario {scenario_id}."
            )
        } else {
            format!(
                "Agent Answers state verifier rejected candidate {candidate_id}: {}.",
                findings.join("; ")
            )
        };
        let params = json!({
            "evidence_locator": format!(
                "temper://agent-answers-evaluation/{}/{}",
                scenario_id,
                candidate_id
            ),
            "result_summary": summary,
            "evaluator_ref": evaluator_ref,
            "metrics": {
                "viability_regression_count": findings.len(),
                "validator_input_count": 4
            },
            "findings": findings,
        });
        Ok((if passed { "Pass" } else { "Fail" }.to_string(), params))
    })();

    match result {
        Ok((action, params)) => temper_wasm_sdk::set_success_result(&action, &params),
        Err(error) => temper_wasm_sdk::set_error_result(&error),
    }
    0
}

fn param_str(params: &Value, key: &str) -> String {
    let pascal = to_pascal_case(key);
    params
        .get(key)
        .or_else(|| params.get(pascal.as_str()))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn to_pascal_case(key: &str) -> String {
    key.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pascal_case_supports_odata_action_params() {
        let params = json!({ "CandidateId": "variant-1" });

        assert_eq!(param_str(&params, "candidate_id"), "variant-1");
    }
}
