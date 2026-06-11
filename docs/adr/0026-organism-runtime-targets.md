# ADR-0026: Organism-Owned Runtime Targets and Evaluator Refs

## Status

Accepted

## Context

ADR-0023 decided that Directed Evolution exercises apps through an explicit
external runtime target — base URL, tenant, Datadog service, auth secret
names. The implementation stopped short: the target lived as a hardcoded
TypeScript block for exactly one app (Agent Answers) in the Mission Control
client, the episode loop's trial prompts used a global public API URL, and
the evaluator app ref that ADR-0018 says every episode freezes was never
populated by the auto-start path. Directed Evolution could not evolve a
second organism against an external runtime.

## Decision

The runtime target and evaluator ref are entity data on the `Organism`:

```
Organism.ConfigureOrganismRuntime(RuntimeBaseUrl, RuntimeTenantId,
    DatadogService, RuntimeAuthEnvVarsJson, EvaluatorRef, ConfiguredBy)
```

- `RuntimeAuthEnvVarsJson` carries environment-variable **names** only;
  secret values never enter Genesis entities (ADR-0023 unchanged).
- Trial queueing reads the organism's target and refuses to queue when no
  target is configured — a misconfigured organism fails loudly instead of
  exercising the wrong runtime.
- The runtime context (base URL, tenant, auth env var names, Datadog
  service) flows into each trial WorkItem's `CorrelationJson` and the
  simulated-user prompt, so the worker resolves credentials and the
  evaluator queries the right Datadog service without any client-side
  configuration.
- The auto-start episode contract resolves `EvaluatorRef` from the request,
  the direction, or the organism row, and fails the start request when none
  is present (ADR-0018 fail-closed).
- Mission Control reads targets from organism rows; the hardcoded
  per-app client block is removed.

## Consequences

- Any registered organism becomes evolvable by dispatching one governed
  configuration action; adding a second organism requires no code change.
- Existing organisms must be configured once before their next episode
  (the Agent Answers organism is configured as part of this effort's
  deployment); until then trial queueing fails with an instructive error.
- Observability joins gain `de.tenant` and `de.role` keys, completing the
  ADR-0018 join-field list.
