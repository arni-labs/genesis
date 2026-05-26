# Directed Evolution — Phase 1 e2e runbook (downvote loop closes)

Goal: a `Downvote` action that does **not** exist on `Answer` becomes a live feature with **no human code edit** — generated, verified by the cascade, selected, merged, and hot-deployed by the loop. Claude plays the human/director.

Single process: temper-platform serves genesis (incl. the evolver) + `stackoverflow-agents` as tenants, plus `/verify/stage` + `/deploy/tenant`, on one port. Daemonless persistence (Turso/SQLite). The only external hop is the coding agent (optional in v1 — `gen_variant` has a deterministic downvote fallback).

## 0. Build artifacts
```
# evolver WASM modules (genesis)
cd genesis && for m in gen_variant run_stage_caller select_winner merge_variant; do
  (cd wasm/$m && cargo build --target wasm32-wasip1 --release)
done
# (SO app is pure IOA — no WASM in Phase 1)
# temper-platform server — the `temper` binary already bundles temper-server's
# `observe` feature (temper-cli dep), so /tdata + /verify/stage + /deploy/tenant
# + /api/wasm are all available with no extra feature flag.
cd temper && cargo build -p temper-cli
```
**WASM loading (CONFIRMED):** `temper serve --app tenant=dir` loads CSDL+IOA only — it does NOT scan `wasm/<name>/`. WASM modules are uploaded to the running server via `POST /api/wasm/modules/{module_name}` (body = compiled `.wasm` bytes; the `temper` binary bundles the `observe` feature by default via temper-cli, so this route is always present), which stores + registers name→hash (recovered from the store on restart). So after building each module to `wasm32-wasip1`, upload it under the genesis tenant, e.g.:
`curl -X POST :3000/api/wasm/modules/gen_variant -H 'X-Tenant-Id: temper-git' --data-binary @genesis/wasm/gen_variant/target/wasm32-wasip1/release/gen_variant.wasm`
(repeat for run_stage_caller, select_winner, merge_variant). Wire `temper_api_url` (+ optional `coding_agent_url`) into the trigger configs; in dev the WASM authz gate is permissive (else allow-list the local host).

## 1. Start the server
```
temper serve --app temper-git=genesis/specs --app stackoverflow-agents=temperpaw/os-apps/stackoverflow-agents/specs \
  --storage turso --port 3000
# /tdata, /verify/stage, /deploy/tenant all on :3000
```

## 2. Seed the v1 FitnessSpec
POST genesis/docs/directed-evolution/v1-fitness-spec.json → create a FitnessSpec (POST /tdata/FitnessSpecs then SetStages/SetPolicy, or POST with fields). Note its Id.

## 3. Run the simulator (emits the unmet intent)
Run sim-ui's simulator against :3000 — it seeds Questions/Answers, casts Upvotes, attempts a `Downvote` (fails: action absent), and emits the unmet intent → creates an `Evolution` (TargetApp=stackoverflow-agents, TargetTenant=stackoverflow-agents, FitnessSpecId=<seeded>, Intent="agents want to downvote low-quality answers", Autonomy=0).

## 4. Drive the loop (Claude plays the human)
The triggers do the work; the human-gated transitions are dispatched by the operator:
- `Evolution.Frame` (problem_statement) → StartGenerating fires `gen_variant` (writes a variant branch adding `Downvote`+`downvotes` to Answer; creates Variant(s); VariantProposed).
- Variant.StartEval fires `run_stage_caller` per stage → parse/l0/l1/budget → Survive (or Kill with counterexample).
- `Evolution.BeginSelection` fires `select_winner` → `Evolution.Select{winner}`.
- `Evolution.Approve` (the human yes/no) fires `merge_variant` → genesis PR merge + Lineage + `POST /deploy/tenant` (hot-deploy the merged Answer spec into the SO tenant) → `Evolution.GoLive`.

## 5. Verify the loop closed (acceptance test)
- `GET /tdata/$metadata` for the SO tenant now shows a `Downvote` action on `Answer`.
- `POST /tdata/Answers('<id>')/Soa.QA.Downvote` succeeds (it did NOT before).
- The Evolution is `Live`; the Variant that broke an invariant (if any seeded) is `Killed` with its counterexample.
- Studio UI shows the elimination bracket + "Downvote is LIVE".

## 6. Record proof + deliver
- Capture the run (server log + OData responses + UI screenshot) into `genesis/proofs/` (use proofs/TEMPLATE.md) and `temperpaw/.proofs/`.
- Run mandatory temper DST + code reviews; `cargo test --workspace` green.
- Commit per repo (claude/directed-evolution) and open **draft PRs**: temper→DataDog/temper, genesis→arni-labs/genesis, temperpaw→nerdsane/temperpaw. Never merge.
