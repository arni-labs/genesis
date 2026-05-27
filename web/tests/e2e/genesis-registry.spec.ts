import { expect, type Page, test } from '@playwright/test';

type EntityRow = {
  entity_id: string;
  status: string;
  fields: Record<string, unknown>;
};

const parentHash = '1111111111111111111111111111111111111111';
const childHash = '2222222222222222222222222222222222222222';
const oldChildHash = '5555555555555555555555555555555555555555';
const parentTreeHash = '3333333333333333333333333333333333333333';
const childTreeHash = '4444444444444444444444444444444444444444';
const oldChildTreeHash = '6666666666666666666666666666666666666666';
const readmeBlobHash = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const manifestBlobHash = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

function row(entityType: string, id: string, status: string, fields: Record<string, unknown>): EntityRow {
  return {
    entity_id: id,
    status,
    fields: {
      Id: id,
      Status: status,
      ...fields
    },
    entity_type: entityType
  } as EntityRow;
}

const apps = [
  row('App', 'app-kernel-core', 'Active', {
    OwnerId: 'team',
    Name: 'kernel-core',
    RepositoryId: 'rp-team-kernel-core',
    LatestVersionHash: parentHash,
    Exports: JSON.stringify(['Repository.IngestPack', 'App.Fork']),
    Description: 'Spec-first kernel primitives',
    Visibility: 'public',
    CreatedAt: '2026-05-19T08:00:00Z',
    UpdatedAt: '2026-05-19T08:01:00Z'
  }),
  row('App', 'app-alice-notes', 'Active', {
    OwnerId: 'alice',
    Name: 'alice-notes',
    RepositoryId: 'rp-alice-notes',
    LatestVersionHash: childHash,
    Exports: JSON.stringify(['notes.view']),
    Description: 'Forked notes workspace',
    Visibility: 'public',
    CreatedAt: '2026-05-19T08:02:00Z',
    UpdatedAt: '2026-05-19T08:03:00Z'
  })
];

const owners = [
  row('Owner', 'team', 'Verified', {
    AccountId: 'team',
    DisplayName: 'Temper Team',
    Contact: 'ops@example.test',
    StorageCapBytes: 104_857_600,
    RateLimitTier: 'pro',
    VerificationProvider: 'oauth',
    VerificationSubject: 'github:temper-team',
    VerifiedAt: '2026-05-19T08:00:00Z'
  }),
  row('Owner', 'alice', 'PendingVerification', {
    AccountId: 'alice',
    DisplayName: 'Alice',
    Contact: 'alice@example.test',
    StorageCapBytes: 104_857_600,
    RateLimitTier: 'free',
    VerificationProvider: 'email_magic_link',
    VerificationSubject: 'alice@example.test'
  })
];

const lineages = [
  row('Lineage', 'ln-alice-notes', 'Active', {
    ChildRepositoryId: 'rp-alice-notes',
    ParentRepositoryId: 'rp-team-kernel-core',
    ParentCommit: parentHash,
    Type: 'fork',
    CreatedBy: 'alice',
    Mutations: JSON.stringify(['rename README.md', 'add notes route']),
    CreatedAt: '2026-05-19T08:04:00Z'
  })
];

const closures = [
  row('Closure', 'cl-test-realpack', 'Durable', {
    Root: 'app-alice-notes',
    Resolved: JSON.stringify({
      'kernel-core': `@${parentHash}`,
      'alice-notes': `@${childHash}`
    }),
    ResolverVersion: '1.0',
    ResolvedAt: '2026-05-19T08:05:00Z',
    ResolvedBy: 'playwright-regression'
  })
];

function base64(value: string): string {
  return Buffer.from(value, 'utf8').toString('base64');
}

