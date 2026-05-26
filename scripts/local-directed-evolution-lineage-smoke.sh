#!/usr/bin/env bash
set -Eeuo pipefail

# Local lineage proof for the directed-evolution demo.
#
# Requires a running Temper server with the Genesis (temper-git) app installed.
# This publishes the Agent Answers seed as a normal Genesis app, advances it
# through two Temper-native schema mutations, installs the selected pinned
# release, and proves the new behavior through OData actions.

BASE_URL="${TEMPER_URL:-http://127.0.0.1:3232}"
BASE_URL="${BASE_URL%/}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEMPER_CARGO_MANIFEST="${TEMPER_CARGO_MANIFEST:-${REPO_ROOT}/../temper-codex-directed-evolution/Cargo.toml}"
TENANT="${TEMPER_TENANT:-default}"
RUN_ID="${RUN_ID:-$(date +%H%M%S)}"
OWNER="${OWNER:-directed-evolution-proof}"
REPO="${REPO:-agent-answers-${RUN_ID}}"
REPO_ID="rp-${OWNER}-${REPO}"
APP_ID="app-${OWNER}-${REPO}"
REMOTE="${BASE_URL}/${OWNER}/${REPO}.git"
EVALUATOR_REPO="${REPO}-evaluation"
EVALUATOR_REPO_ID="rp-${OWNER}-${EVALUATOR_REPO}"
EVALUATOR_APP_ID="app-${OWNER}-${EVALUATOR_REPO}"
EVALUATOR_REMOTE="${BASE_URL}/${OWNER}/${EVALUATOR_REPO}.git"
TARGET_TENANT="${TARGET_TENANT:-evolution-subject-${RUN_ID}}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/directed-evolution-lineage.XXXXXX")"

headers=(
  -H 'Content-Type: application/json'
  -H 'Accept: application/json'
  -H 'X-Temper-Principal-Kind: admin'
  -H 'X-Temper-Principal-Id: evolution-proof'
  -H 'X-Temper-Principal-Scopes: admin:repos admin:owners repo:write pr:write'
)
system_headers=(-H 'X-Temper-Agent-Type: system')

json_escape() { node -e 'process.stdout.write(JSON.stringify(process.argv[1]))' "$1"; }

post_json() {
  local tenant="$1" path="$2" body="$3" out="$4" status
  status="$(curl -sS -o "$out" -w '%{http_code}' -X POST "${headers[@]}" -H "X-Tenant-Id: ${tenant}" -d "$body" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'POST %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,160p' "$out" >&2
    exit 1
  fi
}

post_json_system() {
  local tenant="$1" path="$2" body="$3" out="$4" status
  status="$(curl -sS -o "$out" -w '%{http_code}' -X POST "${headers[@]}" "${system_headers[@]}" -H "X-Tenant-Id: ${tenant}" -d "$body" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'POST %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,160p' "$out" >&2
    exit 1
  fi
}

get_json() {
  local tenant="$1" path="$2" out="$3" status
  status="$(curl -sS -o "$out" -w '%{http_code}' "${headers[@]}" -H "X-Tenant-Id: ${tenant}" "${BASE_URL}${path}")"
  if [[ "$status" != 2* ]]; then
    printf 'GET %s failed with HTTP %s\n' "$path" "$status" >&2
    sed -n '1,160p' "$out" >&2
    exit 1
  fi
}

ensure_endpoint() {
  local endpoint_id="$1" body="$2" status
  local out="${TMP_DIR}/${endpoint_id}.json"
  status="$(curl -sS -o "$out" -w '%{http_code}' -H "X-Tenant-Id: ${TENANT}" "${BASE_URL}/tdata/HttpEndpoints('${endpoint_id}')")"
  if [[ "$status" == 200 ]]; then
    status="$(curl -sS -o "$out" -w '%{http_code}' -X PATCH -H "X-Tenant-Id: ${TENANT}" -H 'Content-Type: application/json' -d "$body" "${BASE_URL}/tdata/HttpEndpoints('${endpoint_id}')")"
    [[ "$status" == 2* ]] || { sed -n '1,160p' "$out" >&2; exit 1; }
    return
  fi
  post_json "$TENANT" /tdata/HttpEndpoints "$body" "$out"
}

verify_specs() {
  cargo run -q --manifest-path "$TEMPER_CARGO_MANIFEST" -p temper-cli -- verify --specs-dir "$1/specs" >/dev/null
}

