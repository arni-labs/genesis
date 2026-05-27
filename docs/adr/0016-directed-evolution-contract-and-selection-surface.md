# ADR-0016: Directed Evolution Contract And Selection Surface

## Status

Proposed.

## Context

Mission Control can show live directions, episodes, variants, promotion, and
genealogy. The next gap is the contract that starts a human-gated growth
episode and the evaluation vocabulary that decides which variants die or win.
Those are not decorative UI details: they tell the human what the brain and the
system agreed to pursue, which lane can move automatically, which metrics are
being measured, which hard rules eliminate a variant, and which scoring rules
rank surviving variants.

The Directed Evolution app now owns episode materialization through
`EpisodeStartRequest` and its WASM trigger. Mission Control must read that live
entity instead of implying that the UI or a worker moved the state machine
imperatively.

## Decision

Mission Control will surface these live Directed Evolution collections:

- `EpisodeStartRequests` for the negotiated start contract and materialization
  handoff.
- `MetricDefinitions` for the metrics the evaluator and selector use.
- `EliminationRules` for hard variant death criteria.
- `ScoringRules` for weighted ranking criteria.
- `AutonomyPolicies` for the visible repair/growth automation lanes.

The UI remains mostly observational. It may pause, resume, stop, pin
constraints, compare variants, inspect variant death reasons, and dismiss a
direction, but it does not let the human override a winner or revise fitness
from the dashboard. Human/brain negotiation still happens outside the Genesis
UI.

Promotion hot-load state is derived from a real runtime ref as well as the
legacy `Materialized` boolean, because live promotion rows can prove install by
recording `RuntimeRef` even when the boolean field is absent.

## Consequences

- The human can see the app-owned start request that moved the state machine.
- Metrics, elimination rules, and scoring weights are visible beside the
  generation and variant matrix.
- The UI can explain why a variant died or won without synthetic fixtures.
- Hot-load success is not misreported as pending when the live row has a
  runtime ref.

## Verification

- Svelte type checks pass.
- Playwright e2e covers episode start request, metrics, hard elimination rules,
  scoring rules, autonomy policy lanes, and hot-loaded runtime evidence.
- Browser verification confirms the live proof tenant renders without
  horizontal overflow.