function treeCanonical(entries: Array<{ mode: string; name: string; sha: string }>): string {
  const body = entries.flatMap((entry) => [
    ...Array.from(Buffer.from(`${entry.mode} ${entry.name}\0`, 'utf8')),
    ...Array.from(Buffer.from(entry.sha, 'hex'))
  ]);
  const header = Array.from(Buffer.from(`tree ${body.length}\0`, 'utf8'));
  return Buffer.from([...header, ...body]).toString('base64');
}

const commits = [
  row('Commit', parentHash, 'Durable', {
    RepositoryId: 'rp-team-kernel-core',
    TreeSha: parentTreeHash,
    ParentShas: '',
    Author: 'Team <team@example.test>',
    Committer: 'Team <team@example.test>',
    Message: 'parent registry commit\n',
    CreatedAt: '2026-05-19T08:00:00Z'
  }),
  row('Commit', childHash, 'Durable', {
    RepositoryId: 'rp-alice-notes',
    TreeSha: childTreeHash,
    ParentShas: oldChildHash,
    Author: 'Alice <alice@example.test>',
    Committer: 'Alice <alice@example.test>',
    Message: 'add notes app\n',
    CreatedAt: '2026-05-19T08:04:00Z'
  }),
  row('Commit', oldChildHash, 'Durable', {
    RepositoryId: 'rp-alice-notes',
    TreeSha: oldChildTreeHash,
    ParentShas: '',
    Author: 'Alice <alice@example.test>',
    Committer: 'Alice <alice@example.test>',
    Message: 'initial notes app\n',
    CreatedAt: '2026-05-19T08:02:00Z'
  })
];

const trees = [
  row('Tree', parentTreeHash, 'Durable', {
    RepositoryId: 'rp-team-kernel-core',
    CanonicalBytes: treeCanonical([
      { mode: '100644', name: 'README.md', sha: readmeBlobHash }
    ])
  }),
  row('Tree', childTreeHash, 'Durable', {
    RepositoryId: 'rp-alice-notes',
    CanonicalBytes: treeCanonical([
      { mode: '100644', name: 'README.md', sha: readmeBlobHash },
      { mode: '100644', name: 'app.toml', sha: manifestBlobHash }
    ])
  }),
  row('Tree', oldChildTreeHash, 'Durable', {
    RepositoryId: 'rp-alice-notes',
    CanonicalBytes: treeCanonical([
      { mode: '100644', name: 'README.md', sha: readmeBlobHash }
    ])
  })
];

const blobs = [
  row('Blob', readmeBlobHash, 'Durable', {
    RepositoryId: 'rp-alice-notes',
    Content: base64('# Alice Notes\n'),
    Size: 14
  }),
  row('Blob', manifestBlobHash, 'Durable', {
    RepositoryId: 'rp-alice-notes',
    Content: base64('name = "alice-notes"\n'),
    Size: 21
  })
];

