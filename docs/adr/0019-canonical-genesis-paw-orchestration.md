# 0019 Canonical Genesis And Paw Orchestration

## Status

Accepted

## Context

Directed Evolution review found that Temper-native app source was spread across Genesis, Temper, and TemperPaw. That makes it hard to answer a simple question: which app version is the organism, which version was promoted, and where should a reviewer inspect the actual files?

The same review also found execution language drift. Mission Control showed "Brain Run" even though execution is worker/provider/run infrastructure shared by Codex workers, future TemperPaw-native agents, Paw Patrol, and Directed Evolution.

## Decision

- Genesis is the canonical home for Temper-native app bundles and version history.
- `directed-evolution` and `temperpaw/paw-orchestration` are authored under
  Genesis `apps/`, specifically `apps/directed-evolution` and
  `apps/temperpaw/paw-orchestration`, not under Temper `os-apps/`.
- GitHub/platform repositories must not be treated as app-source mirrors. They may keep platform code, install tooling, tests, fixtures, docs, pinned Genesis refs, and one tiny immutable first-boot seed for Genesis itself.
- `temperpaw/paw-orchestration` is the canonical shared execution app ref. Its
  bundle-local app name remains `paw-orchestration`. It owns `WorkerProvider`,
  `WorkerAgent`, `WorkItem`, and `WorkerRun`.
- Directed Evolution UI renders execution as worker/provider/run provenance, not brain-run provenance.
- Genesis app details and Mission Control must show actual file/code diffs for app versions, variant comparisons, promotions, and organism genealogy.
- Datadog evidence summaries are first-class review data. Links alone are not sufficient.

## Consequences

- Reviewers inspect app history in Genesis and do not have to guess which repository copy is real.
- Mission Control can explain concurrent worker activity and selector/promoter serialization using the same vocabulary as the platform.
- Legacy tenants may still expose old entity names, so the UI can tolerate old fields while displaying the canonical labels.
