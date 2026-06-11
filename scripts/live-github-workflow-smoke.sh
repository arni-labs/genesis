#!/usr/bin/env bash
set -Eeuo pipefail

# Live smoke for the GitHub workflow layer (RFC-0004 Slice 5, CI
# round-trip gate).
#
# Requires a running Temper server with the temper-git/Genesis app
# bootstrapped (same precondition as live-genesis-install-e2e-smoke.sh,
# which is the canonical boot pattern). The smoke registers the smart
# HTTP + /api/v3 HttpEndpoints rows, mints scoped GitTokens, and proves
# the full workflow with real `git` plus curl against the REST surface:
#
#   create repo (REST) → push (token auth) → anonymous push denied →
#   open PR → author self-approve denied → second-principal approve →
#   merge (merge + squash strategies) → conflicting merge answers 409 →
#   re-clone + fsck → force-push gated on the `force` scope.
#
# REST calls use curl with explicit Authorization headers rather than
# `gh` for determinism (gh requires GH_HOST/auth indirection for
# non-github.com hosts); all git operations use real git.
#
# Tokens (see policies/*.cedar for why each scope is needed):
#   A author:   repo:read repo:write pr:write pr:merge
#               (pr:merge is required by pull_request.cedar's Merge
#               gate — merging via PUT .../merge needs it)
#   B reviewer: repo:read pr:write (different principal: author
#               self-approval denial + second-principal approve)
#   C force:    repo:read repo:write force, same principal as A
#               (ref.cedar ForceUpdate requires repo ownership AND the
#               force scope; C models a force-capable token rotation)

BASE_URL="${TEMPER_URL:-http://127.0.0.1:3188}"
BASE_URL="${BASE_URL%/}"
TENANT="${TEMPER_TENANT:-default}"
RUN_ID="${RUN_ID:-$(date +%H%M%S)}"
WAIT_SECS="${WAIT_SECS:-60}"
KEEP_TMP="${KEEP_TMP:-0}"

PRINCIPAL_A="wf-author-${RUN_ID}"
PRINCIPAL_B="wf-reviewer-${RUN_ID}"
OWNER="$PRINCIPAL_A"
REPO="workflow-${RUN_ID}"
REPO_PATH="${OWNER}/${REPO}"

SCHEME="${BASE_URL%%://*}"
HOST_PORT="${BASE_URL#*://}"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/genesis-workflow-smoke.XXXXXX")"
STEP_RESULTS=()

print_summary() {
  printf '\n%-4s %-38s %s\n' 'step' 'description' 'result'
  printf '%-4s %-38s %s\n' '----' '--------------------------------------' '------'
  # ${arr[@]+...} guard: empty-array expansion under set -u errors on
  # the bash 3.2 that macOS ships.
  local row
  for row in ${STEP_RESULTS[@]+"${STEP_RESULTS[@]}"}; do
    IFS='|' read -r num desc result <<<"$row"
    printf '%-4s %-38s %s\n' "$num" "$desc" "$result"
  done
}

