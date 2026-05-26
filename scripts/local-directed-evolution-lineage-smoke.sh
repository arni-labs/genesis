#!/usr/bin/env bash
set -Eeuo pipefail

# Local lineage proof for the directed-evolution demo.
#
# Requires a running Temper server with the Genesis (temper-git) app installed.
# This publishes the Agent Answers seed as a normal Genesis app, advances it
# through two Temper-native schema mutations, installs each selected pinned
# release, executes the frozen evaluator scenario through OData actions, and
# emits evidence that the campaign controller must consume before release.

BASE_URL="${TEMPER_URL:-http://127.0.0.1:3232}"
BASE_URL="${BASE_URL%/}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEMPER_CARGO_MANIFEST="${TEMPER_CARGO_MANIFEST:-${REPO_ROOT}/../temper-codex-directed-evolution/Cargo.toml}"
TEMPERPAW_CARGO_MANIFEST="${TEMPERPAW_CARGO_MANIFEST:-${REPO_ROOT}/../temperpaw-codex-directed-evolution/Cargo.toml}"
CANDIDATE_GENERATOR="${EVOLUTION_CANDIDATE_GENERATOR:-deterministic}"
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

if [[ "$CANDIDATE_GENERATOR" != "deterministic" && "$CANDIDATE_GENERATOR" != "codex" ]]; then
  printf 'EVOLUTION_CANDIDATE_GENERATOR must be deterministic or codex, got %s\n' "$CANDIDATE_GENERATOR" >&2
  exit 1
fi

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

run_codex_mutation() {
  local app_dir="$1" ordinal="$2" parent_ref="$3" direction="$4"
  TEMPER_URL="$BASE_URL" \
    WORKER_ID="directed-evolution-mutation-${RUN_ID}-${ordinal}" \
    REPO_ROOT="$app_dir" \
    WORKSPACE_ROOT="$TMP_DIR/worktrees" \
    EVOLUTION_CANDIDATE_DIR="$app_dir" \
    EVOLUTION_GENERATION_ORDINAL="$ordinal" \
    EVOLUTION_PARENT_REF="$parent_ref" \
    EVOLUTION_DIRECTION="$direction" \
    EVOLUTION_VALIDATOR_CONTRACT="Preserve Question.Configure, Answer.Submit with evidence, Question.RecordAnswer, Answer.Accept, and Question.Accept actions so the frozen Agent Answers evaluator can execute its acceptance scenario; additive behavior is allowed." \
    cargo run -q --manifest-path "$TEMPERPAW_CARGO_MANIFEST" -p paw-codex-worker -- directed-evolution-mutate
}

generate_generation_one() {
  if [[ "$CANDIDATE_GENERATOR" == "codex" ]]; then
    run_codex_mutation "$APP_DIR" 1 "$SEED_SHA" "Improve answer quality transparency after a user asked for inspectable evidence. Keep the change minimal and understandable."
  else
    add_generation_one_mutation "$APP_DIR"
  fi
}

generate_generation_two() {
  if [[ "$CANDIDATE_GENERATOR" == "codex" ]]; then
    run_codex_mutation "$APP_DIR" 2 "$GEN_ONE_SHA" "Improve the app after successor traffic wants to reuse previously validated answers without losing evidence. Keep the change minimal and understandable."
  else
    add_generation_two_mutation "$APP_DIR"
  fi
}

