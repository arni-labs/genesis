fn route_observer(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    target_entity_type: &str,
    target_entity_id: &str,
    work_item_fields: &Value,
    output: &Value,
) -> Result<Value, String> {
    let target = observer_route_target(
        ctx,
        base_url,
        headers,
        target_entity_type,
        target_entity_id,
        work_item_fields,
        output,
    )?;
    record_scout_signals(ctx, base_url, headers, work_item_fields, output, &target)?;
    let actionable = lookup_bool_deep(output, &["actionable", "Actionable"]).unwrap_or(true);
    if !actionable {
        let reason = nonempty(
            lookup_string_deep(output, &["rationale", "reason", "summary"]),
            "Observer worker marked the target as not actionable.".to_string(),
        );
        if let Some(signal_id) = target.signal_id.as_deref() {
            post_directed_action(
                ctx,
                base_url,
                headers,
                "Signals",
                signal_id,
                "IgnoreSignal",
                json!({ "Reason": reason }),
            )?;
        }
        return Ok(json!({
            "routed": "observer",
            "target_entity_type": target.target_entity_type,
            "target_entity_id": target.target_entity_id,
            "signal_id": target.signal_id,
            "organism_id": target.organism_id,
            "actionable": false,
        }));
    }
    let datadog_evidence_satisfied =
        datadog_evidence_satisfies_required_contract(output, "datadog-measured");

    let worker_run_id = field_str(work_item_fields, &["WorkerRunId"]);
    let proposals = observer_direction_candidates(output);
    let mut routed = Vec::new();

    for (index, proposal) in proposals.iter().enumerate() {
        let pressure_class = nonempty(
            lookup_string_deep(proposal, &["pressure_class", "PressureClass"]),
            target.default_pressure_class.clone(),
        );
        let pressure_summary = nonempty(
            lookup_string_deep(proposal, &["pressure_summary", "summary", "rationale"]),
            target.default_summary.clone(),
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
                "OrganismId": target.organism_id,
                "PressureClass": pressure_class,
                "Summary": pressure_summary,
                "SignalIdsJson": target.signal_ids_json(),
                "EvidenceArtifactId": target.evidence_artifact_id,
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
                "OrganismId": target.organism_id,
                "PressureIdsJson": json!([pressure_id]).to_string(),
                "PressureClass": pressure_class,
                "Title": title,
                "Summary": direction_summary,
                "ProvenanceJson": json!({
                    "target_entity_type": target.target_entity_type,
                    "target_entity_id": target.target_entity_id,
                    "signal_id": target.signal_id,
                    "pressure_id": pressure_id,
                    "observer_output": output,
                    "observer_candidate": proposal,
                    "candidate_index": index,
                    "datadog_evidence_satisfied": datadog_evidence_satisfied,
                }).to_string(),
                "AutonomyLane": autonomy_lane,
                "ProposedAdaptationGoal": proposed_adaptation_goal,
                "ProposedViabilityConstraintsJson": proposed_constraints,
                "WorkerRunId": worker_run_id,
            }),
        )?;
        if let Some(signal_id) = target.signal_id.as_deref() {
            post_directed_action(
                ctx,
                base_url,
                headers,
                "Signals",
                signal_id,
                "LinkSignalToPressure",
                json!({ "PressureId": pressure_id }),
            )?;
        }
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
            &target.organism_id,
            &target.organism_lookup_id,
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
        "target_entity_type": target.target_entity_type,
        "target_entity_id": target.target_entity_id,
        "signal_id": target.signal_id,
        "organism_id": target.organism_id,
        "actionable": true,
        "datadog_evidence_satisfied": datadog_evidence_satisfied,
        "directions": routed,
    }))
}

#[derive(Clone, Debug)]
struct ObserverRouteTarget {
    target_entity_type: String,
    target_entity_id: String,
    organism_id: String,
    organism_lookup_id: String,
    signal_id: Option<String>,
    default_pressure_class: String,
    default_summary: String,
    evidence_artifact_id: String,
}

impl ObserverRouteTarget {
    fn signal_ids_json(&self) -> String {
        match self.signal_id.as_deref() {
            Some(signal_id) if !signal_id.trim().is_empty() => json!([signal_id]).to_string(),
            _ => json!([]).to_string(),
        }
    }
}

