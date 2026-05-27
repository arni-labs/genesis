# ADR-0015: Directed Evolution Organism Genealogy

## Status

Proposed.

## Context

Mission Control now shows the inside of an episode: generation rounds, variant
deaths, follow-up generation pressure, selection, and promotion. The human also
asked for the organism-level view: the Agent Answers specimen should be
inspectable across episodes and campaigns, so it is clear how the app grows and
which promoted child became the new parent for future evolution.

A simple list of versions or lineage edges is not enough. The UI needs to show
the causal chain: parent version, edge, episode, direction, winning variant,
promotion, and hot-load result.

## Decision

Mission Control will render an organism genealogy from live Directed Evolution
entities:

- `OrganismVersions` define specimen versions.
- `LineageEdges` connect parent versions to child versions.
- `Episodes` and `Directions` explain why a child version exists.
- `Variants` identify the winning mutation when recorded.
- `Promotions` show whether the child was published and hot-loaded.

The genealogy remains observational. It does not let the human choose winners or
edit lineage. It highlights the current parent, promoted children, seed
versions, failed or pending promotion edges, and the direction that caused each
growth step.

If the live API does not provide lineage edges, Mission Control must say so
instead of synthesizing a fake tree from app refs.

## Consequences

- The human can inspect the organism as a growing specimen, not only as a set of
  isolated episode records.
- Promotion and hot-load evidence becomes visible at the same level as
  parent-child ancestry.
- Future campaign-level views can layer on top of the same entity relationships
  without changing the data contract.

## Verification

- Svelte type checks pass.
- Directed Evolution e2e coverage asserts the organism history view, current
  parent marker, direction basis, winner variant, and hot-loaded promotion
  state.
- Browser verification confirms the genealogy renders from a live proof tenant
  without horizontal page overflow.
