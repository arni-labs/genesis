fn route_observer(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    signal_id: &str,
    work_item_fields: &Value,
    output: &Value,
) -> Result<Value, String> {
    let actionable = lookup_bool_deep(output, &["actionable", "Actionable"]).unwrap_or(true);
    if !actionable {
        let reason = nonempty(
            lookup_string_deep(output, &["rationale", "reason", "summary"]),
            "Observer worker marked the signal as not actionable.".to_string(),
        );
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Signals",
            signal_id,
            "IgnoreSignal",
            json!({ "Reason": reason }),
        )?;
        return Ok(json!({
            "routed": "observer",
            "signal_id": signal_id,
            "actionable": false,
        }));
    }
    if !datadog_evidence_satisfies_required_contract(output, "datadog-measured") {
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Signals",
            signal_id,
            "FailSignalObservation",
            json!({
                "error": "missing_datadog_evidence",
                "error_message": "Observer output was actionable but did not include structured Datadog evidence with query, time window, result count, interpretation, zero-result meaning, and usable Datadog URL.",
                "integration": "work_item_result_router",
            }),
        )?;
        return Ok(json!({
            "routed": "observer",
            "signal_id": signal_id,
            "actionable": true,
            "failed_closed": "missing_datadog_evidence",
        }));
    }

    let signal = get_entity(ctx, base_url, headers, "Signals", signal_id)?;
    let signal_fields = state_fields(&signal);
    let organism_id = field_str(&signal_fields, &["OrganismId"]);
    let signal_summary = field_str(&signal_fields, &["Summary"]);
    let signal_kind = field_str(&signal_fields, &["SignalKind"]);
    let signal_evidence_artifact_id = field_str(&signal_fields, &["EvidenceArtifactId"]);
    let worker_run_id = field_str(work_item_fields, &["WorkerRunId"]);
    let proposals = observer_direction_candidates(output);
    let mut routed = Vec::new();

    for (index, proposal) in proposals.iter().enumerate() {
        let pressure_class = nonempty(
            lookup_string_deep(proposal, &["pressure_class", "PressureClass"]),
            signal_kind.clone(),
        );
        let pressure_summary = nonempty(
            lookup_string_deep(proposal, &["pressure_summary", "summary", "rationale"]),
            signal_summary.clone(),
        );
        let title = nonempty(
            lookup_string_deep(proposal, &["title", "Title"]),
            format!("Evolve for {pressure_class}"),
        );
        let direction_summary = nonempty(
            lookup_string_deep(
                proposal,
                &["direction_summary", "DirectionSummary", "proposal"],
            ),
            pressure_summary.clone(),
        );
        let autonomy_lane = nonempty(
            lookup_string_deep(proposal, &["autonomy_lane", "AutonomyLane"]),
            if pressure_class.to_ascii_lowercase().contains("repair") {
                "repair-auto".to_string()
            } else {
                "human-approval".to_string()
            },
        );
        let proposed_adaptation_goal = nonempty(
            lookup_string_deep(
                proposal,
                &[
                    "proposed_adaptation_goal",
                    "ProposedAdaptationGoal",
                    "adaptation_goal",
                ],
            ),
            direction_summary.clone(),
        );
        let proposed_constraints =
            lookup_value_deep(proposal, &["proposed_viability_constraints", "constraints"])
                .unwrap_or_else(|| json!([]))
                .to_string();

        let pressure_id = create_entity(ctx, base_url, headers, "Pressures")?;
        let direction_id = create_entity(ctx, base_url, headers, "Directions")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Pressures",
            &pressure_id,
            "InferPressure",
            json!({
                "OrganismId": organism_id,
                "PressureClass": pressure_class,
                "Summary": pressure_summary,
                "SignalIdsJson": json!([signal_id]).to_string(),
                "EvidenceArtifactId": signal_evidence_artifact_id,
                "WorkerRunId": worker_run_id,
            }),
        )?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Directions",
            &direction_id,
            "ProposeDirection",
            json!({
                "OrganismId": organism_id,
                "PressureIdsJson": json!([pressure_id]).to_string(),
                "PressureClass": pressure_class,
                "Title": title,
                "Summary": direction_summary,
                "ProvenanceJson": json!({
                    "signal_id": signal_id,
                    "pressure_id": pressure_id,
                    "observer_output": output,
                    "observer_candidate": proposal,
                    "candidate_index": index,
                }).to_string(),
                "AutonomyLane": autonomy_lane,
                "ProposedAdaptationGoal": proposed_adaptation_goal,
                "ProposedViabilityConstraintsJson": proposed_constraints,
                "WorkerRunId": worker_run_id,
            }),
        )?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Signals",
            signal_id,
            "LinkSignalToPressure",
            json!({ "PressureId": pressure_id }),
        )?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Pressures",
            &pressure_id,
            "FramePressureAsDirection",
            json!({ "DirectionId": direction_id }),
        )?;
        let episode_start_request_id = maybe_auto_start_repair_direction(
            ctx,
            base_url,
            headers,
            output,
            &organism_id,
            &organism_id,
            &direction_id,
            &pressure_class,
            &autonomy_lane,
            &proposed_adaptation_goal,
            &proposed_constraints,
            &worker_run_id,
        )?;
        routed.push(json!({
            "pressure_id": pressure_id,
            "direction_id": direction_id,
            "title": title,
            "episode_start_request_id": episode_start_request_id,
        }));
    }

    Ok(json!({
        "routed": "observer",
        "signal_id": signal_id,
        "actionable": true,
        "directions": routed,
    }))
}