add_generation_one_mutation() {
  local app_dir="$1"
  node - "$app_dir" <<'NODE'
const fs = require('fs');
const dir = process.argv[2];
const spec = `${dir}/specs/answer.ioa.toml`;
let answer = fs.readFileSync(spec, 'utf8');
answer = answer.replace('[[state]]\nname = "created_at"', '[[state]]\nname = "citation_requirement"\ntype = "string"\ninitial = ""\n[[state]]\nname = "created_at"');
answer = answer.replace('"evidence", "created_at"]', '"evidence", "citation_requirement", "created_at"]');
fs.writeFileSync(spec, answer);
const model = `${dir}/specs/model.csdl.xml`;
let xml = fs.readFileSync(model, 'utf8');
xml = xml.replace('<Property Name="Evidence" Type="Edm.String"/><Property Name="CreatedAt"', '<Property Name="Evidence" Type="Edm.String"/><Property Name="CitationRequirement" Type="Edm.String"/><Property Name="CreatedAt"');
xml = xml.replace('<Parameter Name="evidence" Type="Edm.String"/><Parameter Name="created_at"', '<Parameter Name="evidence" Type="Edm.String"/><Parameter Name="citation_requirement" Type="Edm.String"/><Parameter Name="created_at"');
fs.writeFileSync(model, xml);
fs.writeFileSync(`${dir}/adrs/0002-cited-answers.md`, '# ADR 0002: Cited answers\n\nThis generation exposes the citation requirement on submitted answers so validation can reward inspectable evidence.\n');
NODE
}

add_generation_two_mutation() {
  local app_dir="$1"
  node - "$app_dir" <<'NODE'
const fs = require('fs');
const dir = process.argv[2];
const spec = `${dir}/specs/answer.ioa.toml`;
let answer = fs.readFileSync(spec, 'utf8');
answer = answer.replace('[[action]]\nname = "Submit"', '[[state]]\nname = "reuse_count"\ntype = "counter"\ninitial = 0\n\n[[action]]\nname = "Submit"');
answer += '\n[[action]]\nname = "RecordReuse"\nkind = "input"\nfrom = ["Published", "Accepted"]\nto = "Published"\nparams = []\neffect = [{ type = "increment", var = "reuse_count" }]\n';
fs.writeFileSync(spec, answer);
const model = `${dir}/specs/model.csdl.xml`;
let xml = fs.readFileSync(model, 'utf8');
xml = xml.replace('<Property Name="CreatedAt" Type="Edm.String"/>', '<Property Name="CreatedAt" Type="Edm.String"/><Property Name="ReuseCount" Type="Edm.Int64"/>');
xml = xml.replace('<Action Name="Accept" IsBound="true"><Parameter Name="bindingParameter" Type="Genesis.AgentAnswers.Answer"/></Action>', '<Action Name="Accept" IsBound="true"><Parameter Name="bindingParameter" Type="Genesis.AgentAnswers.Answer"/></Action><Action Name="RecordReuse" IsBound="true"><Parameter Name="bindingParameter" Type="Genesis.AgentAnswers.Answer"/></Action>');
fs.writeFileSync(model, xml);
const policy = `${dir}/policies/agent_answers.cedar`;
let cedar = fs.readFileSync(policy, 'utf8');
cedar = cedar.replace('Action::"Submit", Action::"Accept"', 'Action::"Submit", Action::"Accept", Action::"RecordReuse"');
fs.writeFileSync(policy, cedar);
fs.writeFileSync(`${dir}/adrs/0003-observed-reuse.md`, '# ADR 0003: Observed reuse\n\nThis generation records validated answer reuse as behavior that emerged from successor traffic.\n');
NODE
}

printf 'Preparing Genesis smart HTTP endpoints at %s\n' "$BASE_URL"
ensure_endpoint he-info-refs '{"Id":"he-info-refs","PathPrefix":"/{owner}/{repo}.git/info/refs","Methods":"GET","IntegrationModule":"git_refs_advertise","RequiresAuth":false,"TimeoutSecs":60}'
ensure_endpoint he-upload-pack '{"Id":"he-upload-pack","PathPrefix":"/{owner}/{repo}.git/git-upload-pack","Methods":"POST","IntegrationModule":"git_upload_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":20000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728}'
ensure_endpoint he-receive-pack '{"Id":"he-receive-pack","PathPrefix":"/{owner}/{repo}.git/git-receive-pack","Methods":"POST","IntegrationModule":"git_receive_pack","RequiresAuth":false,"TimeoutSecs":300,"MaxFuel":20000000000,"MaxMemory":536870912,"MaxResponseBytes":134217728,"ActionBridgeEntityType":"Repository","ActionBridgeEntityId":"rp-{owner}-{repo}","ActionBridgeAction":"IngestPack","ActionBridgeResponse":"git-receive-pack"}'

