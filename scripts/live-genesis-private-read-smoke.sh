#!/usr/bin/env bash
# Private-read smoke: real git and curl against a running Genesis prove that a
# non-public repository is served only to a reader the rule allows
# (crates/git_auth/src/read.rs), and that public repositories still clone
# anonymously.
#
# Needs a Genesis it may write to (a local server, as in CI's workflow gate):
# it registers the git/REST HttpEndpoints, mints GitTokens, creates repositories
# and flips one with SetVisibility. Never point it at production.
#
#   TEMPER_URL=http://127.0.0.1:3188 TEMPER_API_KEY=... scripts/live-genesis-private-read-smoke.sh

set -Eeuo pipefail

BASE_URL="${TEMPER_URL:-http://127.0.0.1:3188}"
BASE_URL="${BASE_URL%/}"
TENANT="${TEMPER_TENANT:-default}"
RUN_ID="${RUN_ID:-$(date +%H%M%S)}"
OWNER="pr-owner-${RUN_ID}"
SCHEME="${BASE_URL%%://*}"
HOST_PORT="${BASE_URL#*://}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/genesis-private-read.XXXXXX")"
export GIT_TERMINAL_PROMPT=0 GIT_ASKPASS=/usr/bin/false

case "$HOST_PORT" in
  127.0.0.1*|localhost*) ;;
  *) printf 'refusing: %s is not a local Genesis\n' "$BASE_URL" >&2; exit 2 ;;
esac

