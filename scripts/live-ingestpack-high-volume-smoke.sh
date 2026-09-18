#!/usr/bin/env bash
set -euo pipefail

# High-volume live smoke for Repository.IngestPack.
#
# Requires a running Temper server with the temper-git app installed:
#
#   TEMPER_URL=http://127.0.0.1:3137 \
#     scripts/live-ingestpack-high-volume-smoke.sh
#
# The smoke creates FILE_COUNT unique files, pushes a base commit, then pushes a
# closely related second commit. The focused module test proves Git's external
# REF_DELTA selection; this live smoke verifies the same related update through
# the rebuilt WASM, both object identities, the moved ref, and a clone.

BASE_URL="${TEMPER_URL:-http://127.0.0.1:3000}"
BASE_URL="${BASE_URL%/}"
TENANT="${TEMPER_TENANT:-default}"
PRINCIPAL_ID="${TEMPER_PRINCIPAL_ID:-operator}"
FILE_COUNT="${FILE_COUNT:-1000}"
RUN_ID="${RUN_ID:-$(date +%s)-$$}"
OWNER="stress-${RUN_ID}"
REPO="ingestpack-${RUN_ID}"
REPO_ID="rp-${OWNER}-${REPO}"
REF_ID="rf-${REPO_ID}-refs-heads-main"
SCHEME="${BASE_URL%%://*}"
HOST_PORT="${BASE_URL#*://}"

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/temper-ingestpack-stress.XXXXXX")"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

api_headers=(
  -H "X-Tenant-Id: ${TENANT}"
  -H "X-Temper-Principal-Kind: admin"
  -H "X-Temper-Principal-Id: ${PRINCIPAL_ID}"
  -H "X-Temper-Principal-Scopes: admin:repos admin:tokens repo:write pr:write"
  -H "X-Temper-Agent-Type: admin"
  -H "Accept: application/json"
)
json_headers=("${api_headers[@]}" -H "Content-Type: application/json")
system_headers=(
  -H "X-Tenant-Id: ${TENANT}"
  -H "X-Temper-Principal-Kind: admin"
  -H "X-Temper-Principal-Id: ${PRINCIPAL_ID}"
  -H "X-Temper-Agent-Type: system"
  -H "X-Temper-Principal-Scopes: admin:repos repo:write pr:write"
  -H "Accept: application/json"
  -H "Content-Type: application/json"
)

json_escape() {
  node -e 'process.stdout.write(JSON.stringify(process.argv[1]))' "$1"
}

sha256_hex() {
  if command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "$1" | sha256sum | cut -d' ' -f1
  else
    printf '%s' "$1" | shasum -a 256 | cut -d' ' -f1
  fi
}

urlencode() {
  node -e 'process.stdout.write(encodeURIComponent(process.argv[1]))' "$1"
}

post_json() {
  local path="$1"
  local body="$2"
  local out="$TMP_DIR/post-response.json"
  local status
  status="$(curl -sS -o "$out" -w "%{http_code}" -X POST "${json_headers[@]}" -d "$body" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'POST %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,160p' "$out" >&2
    exit 1
  fi
}

post_json_system() {
  local path="$1"
  local body="$2"
  local out="$TMP_DIR/post-system-response.json"
  local status
  status="$(curl -sS -o "$out" -w "%{http_code}" -X POST "${system_headers[@]}" -d "$body" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'POST %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,160p' "$out" >&2
    exit 1
  fi
}

entity_exists() {
  local set_name="$1"
  local entity_id="$2"
  curl -fsS "${api_headers[@]}" "${BASE_URL}/tdata/${set_name}('${entity_id}')" >/dev/null 2>&1
}

ensure_endpoint() {
  local id="$1"
  local body="$2"
  if entity_exists "HttpEndpoints" "$id"; then
    curl -fsS -X PATCH "${api_headers[@]}" -d "$body" "${BASE_URL}/tdata/HttpEndpoints('${id}')" >/dev/null
    return
  fi
  post_json "/tdata/HttpEndpoints" "$body"
}

field_from_entity() {
  local set_name="$1"
  local entity_id="$2"
  local field_name="$3"
  local body="$TMP_DIR/entity-${set_name}-${field_name}.json"
  curl -fsS "${api_headers[@]}" "${BASE_URL}/tdata/${set_name}('${entity_id}')" > "$body"
  node -e '
    const fs = require("fs");
    const row = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    const field = process.argv[2];
    const value = (row.fields && row.fields[field]) ?? row[field] ?? "";
    process.stdout.write(String(value));
  ' "$body" "$field_name"
}

