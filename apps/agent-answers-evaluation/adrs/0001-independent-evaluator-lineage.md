# ADR 0001: Independent Evaluator Lineage

## Status

Accepted.

## Decision

Evaluation is published as a separate native Genesis bundle. Campaigns freeze
an evaluator ref before candidate execution. The brain may author a subsequent
version, including native WASM validators when appropriate, only for a future
generation whose selection design separately records that change.

## Consequences

The system can evolve evaluation practice without silently moving the target
during selection. Evolution Studio renders evaluator and organism refs as
distinct lineages.