fn observer_route_target(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    target_entity_type: &str,
    target_entity_id: &str,
    work_item_fields: &Value,
    output: &Value,
) -> Result<ObserverRouteTarget, String> {
    match target_entity_type {
        "Signal" => {
            let signal = get_entity(ctx, base_url, headers, "Signals", target_entity_id)?;
            let signal_fields = state_fields(&signal);
            let organism_id = field_str(&signal_fields, &["OrganismId"]);
            Ok(ObserverRouteTarget {
                target_entity_type: "Signal".to_string(),
                target_entity_id: target_entity_id.to_string(),
                organism_lookup_id: organism_id.clone(),
                organism_id,
                signal_id: Some(target_entity_id.to_string()),
                default_pressure_class: field_str(&signal_fields, &["SignalKind"]),
                default_summary: field_str(&signal_fields, &["Summary"]),
                evidence_artifact_id: field_str(&signal_fields, &["EvidenceArtifactId"]),
            })
        }
        "Organism" => Ok(observer_route_target_for_organism(
            target_entity_id,
            work_item_fields,
            output,
        )),
        other => Err(format!(
            "observer route does not support target entity type {other}"
        )),
    }
}

fn observer_route_target_for_organism(
    organism_id: &str,
    work_item_fields: &Value,
    output: &Value,
) -> ObserverRouteTarget {
    let default_summary = nonempty(
        lookup_string_deep(output, &["pressure_summary", "summary", "rationale"]),
        nonempty(
            field_str(work_item_fields, &["Summary"]),
            "Observer found evidence-grounded pressure.".to_string(),
        ),
    );
    ObserverRouteTarget {
        target_entity_type: "Organism".to_string(),
        target_entity_id: organism_id.to_string(),
        organism_id: organism_id.to_string(),
        organism_lookup_id: organism_id.to_string(),
        signal_id: None,
        default_pressure_class: nonempty(
            lookup_string_deep(output, &["pressure_class", "PressureClass"]),
            "observation".to_string(),
        ),
        default_summary,
        evidence_artifact_id: field_str(work_item_fields, &["EvidenceArtifactId"]),
    }
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

/// Persist deduplicated signals discovered by a scheduled signal-scout
/// sweep. Scout-recorded signals arrive pre-interpreted (the same
/// observer run already proposed directions), so `signal_observer`
/// skips re-queueing interpretation for Source "signal-scout".
fn record_scout_signals(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_item_fields: &Value,
    output: &Value,
    target: &ObserverRouteTarget,
) -> Result<(), String> {
    let correlation: Value = serde_json::from_str(&field_str(work_item_fields, &["CorrelationJson"]))
        .unwrap_or_else(|_| json!({}));
    let scout_id = lookup_string_deep(&correlation, &["signal_scout_id"]);
    if scout_id.trim().is_empty() {
        return Ok(());
    }
    let Some(signals) = output.get("signals").and_then(Value::as_array) else {
        return Ok(());
    };
    // Mirror the scout prompt's cap so a runaway output cannot flood
    // the signal plane.
    const SCOUT_SIGNALS_MAX: usize = 3;
    for entry in signals.iter().take(SCOUT_SIGNALS_MAX) {
        let fingerprint = lookup_string_deep(entry, &["fingerprint", "Fingerprint"]);
        if fingerprint.trim().is_empty() {
            continue;
        }
        if signal_fingerprint_exists(ctx, base_url, headers, &target.organism_id, &fingerprint)? {
            continue;
        }
        let summary = nonempty(
            lookup_string_deep(entry, &["summary", "Summary"]),
            format!("Scout finding {fingerprint}"),
        );
        let signal_id = create_entity(ctx, base_url, headers, "Signals")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "Signals",
            &signal_id,
            "RecordSignal",
            json!({
                "Source": "signal-scout",
                "SignalKind": nonempty(
                    lookup_string_deep(entry, &["signal_kind", "SignalKind", "source"]),
                    "observability".to_string(),
                ),
                "OrganismId": target.organism_id,
                "Summary": summary,
                "EvidenceArtifactId": target.evidence_artifact_id,
                "Fingerprint": fingerprint,
                "CorrelationJson": json!({
                    "signal_scout_id": scout_id,
                    "evidence": entry.get("evidence").cloned().unwrap_or(Value::Null),
                }).to_string(),
            }),
        )?;
    }
    Ok(())
}

fn signal_fingerprint_exists(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    organism_id: &str,
    fingerprint: &str,
) -> Result<bool, String> {
    let filter = format!(
        "OrganismId eq {} and Fingerprint eq {}",
        odata_string_literal(organism_id),
        odata_string_literal(fingerprint)
    );
    let url = format!("{base_url}/tdata/Signals?$filter={}&$top=1", urlencode(&filter));
    let resp = ctx
        .http_call("GET", &url, headers, "")
        .map_err(|e| format!("signal fingerprint lookup: {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("signal fingerprint lookup returned {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("fingerprint lookup json: {e}"))?;
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .map(|rows| !rows.is_empty())
        .unwrap_or(false))
}
