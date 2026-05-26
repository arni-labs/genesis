#!/usr/bin/env bash
# Phase-1 directed-evolution e2e: a `Downvote` that doesn't exist becomes a
# live feature, generated + verified + merged + hot-deployed by the loop.
# See docs/directed-evolution/e2e-runbook.md. Run from the genesis worktree.
#
# Prereqs (produced by teammates): genesis/wasm/{gen_variant,run_stage_caller,
# select_winner,merge_variant}; the sim-ui simulator. FINALIZE the marked
# (#FIXME) bits against their actual outputs during the live run.
set -euo pipefail

PORT="${PORT:-3000}"
BASE="http://127.0.0.1:${PORT}"
GENESIS="${GENESIS:-/Users/sesh.nalla/Development/genesis-claude-directed-evolution}"
TEMPER="${TEMPER:-/Users/sesh.nalla/Development/temper-claude-directed-evolution}"
SO="${SO:-/Users/sesh.nalla/Development/temperpaw-claude-directed-evolution/os-apps/stackoverflow-agents}"
MODULES=(gen_variant run_stage_caller select_winner merge_variant)

echo "==> 1. Build evolver WASM modules (wasm32-wasip1)"
for m in "${MODULES[@]}"; do
  ( cd "$GENESIS/wasm/$m" && cargo build --target wasm32-wasip1 --release )
done

echo "==> 2. Start temper-platform (one process: /tdata + /verify/stage + /deploy/tenant), Turso storage"
( cd "$TEMPER" && cargo run -q -p temper-cli -- serve \
    --app temper-git="$GENESIS/specs" \
    --app stackoverflow-agents="$SO/specs" \
    --storage turso --port "$PORT" ) &
SERVER_PID=$!
trap 'kill $SERVER_PID 2>/dev/null || true' EXIT
# wait for readiness
until curl -fsS "$BASE/tdata" >/dev/null 2>&1; do sleep 0.5; done
echo "    server up on $BASE (pid $SERVER_PID)"

echo "==> 3. Upload evolver WASM modules (per genesis tenant 'temper-git')"
for m in "${MODULES[@]}"; do
  WASM=$(ls "$GENESIS/wasm/$m"/target/wasm32-wasip1/release/*.wasm 2>/dev/null | head -1) # #FIXME path
  curl -fsS -X POST "$BASE/api/wasm/modules/$m" -H "X-Tenant-Id: temper-git" \
       --data-binary @"$WASM" >/dev/null && echo "    uploaded $m"
done

echo "==> 4. Seed the v1 FitnessSpec"
FS=$(cat "$GENESIS/docs/directed-evolution/v1-fitness-spec.json")
curl -fsS -X POST "$BASE/tdata/FitnessSpecs" -H "X-Tenant-Id: temper-git" \
     -H 'content-type: application/json' \
     -d "{\"Stages\": $(jq -c .stages <<<"$FS" | jq -Rs .), \"SelectionPolicy\": \"lexicographic\", \"Status\": \"Active\"}" # #FIXME exact create shape

echo "==> 5. Run the simulator (emits the downvote unmet intent → creates an Evolution)"
# #FIXME invoke sim-ui's simulator entrypoint (it should create the Evolution).
echo "    (run the sim-ui simulator here; it posts the unmet intent + the Evolution)"

echo "==> 6. Drive the human-gated steps (Claude plays the human) — see runbook."
echo "    Frame -> (auto) StartGenerating/eval/selection -> Select -> Approve -> GoLive"

echo "==> 7. Acceptance: Downvote is now live on Answer"
curl -fsS "$BASE/tdata/\$metadata" -H "X-Tenant-Id: stackoverflow-agents" | grep -q "Downvote" \
  && echo "    PASS: Downvote action present on Answer" \
  || { echo "    FAIL: Downvote not found"; exit 1; }
echo "==> e2e scaffold complete (finalize #FIXME bits against teammate outputs during the live run)"