printf 'Creating repository and seed bundle for %s/%s\n' "$OWNER" "$REPO"
post_json "$TENANT" /tdata/Repositories "{\"Id\":$(json_escape "$REPO_ID"),\"OwnerAccountId\":$(json_escape "$OWNER"),\"Name\":$(json_escape "$REPO"),\"Description\":\"Directed evolution lineage proof\",\"DefaultBranch\":\"main\",\"Visibility\":\"public\"}" "$TMP_DIR/repository.json"
post_json_system "$TENANT" "/tdata/Repositories('${REPO_ID}')/Temper.Git.MarkProvisioned" "{\"LibsqlDbName\":$(json_escape "${REPO_ID}.db")}" "$TMP_DIR/provision.json"
APP_DIR="$TMP_DIR/app"
mkdir -p "$APP_DIR"
cp -R "$REPO_ROOT/apps/agent-answers/." "$APP_DIR/"
git -C "$APP_DIR" init -b main >/dev/null
git -C "$APP_DIR" config user.email 'codex@directed-evolution.local'
git -C "$APP_DIR" config user.name 'Codex directed evolution proof'
verify_specs "$APP_DIR"
git -C "$APP_DIR" add .
git -C "$APP_DIR" commit -m 'Seed Agent Answers organism' >/dev/null
SEED_SHA="$(git -C "$APP_DIR" rev-parse HEAD)"
git -C "$APP_DIR" push "$REMOTE" main >/dev/null
post_json "$TENANT" "/tdata/Apps('${APP_ID}')/Temper.Git.RegisterNewApp?await_integration=true" "{\"Name\":$(json_escape "$REPO"),\"RepositoryId\":$(json_escape "$REPO_ID"),\"Description\":\"Agent Answers evolution organism\",\"Exports\":\"{}\",\"Visibility\":\"public\"}" "$TMP_DIR/register.json"

printf 'Publishing generation one: explicit evidence requirement\n'
add_generation_one_mutation "$APP_DIR"
verify_specs "$APP_DIR"
git -C "$APP_DIR" add .
git -C "$APP_DIR" commit -m 'Generation 1: expose citation requirement' >/dev/null
GEN_ONE_SHA="$(git -C "$APP_DIR" rev-parse HEAD)"
git -C "$APP_DIR" push "$REMOTE" main >/dev/null
post_json "$TENANT" "/tdata/Apps('${APP_ID}')/Temper.Git.PublishNewVersion?await_integration=true" "{\"NewHash\":$(json_escape "$GEN_ONE_SHA"),\"RefName\":\"main\"}" "$TMP_DIR/gen-one.json"

printf 'Publishing generation two: measure observed reuse\n'
add_generation_two_mutation "$APP_DIR"
verify_specs "$APP_DIR"
git -C "$APP_DIR" add .
git -C "$APP_DIR" commit -m 'Generation 2: record validated answer reuse' >/dev/null
GEN_TWO_SHA="$(git -C "$APP_DIR" rev-parse HEAD)"
git -C "$APP_DIR" push "$REMOTE" main >/dev/null
post_json "$TENANT" "/tdata/Apps('${APP_ID}')/Temper.Git.PublishNewVersion?await_integration=true" "{\"NewHash\":$(json_escape "$GEN_TWO_SHA"),\"RefName\":\"main\"}" "$TMP_DIR/gen-two.json"

SEED_REF="${OWNER}/${REPO}@${SEED_SHA}"
GEN_ONE_REF="${OWNER}/${REPO}@${GEN_ONE_SHA}"
GEN_TWO_REF="${OWNER}/${REPO}@${GEN_TWO_SHA}"

printf 'Publishing independent frozen evaluator bundle\n'
post_json "$TENANT" /tdata/Repositories "{\"Id\":$(json_escape "$EVALUATOR_REPO_ID"),\"OwnerAccountId\":$(json_escape "$OWNER"),\"Name\":$(json_escape "$EVALUATOR_REPO"),\"Description\":\"Directed evolution evaluator proof\",\"DefaultBranch\":\"main\",\"Visibility\":\"public\"}" "$TMP_DIR/evaluator-repository.json"
post_json_system "$TENANT" "/tdata/Repositories('${EVALUATOR_REPO_ID}')/Temper.Git.MarkProvisioned" "{\"LibsqlDbName\":$(json_escape "${EVALUATOR_REPO_ID}.db")}" "$TMP_DIR/evaluator-provision.json"
EVALUATOR_DIR="$TMP_DIR/evaluator"
mkdir -p "$EVALUATOR_DIR"
cp -R "$REPO_ROOT/apps/agent-answers-evaluation/." "$EVALUATOR_DIR/"
verify_specs "$EVALUATOR_DIR"
git -C "$EVALUATOR_DIR" init -b main >/dev/null
git -C "$EVALUATOR_DIR" config user.email 'codex@directed-evolution.local'
git -C "$EVALUATOR_DIR" config user.name 'Codex directed evolution proof'
git -C "$EVALUATOR_DIR" add .
git -C "$EVALUATOR_DIR" commit -m 'Freeze Agent Answers evaluator' >/dev/null
EVALUATOR_SHA="$(git -C "$EVALUATOR_DIR" rev-parse HEAD)"
git -C "$EVALUATOR_DIR" push "$EVALUATOR_REMOTE" main >/dev/null
post_json "$TENANT" "/tdata/Apps('${EVALUATOR_APP_ID}')/Temper.Git.RegisterNewApp?await_integration=true" "{\"Name\":$(json_escape "$EVALUATOR_REPO"),\"RepositoryId\":$(json_escape "$EVALUATOR_REPO_ID"),\"Description\":\"Frozen Agent Answers evaluator\",\"Exports\":\"{}\",\"Visibility\":\"public\"}" "$TMP_DIR/evaluator-register.json"
EVALUATOR_REF="${OWNER}/${EVALUATOR_REPO}@${EVALUATOR_SHA}"