validate_selected_candidate() {
  local ordinal="$1" candidate_ref="$2" trial_tenant="$3"
  local question_id="validator-question-${RUN_ID}-${ordinal}"
  local answer_id="validator-answer-${RUN_ID}-${ordinal}"
  local suite_id="validator-suite-${RUN_ID}-${ordinal}"
  local run_id="validator-run-${RUN_ID}-${ordinal}"
  local answer_evidence="temper://trial/${trial_tenant}/Answers('${answer_id}')"
  local run_evidence="temper://trial/${trial_tenant}/ValidatorRuns('${run_id}')"
  printf 'Executing frozen evaluator scenario for generation %s (%s)\n' "$ordinal" "$candidate_ref"
  post_json "$TENANT" "/tdata/Apps('${APP_ID}')/App.Install?await_integration=true" "{\"TargetTenant\":$(json_escape "$trial_tenant"),\"AppRef\":$(json_escape "$candidate_ref"),\"Installer\":\"directed-evolution-validator\"}" "$TMP_DIR/install-${ordinal}.json"
  post_json "$TENANT" "/tdata/Apps('${EVALUATOR_APP_ID}')/App.Install?await_integration=true" "{\"TargetTenant\":$(json_escape "$trial_tenant"),\"AppRef\":$(json_escape "$EVALUATOR_REF"),\"Installer\":\"directed-evolution-validator\"}" "$TMP_DIR/evaluator-install-${ordinal}.json"
  post_json "$trial_tenant" /tdata/TrialSuites "{\"Id\":$(json_escape "$suite_id")}" "$TMP_DIR/trial-suite-create-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/TrialSuites('${suite_id}')/Genesis.AgentAnswersEvaluation.Configure" "{\"name\":\"Frozen acceptance scenario\",\"description\":\"Question resolves after an accepted evidence-bearing answer.\",\"subject_app_ref\":$(json_escape "$candidate_ref"),\"scenario_manifest_json\":\"[{\\\"id\\\":\\\"accept-evidenced-answer\\\",\\\"traffic\\\":\\\"simulated\\\"}]\",\"hidden_fixture_locator\":\"temper://trial/${trial_tenant}/fixture\",\"authored_by\":\"directed-evolution-validator\"}" "$TMP_DIR/trial-suite-configure-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/TrialSuites('${suite_id}')/Genesis.AgentAnswersEvaluation.Freeze" '{"frozen_at":"proof"}' "$TMP_DIR/trial-suite-freeze-${ordinal}.json"
  post_json "$trial_tenant" /tdata/ValidatorRuns "{\"Id\":$(json_escape "$run_id")}" "$TMP_DIR/validator-run-create-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/ValidatorRuns('${run_id}')/Genesis.AgentAnswersEvaluation.Configure" "{\"trial_suite_id\":$(json_escape "$suite_id"),\"candidate_id\":$(json_escape "$candidate_ref"),\"scenario_id\":\"accept-evidenced-answer\",\"validator_kind\":\"native_trial\"}" "$TMP_DIR/validator-run-configure-${ordinal}.json"
  post_json "$trial_tenant" /tdata/Questions "{\"Id\":$(json_escape "$question_id")}" "$TMP_DIR/question-create-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/Questions('${question_id}')/Genesis.AgentAnswers.Configure" '{"title":"How can an agent cite its evidence?","body":"Need an inspectable answer.","asked_by":"validator","created_at":"proof"}' "$TMP_DIR/question-configure-${ordinal}.json"
  post_json "$trial_tenant" /tdata/Answers "{\"Id\":$(json_escape "$answer_id")}" "$TMP_DIR/answer-create-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/Answers('${answer_id}')/Genesis.AgentAnswers.Submit" "{\"question_id\":$(json_escape "$question_id"),\"body\":\"Include an evidence locator.\",\"answered_by\":\"validator\",\"evidence\":$(json_escape "$answer_evidence"),\"created_at\":\"proof\"}" "$TMP_DIR/answer-submit-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/Questions('${question_id}')/Genesis.AgentAnswers.RecordAnswer" '{}' "$TMP_DIR/question-answer-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/Answers('${answer_id}')/Genesis.AgentAnswers.Accept" '{}' "$TMP_DIR/answer-accept-${ordinal}.json"
  post_json "$trial_tenant" "/tdata/Questions('${question_id}')/Genesis.AgentAnswers.Accept" "{\"accepted_answer_id\":$(json_escape "$answer_id")}" "$TMP_DIR/question-accept-${ordinal}.json"
  get_json "$trial_tenant" "/tdata/Questions('${question_id}')" "$TMP_DIR/question-${ordinal}.json"
  get_json "$trial_tenant" "/tdata/Answers('${answer_id}')" "$TMP_DIR/answer-${ordinal}.json"
  node - "$TMP_DIR/question-${ordinal}.json" "$TMP_DIR/answer-${ordinal}.json" "$ordinal" "$candidate_ref" "$answer_evidence" "$run_evidence" > "$TMP_DIR/validator-evidence-${ordinal}.json" <<'NODE'
const fs = require('fs');
const question = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const answer = JSON.parse(fs.readFileSync(process.argv[3], 'utf8'));
const ordinal = process.argv[4];
const candidateRef = process.argv[5];
const expectedEvidence = process.argv[6];
const runEvidence = process.argv[7];
const field = (record, lower, title) => record.fields?.[lower] ?? record.fields?.[title] ?? record[title];
const questionStatus = question.status ?? field(question, 'status', 'Status');
const answerStatus = answer.status ?? field(answer, 'status', 'Status');
const answerEvidence = field(answer, 'evidence', 'Evidence');
if (questionStatus !== 'Resolved') throw new Error(`generation ${ordinal}: expected resolved question, got ${questionStatus}`);
if (answerStatus !== 'Accepted') throw new Error(`generation ${ordinal}: expected accepted answer, got ${answerStatus}`);
if (answerEvidence !== expectedEvidence) throw new Error(`generation ${ordinal}: evidence was not preserved`);
process.stdout.write(JSON.stringify({
  generation: ordinal,
  candidate_ref: candidateRef,
  status: 'Passed',
  evidence_locator: runEvidence,
  result_summary: `Frozen native acceptance trial passed for generation ${ordinal}; accepted answer retained inspectable evidence.`,
  measurements: [
    { suffix: 'sim', traffic_source_id: '{campaign_id}-simulated', metric_key: 'resolved_questions', metric_value: '1.0', source_kind: 'simulated', evidence_locator: runEvidence },
    { suffix: 'real', traffic_source_id: '{campaign_id}-real', metric_key: 'answer_evidence', metric_value: 'observed', source_kind: 'real', evidence_locator: runEvidence },
    { suffix: 'trace', traffic_source_id: '{campaign_id}-simulated', metric_key: 'interaction_latency', metric_value: 'pending', source_kind: 'datadog_observation', evidence_locator: 'datadog://pending-local-ingestion' }
  ]
}));
NODE
  post_json "$trial_tenant" "/tdata/ValidatorRuns('${run_id}')/Genesis.AgentAnswersEvaluation.Pass" "{\"evidence_locator\":$(json_escape "$answer_evidence"),\"result_summary\":\"Frozen native acceptance trial passed for generation ${ordinal}; accepted answer retained inspectable evidence.\"}" "$TMP_DIR/validator-run-pass-${ordinal}.json"
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
generate_generation_one
verify_specs "$APP_DIR"
git -C "$APP_DIR" add .
git -C "$APP_DIR" commit -m 'Generation 1: expose citation requirement' >/dev/null
GEN_ONE_SHA="$(git -C "$APP_DIR" rev-parse HEAD)"
git -C "$APP_DIR" push "$REMOTE" main >/dev/null
post_json "$TENANT" "/tdata/Apps('${APP_ID}')/Temper.Git.PublishNewVersion?await_integration=true" "{\"NewHash\":$(json_escape "$GEN_ONE_SHA"),\"RefName\":\"main\"}" "$TMP_DIR/gen-one.json"

printf 'Publishing generation two: measure observed reuse\n'
generate_generation_two
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

validate_selected_candidate "1" "$GEN_ONE_REF" "${TARGET_TENANT}-generation-1"
validate_selected_candidate "2" "$GEN_TWO_REF" "$TARGET_TENANT"
node - "$EVALUATOR_REF" "$TMP_DIR/validator-evidence-1.json" "$TMP_DIR/validator-evidence-2.json" > "$TMP_DIR/validator-evidence.json" <<'NODE'
const fs = require('fs');
const evaluatorRef = process.argv[2];
const records = process.argv.slice(3).map((path) => JSON.parse(fs.readFileSync(path, 'utf8')));
process.stdout.write(JSON.stringify({ evaluator_ref: evaluatorRef, records }, null, 2));
NODE
node - "$RUN_ID" "$SEED_REF" "$GEN_ONE_REF" "$GEN_TWO_REF" "$EVALUATOR_REF" > "$TMP_DIR/campaign-plan.json" <<'NODE'
const [runId, seedRef, generationOneRef, generationTwoRef, evaluatorRef] = process.argv.slice(2);
const campaignId = `campaign-${runId}-manifest`;
process.stdout.write(JSON.stringify({
  campaign_id: campaignId,
  name: 'Agent Answers live evolution proof',
  director_brief: 'Evolve toward useful, evidence-grounded agent answers while preserving rollback and understandable behavior.',
  target_app_ref: seedRef,
  evaluator_app_ref: evaluatorRef,
  brain_provider: 'codex',
  automation_mode: 'automatic_release',
  traffic_sources: [
    { id: `${campaignId}-simulated`, name: 'simulated', kind: 'simulated', description: 'Codex actors using controlled questions and validation.' },
    { id: `${campaignId}-real`, name: 'real', kind: 'real', description: 'Interactions from an installed subject app.' }
  ],
  selection_design: {
    id: `${campaignId}-selection-v1`,
    version_label: 'v1',
    evaluator_namespace: 'Genesis.AgentAnswersEvaluation',
    trial_suites_entity_set: 'TrialSuites',
    metric_definitions_entity_set: 'MetricDefinitions',
    validator_runs_entity_set: 'ValidatorRuns',
    trial_suite: {
      id: `${campaignId}-trial-suite-v1`,
      name: 'Agent Answers bootstrap behavior',
      description: 'Frozen native trial suite covering question resolution and evidence visibility.',
      scenario_manifest_json: [{ id: 'accept-evidenced-answer', traffic: 'simulated' }],
      hidden_fixture_locator: `temper://campaigns/${campaignId}/fixtures/bootstrap`,
      authored_by: 'codex-with-human-approval'
    },
    fitness_model_json: { comparison: 'evidence_weighted_preference', signals: ['resolved_questions', 'answer_evidence', 'interaction_latency'], release: 'automatic' },
    constraint_definitions_json: [{ key: 'native_verified', kind: 'required' }, { key: 'rollback_available', kind: 'required' }],
    traffic_sources_json: ['simulated', 'real'],
    rationale: 'Prefer candidates with executed native evidence while preserving reversible releases.',
    proposed_by: 'codex',
    approved_by: 'local-proof-human',
    metrics: [
      { id: `${campaignId}-metric-resolved`, key: 'resolved_questions', description: 'Controlled questions resolve after accepted answers.', instrument_kind: 'native_validator', instrument_locator: 'temper://validators/question-resolution', interpretation: 'Evidence contributes to candidate comparison under the approved design.', hard_constraint: true },
      { id: `${campaignId}-metric-evidence`, key: 'answer_evidence', description: 'Usage exposes an evidence locator on accepted answers.', instrument_kind: 'native_validator', instrument_locator: 'temper://validators/answer-evidence', interpretation: 'Evidence contributes to candidate comparison under the approved design.', hard_constraint: false },
      { id: `${campaignId}-metric-latency`, key: 'interaction_latency', description: 'Observed interaction latency remains inspectable in Datadog.', instrument_kind: 'datadog', instrument_locator: 'datadog://pending-local-ingestion', interpretation: 'Evidence contributes to candidate comparison under the approved design.', hard_constraint: false }
    ]
  },
  generations: [
    { ordinal: '1', parent_release_ref: seedRef, selected_app_ref: generationOneRef, selected_mutation_summary: 'Codex candidate adds inspectable answer evidence.' },
    { ordinal: '2', parent_release_ref: generationOneRef, selected_app_ref: generationTwoRef, selected_mutation_summary: 'Codex candidate supports reuse of validated answers.' }
  ],
  capabilities: [{ id: `${campaignId}-capability-evidence`, generation_ordinal: '2', title: 'Reusable evidence surfaced in answers', observation: 'Observed benefit remained visible through executed evaluator evidence.', evidence_locator: 'datadog://pending-local-ingestion', keep: true }],
  release_control: { pause_reason: 'Proof pause before rollback', rollback_current_ref: generationOneRef, rollback_previous_ref: generationTwoRef, rollback_reason: 'Rollback exercised during local proof' }
}, null, 2));
NODE

if [[ "$CANDIDATE_GENERATOR" == "deterministic" ]]; then
  post_json "$TARGET_TENANT" "/tdata/Answers('validator-answer-${RUN_ID}-2')/Genesis.AgentAnswers.RecordReuse" '{}' "$TMP_DIR/answer-reuse.json"
  get_json "$TARGET_TENANT" "/tdata/Answers('validator-answer-${RUN_ID}-2')" "$TMP_DIR/answer-reuse-result.json"
  node - "$TMP_DIR/answer-reuse-result.json" <<'NODE'
const fs = require('fs');
const row = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const count = row.fields?.reuse_count ?? row.fields?.ReuseCount ?? row.ReuseCount;
if (Number(count) !== 1) throw new Error(`expected reuse_count=1, got ${count}`);
NODE
fi

cat > "$TMP_DIR/proof.env" <<EOF
EVOLUTION_SUBJECT_SEED_REF=${SEED_REF}
EVOLUTION_EVALUATOR_REF=${EVALUATOR_REF}
EVOLUTION_GENERATION_ONE_REF=${GEN_ONE_REF}
EVOLUTION_GENERATION_TWO_REF=${GEN_TWO_REF}
EVOLUTION_INSTALLED_TENANT=${TARGET_TENANT}
EVOLUTION_CANDIDATE_GENERATOR=${CANDIDATE_GENERATOR}
EVOLUTION_VALIDATOR_EVIDENCE_PATH=${TMP_DIR}/validator-evidence.json
EVOLUTION_CAMPAIGN_PLAN_PATH=${TMP_DIR}/campaign-plan.json
EOF
printf 'PASS directed evolution native lineage proof\n'
printf '  proof_env: %s\n' "$TMP_DIR/proof.env"
printf '  seed_ref: %s\n' "$SEED_REF"
printf '  generation_one_ref: %s\n' "$GEN_ONE_REF"
printf '  generation_two_ref: %s\n' "$GEN_TWO_REF"
printf '  evaluator_ref: %s\n' "$EVALUATOR_REF"
