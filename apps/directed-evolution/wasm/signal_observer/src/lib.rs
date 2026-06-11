#![allow(dead_code)]

include!("../../common.rs");

temper_side_effect_module! {
    fn run(ctx: Context) -> Result<Value> {
        if ctx.trigger_action != "RecordSignal" {
            return Err(format!(
                "signal_observer: unsupported trigger action {}",
                ctx.trigger_action
            ));
        }

        let signal_id = entity_id(&ctx);
        let fields = fields(&ctx);
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let organism_id = field_str(&fields, &["OrganismId"]);
        let source = field_str(&fields, &["Source"]);
        let signal_kind = field_str(&fields, &["SignalKind"]);
        let summary = field_str(&fields, &["Summary"]);
        let evidence_artifact_id = field_str(&fields, &["EvidenceArtifactId"]);
        let correlation_json = field_str(&fields, &["CorrelationJson"]);
        let fingerprint = field_str(&fields, &["Fingerprint"]);

        // Scout-recorded signals arrive pre-interpreted: the sweep's
        // observer run already proposed directions, so re-queueing an
        // interpretation work item would cascade observers.
        if source == "signal-scout" {
            return Ok(json!({
                "signal_id": signal_id,
                "skipped": "scout-recorded signal is already interpreted",
            }));
        }

        // Fingerprint dedup: repeated findings collapse onto the first
        // signal instead of spawning duplicate observer work.
        if !fingerprint.trim().is_empty() {
            if let Some(existing_id) = duplicate_signal_id(
                &ctx,
                &base_url,
                &headers,
                &organism_id,
                &fingerprint,
                &signal_id,
            )? {
                post_directed_action(
                    &ctx,
                    &base_url,
                    &headers,
                    "Signals",
                    &signal_id,
                    "IgnoreSignal",
                    json!({ "Reason": format!("duplicate of signal {existing_id} (fingerprint {fingerprint})") }),
                )?;
                return Ok(json!({
                    "signal_id": signal_id,
                    "skipped": format!("duplicate of {existing_id}"),
                }));
            }
        }

        let work_item_id = create_entity(&ctx, &base_url, &headers, "WorkItems")?;
        let prompt = observer_prompt(
            &signal_id,
            &organism_id,
            &source,
            &signal_kind,
            &summary,
            &evidence_artifact_id,
            &correlation_json,
        );
        post_paw_orchestration_action(
            &ctx,
            &base_url,
            &headers,
            "WorkItems",
            &work_item_id,
            "QueueWorkItem",
            json!({
                "Role": "observer",
                "TargetEntityType": "Signal",
                "TargetEntityId": signal_id,
                "PromptRef": format!("literal:{prompt}"),
                "ContextRef": format!("signal:{signal_id}"),
                "OutputSchemaRef": "directed-evolution.observer.v1",
                "RequiredCapabilities": "local_codex,datadog_query",
                "Lane": "observer",
                "ExclusiveKey": format!("observer:{signal_id}"),
                "CorrelationJson": json!({
                    "signal_id": signal_id,
                    "organism_id": organism_id,
                    "source": source,
                    "signal_kind": signal_kind,
                    "evidence_artifact_id": evidence_artifact_id,
                }).to_string(),
            }),
        )?;

        Ok(json!({
            "signal_id": signal_id,
            "observer_work_item_id": work_item_id,
        }))
    }
}

/// First other signal carrying this fingerprint for the organism, if
/// any. Archived signals do not suppress new occurrences.
fn duplicate_signal_id(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    organism_id: &str,
    fingerprint: &str,
    own_signal_id: &str,
) -> Result<Option<String>, String> {
    let filter = format!(
        "OrganismId eq {} and Fingerprint eq {}",
        odata_string_literal(organism_id),
        odata_string_literal(fingerprint)
    );
    let url = format!("{base_url}/tdata/Signals?$filter={}&$top=5", urlencode(&filter));
    let resp = ctx
        .http_call("GET", &url, headers, "")
        .map_err(|e| format!("duplicate signal lookup: {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("duplicate signal lookup returned {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("duplicate lookup json: {e}"))?;
    let rows = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for row in rows {
        let id = row
            .get("entity_id")
            .or_else(|| row.get("id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let status = entity_status(&row);
        if !id.is_empty() && id != own_signal_id && status != "Archived" {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn observer_prompt(
    signal_id: &str,
    organism_id: &str,
    source: &str,
    signal_kind: &str,
    summary: &str,
    evidence_artifact_id: &str,
    correlation_json: &str,
) -> String {
    format!(
        "Observe this Directed Evolution signal and infer whether it creates actionable pressure.\n\
SignalId: {signal_id}\n\
OrganismId: {organism_id}\n\
Source: {source}\n\
SignalKind: {signal_kind}\n\
Summary: {summary}\n\
EvidenceArtifactId: {evidence_artifact_id}\n\
CorrelationJson: {correlation_json}\n\n\
Return JSON with: actionable, directions, evidence_scope, and rationale. \
The directions field must contain at least two candidate direction objects when actionable=true. \
Each direction object must include pressure_class, pressure_summary, title, direction_summary, \
autonomy_lane, proposed_adaptation_goal, and proposed_viability_constraints. \
The top-level evidence_scope must include structured Datadog evidence with query, time_window, \
result_count, interpretation, zero_result_meaning, and datadog_url. \
If the signal is user error or noise, set actionable=false and explain why."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_prompt_names_signal_and_actionability() {
        let prompt = observer_prompt(
            "sig-1",
            "org-1",
            "datadog",
            "latency_regression",
            "p95 climbed",
            "ev-1",
            "{}",
        );

        assert!(prompt.contains("SignalId: sig-1"));
        assert!(prompt.contains("actionable=false"));
        assert!(prompt.contains("at least two candidate direction objects"));
        assert!(prompt.contains("structured Datadog evidence"));
        assert!(prompt.contains("proposed_adaptation_goal"));
    }
}
