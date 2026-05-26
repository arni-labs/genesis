// Row-level normalizers: turn an OData `EntityRow` into one of our
// typed Evo.DE structs. Field lookup is case-tolerant (delegates to
// the `field`/`stringField` helpers in $lib/api).

import { field, stringField } from '../api';
import type { EntityRow } from '../types';
import type {
  Evolution,
  EvolutionStatus,
  StageResult,
  StageVerdict,
  Variant,
  VariantStatus,
} from './types';

export function normalizeEvolution(row: EntityRow): Evolution {
  return {
    id: stringField(row, 'Id'),
    targetApp: stringField(row, 'TargetApp'),
    targetTenant: stringField(row, 'TargetTenant'),
    fitnessSpecId: stringField(row, 'FitnessSpecId'),
    intent: stringField(row, 'Intent'),
    problemStatement: stringField(row, 'ProblemStatement'),
    autonomy: Number(field(row, 'Autonomy') ?? 0) || 0,
    variantCount: Number(field(row, 'VariantCount') ?? 0) || 0,
    winnerVariantId: stringField(row, 'WinnerVariantId'),
    mergedRef: stringField(row, 'MergedRef'),
    status: (stringField(row, 'Status') || 'IntentObserved') as EvolutionStatus,
    createdAt: stringField(row, 'CreatedAt'),
    raw: row,
  };
}

export function normalizeVariant(row: EntityRow): Variant {
  return {
    id: stringField(row, 'Id'),
    evolutionId: stringField(row, 'EvolutionId'),
    branchRef: stringField(row, 'BranchRef'),
    commitSha: stringField(row, 'CommitSha'),
    currentStage: Number(field(row, 'CurrentStage') ?? 0) || 0,
    killedAtStage: stringField(row, 'KilledAtStage'),
    objectiveTotal: stringField(row, 'ObjectiveTotal'),
    status: (stringField(row, 'Status') || 'Proposed') as VariantStatus,
    createdAt: stringField(row, 'CreatedAt'),
    raw: row,
  };
}

export function normalizeStageResult(row: EntityRow): StageResult {
  const verdictRaw = (stringField(row, 'Verdict') || 'pending').toLowerCase();
  const verdict: StageVerdict =
    verdictRaw === 'pass' || verdictRaw === 'fail'
      ? (verdictRaw as StageVerdict)
      : 'pending';
  return {
    id: stringField(row, 'Id'),
    variantId: stringField(row, 'VariantId'),
    stageId: stringField(row, 'StageId'),
    evaluator: stringField(row, 'Evaluator'),
    verdict,
    objectiveScores: stringField(row, 'ObjectiveScores'),
    evidence: stringField(row, 'Evidence'),
    createdAt: stringField(row, 'CreatedAt'),
    raw: row,
  };
}
