# Requests for Comment

temper-git uses RFCs for design proposals ahead of implementation.

- Write an RFC when a non-trivial new feature, protocol, or entity is
  about to be built.
- RFC is the HOW; paired ADR (if needed) is the WHY.
- Format: `NNNN-short-title.md`, sequential numbering.

## Open for review

- [0001-architecture.md](0001-architecture.md) — v1 architecture: entity
  model, WASM integrations, kernel primitives (HttpEndpoint + streaming
  WASM I/O), auth flow, phase plan.
- [0002-push-and-clone.md](0002-push-and-clone.md) — remaining slices
  for full `git push` + `git clone` against populated repositories.
- [0003-genesis-app-registry.md](0003-genesis-app-registry.md) — app
  registry layered on the git substrate: content-addressed apps,
  lineage, closures, commons mode, Genesis UI.
- [0004-github-workflow-layer.md](0004-github-workflow-layer.md) —
  pull requests, reviews, branches, server-side merge, REST v3, push
  auth and force-push classification.
- [0005-directed-evolution.md](0005-directed-evolution.md) — evolving
  running apps as organisms: signals → directions → episodes →
  variants → evaluation → selection → promotion → lineage. Relocated
  from nerdsane/temper#280.

## Accepted

(none)

## Rejected

(none)