fn observer_direction_candidates(output: &Value) -> Vec<Value> {
    for key in ["directions", "proposed_directions", "candidate_directions"] {
        if let Some(candidates) = output.get(key).and_then(Value::as_array) {
            let values = candidates
                .iter()
                .filter(|candidate| candidate.as_object().is_some())
                .cloned()
                .collect::<Vec<_>>();
            if !values.is_empty() {
                return values;
            }
        }
    }
    vec![output.clone()]
}

#[allow(clippy::too_many_arguments)]
fn maybe_auto_start_repair_direction(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    observer_output: &Value,
    organism_id: &str,
    organism_lookup_id: &str,
    direction_id: &str,
    pressure_class: &str,
    autonomy_lane: &str,
    adaptation_goal: &str,
    proposed_constraints_json: &str,
    worker_run_id: &str,
) -> Result<String, String> {
    let pressure = pressure_class.to_ascii_lowercase();
    let lane = autonomy_lane.to_ascii_lowercase();
    if !pressure.contains("repair") || !lane.contains("auto") {
        return Ok(String::new());
    }

    let organism = get_entity(ctx, base_url, headers, "Organisms", organism_lookup_id)?;
    let organism_fields = state_fields(&organism);
    let parent_version_id = nonempty(
        field_str(&organism_fields, &["ParentVersionId"]),
        field_str(&organism_fields, &["OrganismVersionId"]),
    );
    if parent_version_id.trim().is_empty() {
        return Err(format!(
            "repair auto-start for direction {direction_id} requires an active organism parent version"
        ));
    }

    let request_id = create_entity(ctx, base_url, headers, "EpisodeStartRequests")?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "EpisodeStartRequests",
        &request_id,
        "SubmitEpisodeStartRequest",
        json!({
            "DirectionId": direction_id,
            "OrganismId": organism_id,
            "ParentVersionId": parent_version_id,
            "AutonomyLane": autonomy_lane,
            "RequestedBy": nonempty(worker_run_id.to_string(), "observer-worker".to_string()),
            "AdaptationGoal": adaptation_goal,
            "HumanNotes": "Repair direction auto-started under the active autonomy policy.",
            "ViabilityConstraintsJson": proposed_constraints_json,
            "MetricsJson": lookup_value_deep(observer_output, &["metric_definitions", "metrics", "MetricDefinitions"])
                .unwrap_or_else(|| json!([]))
                .to_string(),
            "EvaluationStagesJson": lookup_value_deep(observer_output, &["evaluation_stages", "EvaluationStages"])
                .unwrap_or_else(|| json!([]))
                .to_string(),
            "EliminationRulesJson": lookup_value_deep(observer_output, &["elimination_rules", "EliminationRules"])
                .unwrap_or_else(|| json!([]))
                .to_string(),
            "ScoringRulesJson": lookup_value_deep(observer_output, &["scoring_rules", "ScoringRules"])
                .unwrap_or_else(|| json!([]))
                .to_string(),
            "SelectionStatement": nonempty(
                lookup_string_deep(observer_output, &["selection_statement", "SelectionStatement"]),
                "Select the repair variant that restores failing behavior without regressions.".to_string()
            ),
            "ContractJson": json!({
                "source": "observer-worker",
                "reason": "repair-auto",
                "observer_output": observer_output,
            }).to_string(),
            "StartedBy": nonempty(worker_run_id.to_string(), "observer-worker".to_string()),
            "Reason": "Auto-start repair episode under active autonomy policy.",
        }),
    )?;

    Ok(request_id)
}
