# ADR-0011: Directed-Evolution Engine Hosted in Genesis

- Status: Accepted
- Date: 2026-05-26
- Deciders: Genesis maintainers
- Related:
  - ADR-0006 (metadata-only fork via Lineage), ADR-0010 (agent app repair & version evolution)
  - temper ADR-0011 (per-stage verification API)
  - Directed-evolution plan (`~/.claude/plans/lets-plan-this-but-ancient-perlis.md`)

## Context

The directed-evolution loop grows a temper-native app by generating candidate spec variants, filtering them through temper's verification cascade as a multi-stage / multi-objective fitness function, and auto-merging + hot-deploying the winner. It needs a home for its engine state — the `Evolution` episode, the `Variant` candidates, the `FitnessSpec`, and the per-stage `StageResult` records — plus a spectator/director UI.

Genesis is the natural host: a variant **is** a git branch here, selection **is** a PR merge, and provenance **is** `Lineage` + `Closure` — all genesis-native. Co-locating the engine entities in genesis means **one OData surface + one event stream feed genesis's existing web UI with zero federation**, and the human directs by reviewing/merging variant branches in the UI genesis already has.

## Decision

### Add a directed-evolution entity family to the genesis app

Four IOA entities (`specs/{evolution,variant,fitness_spec,stage_result}.ioa.toml`), one `Evo.DE` CSDL schema merged into `specs/model.csdl.xml` (entity sets added to the existing container — a single OData service), and `policies/evolution.cedar`. All pass temper's L0–L3 cascade (verified in isolation). The engine is **organism-agnostic**: an `Evolution` targets a *target app* by id; nothing here is Stack-Overflow-specific.

Engine invariants give the engine itself teeth: `Evolution.MergeRequiresWinner` (cannot reach Merged/Live without `winner_chosen`), `Variant.KilledIsFinal` (a killed variant can never be revived → never selected), `Variant.SurvivedIsFinal`.

### Heavy work via thin WASM that calls out (temper-native rule)

Four thin WASM integrations (`wasm/{gen_variant,run_stage_caller,select_winner,merge_variant}`) orchestrate the loop and `http_call` out for heavy work — the **mutagen** (temperpaw `coding_agent_runner`, headless Claude Code) and the **verifier** (temper-platform `POST /verify/stage`); the verifier cannot run in the wasm32 sandbox (Z3/Stateright). Git ops (branch/commit/PR/merge), Lineage, and hot-deploy are genesis-local entity actions. No host-side Rust business logic.

### UI extends genesis/web

The Evolution Studio (spectator + low-bandwidth yes/no director controls) is a view in genesis's existing SvelteKit UI, fed by the same OData + event stream. Fitness is *set by talking to Claude Code* (NL → compiled FitnessSpec), not edited in the UI.

## Consequences

### Positive
- Zero federation: the UI renders live state + provenance from one data plane.
- Variants/PRs/lineage are genesis-native; directing = the PR review the UI already does.

### Negative
- Genesis's scope expands beyond SCM to host the evolution engine. Accepted: the engine is fundamentally about versioned spec lineage, which is genesis's domain.

### Risks
- Engine WASM `http_call`s to the verifier/mutagen hosts must pass the Cedar WASM gate — allow-listed during wiring; validated by the e2e.

## Non-Goals
- No change to genesis's git/SCM wire-protocol behavior or byte-exact compatibility.
- Budget/cedar/field fitness stages and the bounty/escrow showcase are Phase 2.

## Alternatives Considered
1. **Evolver as a separate app/service** — rejected: forces UI federation across two OData origins; the user explicitly wants one data plane.
2. **Host the engine in temper-platform** — rejected: provenance (branches/PRs/lineage) and the directing UI live in genesis; co-location avoids cross-service glue.
