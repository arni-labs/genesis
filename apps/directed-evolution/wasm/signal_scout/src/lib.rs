#![allow(dead_code)]

include!("../../common.rs");

/// Upper bound on recent-signal fingerprints handed to the observer
/// so its discoveries dedup against what is already recorded.
const KNOWN_FINGERPRINT_LIMIT: usize = 50;

temper_side_effect_module! {
    fn run(ctx: Context) -> Result<Value> {
        if ctx.trigger_action != "StartScout" && ctx.trigger_action != "SweepScoutSignals" {
            return Err(format!(
                "signal_scout: unsupported trigger action {}",
                ctx.trigger_action
            ));
        }

        let scout_id = entity_id(&ctx);
        let fields = fields(&ctx);
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let organism_id = field_str(&fields, &["OrganismId"]);
        if organism_id.trim().is_empty() {
            return Err(format!("signal scout {scout_id} has no OrganismId"));
        }

        let organism = get_entity(&ctx, &base_url, &headers, "Organisms", &organism_id)?;
        let organism_fields = state_fields(&organism);
        let runtime_base_url = field_str(&organism_fields, &["RuntimeBaseUrl"]);
        let runtime_tenant = field_str(&organism_fields, &["RuntimeTenantId"]);
        let datadog_service = field_str(&organism_fields, &["DatadogService"]);
        if runtime_base_url.trim().is_empty() || runtime_tenant.trim().is_empty() {
            return Err(format!(
                "organism {organism_id} has no configured runtime target; dispatch Organism.ConfigureOrganismRuntime before scouting"
            ));
        }

        let known_fingerprints =
            recent_signal_fingerprints(&ctx, &base_url, &headers, &organism_id)?;

        let work_item_id = create_entity(&ctx, &base_url, &headers, "WorkItems")?;
        let prompt = scout_observer_prompt(
            &scout_id,
            &organism_id,
            &runtime_base_url,
            &runtime_tenant,
            &datadog_service,
            &known_fingerprints,
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
                "TargetEntityType": "Organism",
                "TargetEntityId": organism_id,
                "PromptRef": format!("literal:{prompt}"),
                "ContextRef": format!("signal-scout:{scout_id}"),
                "OutputSchemaRef": "directed-evolution.observer.v1",
                "RequiredCapabilities": "local_codex,datadog_query",
                "Lane": "observer",
                // One in-flight sweep per scout: a slow sweep must not pile up.
                "ExclusiveKey": format!("signal-scout:{scout_id}"),
                "CorrelationJson": json!({
                    "signal_scout_id": scout_id,
                    "organism_id": organism_id,
                    "source": "signal-scout",
                    "runtime_base_url": runtime_base_url,
                    "runtime_tenant": runtime_tenant,
                    "runtime_auth_env_vars": field_str(&organism_fields, &["RuntimeAuthEnvVarsJson"]),
                    "datadog_service": datadog_service,
                    "role": "observer",
                }).to_string(),
            }),
        )?;

        Ok(json!({
            "signal_scout_id": scout_id,
            "observer_work_item_id": work_item_id,
            "known_fingerprints": known_fingerprints.len(),
        }))
    }
}

fn recent_signal_fingerprints(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    organism_id: &str,
) -> Result<Vec<String>, String> {
    let filter = format!(
        "OrganismId eq {}",
        odata_string_literal(organism_id)
    );
    let url = format!(
        "{base_url}/tdata/Signals?$filter={}&$orderby=Id desc&$top={KNOWN_FINGERPRINT_LIMIT}",
        urlencode(&filter)
    );
    let resp = ctx
        .http_call("GET", &url, headers, "")
        .map_err(|e| format!("fetch recent signals: {e}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("fetch recent signals returned {}", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("recent signals json: {e}"))?;
    let rows = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(rows
        .iter()
        .map(|row| field_str(&state_fields(row), &["Fingerprint"]))
        .filter(|fp| !fp.trim().is_empty())
        .collect())
}

fn scout_observer_prompt(
    scout_id: &str,
    organism_id: &str,
    runtime_base_url: &str,
    runtime_tenant: &str,
    datadog_service: &str,
    known_fingerprints: &[String],
) -> String {
    format!(
        "Run a scheduled Directed Evolution signal-discovery sweep.\n\
SignalScoutId: {scout_id}\n\
OrganismId: {organism_id}\n\
RuntimeBaseUrl: {runtime_base_url}\n\
RuntimeTenant: {runtime_tenant}\n\
DatadogService: {datadog_service}\n\
KnownSignalFingerprints: {}\n\n\
Discover what is actually happening to this organism's live runtime: query Datadog \
(service above) for errors, latency shifts, failed transitions, and unmet-intent \
trajectory entries in the recent window, and probe the runtime OData surface for \
state inconsistencies. For each genuine finding, include it in your observer output \
both as a direction candidate (per the observer contract) and in a top-level \
`signals` array where each entry has: source (datadog|runtime-probe), signal_kind, \
summary, fingerprint, and evidence. A fingerprint is a short stable slug of the \
finding class (e.g. 'datadog:error:answer-submit-500'), NOT unique per occurrence — \
repeated occurrences of the same problem share one fingerprint. Skip findings whose \
fingerprint is already in KnownSignalFingerprints. Record at most 3 signals per \
sweep; an empty sweep with no findings is a good outcome — return an empty signals \
array rather than inventing problems.",
        json!(known_fingerprints),
    )
}
