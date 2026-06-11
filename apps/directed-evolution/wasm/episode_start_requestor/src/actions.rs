fn start_episode_from_contract(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    request_id: &str,
    contract: EpisodeStartContract,
) -> Result<Value, String> {
    let episode_id = create_entity(ctx, base_url, headers, "Episodes")?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "Episodes",
        &episode_id,
        "BeginEpisodeNegotiation",
        json!({
            "DirectionId": contract.direction_id,
            "OrganismId": contract.organism_id,
            "ParentVersionId": contract.parent_version_id,
            "AutonomyLane": contract.autonomy_lane,
        }),
    )?;

    let metric_ids = activate_metrics(ctx, base_url, headers, &contract.metrics)?;
    let constraint_ids = activate_constraints(
        ctx,
        base_url,
        headers,
        &episode_id,
        &contract.constraints,
        &contract.requested_by,
    )?;
    let elimination_rule_ids = activate_elimination_rules(
        ctx,
        base_url,
        headers,
        &episode_id,
        &contract.elimination_rules,
        &metric_ids,
        &contract.requested_by,
    )?;
    let scoring_rule_ids = activate_scoring_rules(
        ctx,
        base_url,
        headers,
        &episode_id,
        &contract.scoring_rules,
        &metric_ids,
        &contract.requested_by,
    )?;
    let stage_ids = activate_stages(ctx, base_url, headers, &episode_id, &contract.stages)?;
    let simulated_user_plan_id =
        activate_frozen_simulated_user_plan(ctx, base_url, headers, &episode_id, &contract)?;
    let selection_protocol_id = activate_frozen_selection_protocol(
        ctx,
        base_url,
        headers,
        &episode_id,
        &contract,
        &metric_ids,
        &elimination_rule_ids,
        &scoring_rule_ids,
    )?;
    let adaptation_goal_id = create_entity(ctx, base_url, headers, "AdaptationGoals")?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "AdaptationGoals",
        &adaptation_goal_id,
        "ActivateAdaptationGoal",
        json!({
            "EpisodeId": episode_id,
            "GoalStatement": contract.adaptation_goal,
            "CreatedByWorkerRunId": contract.requested_by,
            "HumanNotes": human_notes_with_contract(&contract.human_notes, &contract.contract_json),
        }),
    )?;
    // PlanEpisode records the full protocol graph — frozen plan,
    // frozen protocol, evaluator ref — matching the human-gated path
    // (ADR-0018: nothing can move the goalposts after start).
    post_directed_action(
        ctx,
        base_url,
        headers,
        "Episodes",
        &episode_id,
        "PlanEpisode",
        json!({
            "DirectionId": contract.direction_id,
            "OrganismId": contract.organism_id,
            "ParentVersionId": contract.parent_version_id,
            "AutonomyLane": contract.autonomy_lane,
            "AdaptationGoalId": adaptation_goal_id,
            "ViabilityConstraintIdsJson": json!(constraint_ids).to_string(),
            "MetricDefinitionIdsJson": json!(metric_ids.ids()).to_string(),
            "EvaluationStageIdsJson": json!(stage_ids).to_string(),
            "EliminationRuleIdsJson": json!(elimination_rule_ids).to_string(),
            "ScoringRuleIdsJson": json!(scoring_rule_ids).to_string(),
            "SimulatedUserPlanId": simulated_user_plan_id,
            "SelectionProtocolId": selection_protocol_id,
            "EvaluatorRef": contract.evaluator_ref,
            "OrganismParentRef": contract.parent_version_id,
            "PlannedBy": contract.started_by,
            "PlanSummary": human_notes_with_contract(&contract.human_notes, &contract.contract_json),
        }),
    )?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "Directions",
        &contract.direction_id,
        "SelectDirection",
        json!({
            "EpisodeId": episode_id,
            "SelectedBy": contract.started_by,
            "SelectionNotes": format!("Selected through EpisodeStartRequest {request_id}."),
        }),
    )?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "Episodes",
        &episode_id,
        "StartEpisode",
        json!({
            "StartedBy": contract.started_by,
            "Reason": contract.reason,
        }),
    )?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "EpisodeStartRequests",
        request_id,
        "MarkEpisodeStartRequestStarted",
        json!({
            "EpisodeId": episode_id,
            "Summary": "Episode materialized from negotiated director contract.",
            "EvidenceArtifactId": "",
        }),
    )?;

    Ok(json!({
        "started": true,
        "request_id": request_id,
        "episode_id": episode_id,
    }))
}

fn activate_metrics(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    metrics: &[MetricPlan],
) -> Result<MetricIds, String> {
    let mut pairs = Vec::new();
    for metric in metrics {
        let metric_id = create_entity(ctx, base_url, headers, "MetricDefinitions")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "MetricDefinitions",
            &metric_id,
            "ActivateMetricDefinition",
            json!({
                "MetricName": metric.name,
                "MetricKind": metric.kind,
                "Unit": metric.unit,
                "HigherIsBetter": metric.higher_is_better,
                "Description": metric.description,
            }),
        )?;
        pairs.push((metric.name.clone(), metric_id));
    }
    Ok(MetricIds { pairs })
}