const directedCollections: Record<string, EntityRow[]> = {
  Organisms: [
    row('Organism', 'org-agent-answers', 'Active', {
      Name: 'Agent Answers',
      AppRef: `arni-labs/agent-answers@${childHash}`,
      ParentVersionId: 'ov-agent-answers-parent',
      BaselineEvaluationJson: JSON.stringify(['compile', 'simulated-user'])
    })
  ],
  OrganismVersions: [
    row('OrganismVersion', 'ov-agent-answers-parent', 'Parent', {
      OrganismId: 'org-agent-answers',
      AppRef: `arni-labs/agent-answers@${childHash}`,
      CommitRef: childHash,
      Summary: 'Current production parent'
    })
  ],
  Signals: [
    row('Signal', 'sig-unmet-citation', 'Linked', {
      Source: 'simulated-user-agent',
      SignalKind: 'unmet_intent',
      OrganismId: 'org-agent-answers',
      Summary: 'User agent could not preserve source context after a follow-up question.',
      PressureId: 'pressure-citation'
    })
  ],
  Pressures: [
    row('Pressure', 'pressure-citation', 'Framed', {
      OrganismId: 'org-agent-answers',
      PressureClass: 'growth',
      Summary: 'Follow-up answers need durable citation memory.',
      SignalIdsJson: JSON.stringify(['sig-unmet-citation']),
      DirectionId: 'direction-citation-memory',
      BrainRunId: 'brain-observer'
    })
  ],
  Directions: [
    row('Direction', 'direction-citation-memory', 'Proposed', {
      OrganismId: 'org-agent-answers',
      PressureIdsJson: JSON.stringify(['pressure-citation']),
      PressureClass: 'growth',
      Title: 'Answer citations that survive follow-up',
      Summary: 'Add citation memory so generated answers retain source context across follow-up turns.',
      ProvenanceJson: JSON.stringify({
        signal: 'sig-unmet-citation',
        observer: 'brain-observer',
        basis: 'simulated users repeatedly asked where the answer came from'
      }),
      AutonomyLane: 'growth-human-gated',
      ProposedAdaptationGoal: 'Follow-up answers keep a visible source trail.',
      ProposedViabilityConstraintsJson: JSON.stringify(['Do not reduce answer correctness'])
    })
  ],
  Episodes: [
    row('Episode', 'episode-citation-memory', 'Running', {
      DirectionId: 'direction-citation-memory',
      OrganismId: 'org-agent-answers',
      ParentVersionId: 'ov-agent-answers-parent',
      AutonomyLane: 'growth-human-gated',
      AdaptationGoalId: 'goal-citation-memory',
      SelectionPressureId: 'selection-citation-memory',
      ViabilityConstraintIdsJson: JSON.stringify(['constraint-correctness']),
      EvaluationStageIdsJson: JSON.stringify(['stage-compile', 'stage-simulated-user'])
    })
  ],
  Generations: [
    row('Generation', 'generation-citation-memory-1', 'Evaluating', {
      EpisodeId: 'episode-citation-memory',
      ParentVersionId: 'ov-agent-answers-parent',
      GenerationIndex: 1,
      VariantTargetCount: 2
    })
  ],
  Variants: [
    row('Variant', 'variant-memory-panel', 'Active', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      AppRef: 'arni-labs/agent-answers@variant-a',
      RuntimeRef: 'agent-answers-a.local',
      Summary: 'Adds a source memory panel',
      BrainRunId: 'brain-variant-a'
    }),
    row('Variant', 'variant-hidden-citations', 'Eliminated', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      AppRef: 'arni-labs/agent-answers@variant-b',
      RuntimeRef: 'agent-answers-b.local',
      Summary: 'Stores citations invisibly',
      EliminationRuleId: 'rule-visible-source-trail',
      StageResultId: 'stage-result-b-user',
      EvidenceArtifactId: 'evidence-b-user',
      Reason: 'Simulated users still could not find the source trail.'
    })
  ],
  AdaptationGoals: [
    row('AdaptationGoal', 'goal-citation-memory', 'Active', {
      EpisodeId: 'episode-citation-memory',
      GoalStatement: 'Follow-up answers keep a visible source trail.',
      CreatedByBrainRunId: 'brain-direction'
    })
  ],
  ViabilityConstraints: [
    row('ViabilityConstraint', 'constraint-correctness', 'Active', {
      EpisodeId: 'episode-citation-memory',
      ConstraintStatement: 'Do not reduce answer correctness or source fidelity.',
      ConstraintKind: 'quality'
    })
  ],
  SelectionPressures: [
    row('SelectionPressure', 'selection-citation-memory', 'Active', {
      EpisodeId: 'episode-citation-memory',
      SelectionStatement: 'Prefer variants that improve follow-up source recall without correctness regressions.',
      MetricIdsJson: JSON.stringify(['metric-source-recall']),
      EliminationRuleIdsJson: JSON.stringify(['rule-visible-source-trail']),
      ScoringRuleIdsJson: JSON.stringify(['score-source-recall'])
    })
  ],
  EvaluationStages: [
    row('EvaluationStage', 'stage-compile', 'Active', {
      EpisodeId: 'episode-citation-memory',
      StageName: 'Compile',
      StageKind: 'static',
      SequenceIndex: 1,
      ExecutorKind: 'codex'
    }),
    row('EvaluationStage', 'stage-simulated-user', 'Active', {
      EpisodeId: 'episode-citation-memory',
      StageName: 'AI User Trial',
      StageKind: 'simulated_user',
      SequenceIndex: 2,
      ExecutorKind: 'codex'
    })
  ],
  StageResults: [
    row('StageResult', 'stage-result-a-compile', 'Passed', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      VariantId: 'variant-memory-panel',
      EvaluationStageId: 'stage-compile',
      MetricsJson: JSON.stringify({ build: 'ok' }),
      EvidenceArtifactId: 'evidence-a-compile',
      Summary: 'Build and checks passed'
    }),
    row('StageResult', 'stage-result-a-user', 'Running', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      VariantId: 'variant-memory-panel',
      EvaluationStageId: 'stage-simulated-user',
      Summary: 'Simulated users are testing follow-up recall'
    }),
    row('StageResult', 'stage-result-b-user', 'Eliminated', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      VariantId: 'variant-hidden-citations',
      EvaluationStageId: 'stage-simulated-user',
      EvidenceArtifactId: 'evidence-b-user',
      Reason: 'Source trail remained hidden from simulated users.'
    })
  ],
  MetricDefinitions: [
    row('MetricDefinition', 'metric-source-recall', 'Active', {
      EpisodeId: 'episode-citation-memory',
      MetricName: 'Source recall',
      Unit: 'score',
      DesiredDirection: 'higher'
    })
  ],
  Measurements: [
    row('Measurement', 'measurement-a-source-recall', 'Recorded', {
      MetricDefinitionId: 'metric-source-recall',
      StageResultId: 'stage-result-a-user',
      VariantId: 'variant-memory-panel',
      Value: '0.82',
      Unit: 'score'
    })
  ],
  EvidenceArtifacts: [
    row('EvidenceArtifact', 'evidence-b-user', 'Linked', {
      ArtifactKind: 'simulated_user_trace',
      Uri: 'datadog://trace/variant-b',
      Summary: 'AI user could not locate citations after the follow-up.',
      TargetEntityType: 'Variant',
      TargetEntityId: 'variant-hidden-citations'
    })
  ],
  Trials: [
    row('Trial', 'trial-a-user-1', 'Running', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      VariantId: 'variant-memory-panel',
      SimulatedUserBrainRunId: 'brain-sim-user-a',
      RuntimeRef: 'agent-answers-a.local',
      GoalJson: JSON.stringify({ goal: 'ask follow-up source question' })
    })
  ],
  AutonomyPolicies: [
    row('AutonomyPolicy', 'policy-agent-answers', 'Active', {
      OrganismId: 'org-agent-answers',
      PolicyJson: JSON.stringify({
        repair_lane: 'auto-promote bounded fixes after evaluation',
        growth_lane: 'human approval required before episode start',
        policy_lane: 'human approval required'
      }),
      Summary: 'Repair can move automatically; growth remains human-gated.'
    })
  ],
  WorkItems: [
    row('WorkItem', 'work-variant-a', 'Running', {
      Role: 'variant_generator',
      TargetEntityType: 'Generation',
      TargetEntityId: 'generation-citation-memory-1'
    })
  ],
  BrainRuns: [
    row('BrainRun', 'brain-observer', 'Succeeded', {
      Role: 'observer',
      WorkItemId: 'work-observer',
      AgentKind: 'codex',
      Model: 'codex-cli',
      Summary: 'Observed unmet follow-up citation intent.'
    })
  ]
};