collection_count_for_repo() {
  local set_name="$1"
  local filter
  local top="$(( FILE_COUNT + 100 ))"
  local body="$TMP_DIR/${set_name}.json"
  filter="$(urlencode "RepositoryId eq '${REPO_ID}'")"
  curl -fsS "${api_headers[@]}" "${BASE_URL}/tdata/${set_name}?\$filter=${filter}&\$top=${top}" > "$body"
  node -e '
    const fs = require("fs");
    const body = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
    process.stdout.write(String(Array.isArray(body.value) ? body.value.length : 0));
  ' "$body"
}

object_key_prefix() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//'
}

printf 'Seeding smart-HTTP endpoints for %s\n' "$BASE_URL"
INFO_REFS_ENDPOINT_ID="he-info-refs-${RUN_ID}"
INFO_REFS_PREFIX="/${OWNER}/${REPO}.git/info/refs"
ensure_endpoint "$INFO_REFS_ENDPOINT_ID" \
  "{\"Id\":$(json_escape "$INFO_REFS_ENDPOINT_ID"),\"PathPrefix\":$(json_escape "$INFO_REFS_PREFIX"),\"Methods\":\"GET\",\"IntegrationModule\":\"git_refs_advertise\",\"RequiresAuth\":false,\"TimeoutSecs\":60}"
ensure_endpoint "he-info-refs" \
  '{"Id":"he-info-refs","PathPrefix":"/{owner}/{repo}.git/info/refs","Methods":"GET","IntegrationModule":"git_refs_advertise","RequiresAuth":false,"TimeoutSecs":60}'
ensure_endpoint "he-upload-pack" \
  '{"Id":"he-upload-pack","PathPrefix":"/{owner}/{repo}.git/git-upload-pack","Methods":"POST","IntegrationModule":"git_upload_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":100000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728}'
ensure_endpoint "he-receive-pack" \
  '{"Id":"he-receive-pack","PathPrefix":"/{owner}/{repo}.git/git-receive-pack","Methods":"POST","IntegrationModule":"git_receive_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":20000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728,"ActionBridgeEntityType":"Repository","ActionBridgeEntityId":"rp-{owner}-{repo}","ActionBridgeAction":"IngestPack","ActionBridgeResponse":"git-receive-pack"}'

TOKEN_SECRET="$(openssl rand -hex 20)"
TOKEN_HASH="$(sha256_hex "$TOKEN_SECRET")"
post_json "/tdata/GitTokens" \
  "{\"Id\":\"gt-${RUN_ID}\",\"PrincipalId\":$(json_escape "$OWNER"),\"HashedSecret\":$(json_escape "$TOKEN_HASH"),\"KeyPrefix\":$(json_escape "${TOKEN_SECRET:0:8}"),\"Scopes\":\"repo:read,repo:write\",\"ExpiresAt\":\"2030-01-01T00:00:00Z\"}"
AUTH_BASIC="$(printf '%s:x' "$TOKEN_SECRET" | base64 | tr -d '\n')"
GIT_AUTH_HEADER="Authorization: Basic ${AUTH_BASIC}"
REMOTE="${BASE_URL}/${OWNER}/${REPO}.git"
OBJECT_KEY_PREFIX="$(object_key_prefix "$REPO_ID")"

printf 'Creating Repository %s\n' "$REPO_ID"
post_json "/tdata/Repositories" \
  "{\"Id\":$(json_escape "$REPO_ID"),\"OwnerAccountId\":$(json_escape "$OWNER"),\"Name\":$(json_escape "$REPO"),\"Description\":\"IngestPack high-volume smoke\",\"DefaultBranch\":\"main\",\"Visibility\":\"public\"}"

post_json_system "/tdata/Repositories('${REPO_ID}')/Temper.Git.MarkProvisioned" \
  "{\"LibsqlDbName\":$(json_escape "${REPO_ID}.db")}"

WORK="$TMP_DIR/work"
mkdir -p "$WORK/files"
git -C "$WORK" init -b main >/dev/null
git -C "$WORK" config user.email "stress@example.invalid"
git -C "$WORK" config user.name "IngestPack Stress"

printf 'Creating %s unique files\n' "$FILE_COUNT"
for i in $(seq 1 "$FILE_COUNT"); do
  printf 'stress file %04d for %s\n' "$i" "$RUN_ID" > "$WORK/files/file-$(printf '%04d' "$i").txt"
done
for line in $(seq 1 4096); do
  printf 'stable package payload line %04d: abcdefghijklmnopqrstuvwxyz0123456789\n' "$line" \
    >> "$WORK/files/file-0001.txt"
done
git -C "$WORK" add files
git -C "$WORK" commit -m "stress ${FILE_COUNT} files" >/dev/null
BASE_COMMIT_SHA="$(git -C "$WORK" rev-parse HEAD)"
BASE_BLOB_SHA="$(git -C "$WORK" rev-parse HEAD:files/file-0001.txt)"
PACK_OBJECTS="$(git -C "$WORK" rev-list --objects --all | wc -l | tr -d ' ')"

