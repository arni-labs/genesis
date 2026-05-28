# ADR-0017: Directed Evolution Gap Closure

## Status

Accepted.

## Context

The first Directed Evolution proof produced real Temper entities, Codex worker
runs, variants, trials, stage results, promotion, and lineage. Review exposed
four architectural gaps:

- Temper-native app bundles were split across Temper, TemperPaw, and Genesis
  instead of treating Genesis as the canonical app source.
- The Mission Control UI compressed directions, episodes, variants, evidence,
  and lineage into one broad dashboard, which made provenance hard to follow.
- The UI showed metrics, scores, Datadog links, and elimination reasons without
  making clear whether they were brain-judged, agent-observed, WASM-computed,
  runtime-measured, Datadog-measured, or state-verified.
- Genesis catalog and Directed Evolution views did not expose real app/spec
  diffs for variants, promotions, or organism lineage.

## Decision

Genesis is the canonical source for Temper-native app bundles used by Directed
Evolution. GitHub mirrors and platform repositories may keep fixtures,
bootstrap copies, worker code, or install references, but the authoritative
organism and evaluator app refs are Genesis refs.

Mission Control is split into three primary product views:

- Directions: all suggested, active, completed, dismissed, auto-started, and
  human-gated directions.
- Direction Detail: one direction's episodes, generations, variants, trials,
  evaluations, eliminations, evidence, selection, promotion, and diffs.
- Organism Genealogy: organism versions over time, including parent-to-child
  diffs, responsible direction/episode, current parent, and failed branches
  where useful.

The UI must render evidence as first-class summarized records. Raw Datadog URLs
are secondary links; the primary surface must show query, time window, result
count, interpretation, and whether zero results meant success or failure.

The UI must render actual diffs wherever a human needs to understand change:
variant inspect, variant compare, promotion result, app catalog details, and
organism genealogy.

## Consequences

Directed Evolution pages should read mostly from Temper/Genesis entities and
avoid hidden UI-local workflow. UI actions remain operational and
low-ambiguity; human-brain negotiation remains in chat.

Existing live proof data is legacy. New UI code may show it, but the gap
closure acceptance proof uses a fresh episode with the new model.