async function mockOData(page: Page) {
  await page.route('**/tdata/Apps', async (route) => {
    await route.fulfill({ json: { value: apps } });
  });
  await page.route('**/tdata/Lineages', async (route) => {
    await route.fulfill({ json: { value: lineages } });
  });
  await page.route('**/tdata/Closures', async (route) => {
    await route.fulfill({ json: { value: closures } });
  });
  await page.route('**/tdata/Owners', async (route) => {
    if (route.request().method() === 'POST') {
      const body = route.request().postDataJSON() as Record<string, unknown>;
      if (
        body.Id === 'newco' &&
        (body.VerificationProvider !== 'oauth' ||
          body.VerificationSubject !== 'github:newco')
      ) {
        await route.fulfill({
          status: 400,
          json: {
            error: {
              message: `unexpected verification payload ${JSON.stringify(body)}`
            }
          }
        });
        return;
      }
      await route.fulfill({
        status: 201,
        json: row('Owner', String(body.Id), 'PendingVerification', body)
      });
      return;
    }
    await route.fulfill({ json: { value: owners } });
  });
  await page.route('**/tdata/Commits*', async (route) => {
    await route.fulfill({ json: { value: commits } });
  });
  await page.route('**/tdata/Trees*', async (route) => {
    await route.fulfill({ json: { value: trees } });
  });
  await page.route('**/tdata/Blobs*', async (route) => {
    await route.fulfill({ json: { value: blobs } });
  });

  for (const collection of Object.keys(directedCollections)) {
    await page.route(`**/tdata/${collection}**`, async (route) => {
      const request = route.request();
      if (request.method() === 'POST') {
        const url = request.url();
        if (collection === 'Episodes' && url.includes('PauseEpisode')) {
          directedCollections.Episodes[0].status = 'Paused';
          directedCollections.Episodes[0].fields.Status = 'Paused';
        }
        if (collection === 'Directions' && url.includes('DismissDirection')) {
          directedCollections.Directions[0].status = 'Dismissed';
          directedCollections.Directions[0].fields.Status = 'Dismissed';
        }
        if (collection === 'ViabilityConstraints' && url.includes('PinViabilityConstraint')) {
          directedCollections.ViabilityConstraints[0].status = 'Pinned';
          directedCollections.ViabilityConstraints[0].fields.Status = 'Pinned';
        }
        await route.fulfill({ status: 200, json: directedCollections[collection][0] });
        return;
      }
      await route.fulfill({ json: { value: directedCollections[collection] } });
    });
  }
}

