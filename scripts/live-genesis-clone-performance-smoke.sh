#!/usr/bin/env bash
set -euo pipefail

# Read-only live smoke for Genesis smart-HTTP clone/fetch performance.
# It creates only local temporary directories and never mutates Genesis data.

BASE_URL="${BASE_URL:-${GENESIS_URL:-https://genesis-production-164d.up.railway.app}}"
TENANT="${TENANT:-default}"
OWNER="${OWNER:-temperpaw}"
# paw-patrol is currently the largest production app whose ref points at a
# valid Git commit. paw-agent is intentionally excluded until its historical
# non-commit app hash is repaired through a governed Genesis update.
REPO="${REPO:-paw-patrol}"
TINY_OWNER="${TINY_OWNER:-genesis-e2e}"
TINY_REPO="${TINY_REPO:-tiny-notes-rail140900}"
LARGE_WARM_LIMIT_SECS="${LARGE_WARM_LIMIT_SECS:-${PAW_AGENT_WARM_LIMIT_SECS:-30}}"
TINY_LIMIT_SECS="${TINY_LIMIT_SECS:-5}"
FIRST_BYTE_LIMIT_SECS="${FIRST_BYTE_LIMIT_SECS:-10}"
CLONE_MAX_SECS="${CLONE_MAX_SECS:-120}"

now_ms() {
  python3 - <<'PY'
import time
print(int(time.time() * 1000))
PY
}

elapsed_ms() {
  local start="$1"
  local end
  end="$(now_ms)"
  printf '%s' "$((end - start))"
}

measure() {
  local label="$1"
  shift
  local start
  start="$(now_ms)"
  set +e
  "$@"
  local status="$?"
  set -e
  local elapsed
  elapsed="$(elapsed_ms "$start")"
  printf '%s elapsed_ms=%s status=%s\n' "$label" "$elapsed" "$status"
  MEASURED_MS="$elapsed"
  return "$status"
}

with_timeout() {
  local seconds="$1"
  shift
  LC_ALL=C LANG=C perl -e 'alarm shift; exec @ARGV' "$seconds" "$@"
}

assert_under() {
  local label="$1"
  local elapsed_ms="$2"
  local limit_secs="$3"
  local limit_ms="$((limit_secs * 1000))"
  if [ "$elapsed_ms" -gt "$limit_ms" ]; then
    printf 'FAIL %s took %sms, over %ss\n' "$label" "$elapsed_ms" "$limit_secs" >&2
    exit 1
  fi
}

tmp="$(mktemp -d "${TMPDIR:-/tmp}/genesis-clone-smoke.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT

printf 'Genesis clone performance smoke\n'
printf 'base=%s tenant=%s large=%s/%s tiny=%s/%s\n' \
  "$BASE_URL" "$TENANT" "$OWNER" "$REPO" "$TINY_OWNER" "$TINY_REPO"

measure "ls_remote_large" \
  git -c http.extraHeader="X-Tenant-Id: $TENANT" \
    ls-remote "$BASE_URL/$OWNER/$REPO.git"

first_byte_start="$(now_ms)"
curl -sS --max-time "$FIRST_BYTE_LIMIT_SECS" \
  -H "X-Tenant-Id: $TENANT" \
  "$BASE_URL/$OWNER/$REPO.git/info/refs?service=git-upload-pack" \
  | head -c 64 >/dev/null
first_byte_ms="$(elapsed_ms "$first_byte_start")"
printf 'info_refs_first_byte_large elapsed_ms=%s\n' "$first_byte_ms"
assert_under "large info/refs first byte" "$first_byte_ms" "$FIRST_BYTE_LIMIT_SECS"

if git -c http.extraHeader="X-Tenant-Id: $TENANT" \
  ls-remote "$BASE_URL/$TINY_OWNER/$TINY_REPO.git" >/dev/null 2>&1; then
  measure "clone_tiny" \
    with_timeout "$TINY_LIMIT_SECS" git -c http.extraHeader="X-Tenant-Id: $TENANT" \
      clone --quiet "$BASE_URL/$TINY_OWNER/$TINY_REPO.git" "$tmp/tiny"
  assert_under "tiny clone" "$MEASURED_MS" "$TINY_LIMIT_SECS"
else
  printf 'SKIP tiny clone: %s/%s not present\n' "$TINY_OWNER" "$TINY_REPO"
fi

measure "clone_large_warmup" \
  with_timeout "$CLONE_MAX_SECS" git -c http.extraHeader="X-Tenant-Id: $TENANT" \
    clone --quiet "$BASE_URL/$OWNER/$REPO.git" "$tmp/large-warmup"

rm -rf "$tmp/large"
measure "clone_large_warm" \
  with_timeout "$LARGE_WARM_LIMIT_SECS" git -c http.extraHeader="X-Tenant-Id: $TENANT" \
    clone --quiet "$BASE_URL/$OWNER/$REPO.git" "$tmp/large"
assert_under "large warm clone" "$MEASURED_MS" "$LARGE_WARM_LIMIT_SECS"

git -C "$tmp/large" fsck --no-progress >/dev/null
printf 'PASS Genesis clone performance smoke\n'