fn activate_constraints(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    episode_id: &str,
    constraints: &[ConstraintPlan],
    requested_by: &str,
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for constraint in constraints {
        let constraint_id = create_entity(ctx, base_url, headers, "ViabilityConstraints")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "ViabilityConstraints",
            &constraint_id,
            "ActivateViabilityConstraint",
            json!({
                "EpisodeId": episode_id,
                "ConstraintStatement": constraint.statement,
                "ConstraintKind": constraint.kind,
                "CreatedByWorkerRunId": requested_by,
            }),
        )?;
        ids.push(constraint_id);
    }
    Ok(ids)
}

fn activate_elimination_rules(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    episode_id: &str,
    rules: &[RulePlan],
    metric_ids: &MetricIds,
    requested_by: &str,
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for rule in rules {
        let rule_id = create_entity(ctx, base_url, headers, "EliminationRules")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "EliminationRules",
            &rule_id,
            "ActivateEliminationRule",
            json!({
                "EpisodeId": episode_id,
                "RuleStatement": rule.statement,
                "MetricIdsJson": json!(metric_ids.ids_for_names(&rule.metric_names)).to_string(),
                "ThresholdJson": rule.threshold_json,
                "CreatedByWorkerRunId": requested_by,
            }),
        )?;
        ids.push(rule_id);
    }
    Ok(ids)
}

fn activate_scoring_rules(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    episode_id: &str,
    rules: &[ScoringRulePlan],
    metric_ids: &MetricIds,
    requested_by: &str,
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for rule in rules {
        let rule_id = create_entity(ctx, base_url, headers, "ScoringRules")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "ScoringRules",
            &rule_id,
            "ActivateScoringRule",
            json!({
                "EpisodeId": episode_id,
                "RuleStatement": rule.statement,
                "MetricIdsJson": json!(metric_ids.ids_for_names(&rule.metric_names)).to_string(),
                "Weight": rule.weight,
                "CreatedByWorkerRunId": requested_by,
            }),
        )?;
        ids.push(rule_id);
    }
    Ok(ids)
}

fn activate_frozen_simulated_user_plan(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    episode_id: &str,
    contract: &EpisodeStartContract,
) -> Result<String, String> {
    let plan_id = create_entity(ctx, base_url, headers, "SimulatedUserPlans")?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "SimulatedUserPlans",
        &plan_id,
        "ActivateSimulatedUserPlan",
        json!({
            "EpisodeId": episode_id,
            "UsersPerVariant": contract.simulated_user_plan.users_per_variant,
            "RunsPerPersona": contract.simulated_user_plan.runs_per_persona,
            "PersonasJson": contract.simulated_user_plan.personas_json,
            "GoalsJson": contract.simulated_user_plan.goals_json,
            "CreatedBy": contract.requested_by,
            "HumanDecisionSummary": contract.human_notes,
        }),
    )?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "SimulatedUserPlans",
        &plan_id,
        "FreezeSimulatedUserPlan",
        json!({
            "FrozenBy": contract.started_by,
            "Reason": "Frozen before Episode.Start so the simulated-user census cannot drift mid-generation.",
        }),
    )?;
    Ok(plan_id)
}

fn activate_frozen_selection_protocol(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    episode_id: &str,
    contract: &EpisodeStartContract,
    metric_ids: &MetricIds,
    elimination_rule_ids: &[String],
    scoring_rule_ids: &[String],
) -> Result<String, String> {
    let protocol_id = create_entity(ctx, base_url, headers, "SelectionProtocols")?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "SelectionProtocols",
        &protocol_id,
        "ActivateSelectionProtocol",
        json!({
            "EpisodeId": episode_id,
            "SelectionStatement": contract.selection_statement,
            "MetricIdsJson": json!(metric_ids.ids()).to_string(),
            "EliminationRuleIdsJson": json!(elimination_rule_ids).to_string(),
            "ScoringRuleIdsJson": json!(scoring_rule_ids).to_string(),
            "EvaluatorRef": contract.evaluator_ref,
            "DecisionPolicy": contract.decision_policy,
            "CreatedByWorkerRunId": contract.requested_by,
        }),
    )?;
    post_directed_action(
        ctx,
        base_url,
        headers,
        "SelectionProtocols",
        &protocol_id,
        "FreezeSelectionProtocol",
        json!({
            "FrozenBy": contract.started_by,
            "Reason": "Frozen before Episode.Start so variants cannot move the goalposts.",
        }),
    )?;
    Ok(protocol_id)
}

fn activate_stages(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    episode_id: &str,
    stages: &[StagePlan],
) -> Result<Vec<String>, String> {
    let mut ids = Vec::new();
    for (index, stage) in stages.iter().enumerate() {
        let stage_id = create_entity(ctx, base_url, headers, "EvaluationStages")?;
        post_directed_action(
            ctx,
            base_url,
            headers,
            "EvaluationStages",
            &stage_id,
            "ActivateEvaluationStage",
            json!({
                "EpisodeId": episode_id,
                "StageName": stage.name,
                "StageKind": stage.kind,
                "SequenceIndex": index + 1,
                "RequiredEvidenceJson": json!(stage.required_evidence).to_string(),
                "ExecutorKind": stage.executor,
            }),
        )?;
        ids.push(stage_id);
    }
    Ok(ids)
}