test.beforeEach(async ({ page }) => {
  await mockOData(page);
});

test('renders browse, lineage, closures, and Genesis install surfaces without browser errors', async ({
  page
}) => {
  const browserErrors: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') {
      browserErrors.push(message.text());
    }
  });
  page.on('pageerror', (error) => browserErrors.push(error.message));

  await page.goto('/');

  await expect(page.getByRole('heading', { name: 'Genesis.' })).toBeVisible();
  await expect(page.getByText('Live')).toBeVisible();
  await expect(page.getByRole('link', { name: /alice-notes/ })).toBeVisible();
  await page.getByRole('link', { name: /alice-notes/ }).click();

  await expect(page.getByRole('heading', { name: 'alice-notes' })).toBeVisible();
  await expect(page.getByRole('button', { name: /app.toml/ })).toBeVisible();
  await page.getByRole('button', { name: /app.toml/ }).click();
  await expect(page.getByText('name = "alice-notes"')).toBeVisible();

  await page.getByRole('tab', { name: 'Versions' }).click();
  const versionsPanel = page.getByRole('tabpanel', { name: 'Versions' });
  await expect(page.getByText('Version Chain')).toBeVisible();
  await expect(page.getByRole('button', { name: /add notes app/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /initial notes app/ })).toBeVisible();
  await expect(versionsPanel.getByText(`alice/alice-notes@${childHash}`, { exact: true })).toBeVisible();
  await page.getByRole('button', { name: /initial notes app/ }).click();
  await expect(versionsPanel.getByText(`alice/alice-notes@${oldChildHash}`, { exact: true })).toBeVisible();

  await page.getByRole('tab', { name: 'Overview' }).click();
  await expect(page.getByText('Alice', { exact: true })).toBeVisible();
  await expect(page.getByText('cl-test-realpack')).toBeVisible();
  await expect(page.getByText(/kernel-core:/)).toBeVisible();

  await page.getByRole('tab', { name: 'Lineage' }).click();
  await expect(page.getByText('team/kernel-core')).toBeVisible();
  await expect(
    page.getByRole('tabpanel', { name: 'Lineage' }).getByText('alice/alice-notes', { exact: true })
  ).toBeVisible();
  await expect(page.getByLabel('Lineage graph')).toBeVisible();

  await page.getByRole('tab', { name: 'Install' }).click();
  const installPanel = page.getByRole('tabpanel', { name: 'Install', exact: true });
  await expect(
    installPanel.getByText(`/tdata/Apps('app-alice-notes')/App.Install`)
  ).toBeVisible();
  await expect(
    installPanel.getByText(`temper install alice/alice-notes@${childHash} --tenant default --url`)
  ).toBeVisible();
  await expect(
    installPanel.getByText(`temper.install_app({"app_ref":"alice/alice-notes@${childHash}"`)
  ).toBeVisible();
  await expect(installPanel.getByText('git clone')).toBeVisible();

  await page.goto('/');
  await page.getByPlaceholder('Search apps, owners, hashes').fill('kernel');
  await expect(page.getByRole('link', { name: /kernel-core/ })).toBeVisible();
  await expect(page.getByRole('link', { name: /alice-notes/ })).toHaveCount(0);
  await page.getByPlaceholder('Search apps, owners, hashes').fill('');
  await expect(page.getByRole('button', { name: 'Account' })).toHaveCount(0);
  await expect(page.getByText('Claim Namespace')).toHaveCount(0);

  const horizontalOverflow = await page.evaluate(() => {
    const root = document.documentElement;
    return root.scrollWidth - root.clientWidth;
  });
  expect(horizontalOverflow).toBeLessThanOrEqual(1);
  expect(browserErrors).toEqual([]);
});