cleanup() {
  local status=$?
  if [[ "$status" -ne 0 ]]; then
    print_summary || true
    printf 'FAIL — evidence preserved in %s\n' "$TMP_DIR" >&2
  elif [[ "$KEEP_TMP" == "1" ]]; then
    printf 'Temp dir preserved: %s\n' "$TMP_DIR"
  else
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT
trap 'printf "FAIL line %s status %s\n" "$LINENO" "$?" >&2' ERR

pass() {
  # pass <step-number> <description> <evidence>
  STEP_RESULTS+=("$1|$2|PASS")
  printf 'PASS step %s — %s (%s)\n' "$1" "$2" "$3"
}

json_escape() {
  node -e 'process.stdout.write(JSON.stringify(process.argv[1]))' "$1"
}

# json_field <file> <dot.path> — prints the value, fails when absent.
json_field() {
  node -e '
const fs = require("fs");
const j = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
let v = j;
for (const k of process.argv[2].split(".")) { v = v?.[k]; }
if (v === undefined || v === null) process.exit(1);
process.stdout.write(String(v));
' "$1" "$2"
}

sha256_hex() {
  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "$1" | sha256sum | cut -d' ' -f1
  else
    printf '%s' "$1" | shasum -a 256 | cut -d' ' -f1
  fi
}

# Admin/operator headers — same operator identity the install smoke
# uses for entity seeding; admin:tokens + agent_type=admin satisfy
# git_token.cedar's mint-for-another-principal rule.
admin_headers=(
  -H "Content-Type: application/json"
  -H "Accept: application/json"
  -H "X-Tenant-Id: ${TENANT}"
  -H "X-Temper-Principal-Kind: admin"
  -H "X-Temper-Principal-Id: operator"
  -H "X-Temper-Principal-Scopes: admin:platform admin:repos admin:owners admin:tokens repo:write pr:write"
  -H "X-Temper-Agent-Type: admin"
)

admin_post() {
  local path="$1" body="$2" out="$3" status
  status="$(curl -sS -o "$out" -w "%{http_code}" -X POST "${admin_headers[@]}" -d "$body" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'POST %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,120p' "$out" >&2
    exit 1
  fi
}

admin_get() {
  local path="$1" out="$2" status
  status="$(curl -sS -o "$out" -w "%{http_code}" "${admin_headers[@]}" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'GET %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,120p' "$out" >&2
    exit 1
  fi
}

# rest <method> <path> <token> <json-body-or-empty> <out> — prints the
# HTTP status; never exits, callers assert the status they expect.
rest() {
  local method="$1" path="$2" token="$3" body="$4" out="$5"
  local -a args=(-sS -o "$out" -w "%{http_code}" -X "$method" -H "Accept: application/json")
  if [[ -n "$token" ]]; then
    args+=(-H "Authorization: token ${token}")
  fi
  if [[ -n "$body" ]]; then
    args+=(-H "Content-Type: application/json" -d "$body")
  fi
  curl "${args[@]}" "${BASE_URL}${path}"
}

assert_status() {
  # assert_status <got> <want> <label> <body-file>
  if [[ "$1" != "$2" ]]; then
    printf 'ASSERT %s: expected HTTP %s, got %s\n' "$3" "$2" "$1" >&2
    sed -n '1,120p' "$4" >&2
    exit 1
  fi
}

# expect_git_fail <stderr-out> <cmd...> — asserts the git command exits
# non-zero, captures stderr for the caller's evidence checks.
expect_git_fail() {
  local err_out="$1"
  shift
  if "$@" >"$TMP_DIR/git-stdout.log" 2>"$err_out"; then
    printf 'ASSERT: expected failure but command succeeded: %s\n' "$*" >&2
    sed -n '1,40p' "$err_out" >&2
    exit 1
  fi
}

ensure_endpoint() {
  local endpoint_id="$1" body="$2"
  local out="${TMP_DIR}/${endpoint_id}.json" status
  status="$(curl -sS -o "$out" -w "%{http_code}" -H "X-Tenant-Id: ${TENANT}" "${BASE_URL}/tdata/HttpEndpoints('${endpoint_id}')")"
  if [[ "$status" == "200" ]]; then
    status="$(curl -sS -o "$out" -w "%{http_code}" -X PATCH -H "X-Tenant-Id: ${TENANT}" -H 'Content-Type: application/json' -d "$body" "${BASE_URL}/tdata/HttpEndpoints('${endpoint_id}')")"
    if [[ "$status" != 2* ]]; then
      printf 'PATCH HttpEndpoint %s failed with HTTP %s\n' "$endpoint_id" "$status" >&2
      sed -n '1,120p' "$out" >&2
      exit 1
    fi
    return
  fi
  admin_post "/tdata/HttpEndpoints" "$body" "$out"
}

# Same endpoint rows the install smoke registers (the modules resolve
# GitTokens themselves, so RequiresAuth stays false at the kernel).
register_endpoints() {
  ensure_endpoint "he-info-refs" \
    '{"Id":"he-info-refs","PathPrefix":"/{owner}/{repo}.git/info/refs","Methods":"GET","IntegrationModule":"git_refs_advertise","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-upload-pack" \
    '{"Id":"he-upload-pack","PathPrefix":"/{owner}/{repo}.git/git-upload-pack","Methods":"POST","IntegrationModule":"git_upload_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":20000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728}'
  ensure_endpoint "he-receive-pack" \
    '{"Id":"he-receive-pack","PathPrefix":"/{owner}/{repo}.git/git-receive-pack","Methods":"POST","IntegrationModule":"git_receive_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":20000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728,"ActionBridgeEntityType":"Repository","ActionBridgeEntityId":"rp-{owner}-{repo}","ActionBridgeAction":"IngestPack","ActionBridgeResponse":"git-receive-pack"}'
  ensure_endpoint "he-api-user-repos" \
    '{"Id":"he-api-user-repos","PathPrefix":"/api/v3/user/repos","Methods":"POST","IntegrationModule":"github_rest_repos","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-api-repo" \
    '{"Id":"he-api-repo","PathPrefix":"/api/v3/repos/{owner}/{repo}","Methods":"GET","IntegrationModule":"github_rest_repos","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-api-branches" \
    '{"Id":"he-api-branches","PathPrefix":"/api/v3/repos/{owner}/{repo}/branches","Methods":"GET","IntegrationModule":"github_rest_refs","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-api-git-refs" \
    '{"Id":"he-api-git-refs","PathPrefix":"/api/v3/repos/{owner}/{repo}/git/refs","Methods":"GET,POST,PATCH,DELETE","IntegrationModule":"github_rest_refs","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-api-git-ref" \
    '{"Id":"he-api-git-ref","PathPrefix":"/api/v3/repos/{owner}/{repo}/git/ref","Methods":"GET","IntegrationModule":"github_rest_refs","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-api-matching-refs" \
    '{"Id":"he-api-matching-refs","PathPrefix":"/api/v3/repos/{owner}/{repo}/git/matching-refs","Methods":"GET","IntegrationModule":"github_rest_refs","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint "he-api-pulls" \
    '{"Id":"he-api-pulls","PathPrefix":"/api/v3/repos/{owner}/{repo}/pulls","Methods":"GET,POST,PATCH,PUT","IntegrationModule":"github_rest_pulls","RequiresAuth":false,"TimeoutSecs":120}'
}

# mint_token <row-id> <principal> <secret> <scopes-csv> — GitToken rows
# store only the SHA-256 of the secret (specs/git_token.ioa.toml); the
# resolver in crates/git_auth looks up HashedSecret and requires
# Status=Active.
mint_token() {
  local row_id="$1" principal="$2" secret="$3" scopes="$4"
  local hashed
  hashed="$(sha256_hex "$secret")"
  admin_post "/tdata/GitTokens" \
    "{\"Id\":$(json_escape "$row_id"),\"PrincipalId\":$(json_escape "$principal"),\"HashedSecret\":$(json_escape "$hashed"),\"KeyPrefix\":$(json_escape "${secret:0:8}"),\"Scopes\":$(json_escape "$scopes"),\"ExpiresAt\":\"2030-01-01T00:00:00Z\"}" \
    "${TMP_DIR}/token-${row_id}.json"
  admin_get "/tdata/GitTokens('${row_id}')" "${TMP_DIR}/token-read-${row_id}.json"
  local got_principal
  got_principal="$(json_field "${TMP_DIR}/token-read-${row_id}.json" "fields.PrincipalId" 2>/dev/null \
    || json_field "${TMP_DIR}/token-read-${row_id}.json" "PrincipalId")"
  if [[ "$got_principal" != "$principal" ]]; then
    printf 'GitToken %s row PrincipalId mismatch: %s\n' "$row_id" "$got_principal" >&2
    exit 1
  fi
}

remote_url() {
  # remote_url <token-or-empty> — token rides as the Basic username
  # (crates/git_auth reads the username as the bearer).
  if [[ -n "$1" ]]; then
    printf '%s://%s:x@%s/%s.git' "$SCHEME" "$1" "$HOST_PORT" "$REPO_PATH"
  else
    printf '%s://%s/%s.git' "$SCHEME" "$HOST_PORT" "$REPO_PATH"
  fi
}

rest_ref_sha() {
  # rest_ref_sha <branch> <out> — prints HTTP status; sha is in <out>.
  rest GET "/api/v3/repos/${REPO_PATH}/git/ref/heads/$1" "$TOKEN_A" "" "$2"
}

git_in() {
  git -C "$1" "${@:2}"
}

new_branch_commit() {
  # new_branch_commit <branch> <file> <content> <message> — branches
  # off the current local main and pushes as A.
  git_in "$SRC" checkout -q main
  git_in "$SRC" checkout -q -b "$1"
  printf '%s\n' "$3" > "${SRC}/$2"
  git_in "$SRC" add "$2"
  git_in "$SRC" commit -q -m "$4"
  git_in "$SRC" push -q origin "$1"
}

sync_main() {
  # Server-side merges advance main behind the local clone's back.
  git_in "$SRC" checkout -q main
  git_in "$SRC" fetch -q origin
  git_in "$SRC" merge -q --ff-only origin/main
}

open_pull() {
  # open_pull <head-branch> <title> → prints PR number
  local out="${TMP_DIR}/pr-$1.json" status
  status="$(rest POST "/api/v3/repos/${REPO_PATH}/pulls" "$TOKEN_A" \
    "{\"title\":$(json_escape "$2"),\"head\":$(json_escape "$1"),\"base\":\"main\"}" "$out")"
  assert_status "$status" "201" "POST pulls ($1)" "$out"
  json_field "$out" "number"
}

approve_as_b() {
  # approve_as_b <pr-number>
  local out="${TMP_DIR}/approve-$1.json" status
  status="$(rest POST "/api/v3/repos/${REPO_PATH}/pulls/$1/reviews" "$TOKEN_B" \
    '{"event":"APPROVE","body":"second-principal approval"}' "$out")"
  assert_status "$status" "200" "POST reviews approve PR $1" "$out"
}

merge_pull() {
  # merge_pull <pr-number> <merge_method> <out> — prints HTTP status.
  rest PUT "/api/v3/repos/${REPO_PATH}/pulls/$1/merge" "$TOKEN_A" \
    "{\"merge_method\":\"$2\"}" "$3"
}

# ---------------------------------------------------------------------
# Step 1 — preconditions: server reachable, endpoints registered,
# three scoped GitTokens minted (hashed-secret rows).
# ---------------------------------------------------------------------

deadline=$((SECONDS + WAIT_SECS))
until curl -fsS -H "X-Tenant-Id: ${TENANT}" "${BASE_URL}/tdata/Apps?\$top=1" >/dev/null 2>&1; do
  if [[ "$SECONDS" -ge "$deadline" ]]; then
    printf 'Server at %s not reachable within %ss\n' "$BASE_URL" "$WAIT_SECS" >&2
    exit 1
  fi
  sleep 1
done

register_endpoints

TOKEN_A="$(openssl rand -hex 20)"
TOKEN_B="$(openssl rand -hex 20)"
TOKEN_C="$(openssl rand -hex 20)"
mint_token "gt-a-${RUN_ID}" "$PRINCIPAL_A" "$TOKEN_A" "repo:read,repo:write,pr:write,pr:merge"
mint_token "gt-b-${RUN_ID}" "$PRINCIPAL_B" "$TOKEN_B" "repo:read,pr:write"
mint_token "gt-c-${RUN_ID}" "$PRINCIPAL_A" "$TOKEN_C" "repo:read,repo:write,force"
pass 1 "preconditions + scoped tokens" "server up at ${BASE_URL}; tokens gt-{a,b,c}-${RUN_ID} minted (A=author, B=reviewer, C=force)"

# ---------------------------------------------------------------------
# Step 2 — POST /api/v3/user/repos as A → 201; GET → 200 + shape.
# ---------------------------------------------------------------------

out="${TMP_DIR}/repo-create.json"
status="$(rest POST "/api/v3/user/repos" "$TOKEN_A" "{\"name\":$(json_escape "$REPO")}" "$out")"
assert_status "$status" "201" "POST /api/v3/user/repos" "$out"
full_name="$(json_field "$out" "full_name")"
default_branch="$(json_field "$out" "default_branch")"
if [[ "$full_name" != "$REPO_PATH" || "$default_branch" != "main" ]]; then
  printf 'repo shape mismatch: full_name=%s default_branch=%s\n' "$full_name" "$default_branch" >&2
  exit 1
fi
out="${TMP_DIR}/repo-get.json"
status="$(rest GET "/api/v3/repos/${REPO_PATH}" "$TOKEN_A" "" "$out")"
assert_status "$status" "200" "GET /api/v3/repos/${REPO_PATH}" "$out"
pass 2 "create + read repository (REST)" "201 then 200; full_name=${full_name} default_branch=${default_branch}"

# ---------------------------------------------------------------------
# Step 3 — real git: seed main, then commit + push a feature branch
# with A's token as Basic-auth username.
# ---------------------------------------------------------------------

SRC="${TMP_DIR}/src"
mkdir -p "$SRC"
git_in "$SRC" init -q -b main
git_in "$SRC" config user.email "workflow-smoke@genesis.local"
git_in "$SRC" config user.name "Genesis Workflow Smoke"
git_in "$SRC" remote add origin "$(remote_url "$TOKEN_A")"
printf '# workflow smoke %s\n' "$RUN_ID" > "${SRC}/README.md"
git_in "$SRC" add README.md
git_in "$SRC" commit -q -m "Seed main"
git_in "$SRC" push -q origin main

new_branch_commit "feature-merge" "feature-merge.txt" "merge me ${RUN_ID}" "Add feature-merge"
FEATURE_SHA="$(git_in "$SRC" rev-parse feature-merge)"
out="${TMP_DIR}/ref-feature-merge.json"
status="$(rest_ref_sha "feature-merge" "$out")"
assert_status "$status" "200" "GET git/ref/heads/feature-merge" "$out"
server_sha="$(json_field "$out" "object.sha")"
if [[ "$server_sha" != "$FEATURE_SHA" ]]; then
  printf 'feature-merge tip mismatch: server %s local %s\n' "$server_sha" "$FEATURE_SHA" >&2
  exit 1
fi
pass 3 "authed commit + push (token A)" "main + feature-merge pushed; server tip ${server_sha} == local"

# ---------------------------------------------------------------------
# Step 4 — anonymous push must fail with the 401 challenge; a bad
# token must fail with git's Authentication error.
# ---------------------------------------------------------------------

git_in "$SRC" checkout -q -b anon-probe
printf 'anon %s\n' "$RUN_ID" > "${SRC}/anon-probe.txt"
git_in "$SRC" add anon-probe.txt
git_in "$SRC" commit -q -m "Anonymous probe"

# `-c credential.helper=` keeps a developer keychain from silently
# supplying valid credentials to the denial cases.
anon_err="${TMP_DIR}/push-anon.err"
expect_git_fail "$anon_err" \
  env GIT_TERMINAL_PROMPT=0 git -C "$SRC" -c credential.helper= push "$(remote_url "")" anon-probe
if ! grep -Eqi 'authentication|terminal prompts disabled|401' "$anon_err"; then
  printf 'anonymous push stderr lacked auth challenge evidence:\n' >&2
  sed -n '1,40p' "$anon_err" >&2
  exit 1
fi

bad_err="${TMP_DIR}/push-bad-token.err"
expect_git_fail "$bad_err" \
  env GIT_TERMINAL_PROMPT=0 git -C "$SRC" -c credential.helper= push "$(remote_url "0000000000000000000000000000000000000000")" anon-probe
if ! grep -qi 'authentication' "$bad_err"; then
  printf 'bad-token push stderr lacked "Authentication":\n' >&2
  sed -n '1,40p' "$bad_err" >&2
  exit 1
fi

out="${TMP_DIR}/ref-anon-probe.json"
status="$(rest_ref_sha "anon-probe" "$out")"
assert_status "$status" "404" "GET git/ref/heads/anon-probe (must not exist)" "$out"
pass 4 "anonymous + bad-token push denied" "both pushes failed; stderr shows auth challenge; anon-probe absent on server (404)"

# ---------------------------------------------------------------------
# Step 5 — open a PR via REST as A; read it back open.
# ---------------------------------------------------------------------

PR_MAIN="$(open_pull "feature-merge" "Merge feature-merge")"
out="${TMP_DIR}/pr-main-get.json"
status="$(rest GET "/api/v3/repos/${REPO_PATH}/pulls/${PR_MAIN}" "$TOKEN_A" "" "$out")"
assert_status "$status" "200" "GET pulls/${PR_MAIN}" "$out"
pr_state="$(json_field "$out" "state")"
if [[ "$pr_state" != "open" ]]; then
  printf 'PR %s expected open, got %s\n' "$PR_MAIN" "$pr_state" >&2
  exit 1
fi
pass 5 "open pull request (REST)" "PR #${PR_MAIN} created, state=open"

# ---------------------------------------------------------------------
# Step 6 — author self-approval denied; reviewer (B) approval moves
# the lifecycle to Approved.
# ---------------------------------------------------------------------

out="${TMP_DIR}/self-approve.json"
status="$(rest POST "/api/v3/repos/${REPO_PATH}/pulls/${PR_MAIN}/reviews" "$TOKEN_A" '{"event":"APPROVE"}' "$out")"
assert_status "$status" "422" "author self-approve (must be denied)" "$out"
self_msg="$(json_field "$out" "message")"

approve_as_b "$PR_MAIN"

admin_get "/tdata/PullRequests?\$filter=RepositoryId%20eq%20'rp-${OWNER}-${REPO}'" "${TMP_DIR}/pr-rows.json"
pr_status="$(node -e '
const fs = require("fs");
const rows = JSON.parse(fs.readFileSync(process.argv[1], "utf8")).value || [];
const number = Number(process.argv[2]);
const row = rows.find((r) => Number(r.fields?.Number ?? r.Number) === number);
if (!row) process.exit(1);
process.stdout.write(String(row.status ?? row.Status ?? row.fields?.Status ?? ""));
' "${TMP_DIR}/pr-rows.json" "$PR_MAIN")"
if [[ "$pr_status" != "Approved" ]]; then
  printf 'PR %s entity status expected Approved, got %s\n' "$PR_MAIN" "$pr_status" >&2
  exit 1
fi
pass 6 "review gates (self-deny + approve)" "A self-approve → 422 \"${self_msg}\"; B approve → 200; entity status=Approved"

# ---------------------------------------------------------------------
# Step 7 — merge (merge_method=merge) → 200 merged:true; re-clone
# shows the two-parent merge commit; fsck clean.
# ---------------------------------------------------------------------

out="${TMP_DIR}/merge-main.json"
status="$(merge_pull "$PR_MAIN" "merge" "$out")"
assert_status "$status" "200" "PUT pulls/${PR_MAIN}/merge" "$out"
merged="$(json_field "$out" "merged")"
MERGE_SHA="$(json_field "$out" "sha")"
if [[ "$merged" != "true" || -z "$MERGE_SHA" ]]; then
  printf 'merge response shape mismatch: merged=%s sha=%s\n' "$merged" "$MERGE_SHA" >&2
  exit 1
fi
out="${TMP_DIR}/pr-main-after-merge.json"
status="$(rest GET "/api/v3/repos/${REPO_PATH}/pulls/${PR_MAIN}" "$TOKEN_A" "" "$out")"
assert_status "$status" "200" "GET pulls/${PR_MAIN} after merge" "$out"
if [[ "$(json_field "$out" "state")" != "closed" || "$(json_field "$out" "merged")" != "true" ]]; then
  printf 'PR %s expected closed+merged after merge\n' "$PR_MAIN" >&2
  exit 1
fi

CLONE_MERGE="${TMP_DIR}/clone-merge"
git clone -q "$(remote_url "$TOKEN_A")" "$CLONE_MERGE"
parent_words="$(git_in "$CLONE_MERGE" rev-list --parents -n 1 "$MERGE_SHA" | wc -w | tr -d ' ')"
if [[ "$parent_words" != "3" ]]; then
  printf 'merge commit %s expected 2 parents, rev-list gave %s fields\n' "$MERGE_SHA" "$parent_words" >&2
  exit 1
fi
if ! git_in "$CLONE_MERGE" log --format=%H | grep -q "^${MERGE_SHA}$"; then
  printf 'merge commit %s missing from fresh clone log\n' "$MERGE_SHA" >&2
  exit 1
fi
if [[ ! -f "${CLONE_MERGE}/feature-merge.txt" ]]; then
  printf 'feature-merge.txt missing from merged main\n' >&2
  exit 1
fi
git_in "$CLONE_MERGE" fsck --full > "${TMP_DIR}/fsck-merge.log" 2>&1
pass 7 "merge strategy=merge + re-clone + fsck" "merged:true sha=${MERGE_SHA} (2 parents); PR closed+merged; fsck clean"

# ---------------------------------------------------------------------
# Step 8 — conflicting merge answers 409. Two branches change the SAME
# file from the same base; the first merges, the second conflicts.
# ---------------------------------------------------------------------

sync_main
printf 'base\n' > "${SRC}/shared.txt"
git_in "$SRC" add shared.txt
git_in "$SRC" commit -q -m "Add shared.txt base"
git_in "$SRC" push -q origin main

new_branch_commit "conflict-one" "shared.txt" "alpha ${RUN_ID}" "conflict-one rewrites shared.txt"
new_branch_commit "conflict-two" "shared.txt" "beta ${RUN_ID}" "conflict-two rewrites shared.txt"

PR_C1="$(open_pull "conflict-one" "Conflict one")"
approve_as_b "$PR_C1"
out="${TMP_DIR}/merge-conflict-one.json"
status="$(merge_pull "$PR_C1" "merge" "$out")"
assert_status "$status" "200" "PUT pulls/${PR_C1}/merge (conflict-one)" "$out"

PR_C2="$(open_pull "conflict-two" "Conflict two")"
approve_as_b "$PR_C2"
out="${TMP_DIR}/merge-conflict-two.json"
status="$(merge_pull "$PR_C2" "merge" "$out")"
assert_status "$status" "409" "PUT pulls/${PR_C2}/merge (must conflict)" "$out"
conflict_msg="$(json_field "$out" "message")"
if ! grep -qi 'conflict' <<<"$conflict_msg"; then
  printf '409 body does not mention conflict: %s\n' "$conflict_msg" >&2
  exit 1
fi
# The 409 must leave no side effects: a conflicting merge dispatches
# Repository.MergePullRequest, whose PR -> Merged transition is a sub-write
# applied only on a clean merge. The PR must still be open/unmerged.
out="${TMP_DIR}/pr-conflict-two-after-409.json"
status="$(rest GET "/api/v3/repos/${REPO_PATH}/pulls/${PR_C2}" "$TOKEN_A" "" "$out")"
assert_status "$status" "200" "GET pulls/${PR_C2} after 409" "$out"
if [[ "$(json_field "$out" "state")" != "open" || "$(json_field "$out" "merged")" == "true" ]]; then
  printf 'conflict PR %s wrongly merged after 409: state=%s merged=%s\n' \
    "$PR_C2" "$(json_field "$out" "state")" "$(json_field "$out" "merged")" >&2
  exit 1
fi
pass 8 "conflicting merge → 409 (no side effect)" "PR #${PR_C1} merged; PR #${PR_C2} → 409 \"${conflict_msg}\"; PR #${PR_C2} still open/unmerged"

# ---------------------------------------------------------------------
# Step 9 — squash strategy on a third branch → single squash commit.
# ---------------------------------------------------------------------

sync_main
git_in "$SRC" checkout -q -b squash-work
printf 'squash one\n' > "${SRC}/squash.txt"
git_in "$SRC" add squash.txt
git_in "$SRC" commit -q -m "squash commit one"
printf 'squash two\n' >> "${SRC}/squash.txt"
git_in "$SRC" add squash.txt
git_in "$SRC" commit -q -m "squash commit two"
git_in "$SRC" push -q origin squash-work

PR_SQ="$(open_pull "squash-work" "Squash work")"
approve_as_b "$PR_SQ"
out="${TMP_DIR}/merge-squash.json"
status="$(merge_pull "$PR_SQ" "squash" "$out")"
assert_status "$status" "200" "PUT pulls/${PR_SQ}/merge squash" "$out"
SQUASH_SHA="$(json_field "$out" "sha")"
if [[ "$(json_field "$out" "merged")" != "true" ]]; then
  printf 'squash merge expected merged:true\n' >&2
  exit 1
fi

CLONE_SQUASH="${TMP_DIR}/clone-squash"
git clone -q "$(remote_url "$TOKEN_A")" "$CLONE_SQUASH"
main_tip="$(git_in "$CLONE_SQUASH" rev-parse HEAD)"
if [[ "$main_tip" != "$SQUASH_SHA" ]]; then
  printf 'main tip %s != squash sha %s\n' "$main_tip" "$SQUASH_SHA" >&2
  exit 1
fi
parent_words="$(git_in "$CLONE_SQUASH" rev-list --parents -n 1 "$SQUASH_SHA" | wc -w | tr -d ' ')"
if [[ "$parent_words" != "2" ]]; then
  printf 'squash commit %s expected 1 parent, rev-list gave %s fields\n' "$SQUASH_SHA" "$parent_words" >&2
  exit 1
fi
if [[ "$(cat "${CLONE_SQUASH}/squash.txt")" != "$(printf 'squash one\nsquash two')" ]]; then
  printf 'squash.txt content mismatch after squash merge\n' >&2
  exit 1
fi
git_in "$CLONE_SQUASH" fsck --full > "${TMP_DIR}/fsck-squash.log" 2>&1
pass 9 "merge strategy=squash + re-clone + fsck" "merged:true sha=${SQUASH_SHA} (1 parent, both edits squashed); fsck clean"

# ---------------------------------------------------------------------
# Step 10 — force-push: non-FF rejected without the force scope (A),
# accepted with it (C, same principal).
# ---------------------------------------------------------------------

sync_main
git_in "$SRC" checkout -q -b force-lab
printf 'force v1\n' > "${SRC}/force-lab.txt"
git_in "$SRC" add force-lab.txt
git_in "$SRC" commit -q -m "force-lab v1"
git_in "$SRC" push -q origin force-lab
V1_SHA="$(git_in "$SRC" rev-parse force-lab)"

printf 'force v2\n' > "${SRC}/force-lab.txt"
git_in "$SRC" add force-lab.txt
git_in "$SRC" commit -q --amend -m "force-lab v2 (rewritten)"
V2_SHA="$(git_in "$SRC" rev-parse force-lab)"

force_err="${TMP_DIR}/force-as-a.err"
expect_git_fail "$force_err" \
  env GIT_TERMINAL_PROMPT=0 git -C "$SRC" -c credential.helper= push --force "$(remote_url "$TOKEN_A")" force-lab
out="${TMP_DIR}/ref-force-lab-after-deny.json"
status="$(rest_ref_sha "force-lab" "$out")"
assert_status "$status" "200" "GET git/ref/heads/force-lab after denied force" "$out"
if [[ "$(json_field "$out" "object.sha")" != "$V1_SHA" ]]; then
  printf 'force-lab tip moved despite denial: %s\n' "$(json_field "$out" "object.sha")" >&2
  exit 1
fi

GIT_TERMINAL_PROMPT=0 git -C "$SRC" push -q --force "$(remote_url "$TOKEN_C")" force-lab
out="${TMP_DIR}/ref-force-lab-after-allow.json"
status="$(rest_ref_sha "force-lab" "$out")"
assert_status "$status" "200" "GET git/ref/heads/force-lab after allowed force" "$out"
if [[ "$(json_field "$out" "object.sha")" != "$V2_SHA" ]]; then
  printf 'force-lab tip expected %s after C force-push, got %s\n' "$V2_SHA" "$(json_field "$out" "object.sha")" >&2
  exit 1
fi
pass 10 "force-push scope gate" "A (no force scope) rejected, tip stayed ${V1_SHA}; C (force scope) accepted, tip now ${V2_SHA}"

# ---------------------------------------------------------------------
# Step 11 — summary.
# ---------------------------------------------------------------------

print_summary
printf '\nPASS live github workflow smoke\n'
printf '  base_url: %s\n' "$BASE_URL"
printf '  repo:     %s\n' "$REPO_PATH"
printf '  merge:    %s\n' "$MERGE_SHA"
printf '  squash:   %s\n' "$SQUASH_SHA"