cleanup() {
  local status=$?
  if [[ "$status" -ne 0 ]]; then
    printf 'FAIL; evidence in %s\n' "$TMP_DIR" >&2
  else
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT

pass() { printf 'PASS %s\n' "$1"; }
fail() { printf 'FAIL %s\n' "$1" >&2; exit 1; }

sha256_hex() { printf '%s' "$1" | shasum -a 256 | cut -d' ' -f1; }

admin_headers=(
  -H "Content-Type: application/json"
  -H "Accept: application/json"
  -H "X-Tenant-Id: ${TENANT}"
  ${TEMPER_API_KEY:+-H "Authorization: Bearer ${TEMPER_API_KEY}"}
)

# admin <method> <path> <body-or-empty> -> prints HTTP status, body in $TMP_DIR/last.json
admin() {
  local -a args=(-sS -o "$TMP_DIR/last.json" -w "%{http_code}" -X "$1" "${admin_headers[@]}")
  [[ -n "$3" ]] && args+=(-d "$3")
  curl "${args[@]}" "${BASE_URL}$2"
}

must() {
  local status
  status="$(admin "$@")"
  [[ "$status" == 2* ]] || { sed -n '1,40p' "$TMP_DIR/last.json" >&2; fail "$1 $2 -> HTTP $status"; }
}

ensure_endpoint() {
  local id="$1" body="$2"
  if [[ "$(admin GET "/tdata/HttpEndpoints('${id}')" "")" == "200" ]]; then
    must PATCH "/tdata/HttpEndpoints('${id}')" "$body"
  else
    must POST "/tdata/HttpEndpoints" "$body"
  fi
}

register_endpoints() {
  ensure_endpoint he-info-refs '{"Id":"he-info-refs","PathPrefix":"/{owner}/{repo}.git/info/refs","Methods":"GET","IntegrationModule":"git_refs_advertise","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint he-upload-pack '{"Id":"he-upload-pack","PathPrefix":"/{owner}/{repo}.git/git-upload-pack","Methods":"POST","IntegrationModule":"git_upload_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":100000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728}'
  ensure_endpoint he-receive-pack '{"Id":"he-receive-pack","PathPrefix":"/{owner}/{repo}.git/git-receive-pack","Methods":"POST","IntegrationModule":"git_receive_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":20000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728,"ActionBridgeEntityType":"Repository","ActionBridgeEntityId":"rp-{owner}-{repo}","ActionBridgeAction":"IngestPack","ActionBridgeResponse":"git-receive-pack"}'
  ensure_endpoint he-api-user-repos '{"Id":"he-api-user-repos","PathPrefix":"/api/v3/user/repos","Methods":"POST","IntegrationModule":"github_rest_repos","RequiresAuth":false,"TimeoutSecs":60}'
  ensure_endpoint he-api-repo '{"Id":"he-api-repo","PathPrefix":"/api/v3/repos/{owner}/{repo}","Methods":"GET","IntegrationModule":"github_rest_repos","RequiresAuth":false,"TimeoutSecs":60}'
}

# mint <row-id> <principal> <secret> <scopes> [repositories]
mint() {
  local repos=""
  [[ -n "${5:-}" ]] && repos=",\"Repositories\":\"$5\""
  must POST "/tdata/GitTokens" "{\"Id\":\"$1\",\"PrincipalId\":\"$2\",\"HashedSecret\":\"$(sha256_hex "$3")\",\"KeyPrefix\":\"${3:0:8}\",\"Scopes\":\"$4\",\"ExpiresAt\":\"2030-01-01T00:00:00Z\"${repos}}"
}

url() { # url <repo> [token]
  if [[ -n "${2:-}" ]]; then
    printf '%s://%s:x@%s/%s/%s.git' "$SCHEME" "$2" "$HOST_PORT" "$OWNER" "$1"
  else
    printf '%s://%s/%s/%s.git' "$SCHEME" "$HOST_PORT" "$OWNER" "$1"
  fi
}

create_and_seed() { # create_and_seed <repo> <visibility>
  local status
  status="$(curl -sS -o "$TMP_DIR/create.json" -w "%{http_code}" -X POST \
    -H "Authorization: token ${T_OWNER}" -H "Content-Type: application/json" \
    -d "{\"name\":\"$1\",\"visibility\":\"$2\"}" "${BASE_URL}/api/v3/user/repos")"
  [[ "$status" == "201" ]] || { cat "$TMP_DIR/create.json" >&2; fail "create $1 -> $status"; }
  local src="$TMP_DIR/src-$1"
  git init -q -b main "$src"
  git -C "$src" -c user.email=s@genesis.local -c user.name=smoke commit -q --allow-empty -m "seed $1 ${RUN_ID}"
  git -C "$src" push -q "$(url "$1" "$T_OWNER")" main
  git -C "$src" rev-parse HEAD
}

# clone_as <label> <repo> <token-or-empty> <want: ok|fail> [stderr-pattern]
clone_as() {
  local dest="$TMP_DIR/clone-$1" err="$TMP_DIR/err-$1"
  if git clone -q "$(url "$2" "$3")" "$dest" 2>"$err"; then
    [[ "$4" == ok ]] || fail "$1: clone of $2 succeeded, expected refusal"
  else
    [[ "$4" == fail ]] || { cat "$err" >&2; fail "$1: clone of $2 failed"; }
    if [[ -n "${5:-}" ]] && ! grep -qi "$5" "$err"; then
      cat "$err" >&2; fail "$1: refusal did not mention '$5'"
    fi
  fi
}

upload_pack_status() { # raw POST, no credential, a want for <sha>
  local body
  body="$(printf '0032want %s\n00000009done\n' "$2")"
  curl -sS -o "$TMP_DIR/up.bin" -w "%{http_code}" -X POST \
    -H "Content-Type: application/x-git-upload-pack-request" \
    --data-binary "$body" "${BASE_URL}/${OWNER}/$1.git/git-upload-pack"
}

register_endpoints
T_OWNER="$(openssl rand -hex 20)"; T_OTHER="$(openssl rand -hex 20)"
T_SCOPED="$(openssl rand -hex 20)"; T_ADMIN="$(openssl rand -hex 20)"
mint "gt-owner-${RUN_ID}" "$OWNER" "$T_OWNER" "repo:read,repo:write"
mint "gt-other-${RUN_ID}" "pr-other-${RUN_ID}" "$T_OTHER" "repo:read,repo:write"
mint "gt-scoped-${RUN_ID}" "pr-ci-${RUN_ID}" "$T_SCOPED" "repo:read" "${OWNER}/priv"
mint "gt-admin-${RUN_ID}" "pr-admin-${RUN_ID}" "$T_ADMIN" "repo:read,admin:repos"
pass "endpoints + 4 tokens (owner, other, scoped to ${OWNER}/priv, admin:repos)"

PUB_SHA="$(create_and_seed pub public)"
PRIV_SHA="$(create_and_seed priv private)"
create_and_seed priv2 private >/dev/null
pass "created ${OWNER}/{pub,priv,priv2} and pushed as the owner"

clone_as anon-pub pub "" ok
[[ "$(git -C "$TMP_DIR/clone-anon-pub" rev-parse HEAD)" == "$PUB_SHA" ]] || fail "public clone tip mismatch"
pass "anonymous clone of a public repository"

clone_as anon-priv priv "" fail "Authentication failed\|could not read Username\|terminal prompts disabled"
status="$(curl -sS -o /dev/null -w "%{http_code}" "${BASE_URL}/${OWNER}/priv.git/info/refs?service=git-upload-pack")"
[[ "$status" == "401" ]] || fail "anonymous info/refs on private -> $status, want 401"
status="$(upload_pack_status priv "$PRIV_SHA")"
[[ "$status" == "401" ]] || fail "anonymous upload-pack on private -> $status, want 401"
status="$(curl -sS -o /dev/null -w "%{http_code}" "${BASE_URL}/${OWNER}/no-such-${RUN_ID}.git/info/refs?service=git-upload-pack")"
[[ "$status" == "401" ]] || fail "anonymous info/refs on a missing repository -> $status, want 401"
pass "anonymous clone, info/refs and upload-pack of a private repository refused (401)"

clone_as owner-priv priv "$T_OWNER" ok
[[ "$(git -C "$TMP_DIR/clone-owner-priv" rev-parse HEAD)" == "$PRIV_SHA" ]] || fail "owner clone tip mismatch"
clone_as admin-priv priv "$T_ADMIN" ok
clone_as scoped-priv priv "$T_SCOPED" ok
pass "owner, admin:repos and the scoped read token clone the private repository"

clone_as other-priv priv "$T_OTHER" fail "not found"
clone_as scoped-priv2 priv2 "$T_SCOPED" fail "not found"
pass "another owner's token and the scoped token on an unlisted repository get 404"

status="$(curl -sS -o /dev/null -w "%{http_code}" "${BASE_URL}/api/v3/repos/${OWNER}/priv")"
[[ "$status" == "404" ]] || fail "anonymous REST GET private -> $status, want 404"
status="$(curl -sS -o /dev/null -w "%{http_code}" -H "Authorization: token ${T_OTHER}" "${BASE_URL}/api/v3/repos/${OWNER}/priv")"
[[ "$status" == "404" ]] || fail "other-token REST GET private -> $status, want 404"
status="$(curl -sS -o /dev/null -w "%{http_code}" -H "Authorization: token ${T_OWNER}" "${BASE_URL}/api/v3/repos/${OWNER}/priv")"
[[ "$status" == "200" ]] || fail "owner REST GET private -> $status, want 200"
pass "REST repository read follows the same rule"

must POST "/tdata/Repositories('rp-${OWNER}-pub')/Temper.Git.SetVisibility" '{"Visibility":"private"}'
clone_as anon-pub-flipped pub "" fail "Authentication failed\|could not read Username\|terminal prompts disabled"
must POST "/tdata/Repositories('rp-${OWNER}-pub')/Temper.Git.SetVisibility" '{"Visibility":"public"}'
clone_as anon-pub-back pub "" ok
pass "SetVisibility makes a repository private and public again"

printf 'ALL PASS (run %s)\n' "$RUN_ID"