test('renders live Directed Evolution mission control and dispatches real controls', async ({
  page
}) => {
  const actionUrls: string[] = [];
  page.on('request', (request) => {
    if (request.method() === 'POST' && request.url().includes('Temper.DirectedEvolution')) {
      actionUrls.push(request.url());
    }
  });

  await page.goto('/genesis/evolution');

  await expect(page.getByRole('heading', { name: 'Agent Answers' })).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Answer citations that survive follow-up' }).first()
  ).toBeVisible();
  await expect(
    page.getByText('Follow-up answers keep a visible source trail.', { exact: true }).first()
  ).toBeVisible();
  await expect(page.getByText('AI User Trial')).toBeVisible();
  await expect(page.getByText('Simulated users still could not find the source trail.')).toBeVisible();
  await expect(page.getByText('repair_lane')).toBeVisible();

  await page.getByRole('button', { name: 'Pause' }).click();
  await expect(page.getByText('Paused')).toBeVisible();

  await page.getByRole('button', { name: 'Pin' }).click();
  await expect(page.getByText('Pinned')).toBeVisible();

  await page.getByRole('button', { name: 'Compare' }).first().click();
  await expect(page.getByText('Variant Compare')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Adds a source memory panel' })).toBeVisible();

  await page.getByRole('button', { name: 'Dismiss' }).click();
  await expect(page.getByText('Dismissed')).toBeVisible();

  expect(actionUrls.some((url) => url.includes('/Episodes') && url.includes('PauseEpisode'))).toBe(
    true
  );
  expect(
    actionUrls.some(
      (url) => url.includes('/ViabilityConstraints') && url.includes('PinViabilityConstraint')
    )
  ).toBe(true);
  expect(
    actionUrls.some((url) => url.includes('/Directions') && url.includes('DismissDirection'))
  ).toBe(true);
});
