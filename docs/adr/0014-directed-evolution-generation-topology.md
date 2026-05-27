# ADR-0014: Directed Evolution Generation Topology

## Status

Proposed.

## Context

The live Directed Evolution repair proof showed an important product shape:
generation 1 can fail honestly, the result router can create a follow-up
generation from prior elimination evidence, and later generations can produce
multiple live-working survivors before selection. Mission Control currently
renders variants and stage results, but mostly flattens them inside the
selected episode. That hides the evolutionary structure the human explicitly
wants to see: rounds, follow-up causality, death pressure, survivors, and the
winner's path.

## Decision

Mission Control will render a live generation topology for each selected
episode. The topology groups variants by `Generation`, shows generation status,
target count, survivor count, winner marker, and failure/follow-up reason, and
keeps inspect/compare actions on each variant. It must be derived from live
`Generations`, `Variants`, and `StageResults`; it must not synthesize rounds
from local fixtures or assume a single-generation episode.

The first implementation is a compact timeline/bracket inside the episode
panel. It remains observational and uses the existing inspect/compare controls
instead of adding new human-brain negotiation points.

## Consequences

- A human can see when the organism learned from failed variants rather than
  merely seeing a flat list of deaths.
- Follow-up generation behavior is visible in the UI, making the evidence-fed
  generation router auditable.
- Selector outcomes are clearer because the winning variant appears in its
  generation context.
- Future Mission Control work can build a richer organism-scale phylogeny on
  the same live entity relationships.

## Verification

- Svelte type checks pass.
- Existing Mission Control e2e coverage asserts generation topology text from
  live OData fixtures, including a failed first generation and a follow-up
  generation.
- Browser verification confirms the generation topology is visible without
  horizontal page overflow on desktop and mobile.
