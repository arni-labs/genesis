# ADR 0001: Independent Evaluator Lineage

## Status

Accepted.

## Decision

Evaluation is published as a separate native Genesis bundle. Campaigns freeze
an evaluator ref before candidate execution. The brain may author a subsequent
version, including native WASM validators when appropriate, only for a future
generation whose selection design separately records that change.

Evaluator-owned trial metric definitions use the `TrialMetricDefinition` entity
name so they do not collide with Directed Evolution's global `MetricDefinition`
entity in a shared tenant.

## Consequences

The system can evolve evaluation practice without silently moving the target
during selection. Evolution Studio renders evaluator and organism refs as
distinct lineages.
