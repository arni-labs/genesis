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
#
# REST PR latency (open + merge) runs when the REST env is provided:
#   GENESIS_API=https://<genesis-host>/api/v3   GENESIS_REST_REPO=owner/name
#   GENESIS_TOKEN_A=<author: repo:write,pr:write,pr:merge>
#   GENESIS_TOKEN_B=<reviewer: pr:write>        (approve step, untimed —
#                                                Genesis gates merge on Approved)
#   GITHUB_API=https://api.github.com           GITHUB_REST_REPO=owner/name
#   GITHUB_TOKEN=<token with repo scope>
# Each run pushes a throwaway branch (untimed), times POST /pulls, then
# times PUT /pulls/N/merge. GitHub merges directly; Genesis approves as B
# between the two timed calls (untimed) to keep the measured ops identical.

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

  # Cold clone: fresh directory every run. The received packfile bytes
  # approximate wire size (git stores the fetched pack verbatim; .idx is
  # built locally and excluded). Throughput = pack bytes / median time.
  local times=() i pack_bytes=0
  for i in $(seq 1 "$RUNS"); do
    local dest="$WORKDIR/$label-cold-$i" start end
    start="$(now_ms)"
    git clone --quiet "$url" "$dest"
    end="$(now_ms)"
    times+=($((end - start)))
    if [ "$i" = "1" ]; then
      pack_bytes=$(find "$dest/.git/objects/pack" -name '*.pack' -exec stat -f %z {} + 2>/dev/null \
        || find "$dest/.git/objects/pack" -name '*.pack' -exec stat -c %s {} + 2>/dev/null \
        || echo 0)
      pack_bytes=$(echo "$pack_bytes" | python3 -c 'import sys; print(sum(int(l) for l in sys.stdin if l.strip()))')
    fi
    rm -rf "$dest"
  done
  local p50; p50=$(median "${times[@]}")
  echo "$label cold-clone p50_ms=$p50 runs=${RUNS} samples=[${times[*]}]"
  echo "$label pack-wire-bytes=$pack_bytes ($(python3 -c "print(f'{$pack_bytes/1048576:.2f}')") MiB)"
  if [ "$p50" -gt 0 ]; then
    echo "$label clone-throughput_MiBps=$(python3 -c "print(f'{($pack_bytes/1048576)/($p50/1000):.2f}')")"
  fi

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

# --- REST PR open + merge latency -----------------------------------
# Times exactly two calls per run on each host: POST .../pulls (open)
# and PUT .../pulls/{n}/merge. Branch setup pushes and the Genesis
# approval (reviewer B) are untimed plumbing between them.

rest_call() { # out_file method url token json_body -> prints http_code
  local out="$1" method="$2" url="$3" token="$4" body="${5:-}"
  curl -sS -o "$out" -w '%{http_code}' -X "$method" \
    -H "Authorization: token $token" \
    -H "Accept: application/vnd.github+json" \
    -H "Content-Type: application/json" \
    ${body:+-d "$body"} "$url"
}

json_get() { # file dotted.path
  python3 - "$1" "$2" <<'PY'
import json, sys
obj = json.load(open(sys.argv[1]))
for key in sys.argv[2].split('.'):
    obj = obj[int(key)] if isinstance(obj, list) else obj[key]
print(obj)
PY
}

bench_rest_pr() { # label api repo clone_url author_token reviewer_token
  local label="$1" api="$2" repo="$3" clone_url="$4" author="$5" reviewer="${6:-}"
  echo "=== $label REST PR open+merge ($api/repos/$repo) ==="
  local work="$WORKDIR/$label-rest"
  git clone --quiet "$clone_url" "$work"
  git -C "$work" config user.email bench@example.com
  git -C "$work" config user.name "Bench Probe"
  local default_branch
  default_branch=$(git -C "$work" rev-parse --abbrev-ref HEAD)
  local open_times=() merge_times=() i
  for i in $(seq 1 "$RUNS"); do
    local branch="bench-pr-$$-$i" out start end pr_number
    git -C "$work" checkout --quiet "$default_branch"
    git -C "$work" pull --quiet origin "$default_branch"
    git -C "$work" checkout --quiet -b "$branch"
    printf 'bench pr probe %s run %s\n' "$$" "$i" > "$work/bench-pr-probe.txt"
    git -C "$work" add bench-pr-probe.txt
    git -C "$work" commit --quiet -m "bench pr probe $i"
    git -C "$work" push --quiet origin "$branch"

    out="$WORKDIR/$label-pr-open-$i.json"
    start="$(now_ms)"
    code=$(rest_call "$out" POST "$api/repos/$repo/pulls" "$author" \
      "{\"title\":\"bench pr $i\",\"head\":\"$branch\",\"base\":\"$default_branch\",\"body\":\"latency probe\"}")
    end="$(now_ms)"
    [ "$code" = "201" ] || { echo "$label PR open FAILED status=$code body=$(head -c 200 "$out")"; return 1; }
    open_times+=($((end - start)))
    pr_number=$(json_get "$out" number)

    if [ -n "$reviewer" ]; then # Genesis: merge is gated on Approved
      out="$WORKDIR/$label-pr-approve-$i.json"
      code=$(rest_call "$out" POST "$api/repos/$repo/pulls/$pr_number/reviews" "$reviewer" \
        '{"event":"APPROVE","body":"bench approve"}')
      [ "$code" = "200" ] || { echo "$label PR approve FAILED status=$code body=$(head -c 200 "$out")"; return 1; }
    fi

    out="$WORKDIR/$label-pr-merge-$i.json"
    start="$(now_ms)"
    code=$(rest_call "$out" PUT "$api/repos/$repo/pulls/$pr_number/merge" "$author" \
      '{"merge_method":"merge"}')
    end="$(now_ms)"
    [ "$code" = "200" ] || { echo "$label PR merge FAILED status=$code body=$(head -c 200 "$out")"; return 1; }
    merge_times+=($((end - start)))
  done
  echo "$label pr-open p50_ms=$(median "${open_times[@]}") runs=${RUNS} samples=[${open_times[*]}]"
  echo "$label pr-merge p50_ms=$(median "${merge_times[@]}") runs=${RUNS} samples=[${merge_times[*]}]"
  rm -rf "$work"
}

if [ -n "${GENESIS_API:-}" ] && [ -n "${GENESIS_REST_REPO:-}" ] && [ -n "${GENESIS_TOKEN_A:-}" ] && [ -n "${GENESIS_TOKEN_B:-}" ]; then
  bench_rest_pr genesis "$GENESIS_API" "$GENESIS_REST_REPO" "$GENESIS_REMOTE" "$GENESIS_TOKEN_A" "$GENESIS_TOKEN_B"
else
  echo "genesis REST PR bench SKIPPED (GENESIS_API/GENESIS_REST_REPO/GENESIS_TOKEN_A/GENESIS_TOKEN_B not all set)"
fi
if [ -n "${GITHUB_API:-}" ] && [ -n "${GITHUB_REST_REPO:-}" ] && [ -n "${GITHUB_TOKEN:-}" ]; then
  bench_rest_pr github "$GITHUB_API" "$GITHUB_REST_REPO" "$GITHUB_REMOTE" "$GITHUB_TOKEN"
else
  echo "github REST PR bench SKIPPED (GITHUB_API/GITHUB_REST_REPO/GITHUB_TOKEN not all set)"
fi

echo "Done. Record p50 pairs and derivations in docs/PERFORMANCE.md."
