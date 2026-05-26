// Barrel export for the studio data layer. The studio surface is
// stable; the impl is split across files so each one stays under
// 500 lines and has a single responsibility.

export type {
  ActionResult,
  Evolution,
  EvolutionStatus,
  EvolutionStudioSnapshot,
  StageResult,
  StageVerdict,
  Variant,
  VariantStatus,
} from './types';
export { STAGE_DEFAULT } from './types';

export { loadStudio } from './loader';

export {
  approveEvolution,
  pickWinner,
  rejectEvolution,
  revertEvolution,
} from './actions';

export {
  EVOLUTION_FLOW,
  evolutionStatusTone,
  flowIndex,
  isEvolutionLive,
  parseEvidence,
  selectStageResultForCell,
  selectVariantsForEvolution,
  variantStatusTone,
  verdictTone,
} from './helpers';

export { fixtureSnapshot } from './fixture';
