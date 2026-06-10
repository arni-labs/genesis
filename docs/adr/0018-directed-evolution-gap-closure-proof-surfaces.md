# 0018 Directed Evolution Gap Closure Proof Surfaces

## Status

Accepted

## Context

Directed Evolution Mission Control needs to be reviewable without relying on chat explanations. A reviewer must see what changed, what evidence judged each variant, which parts were mechanical versus brain-judged, and why a winner was promoted. The previous implementation recorded high-level mutation summaries and evidence links, but it did not carry code hunks through the evolution entities, did not show commit diffs in Genesis app exploration, routed simulated users too close to stage pass/fail decisions, and treated Datadog evidence as informational instead of required when a stage declared Datadog-measured metrics.

## Decision

- `Mutation` records carry a `DiffPatch` alongside changed files and diff refs. Mission Control renders those hunks in variant inspection, comparison, promotion, and organism genealogy views.
- Genesis app version exploration computes per-commit file diffs from stored trees/blobs and renders GitHub-like changed files and hunks for every selected commit.
- Simulated-user work items target `Trial` entities. They can only record journeys, observations, friction, and blockers. Stage pass/fail remains the responsibility of evaluator roles targeting `StageResult`.
- Simulated-user stages queue all trials from the frozen `SimulatedUserPlan`; once those trials finish, a `viability_evaluator` work item evaluates the recorded observations.
- Telemetry stages use the `telemetry_evaluator` role. If a stage declares Datadog-measured evidence, the router fails the stage unless the evaluator output includes structured Datadog evidence with a query, time window, result count, interpretation, and explicit zero-result meaning.
- Metric threshold elimination rules are enforced by the router after evaluator output is parsed. Codex may explain the judgment, but declared hard thresholds are mechanical.
- Directed Evolution runtime and worker prompts carry join fields for Datadog correlation: tenant, episode, direction, generation, variant, stage, trial, persona/run, work item, app ref, runtime ref, and role.

## Consequences

- The proof cycle can be audited from Temper entities and the UI: code diff, simulated-user journeys, Datadog query summaries, threshold rule application, selector rationale, and promotion are all visible.
- Datadog-required stages fail closed when telemetry is missing or only linked opaquely.
- Variants cannot pass because a simulated-user Codex run returned `passed`; simulated users produce observations only.
- The UI and app catalog must handle compact patch rendering and remain responsive on mobile.
