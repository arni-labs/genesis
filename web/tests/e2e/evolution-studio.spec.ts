import { expect, type Page, test } from '@playwright/test';

type Row = { entity_id: string; status: string; fields: Record<string, unknown> };
const row = (id: string, status: string, fields: Record<string, unknown>): Row => ({ entity_id: id, status, fields: { Id: id, Status: status, ...fields } });

function evolutionFixture() {
  const campaign = row('campaign-proof', 'Running', { Name: 'Agent Answers live evolution proof', DirectorBrief: 'Preserve understandable behavior while evolving from evidence.', TargetAppRef: 'demo/agent-answers@seed', ActiveSelectionDesignId: 'selection-1', ActiveEvaluatorRef: 'demo/agent-answers-evaluation@frozen', CurrentReleaseRef: 'demo/agent-answers@gen2', PreviousReleaseRef: 'demo/agent-answers@gen1', GenerationCount: 2, AutomationMode: 'automatic_release', BrainProvider: 'codex' });
  const collections: Record<string, Row[]> = {
    Campaigns: [campaign],
    SelectionDesigns: [row('selection-1', 'Frozen', { CampaignId: 'campaign-proof', EvaluatorAppRef: 'demo/agent-answers-evaluation@frozen', Rationale: 'Use mixed evidence under a frozen judge.' })],
    Generations: [row('generation-1', 'Released', { CampaignId: 'campaign-proof', Ordinal: '1', ReleasedAppRef: 'demo/agent-answers@gen1', SelectionReason: 'Resolved controlled tasks.' }), row('generation-2', 'Released', { CampaignId: 'campaign-proof', Ordinal: '2', ReleasedAppRef: 'demo/agent-answers@gen2', SelectionReason: 'Improved observed usage.' })],
    Candidates: [row('candidate-2', 'Released', { CampaignId: 'campaign-proof', GenerationId: 'generation-2' })],
    Measurements: [row('measure-1', 'Recorded', { CampaignId: 'campaign-proof', MetricKey: 'answer_evidence', MetricValue: 'observed', SourceKind: 'real', EvidenceLocator: 'trace:proof', Notes: 'Browser use confirmed.' })],
    TrafficSources: [row('traffic-real', 'Active', { CampaignId: 'campaign-proof', Name: 'Browser usage', Kind: 'real' }), row('traffic-sim', 'Active', { CampaignId: 'campaign-proof', Name: 'Codex actors', Kind: 'simulated' })],
    EmergentCapabilities: [row('capability-1', 'Kept', { CampaignId: 'campaign-proof', Title: 'Evidence citations', Observation: 'Answers include proof locators.' })],
    Interventions: [],
    TrialSuites: [row('campaign-proof-trial-suite-v1', 'Frozen', { SubjectAppRef: 'demo/agent-answers@seed' })],
    MetricDefinitions: [row('campaign-proof-resolved_questions', 'Frozen', { MetricKey: 'resolved_questions' }), row('campaign-proof-answer_evidence', 'Frozen', { MetricKey: 'answer_evidence' })],
    ValidatorRuns: [row('campaign-proof-validator-1', 'Passed', { CandidateAppRef: 'demo/agent-answers@gen1' }), row('campaign-proof-validator-2', 'Passed', { CandidateAppRef: 'demo/agent-answers@gen2' })]
  };
  return collections;
}

async function mockEvolution(page: Page) {
  const collections = evolutionFixture();
  for (const name of Object.keys(collections).filter((name) => name !== 'Interventions')) {
    await page.route(`**/tdata/${name}`, async (route) => {
      if (route.request().method() === 'POST') {
        const payload = route.request().postDataJSON() as { Id: string };
        collections[name].push(row(payload.Id, 'Requested', {}));
        return route.fulfill({ status: 201, json: collections[name].at(-1) });
      }
      return route.fulfill({ json: { value: collections[name] } });
    });
  }
  await page.route('**/tdata/Interventions*', async (route) => {
    if (route.request().url().includes('Genesis.Evolution.Configure')) { const payload = route.request().postDataJSON() as Record<string, unknown>; collections.Interventions[0].fields.CampaignId = payload.campaign_id; return route.fulfill({ json: collections.Interventions[0] }); }
    if (route.request().method() === 'POST') { const payload = route.request().postDataJSON() as { Id: string }; collections.Interventions.push(row(payload.Id, 'Requested', { CampaignId: 'campaign-proof' })); return route.fulfill({ status: 201, json: collections.Interventions[0] }); }
    return route.fulfill({ json: { value: collections.Interventions } });
  });
  await page.route('**/tdata/Campaigns(*)/Genesis.Evolution.Pause', async (route) => { collections.Campaigns[0].status = 'Paused'; collections.Campaigns[0].fields.Status = 'Paused'; await route.fulfill({ json: collections.Campaigns[0] }); });
  await page.route('**/tdata/Campaigns(*)/Genesis.Evolution.Rollback', async (route) => route.fulfill({ json: collections.Campaigns[0] }));
}

