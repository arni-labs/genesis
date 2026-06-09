import { expect, type Page, test } from '@playwright/test';

type EntityRow = {
  entity_id: string;
  status: string;
  fields: Record<string, unknown>;
};

const parentHash = '1111111111111111111111111111111111111111';
const childHash = '2222222222222222222222222222222222222222';
const oldChildHash = '5555555555555555555555555555555555555555';
const newerChildRefHash = '7777777777777777777777777777777777777777';
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

const refs = [
  row('Ref', 'ref-rp-team-kernel-core-main', 'Active', {
    RepositoryId: 'rp-team-kernel-core',
    Name: 'refs/heads/main',
    TargetCommitSha: parentHash,
    Kind: 'branch',
    UpdatedAt: '2026-05-19T08:01:00Z'
  }),
  row('Ref', 'ref-rp-alice-notes-main', 'Active', {
    RepositoryId: 'rp-alice-notes',
    Name: 'refs/heads/main',
    TargetCommitSha: newerChildRefHash,
    Kind: 'branch',
    UpdatedAt: '2026-05-19T08:06:00Z'
  })
];

const installations = [
  row('AppInstallation', 'install-alice-notes-live', 'Installed', {
    AppId: 'app-alice-notes',
    AppRef: `alice/alice-notes@${childHash}`,
    VersionHash: childHash,
    FollowPolicy: 'pinned',
    TargetTenant: 'live',
    ClosureId: 'cl-test-realpack',
    Installer: 'playwright-regression',
    Message: 'Installed pinned fixture for registry provenance.',
    CreatedAt: '2026-05-19T08:06:00Z',
    InstalledAt: '2026-05-19T08:07:00Z'
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

const directedCollectionFixtures: Record<string, EntityRow[]> = {
  Organisms: [
    row('Organism', 'org-agent-answers', 'Active', {
      Name: 'Agent Answers',
      AppRef: 'arni-labs/agent-answers@citation-winner',
      ParentVersionId: 'ov-agent-answers-parent',
      OrganismVersionId: 'ov-agent-answers-citation',
      PromotionId: 'promotion-citation-memory',
      Summary: 'Current parent is aligned to the promoted app ref.',
      BaselineEvaluationJson: JSON.stringify(['compile', 'simulated-user'])
    })
  ],
  OrganismVersions: [
    row('OrganismVersion', 'ov-agent-answers-parent', 'Superseded', {
      OrganismId: 'org-agent-answers',
      AppRef: 'arni-labs/agent-answers@seed-parent',
      CommitRef: childHash,
      Summary: 'Seed production parent'
    }),
    row('OrganismVersion', 'ov-agent-answers-citation', 'Parent', {
      OrganismId: 'org-agent-answers',
      AppRef: 'arni-labs/agent-answers@citation-winner',
      PromotionId: 'promotion-citation-memory',
      Summary: 'Promoted organism with durable citation memory'
    })
  ],
  LineageEdges: [
    row('LineageEdge', 'edge-citation-memory', 'Recorded', {
      OrganismId: 'org-agent-answers',
      ParentVersionId: 'ov-agent-answers-parent',
      ChildVersionId: 'ov-agent-answers-citation',
      EpisodeId: 'episode-citation-memory',
      PromotionId: 'promotion-citation-memory',
      Summary: 'Citation memory evolved from the seed organism'
    })
  ],
  Signals: [
    row('Signal', 'sig-unmet-citation', 'Linked', {
      Source: 'simulated-user-agent',
      SignalKind: 'unmet_intent',
      OrganismId: 'org-agent-answers',
      Summary: 'User agent could not preserve source context after a follow-up question.',
      EvidenceArtifactId: 'evidence-signal-citation',
      PressureId: 'pressure-citation'
    })
  ],
  Pressures: [
    row('Pressure', 'pressure-citation', 'Framed', {
      OrganismId: 'org-agent-answers',
      PressureClass: 'growth',
      Summary: 'Follow-up answers need durable citation memory.',
      SignalIdsJson: JSON.stringify(['sig-unmet-citation']),
      EvidenceArtifactId: 'evidence-signal-citation',
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
      ProposedViabilityConstraintsJson: JSON.stringify(['Do not reduce answer correctness']),
      BrainRunId: 'brain-observer'
    }),
    row('Direction', 'direction-comparison-preview', 'Approved', {
      OrganismId: 'org-agent-answers',
      PressureIdsJson: JSON.stringify([]),
      PressureClass: 'growth',
      Title: 'Publish comparison winner',
      Summary: 'Promote the comparison preview variant after selection.',
      AutonomyLane: 'growth-human-gated',
      ProposedAdaptationGoal: 'Humans can compare candidate answers before acceptance.',
      ProposedViabilityConstraintsJson: JSON.stringify(['Keep answer latency bounded'])
    }),
    row('Direction', 'direction-broken-publish', 'Approved', {
      OrganismId: 'org-agent-answers',
      PressureIdsJson: JSON.stringify([]),
      PressureClass: 'repair',
      Title: 'Recover failed publish',
      Summary: 'Show the operator when a selected winner fails canonical materialization.',
      AutonomyLane: 'repair-auto'
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
      SelectionProtocolId: 'protocol-citation-memory',
      SimulatedUserPlanId: 'sim-plan-citation-memory',
      EvaluatorRef: 'genesis://nerdsane/agent-answers-evaluation@frozen',
      PlannedBy: 'codex-as-human-director',
      PlanSummary: 'Codex-as-director authored semantic episode entities before start.',
      ViabilityConstraintIdsJson: JSON.stringify(['constraint-correctness']),
      EvaluationStageIdsJson: JSON.stringify(['stage-compile', 'stage-simulated-user']),
      EliminationRuleIdsJson: JSON.stringify(['rule-visible-source-trail']),
      ScoringRuleIdsJson: JSON.stringify(['score-source-recall']),
      PromotionId: 'promotion-citation-memory'
    }),
    row('Episode', 'episode-comparison-preview', 'Promoting', {
      DirectionId: 'direction-comparison-preview',
      OrganismId: 'org-agent-answers',
      ParentVersionId: 'ov-agent-answers-parent',
      AutonomyLane: 'growth-human-gated',
      WinningVariantId: 'variant-comparison-preview',
      PromotionId: 'promotion-comparison-preview'
    }),
    row('Episode', 'episode-broken-publish', 'Failed', {
      DirectionId: 'direction-broken-publish',
      OrganismId: 'org-agent-answers',
      ParentVersionId: 'ov-agent-answers-parent',
      AutonomyLane: 'repair-auto',
      WinningVariantId: 'variant-broken-publish',
      PromotionId: 'promotion-broken-publish',
      Summary: 'Promotion materialization failed.'
    })
  ],
  Generations: [
    row('Generation', 'generation-citation-memory-1', 'Failed', {
      EpisodeId: 'episode-citation-memory',
      ParentVersionId: 'ov-agent-answers-parent',
      GenerationIndex: 1,
      VariantTargetCount: 2,
      FailureReason: 'All variants were eliminated before selection. Queued follow-up generation 2 with prior elimination evidence.'
    }),
    row('Generation', 'generation-citation-memory-2', 'Evaluating', {
      EpisodeId: 'episode-citation-memory',
      ParentVersionId: 'ov-agent-answers-parent',
      GenerationIndex: 2,
      VariantTargetCount: 2
    })
  ],
  Variants: [
    row('Variant', 'variant-memory-panel', 'Active', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-2',
      AppRef: 'arni-labs/agent-answers@variant-a',
      RuntimeRef: 'agent-answers-a.local',
      Summary: 'Adds a source memory panel',
      BrainRunId: 'brain-variant-a',
      MutationId: 'mutation-memory-panel',
      PromotionId: 'promotion-citation-memory'
    }),
    row('Variant', 'variant-hidden-citations', 'Eliminated', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      AppRef: 'arni-labs/agent-answers@variant-b',
      RuntimeRef: 'agent-answers-b.local',
      Summary: 'Stores citations invisibly',
      MutationId: 'mutation-hidden-citations',
      EliminationRuleId: 'rule-visible-source-trail',
      StageResultId: 'stage-result-b-user',
      EvidenceArtifactId: 'evidence-b-user',
      Reason: 'Simulated users still could not find the source trail.'
    }),
    row('Variant', 'variant-flat-sources', 'Eliminated', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-1',
      AppRef: 'arni-labs/agent-answers@variant-flat',
      RuntimeRef: 'agent-answers-flat.local',
      Summary: 'Adds a flat source note without follow-up recall',
      MutationId: 'mutation-flat-sources',
      Reason: 'Follow-up questions still lost the source relationship.'
    }),
    row('Variant', 'variant-comparison-preview', 'Selected', {
      EpisodeId: 'episode-comparison-preview',
      AppRef: 'arni-labs/agent-answers@comparison-preview',
      RuntimeRef: 'agent-answers-comparison.local',
      Summary: 'Adds a candidate answer comparison preview',
      PromotionId: 'promotion-comparison-preview'
    }),
    row('Variant', 'variant-broken-publish', 'Selected', {
      EpisodeId: 'episode-broken-publish',
      AppRef: 'arni-labs/agent-answers@broken-publish',
      Summary: 'Selected repair winner with a broken publish handoff',
      PromotionId: 'promotion-broken-publish'
    })
  ],
  Promotions: [
    row('Promotion', 'promotion-citation-memory', 'Promoted', {
      EpisodeId: 'episode-citation-memory',
      WinningVariantId: 'variant-memory-panel',
      ParentVersionId: 'ov-agent-answers-parent',
      NewOrganismVersionId: 'ov-agent-answers-citation',
      AppRef: 'arni-labs/agent-answers@citation-winner',
      CanonicalAppRef: 'arni-labs/agent-answers@citation-winner',
      ProductionTenant: 'default',
      RuntimeRef: 'temper://tenant/default/app/arni-labs/agent-answers@citation-winner',
      Materialized: true,
      Summary: 'Published the winner and hot-loaded it into the production tenant.'
    }),
    row('Promotion', 'promotion-comparison-preview', 'Promoted', {
      EpisodeId: 'episode-comparison-preview',
      WinningVariantId: 'variant-comparison-preview',
      ParentVersionId: 'ov-agent-answers-parent',
      NewOrganismVersionId: 'ov-agent-answers-comparison-preview',
      AppRef: 'arni-labs/agent-answers@comparison-preview',
      CanonicalAppRef: 'arni-labs/agent-answers@comparison-preview-canonical',
      ProductionTenant: 'default',
      Materialized: false,
      Summary: 'Winner selected and waiting for production install.'
    }),
    row('Promotion', 'promotion-broken-publish', 'Failed', {
      EpisodeId: 'episode-broken-publish',
      WinningVariantId: 'variant-broken-publish',
      ParentVersionId: 'ov-agent-answers-parent',
      NewOrganismVersionId: 'ov-agent-answers-broken-publish',
      AppRef: 'arni-labs/agent-answers@broken-publish',
      ProductionTenant: 'default',
      Materialized: false,
      MaterializationFailed: true,
      FailureReason: 'Genesis publish rejected the app bundle digest.'
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
  SelectionProtocols: [
    row('SelectionProtocol', 'protocol-citation-memory', 'Frozen', {
      EpisodeId: 'episode-citation-memory',
      SelectionStatement: 'Prefer variants that improve follow-up source recall without correctness regressions.',
      MetricDefinitionIdsJson: JSON.stringify(['metric-source-recall']),
      EliminationRuleIdsJson: JSON.stringify(['rule-visible-source-trail']),
      ScoringRuleIdsJson: JSON.stringify(['score-source-recall']),
      SelectedBy: 'codex-chat',
      HumanDecisionSummary: 'Human approved three simulated users and a frozen evaluator before start.',
      FrozenAt: '2026-05-27T08:00:00Z'
    })
  ],
  EliminationRules: [
    row('EliminationRule', 'rule-visible-source-trail', 'Active', {
      EpisodeId: 'episode-citation-memory',
      RuleStatement: 'Eliminate hidden citations',
      MetricIdsJson: JSON.stringify(['metric-source-recall']),
      ThresholdJson: JSON.stringify({ min: 0.8 })
    })
  ],
  ScoringRules: [
    row('ScoringRule', 'score-source-recall', 'Active', {
      EpisodeId: 'episode-citation-memory',
      RuleStatement: 'Prefer source recall',
      MetricIdsJson: JSON.stringify(['metric-source-recall']),
      Weight: '0.7'
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
      GenerationId: 'generation-citation-memory-2',
      VariantId: 'variant-memory-panel',
      EvaluationStageId: 'stage-compile',
      MetricsJson: JSON.stringify({ build: 'ok' }),
      EvidenceArtifactId: 'evidence-a-compile',
      Summary: 'Build and checks passed'
    }),
    row('StageResult', 'stage-result-a-user', 'Running', {
      EpisodeId: 'episode-citation-memory',
      GenerationId: 'generation-citation-memory-2',
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
      DesiredDirection: 'higher',
      HigherIsBetter: 'true',
      Description: 'Simulated users can recover the answer source trail.'
    })
  ],
  SimulatedUserPlans: [
    row('SimulatedUserPlan', 'sim-plan-citation-memory', 'Frozen', {
      EpisodeId: 'episode-citation-memory',
      UsersPerVariant: '3',
      RunsPerPersona: '2',
      PersonasJson: JSON.stringify([
        { name: 'answer seeker', goal_style: 'asks a follow-up source question' },
        { name: 'careful reviewer', goal_style: 'checks whether citations are readable' },
        { name: 'returning maintainer', goal_style: 'verifies accepted answers remain visible' }
      ]),
      GoalsJson: JSON.stringify([
        'Ask a question and find the accepted answer source trail.',
        'Accept an answer and confirm citations remain readable.'
      ]),
      HumanDecisionSummary: 'Codex-as-human required 3 personas and 2 runs per persona per variant.',
      FrozenAt: '2026-05-27T08:01:00Z'
    })
  ],
  Mutations: [
    row('Mutation', 'mutation-memory-panel', 'Recorded', {
      VariantId: 'variant-memory-panel',
      Summary: 'Adds a source memory panel with readable citation readback.',
      ChangedFilesJson: JSON.stringify([
        'apps/agent-answers/specs/answer.ioa.toml',
        'apps/agent-answers/specs/model.csdl.xml'
      ]),
      DiffRef: 'git://arni-labs/agent-answers/compare/seed-parent...variant-a',
      BrainRunId: 'brain-variant-a'
    }),
    row('Mutation', 'mutation-hidden-citations', 'Recorded', {
      VariantId: 'variant-hidden-citations',
      Summary: 'Stores citations invisibly without showing a source trail.',
      ChangedFilesJson: JSON.stringify(['apps/agent-answers/specs/answer.ioa.toml']),
      DiffRef: 'git://arni-labs/agent-answers/compare/seed-parent...variant-b',
      BrainRunId: 'brain-variant-b'
    }),
    row('Mutation', 'mutation-flat-sources', 'Recorded', {
      VariantId: 'variant-flat-sources',
      Summary: 'Adds a flat source note without preserving follow-up relationship.',
      ChangedFilesJson: JSON.stringify(['apps/agent-answers/APP.md']),
      DiffRef: 'git://arni-labs/agent-answers/compare/seed-parent...variant-flat',
      BrainRunId: 'brain-variant-flat'
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
    row('EvidenceArtifact', 'evidence-signal-citation', 'Linked', {
      ArtifactKind: 'datadog_signal',
      Uri: 'https://app.datadoghq.com/logs?query=service%3Aagent-answers%20source-context',
      Summary: 'Datadog and simulated-user evidence showed repeated source-context loss.',
      TargetEntityType: 'Signal',
      TargetEntityId: 'sig-unmet-citation'
    }),
    row('EvidenceArtifact', 'evidence-b-user', 'Linked', {
      ArtifactKind: 'simulated_user_trace',
      Uri: 'https://app.datadoghq.com/logs?query=service%3Atemperpaw%20variant-hidden-citations',
      Summary: 'AI user could not locate citations after the follow-up.',
      CorrelationJson: JSON.stringify({
        output: {
          evidence_scope: [
            {
              surface: 'logs',
              query: 'service:temperpaw variant-hidden-citations',
              result_summary: 'No user-visible citation trail appeared in the simulated-user run.',
              datadog_url:
                'https://app.datadoghq.com/logs?query=service%3Atemperpaw%20variant-hidden-citations'
            }
          ]
        }
      }),
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

function cloneDirectedCollections(): Record<string, EntityRow[]> {
  return Object.fromEntries(
    Object.entries(directedCollectionFixtures).map(([collection, rows]) => [
      collection,
      rows.map((item) => JSON.parse(JSON.stringify(item)) as EntityRow)
    ])
  );
}

async function mockOData(page: Page) {
  const directedCollections = cloneDirectedCollections();

  await page.route('**/tdata/Apps', async (route) => {
    await route.fulfill({ json: { value: apps } });
  });
  await page.route('**/tdata/Lineages', async (route) => {
    await route.fulfill({ json: { value: lineages } });
  });
  await page.route('**/tdata/Closures', async (route) => {
    await route.fulfill({ json: { value: closures } });
  });
  await page.route('**/tdata/AppInstallations', async (route) => {
    await route.fulfill({ json: { value: installations } });
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
  await page.route('**/tdata/Refs*', async (route) => {
    await route.fulfill({ json: { value: refs } });
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
  await page.goto('/genesis/app/app-alice-notes');

  await expect(page.getByRole('heading', { name: 'alice-notes' })).toBeVisible();
  await expect(page.getByRole('button', { name: /app.toml/ })).toBeVisible();
  await page.getByRole('button', { name: /app.toml/ }).click();
  await expect(page.getByText('name = "alice-notes"')).toBeVisible();

  await page.getByRole('tab', { name: 'Versions' }).click();
  const versionsPanel = page.getByRole('tabpanel', { name: 'Versions' });
  await expect(page.getByText('Version Chain')).toBeVisible();
  await expect(versionsPanel.getByText('Repo head')).toBeVisible();
  await expect(
    versionsPanel.getByText(/is pushed, but Genesis latest is still/)
  ).toBeVisible();
  await expect(page.getByRole('button', { name: /add notes app/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /initial notes app/ })).toBeVisible();
  await expect(versionsPanel.getByText(`alice/alice-notes@${childHash}`, { exact: true })).toBeVisible();
  await page.getByRole('button', { name: /initial notes app/ }).click();
  await expect(versionsPanel.getByText(`alice/alice-notes@${oldChildHash}`, { exact: true })).toBeVisible();

  await page.getByRole('tab', { name: 'Overview' }).click();
  await expect(page.getByText('Alice', { exact: true })).toBeVisible();
  await expect(page.getByText('Runtime provenance')).toBeVisible();
  await expect(page.getByText('live', { exact: true })).toBeVisible();
  await expect(page.getByText('pinned', { exact: true })).toBeVisible();
  await expect(page.getByText(/Repo head .* is newer than Genesis latest/)).toBeVisible();
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
  await expect(installPanel.getByText('"FollowPolicy":"pinned"')).toBeVisible();
  await expect(
    installPanel.getByText(
      `temper install alice/alice-notes@${childHash} --tenant default --url`
    )
  ).toBeVisible();
  await expect(installPanel.getByText('--follow-policy pinned')).toBeVisible();
  await expect(
    installPanel.getByText(`temper.install_app({"app_ref":"alice/alice-notes@${childHash}"`)
  ).toBeVisible();
  await expect(installPanel.getByText('"follow_policy":"pinned"')).toBeVisible();
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
  await expect(page.getByText('Authored Protocol', { exact: true })).toBeVisible();
  await expect(page.getByText('codex-as-human-director')).toBeVisible();
  await expect(page.getByText('Protocol And Lab')).toBeVisible();
  await expect(page.getByText('3 personas × 2 runs')).toBeVisible();
  await expect(page.getByText('genesis://nerdsane/agent-answers-evaluation@frozen')).toBeVisible();
  await expect(page.getByText('Metrics & Rules')).toBeVisible();
  await expect(page.getByText('Source recall', { exact: true })).toBeVisible();
  await expect(page.getByText('Eliminate hidden citations')).toBeVisible();
  await expect(page.getByText('Weight 0.7')).toBeVisible();
  await expect(page.getByText('Generation Topology')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Generation 1' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Generation 2' })).toBeVisible();
  await expect(page.getByText('evidence-fed follow-up')).toBeVisible();
  await expect(
    page.getByText('All variants were eliminated before selection. Queued follow-up generation 2')
  ).toBeVisible();
  await expect(page.getByText('Simulated users still could not find the source trail.').first()).toBeVisible();
  await expect(page.getByText('Materialized').first()).toBeVisible();
  await expect(page.getByText('Materializing')).toBeVisible();
  await expect(page.getByText('Failed Installs')).toBeVisible();
  await expect(page.getByText('Hot-loaded').first()).toBeVisible();
  await expect(
    page.getByText('temper://tenant/default/app/arni-labs/agent-answers@citation-winner', {
      exact: true
    })
  ).toBeVisible();
  await expect(page.getByText('Answer citations that survive follow-up').first()).toBeVisible();

  await page.getByRole('button', { name: 'Directions' }).click();
  await expect(page.getByText('Directions View')).toBeVisible();
  await expect(page.getByText('Publish comparison winner')).toBeVisible();
  await expect(page.getByText('growth-human-gated').first()).toBeVisible();
  await expect(page.getByText('Follow-up answers need durable citation memory.').first()).toBeVisible();
  await expect(page.getByText('User agent could not preserve source context')).toBeVisible();
  await expect(page.getByText('Datadog and simulated-user evidence')).toBeVisible();

  await page.getByRole('button', { name: 'Organism Genealogy' }).click({ force: true });
  await expect(page.getByText('Specimen History')).toBeVisible();
  await expect(page.getByText('current parent', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('mutation edge')).toBeVisible();
  await expect(page.getByText('winner variant')).toBeVisible();
  await expect(page.getByText('Winner: Adds a source memory panel')).toBeVisible();
  await expect(page.getByText('Citation memory evolved from the seed organism')).toBeVisible();

  await page.getByRole('button', { name: 'Direction Detail' }).click();
  const evolutionOverflow = await page.evaluate(() => {
    const root = document.documentElement;
    return root.scrollWidth - root.clientWidth;
  });
  expect(evolutionOverflow).toBeLessThanOrEqual(1);

  await page.getByRole('button', { name: /Open episode Publish comparison winner/ }).click();
  await expect(page.getByText('Hot-load pending').first()).toBeVisible();
  await expect(
    page.getByText('Winner selected. Promoter is publishing the canonical app ref')
  ).toBeVisible();
  await expect(page.getByText('Canonical ref: arni-labs/agent-answers@comparison-preview-canonical')).toBeVisible();
  await expect(page.getByText('Runtime: default')).toBeVisible();

  await page.getByRole('button', { name: /Open episode Recover failed publish/ }).click();
  await expect(page.getByText('Materialization failed').first()).toBeVisible();
  await expect(page.getByText('Genesis publish rejected the app bundle digest.')).toBeVisible();
  await expect(page.getByText('Runtime: default')).toBeVisible();

  await page.getByRole('button', { name: /Open episode Answer citations that survive follow-up/ }).click();

  await page.getByRole('button', { name: 'Inspect' }).first().click();
  await expect(page.getByText('App/spec diff')).toBeVisible();
  await expect(page.getByText('apps/agent-answers/specs/answer.ioa.toml')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Datadog', exact: true })).toHaveAttribute(
    'href',
    /app\.datadoghq\.com\/logs/
  );

  await page.getByRole('button', { name: 'Pause' }).click();
  await expect(page.getByText('Paused').first()).toBeVisible();

  await page.getByRole('button', { name: 'Pin' }).click({ force: true });
  await expect(page.getByText('Pinned').first()).toBeVisible();

  await page.getByRole('button', { name: 'Compare' }).first().click();
  await expect(page.getByText('Variant Compare')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Stores citations invisibly' }).first()).toBeVisible();

  expect(actionUrls.some((url) => url.includes('/Episodes') && url.includes('PauseEpisode'))).toBe(
    true
  );
  expect(
    actionUrls.some(
      (url) => url.includes('/ViabilityConstraints') && url.includes('PinViabilityConstraint')
    )
  ).toBe(true);
});