printf 'Pushing base with %s files (%s git objects) to %s\n' "$FILE_COUNT" "$PACK_OBJECTS" "$REMOTE"
start_ms="$(node -e 'process.stdout.write(String(Date.now()))')"
if ! git -C "$WORK" -c http.extraHeader="$GIT_AUTH_HEADER" push "$REMOTE" main > "$TMP_DIR/push.log" 2>&1; then
  sed -n '1,160p' "$TMP_DIR/push.log" >&2
  exit 1
fi

BASE_BLOB_ID="${OBJECT_KEY_PREFIX}-${BASE_BLOB_SHA}"
if ! entity_exists "Blobs" "$BASE_BLOB_ID"; then
  printf 'Base blob missing at repository-scoped identity %s\n' "$BASE_BLOB_ID" >&2
  exit 1
fi

printf 'bounded thin-pack update for %s\n' "$RUN_ID" >> "$WORK/files/file-0001.txt"
git -C "$WORK" add files/file-0001.txt
git -C "$WORK" commit -m "thin-pack update" >/dev/null
COMMIT_SHA="$(git -C "$WORK" rev-parse HEAD)"
TARGET_BLOB_SHA="$(git -C "$WORK" rev-parse HEAD:files/file-0001.txt)"
if ! git -C "$WORK" -c http.extraHeader="$GIT_AUTH_HEADER" push "$REMOTE" main >> "$TMP_DIR/push.log" 2>&1; then
  sed -n '1,240p' "$TMP_DIR/push.log" >&2
  exit 1
fi
end_ms="$(node -e 'process.stdout.write(String(Date.now()))')"
push_ms="$(( end_ms - start_ms ))"

TARGET_BLOB_ID="${OBJECT_KEY_PREFIX}-${TARGET_BLOB_SHA}"
if ! entity_exists "Blobs" "$TARGET_BLOB_ID"; then
  printf 'Expanded thin-pack blob missing at repository-scoped identity %s\n' "$TARGET_BLOB_ID" >&2
  exit 1
fi

TARGET_SHA="$(field_from_entity Refs "$REF_ID" TargetCommitSha)"
if [[ "$TARGET_SHA" != "$COMMIT_SHA" ]]; then
  printf 'Stored Ref target mismatch: got %s, expected %s\n' "$TARGET_SHA" "$COMMIT_SHA" >&2
  sed -n '1,160p' "$TMP_DIR/push.log" >&2
  exit 1
fi

BLOB_COUNT="$(collection_count_for_repo Blobs)"
COMMIT_COUNT="$(collection_count_for_repo Commits)"
TREE_COUNT="$(collection_count_for_repo Trees)"
EXPECTED_BLOB_COUNT="$(( FILE_COUNT + 1 ))"
if [[ "$BLOB_COUNT" -ne "$EXPECTED_BLOB_COUNT" ]]; then
  printf 'Expected %s Blob rows, got %s\n' "$EXPECTED_BLOB_COUNT" "$BLOB_COUNT" >&2
  exit 1
fi
if [[ "$COMMIT_COUNT" -lt 2 ]]; then
  printf 'Expected at least two Commit rows, got %s\n' "$COMMIT_COUNT" >&2
  exit 1
fi
if [[ "$TREE_COUNT" -lt 1 ]]; then
  printf 'Expected at least one Tree row, got %s\n' "$TREE_COUNT" >&2
  exit 1
fi

git -c http.extraHeader="$GIT_AUTH_HEADER" clone "$REMOTE" "$TMP_DIR/clone" > "$TMP_DIR/clone.log" 2>&1
CLONED_SHA="$(git -C "$TMP_DIR/clone" rev-parse HEAD)"
if [[ "$CLONED_SHA" != "$COMMIT_SHA" ]]; then
  printf 'Clone HEAD mismatch: got %s, expected %s\n' "$CLONED_SHA" "$COMMIT_SHA" >&2
  sed -n '1,160p' "$TMP_DIR/clone.log" >&2
  exit 1
fi
diff -qr "$WORK/files" "$TMP_DIR/clone/files" > "$TMP_DIR/diff.log"

printf 'PASS IngestPack high-volume smoke\n'
printf '  run: %s\n' "$RUN_ID"
printf '  repository: %s\n' "$REPO_ID"
printf '  files: %s\n' "$FILE_COUNT"
printf '  git objects: %s\n' "$PACK_OBJECTS"
printf '  push_ms: %s\n' "$push_ms"
printf '  blobs/commits/trees: %s/%s/%s\n' "$BLOB_COUNT" "$COMMIT_COUNT" "$TREE_COUNT"
printf '  base: %s -> %s\n' "$BASE_COMMIT_SHA" "$BASE_BLOB_ID"
printf '  thin-pack target: %s\n' "$TARGET_BLOB_ID"
printf '  ref: %s -> %s\n' "$REF_ID" "$TARGET_SHA"
