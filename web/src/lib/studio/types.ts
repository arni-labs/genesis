// Evolution Studio — type definitions.
//
// Mirrors the Evo.DE schema in genesis/specs/model.csdl.xml. Keep
// these in lockstep with the CSDL; the OData layer hands us strings
// and we shape them into these structs.

import type { EntityRow } from '../types';

export type EvolutionStatus =
  | 'IntentObserved'
  | 'Framed'
  | 'Generating'
  | 'Evaluating'
  | 'Selecting'
  | 'AwaitingApproval'
  | 'Merged'
  | 'Live'
  | 'Reverted';

export type VariantStatus = 'Proposed' | 'Evaluating' | 'Survived' | 'Killed';

export type StageVerdict = 'pass' | 'fail' | 'pending';

export type Evolution = {
  id: string;
  targetApp: string;
  targetTenant: string;
  fitnessSpecId: string;
  intent: string;
  problemStatement: string;
  autonomy: number;
  variantCount: number;
  winnerVariantId: string;
  mergedRef: string;
  status: EvolutionStatus;
  createdAt: string;
  raw?: EntityRow;
};

export type Variant = {
  id: string;
  evolutionId: string;
  branchRef: string;
  commitSha: string;
  currentStage: number;
  killedAtStage: string;
  objectiveTotal: string;
  status: VariantStatus;
  createdAt: string;
  raw?: EntityRow;
};

export type StageResult = {
  id: string;
  variantId: string;
  stageId: string;
  evaluator: string;
  verdict: StageVerdict;
  objectiveScores: string;
  evidence: string;
  createdAt: string;
  raw?: EntityRow;
};

export type EvolutionStudioSnapshot = {
  evolutions: Evolution[];
  variants: Variant[];
  stageResults: StageResult[];
  stageOrder: string[];
  source: 'live' | 'fixture' | 'mixed';
  warnings: string[];
};

export type ActionResult = {
  ok: boolean;
  status: number;
  message?: string;
};

// v1 stage list — locked in the plan: lexicographic
// [parse, l0, l1, budget]. Used as the default when no live data
// has populated the stage_id history yet.
export const STAGE_DEFAULT: ReadonlyArray<string> = [
  'parse',
  'l0',
  'l1',
  'budget',
];