printf 'Installing selected pinned release %s\n' "$GEN_TWO_REF"
post_json "$TENANT" "/tdata/Apps('${APP_ID}')/App.Install?await_integration=true" "{\"TargetTenant\":$(json_escape "$TARGET_TENANT"),\"AppRef\":$(json_escape "$GEN_TWO_REF"),\"Installer\":\"directed-evolution-proof\"}" "$TMP_DIR/install.json"
post_json "$TENANT" "/tdata/Apps('${EVALUATOR_APP_ID}')/App.Install?await_integration=true" "{\"TargetTenant\":$(json_escape "$TARGET_TENANT"),\"AppRef\":$(json_escape "$EVALUATOR_REF"),\"Installer\":\"directed-evolution-proof\"}" "$TMP_DIR/evaluator-install.json"

QUESTION_ID="question-${RUN_ID}"
ANSWER_ID="answer-${RUN_ID}"
post_json "$TARGET_TENANT" /tdata/Questions "{\"Id\":$(json_escape "$QUESTION_ID")}" "$TMP_DIR/question-create.json"
post_json "$TARGET_TENANT" "/tdata/Questions('${QUESTION_ID}')/Genesis.AgentAnswers.Configure" '{"title":"How can an agent cite its evidence?","body":"Need an inspectable answer.","asked_by":"successor","created_at":"proof"}' "$TMP_DIR/question-configure.json"
post_json "$TARGET_TENANT" /tdata/Answers "{\"Id\":$(json_escape "$ANSWER_ID")}" "$TMP_DIR/answer-create.json"
post_json "$TARGET_TENANT" "/tdata/Answers('${ANSWER_ID}')/Genesis.AgentAnswers.Submit" "{\"question_id\":$(json_escape "$QUESTION_ID"),\"body\":\"Include an evidence locator.\",\"answered_by\":\"pioneer\",\"evidence\":\"temper://proof\",\"citation_requirement\":\"required\",\"created_at\":\"proof\"}" "$TMP_DIR/answer-submit.json"
post_json "$TARGET_TENANT" "/tdata/Answers('${ANSWER_ID}')/Genesis.AgentAnswers.RecordReuse" '{}' "$TMP_DIR/answer-reuse.json"
get_json "$TARGET_TENANT" "/tdata/Answers('${ANSWER_ID}')" "$TMP_DIR/answer.json"
node - "$TMP_DIR/answer.json" <<'NODE'
const fs = require('fs');
const row = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const count = row.fields?.reuse_count ?? row.fields?.ReuseCount ?? row.ReuseCount;
if (Number(count) !== 1) throw new Error(`expected reuse_count=1, got ${count}`);
NODE

cat > "$TMP_DIR/proof.env" <<EOF
EVOLUTION_SUBJECT_SEED_REF=${SEED_REF}
EVOLUTION_EVALUATOR_REF=${EVALUATOR_REF}
EVOLUTION_GENERATION_ONE_REF=${GEN_ONE_REF}
EVOLUTION_GENERATION_TWO_REF=${GEN_TWO_REF}
EVOLUTION_INSTALLED_TENANT=${TARGET_TENANT}
EOF
printf 'PASS directed evolution native lineage proof\n'
printf '  proof_env: %s\n' "$TMP_DIR/proof.env"
printf '  seed_ref: %s\n' "$SEED_REF"
printf '  generation_one_ref: %s\n' "$GEN_ONE_REF"
printf '  generation_two_ref: %s\n' "$GEN_TWO_REF"
printf '  evaluator_ref: %s\n' "$EVALUATOR_REF"
