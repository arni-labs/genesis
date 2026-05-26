# ADR 0001: Native Directed Evolution Protocol

## Status

Accepted.

## Decision

Directed evolution is delivered as a Genesis-published Temper-native app, not
as kernel behavior. Genesis owns immutable application and evaluator bundle
refs and lineage; this app owns mutable campaign execution and release-control
entities. Datadog evidence is attached to measurements but does not replace
canonical Temper state.

Selection is not a fixed global score vector. Each campaign uses a typed,
brain-proposed, human-approved `SelectionDesign`, frozen before a generation
runs. Its evaluator bundle ref is separate from the candidate app ref to make
changes to the organism and changes to the judge independently reviewable.
Candidate generation is outside the protocol: V1 asks Codex through TemperPaw
to alter only the subject's Temper-native bundle, then Genesis publishes the
verified candidate as an immutable ref. This keeps the protocol reusable and
prevents a demo-specific mutation script from defining evolution.

The evaluator app owns frozen `TrialSuite` and `MetricDefinition` records plus
per-candidate `ValidatorRun` results. The protocol's `Measurement` rows point
to that evidence, simulated usage or Datadog telemetry instead of inventing a
single global fitness number.

## Consequences

Codex or a future TemperPaw-native brain can drive the same entity actions.
Approved campaigns may automatically release a survivor, so `Campaign.Pause`
and `Campaign.Rollback` are required protocol actions and are surfaced in the
Studio UI.
