#!/usr/bin/env bash
set -euo pipefail

# Genesis-vs-GitHub performance benchmark (docs/PERFORMANCE.md).
#
# Measures the same operations against the same repository hosted on
# both remotes and reports per-operation medians. The bar (plan §C4):
# Genesis within ~1x of GitHub per operation class.
#
#   GENESIS_REMOTE=<url of repo on genesis> \
#   GITHUB_REMOTE=<url of same repo on github> \
#   RUNS=5 scripts/bench-genesis-vs-github.sh
#
# Optional: GENESIS_TOKEN / GITHUB_TOKEN for authenticated push probes
# (push measurement is skipped for a remote without its token).
# Read-only against both remotes except the push probe, which pushes a
# throwaway ref (refs/heads/bench-probe-<pid>) and deletes it.

RUNS="${RUNS:-5}"
GENESIS_REMOTE="${GENESIS_REMOTE:?set GENESIS_REMOTE}"
GITHUB_REMOTE="${GITHUB_REMOTE:?set GITHUB_REMOTE}"
WORKDIR="$(mktemp -d /tmp/genesis-bench.XXXXXX)"
trap 'rm -rf "$WORKDIR"' EXIT

now_ms() { python3 -c 'import time; print(int(time.time()*1000))'; }

median() {
  python3 - "$@" <<'PY'
import sys
vals = sorted(int(v) for v in sys.argv[1:])
n = len(vals)
print(vals[n//2] if n % 2 else (vals[n//2-1]+vals[n//2])//2)
PY
}

measure_runs() { # name cmd...
  local name="$1"; shift
  local times=()
  for _ in $(seq 1 "$RUNS"); do
    local start end
    start="$(now_ms)"
    "$@" >/dev/null 2>&1 || { echo "$name FAILED"; return 1; }
    end="$(now_ms)"
    times+=($((end - start)))
  done
  echo "$name p50_ms=$(median "${times[@]}") runs=${RUNS} samples=[${times[*]}]"
}

bench_remote() { # label url token_env
  local label="$1" url="$2" token="${3:-}"
  echo "=== $label ($url) ==="

  measure_runs "$label ls-remote" git ls-remote "$url" HEAD

  # Cold clone: fresh directory every run.
  local times=() i
  for i in $(seq 1 "$RUNS"); do
    local dest="$WORKDIR/$label-cold-$i" start end
    start="$(now_ms)"
    git clone --quiet "$url" "$dest"
    end="$(now_ms)"
    times+=($((end - start)))
    rm -rf "$dest"
  done
  echo "$label cold-clone p50_ms=$(median "${times[@]}") runs=${RUNS} samples=[${times[*]}]"

  # Warm fetch: clone once, then fetch repeatedly.
  local warm="$WORKDIR/$label-warm"
  git clone --quiet "$url" "$warm"
  measure_runs "$label warm-fetch" git -C "$warm" fetch --quiet

  # Push probe: tiny commit on a throwaway ref, then delete it.
  if [ -n "$token" ]; then
    local probe_ref="refs/heads/bench-probe-$$"
    times=()
    for i in $(seq 1 "$RUNS"); do
      git -C "$warm" commit --quiet --allow-empty -m "bench probe $i"
      local start end
      start="$(now_ms)"
      git -C "$warm" push --quiet "$url" "HEAD:$probe_ref"
      end="$(now_ms)"
      times+=($((end - start)))
    done
    git -C "$warm" push --quiet "$url" ":$probe_ref" || true
    echo "$label push p50_ms=$(median "${times[@]}") runs=${RUNS} samples=[${times[*]}]"
  else
    echo "$label push SKIPPED (no token configured)"
  fi
  rm -rf "$warm"
}

bench_remote genesis "$GENESIS_REMOTE" "${GENESIS_TOKEN:-}"
bench_remote github "$GITHUB_REMOTE" "${GITHUB_TOKEN:-}"

echo "Done. Record p50 pairs and derivations in docs/PERFORMANCE.md."
