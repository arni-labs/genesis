// Studio fixture — renders an "interesting" elimination episode mid
// flight (some variants killed at different stages, one survived,
// plus a second episode in the Live state so the celebration banner
// shows up). Used when the data plane isn't reachable, OR when the
// designer toggles "Use fixture" in the UI.
//
// IMPORTANT: this fixture is read-only sample data. Mutations dispatched
// from the UI will fail with status=0 (no API base) — the action
// dispatcher in actions.ts handles that gracefully.

import type {
  EvolutionStudioSnapshot,
  Evolution,
  Variant,
  StageResult,
  StageVerdict,
} from './types';
import { STAGE_DEFAULT } from './types';

export function fixtureSnapshot(): EvolutionStudioSnapshot {
  const evolutionId = '00000001-0000-4000-8000-000000000001';
  const evolutionLiveId = '00000002-0000-4000-8000-000000000002';

  const variantIds = [
    'v-001-add-downvote',
    'v-002-no-counter',
    'v-003-bad-guard',
    'v-004-runaway-loop',
    'v-005-clean-impl',
  ];
  const liveVariant = 'v-100-merged-downvote';

  const evolutions: Evolution[] = [
    {
      id: evolutionId,
      targetApp: 'stackoverflow-agents',
      targetTenant: 'stackoverflow-agents',
      fitnessSpecId: 'fs-001-downvote',
      intent: 'agents want to downvote low-quality answers',
      problemStatement:
        'Add Downvote action + downvotes counter to Answer; keep ScoreConsistent.',
      autonomy: 0,
      variantCount: variantIds.length,
      winnerVariantId: 'v-005-clean-impl',
      mergedRef: '',
      status: 'AwaitingApproval',
      createdAt: '2026-05-26T01:30:00Z',
    },
    {
      id: evolutionLiveId,
      targetApp: 'stackoverflow-agents',
      targetTenant: 'stackoverflow-agents',
      fitnessSpecId: 'fs-001-downvote',
      intent: 'agents want a "needs improvement" flag (demo)',
      problemStatement:
        'Earlier episode — already merged & live. Here so the studio shows the LIVE banner.',
      autonomy: 1,
      variantCount: 1,
      winnerVariantId: liveVariant,
      mergedRef: 'main@deadbeef',
      status: 'Live',
      createdAt: '2026-05-25T18:00:00Z',
    },
  ];

  const variants: Variant[] = [
    {
      id: variantIds[0],
      evolutionId,
      branchRef: 'evolver/v-001-add-downvote',
      commitSha: 'aaaa1111',
      currentStage: 2,
      killedAtStage: 'l1',
      objectiveTotal: '{"parse":1,"l0":1,"l1":0}',
      status: 'Killed',
      createdAt: '2026-05-26T01:31:00Z',
    },
    {
      id: variantIds[1],
      evolutionId,
      branchRef: 'evolver/v-002-no-counter',
      commitSha: 'bbbb2222',
      currentStage: 1,
      killedAtStage: 'l0',
      objectiveTotal: '{"parse":1,"l0":0}',
      status: 'Killed',
      createdAt: '2026-05-26T01:31:30Z',
    },
    {
      id: variantIds[2],
      evolutionId,
      branchRef: 'evolver/v-003-bad-guard',
      commitSha: 'cccc3333',
      currentStage: 0,
      killedAtStage: 'parse',
      objectiveTotal: '{"parse":0}',
      status: 'Killed',
      createdAt: '2026-05-26T01:32:00Z',
    },
    {
      id: variantIds[3],
      evolutionId,
      branchRef: 'evolver/v-004-runaway-loop',
      commitSha: 'dddd4444',
      currentStage: 3,
      killedAtStage: 'budget',
      objectiveTotal: '{"parse":1,"l0":1,"l1":1,"budget":0}',
      status: 'Killed',
      createdAt: '2026-05-26T01:32:30Z',
    },
    {
      id: variantIds[4],
      evolutionId,
      branchRef: 'evolver/v-005-clean-impl',
      commitSha: 'eeee5555',
      currentStage: 4,
      killedAtStage: '',
      objectiveTotal: '{"parse":1,"l0":1,"l1":1,"budget":1}',
      status: 'Survived',
      createdAt: '2026-05-26T01:33:00Z',
    },
    {
      id: liveVariant,
      evolutionId: evolutionLiveId,
      branchRef: 'evolver/v-100-merged-downvote',
      commitSha: 'f00ff00f',
      currentStage: 4,
      killedAtStage: '',
      objectiveTotal: '{"parse":1,"l0":1,"l1":1,"budget":1}',
      status: 'Survived',
      createdAt: '2026-05-25T18:15:00Z',
    },
  ];

  const stages = STAGE_DEFAULT;
  const stageResults: StageResult[] = [];
  function rec(
    variantId: string,
    stageId: string,
    verdict: StageVerdict,
    evidence = '',
  ) {
    stageResults.push({
      id: `sr-${variantId}-${stageId}`,
      variantId,
      stageId,
      evaluator: stageId,
      verdict,
      objectiveScores: verdict === 'pass' ? '{"score":1}' : '{"score":0}',
      evidence,
      createdAt: '2026-05-26T01:34:00Z',
    });
  }

  // v-001: parse pass, l0 pass, l1 fail (counter sign inverted)
  rec(variantIds[0], 'parse', 'pass');
  rec(variantIds[0], 'l0', 'pass');
  rec(
    variantIds[0],
    'l1',
    'fail',
    JSON.stringify({
      stage: 'l1',
      property: 'ScoreConsistent',
      counterexample: {
        trace: ['Upvote', 'Downvote', 'Downvote'],
        state: { upvotes: 1, downvotes: 2, score: -1 },
        violation: 'score != upvotes - downvotes (sign flipped)',
      },
    }),
  );

  // v-002: parse pass, l0 fail (no `downvotes` state declared)
  rec(variantIds[1], 'parse', 'pass');
  rec(
    variantIds[1],
    'l0',
    'fail',
    JSON.stringify({
      stage: 'l0',
      property: 'Reachable',
      counterexample: {
        violation: 'Downvote effect references undefined state var `downvotes`',
      },
    }),
  );

  // v-003: parse fail (TOML syntax error)
  rec(
    variantIds[2],
    'parse',
    'fail',
    JSON.stringify({
      stage: 'parse',
      violation: 'invalid action.guard: expected expression, got `>>`',
    }),
  );

  // v-004: parse + l0 + l1 pass, budget fail (loops too long)
  rec(variantIds[3], 'parse', 'pass');
  rec(variantIds[3], 'l0', 'pass');
  rec(variantIds[3], 'l1', 'pass');
  rec(
    variantIds[3],
    'budget',
    'fail',
    JSON.stringify({
      stage: 'budget',
      violation: 'WasmEngine::invoke exceeded fuel budget (2_000_000) by 3.4x',
    }),
  );

  // v-005: all pass
  for (const s of stages) rec(variantIds[4], s, 'pass');
  // Live variant: all pass
  for (const s of stages) rec(liveVariant, s, 'pass');

  return {
    evolutions,
    variants,
    stageResults,
    stageOrder: stages.slice(),
    source: 'fixture',
    warnings: [
      'rendering fixture snapshot — set VITE_TEMPER_API_BASE to a running platform for live data',
    ],
  };
}
