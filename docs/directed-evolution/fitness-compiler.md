# The Fitness Compiler — natural language → FitnessSpec

In directed evolution the human **directs** by setting selection pressure in natural language; **Claude Code (the operator) compiles that into a formal `FitnessSpec`** — the ordered, multi-stage / multi-objective fitness the engine runs (`run_stage_caller` walks `FitnessSpec.Stages`). The human never hand-edits JSON: they speak intent, Claude shows the compiled spec, the human approves it. The Studio UI is observe + yes/no only — fitness is set here, in conversation.

## The catalog (what can be measured)
A stage references exactly one **evaluator** by id. A metric exists only if a trusted evaluator computes it — the brain never scores; it composes from this fixed set:

| evaluator | what it gates/scores | backing |
|---|---|---|
| `parse` | the spec parses | temper-spec |
| `l0` | guards satisfiable, invariants inductive | SMT / Z3 |
| `l1` | no reachable safety violation; `state_space` score | Stateright |
| `l2` | idempotent / crash-safe under faults | sim + fault injection (Phase 2) |
| `l3` | survives random sequences | proptest (Phase 2) |
| `budget` | within WASM fuel/memory | temper-wasm (real in Phase 2; pass-through v1) |
| `cedar` | authorized / no privacy leak | temper-authz (Phase 2) |
| `field` | intent actually met in production | temper-observe telemetry (Phase 2) |

## Stage shape (one entry of `FitnessSpec.Stages`)
```
{ "stage_id", "evaluator", "kind": "gate|score|gate+score",
  "gate_predicates": [...], "score_objectives": [{"name","direction":"min|max","weight"}],
  "on_fail": "kill|continue|warn" }
```
Top-level: `selection_policy` (`lexicographic|weighted|pareto`) + `tie_break`.

## Example compilation (v1 downvote — see v1-fitness-spec.json)
**Human:** *"Add downvote. Correctness first, keep it cheap, never break the existing invariants. Ask me before merging."*

Claude compiles to:
- *correctness first* → `parse`, `l0`, `l1` as hard gates (`on_fail: kill`).
- *never break invariants* → `l1` (invariants are model-checked there).
- *keep it cheap* → `budget` gate + minimize the `l1` `state_space` objective.
- *ask before merging* → `Evolution.autonomy = 0` (human approves `Approve`/Merge) — set on the **Evolution**, not the FitnessSpec.
- `selection_policy = lexicographic`, `tie_break = state_space` (simpler variant wins ties).

The human sees this and ✓s it; then the loop runs under that pressure.