test('shows a transparent two-generation campaign and records human intervention', async ({ page }) => {
  await mockEvolution(page);
  await page.goto('/genesis/evolution');
  await expect(page.getByRole('heading', { name: 'Agent Answers live evolution proof' })).toBeVisible();
  await expect(page.getByText('Subject lineage')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Frozen judge' })).toBeVisible();
  await expect(page.getByText('2 native trial runs / 2 frozen measures')).toBeVisible();
  await expect(page.getByText('answer_evidence')).toBeVisible();
  await expect(page.getByText('Evidence citations')).toBeVisible();
  await page.getByLabel('New direction').fill('Preserve evidence citations in future survivors.');
  await page.getByRole('button', { name: 'Record direction' }).click();
  await expect(page.getByText('1 recorded / 1 candidate artifacts')).toBeVisible();
  await page.getByRole('button', { name: 'Pause' }).click();
  await expect(page.getByRole('button', { name: 'Resume' })).toBeVisible();
});

test('supports real subject traffic: ask, answer, and accept', async ({ page }) => {
  const questions: Row[] = [];
  const answers: Row[] = [];
  await page.route('**/tdata/Questions', async (route) => {
    if (route.request().method() === 'POST') { const payload = route.request().postDataJSON() as { Id: string }; questions.push(row(payload.Id, 'Draft', {})); return route.fulfill({ status: 201, json: questions[0] }); }
    return route.fulfill({ json: { value: questions } });
  });
  await page.route('**/tdata/Answers', async (route) => {
    if (route.request().method() === 'POST') { const payload = route.request().postDataJSON() as { Id: string }; answers.push(row(payload.Id, 'Draft', {})); return route.fulfill({ status: 201, json: answers[0] }); }
    return route.fulfill({ json: { value: answers } });
  });
  await page.route('**/tdata/Questions(*)/Genesis.AgentAnswers.Configure', async (route) => { const data = route.request().postDataJSON() as Record<string, unknown>; questions[0] = row(questions[0].entity_id, 'Open', { Title: data.title, Body: data.body, AskedBy: data.asked_by, AnswerCount: 0 }); await route.fulfill({ json: questions[0] }); });
  await page.route('**/tdata/Answers(*)/Genesis.AgentAnswers.Submit', async (route) => { const data = route.request().postDataJSON() as Record<string, unknown>; answers[0] = row(answers[0].entity_id, 'Published', { QuestionId: data.question_id, Body: data.body, AnsweredBy: data.answered_by, Evidence: data.evidence }); await route.fulfill({ json: answers[0] }); });
  await page.route('**/tdata/Questions(*)/Genesis.AgentAnswers.RecordAnswer', async (route) => { questions[0].fields.AnswerCount = 1; questions[0].status = 'Answered'; questions[0].fields.Status = 'Answered'; await route.fulfill({ json: questions[0] }); });
  await page.route('**/tdata/Answers(*)/Genesis.AgentAnswers.Accept', async (route) => { answers[0].status = 'Accepted'; answers[0].fields.Status = 'Accepted'; await route.fulfill({ json: answers[0] }); });
  await page.route('**/tdata/Questions(*)/Genesis.AgentAnswers.Accept', async (route) => { questions[0].status = 'Resolved'; questions[0].fields.Status = 'Resolved'; await route.fulfill({ json: questions[0] }); });

  await page.goto('/genesis/answers');
  await page.getByRole('button', { name: 'Post question' }).click();
  await expect(page.getByText('How should an agent preserve proof')).toBeVisible();
  await page.getByRole('button', { name: 'Answer' }).click();
  await page.getByLabel('Answer body').fill('Attach the trace locator and validation result.');
  await page.getByLabel('Evidence').fill('trace:proof-1');
  await page.getByRole('button', { name: 'Submit answer' }).click();
  await expect(page.getByText('Attach the trace locator')).toBeVisible();
  await page.getByRole('button', { name: 'Accept' }).click();
  await expect(page.getByText('/ accepted')).toBeVisible();
});
