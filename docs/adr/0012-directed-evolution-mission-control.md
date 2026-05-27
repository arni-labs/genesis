# ADR-0012: Directed Evolution Mission Control

## Status

Proposed.

## Context

Directed Evolution needs a human-facing surface that is beautiful, legible, and
truthful. The previous Claude UI direction was closer to the target: a
mission-control/game-dashboard style view with progress, brackets, eliminations,
and lineage. The previous Codex UI had stronger proof discipline. The target
Genesis experience needs both.

The UI must not become a fixture-backed demo. It should only show an evolution
as working when the underlying Temper/Genesis entities, worker outputs,
evaluation results, and evidence exist.

## Decision

Genesis will host Mission Control for Directed Evolution. Mission Control reads
live Directed Evolution control-plane entities and renders:

- direction queue and direction provenance
- active autonomy lanes
- episode progress
- generation and variant brackets
- evaluation stages and stage results
- death reports explaining eliminations
- variant comparison
- AI simulated user trials
- selector explanation
- promotion result
- organism lineage and specimen history

The UI is primarily observational. It may dispatch low-ambiguity operational
actions such as pause, resume, stop, dismiss direction, pin viability
constraint, inspect death report, and compare variants. It does not host the
human-brain negotiation flow, does not provide fitness/evaluation approval
forms, and does not let the human manually override the winner.

### Live Data Contract

Mission Control must render from entity state, not from local fixtures, for the
production path. Development fixtures may exist for component work, but the app
must visibly distinguish fixture/dev mode from live mode and the end-to-end
acceptance path must use live data.

Mission Control reads Directed Evolution from a dedicated control tenant, not
necessarily the Genesis registry tenant. The deployed build pins the current
live control tenant and the route also accepts a `tenant` query parameter so a
different control plane can be inspected without rebuilding the app.

Operational actions sent from Mission Control identify the caller as a human
agent (`x-temper-principal-kind: agent`, `x-temper-agent-type: human`) so Cedar
evaluates the same action matrix used by chat-mediated human direction.

### Agent Answers Organism

Genesis will expose Agent Answers as the first organism for the v1 proof. The
organism must have a parent version, runnable app refs or variant refs,
evaluation evidence, and lineage after promotion.

### Design Direction

The UI should feel like a polished mission-control dashboard: dense enough for
real observation, visually energetic enough to make evolution legible, and
clear about why variants survive or die. It should use progress, stages,
brackets, timelines, evidence links, and lineage visualization rather than
marketing copy or explanatory filler.

## Consequences

- Genesis becomes the primary human window into Directed Evolution.
- Temper remains the source of truth for state.
- TemperPaw remains the local execution plane for Codex brains.
- UI completion is gated by live end-to-end proof, not static screenshots.

## Verification

- Component tests cover empty, active, eliminated, selected, promoted, and
  failed episode states.
- Browser verification covers desktop and mobile Mission Control views.
- The local e2e proof starts from Agent Answers, runs an episode, and shows the
  resulting direction, variants, eliminations, selection, promotion, and
  lineage in Mission Control.
- No production-mode Mission Control view is backed only by fixture data.
