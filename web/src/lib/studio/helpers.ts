// Studio helpers — derived selectors, tone helpers, and evidence
// parsing. Pure (no I/O), so they can be safely called from templates.

import { stringValue } from '../api';
import type {
  Evolution,
  EvolutionStatus,
  EvolutionStudioSnapshot,
  StageResult,
  StageVerdict,
  Variant,
  VariantStatus,
} from './types';

export function selectVariantsForEvolution(
  snapshot: EvolutionStudioSnapshot,
  evolutionId: string,
): Variant[] {
  return snapshot.variants
    .filter((v) => v.evolutionId === evolutionId)
    .sort((a, b) => a.createdAt.localeCompare(b.createdAt));
}

export function selectStageResultForCell(
  snapshot: EvolutionStudioSnapshot,
  variantId: string,
  stageId: string,
): StageResult | null {
  return (
    snapshot.stageResults.find(
      (r) => r.variantId === variantId && r.stageId === stageId,
    ) ?? null
  );
}

export function isEvolutionLive(e: Evolution): boolean {
  return e.status === 'Live';
}

// ─── Tone helpers ──────────────────────────────────────────────────

export function evolutionStatusTone(
  status: EvolutionStatus,
): 'success' | 'warning' | 'danger' | 'neutral' | 'primary' | 'secondary' {
  switch (status) {
    case 'Live':
      return 'success';
    case 'Merged':
      return 'primary';
    case 'AwaitingApproval':
      return 'warning';
    case 'Selecting':
      return 'secondary';
    case 'Reverted':
      return 'danger';
    default:
      return 'neutral';
  }
}

export function variantStatusTone(
  status: VariantStatus,
): 'success' | 'warning' | 'danger' | 'neutral' {
  switch (status) {
    case 'Survived':
      return 'success';
    case 'Killed':
      return 'danger';
    case 'Evaluating':
      return 'warning';
    default:
      return 'neutral';
  }
}

export function verdictTone(v: StageVerdict): 'success' | 'danger' | 'neutral' {
  if (v === 'pass') return 'success';
  if (v === 'fail') return 'danger';
  return 'neutral';
}

// ─── Evidence parsing ──────────────────────────────────────────────

export type ParsedEvidence = {
  stage?: string;
  property?: string;
  violation?: string;
  counterexample?: unknown;
  raw?: string;
};

export function parseEvidence(evidence: string): ParsedEvidence {
  if (!evidence || !evidence.trim()) return {};
  try {
    const parsed = JSON.parse(evidence);
    if (parsed && typeof parsed === 'object') {
      const obj = parsed as Record<string, unknown>;
      const ce = obj.counterexample as Record<string, unknown> | undefined;
      return {
        stage: stringValue(obj.stage),
        property: stringValue(obj.property),
        violation:
          stringValue(obj.violation) ?? stringValue(ce?.violation),
        counterexample: obj.counterexample,
        raw: evidence,
      };
    }
  } catch (_) {
    /* fall through */
  }
  return { raw: evidence };
}

// ─── Flow indicator ────────────────────────────────────────────────

const FLOW: EvolutionStatus[] = [
  'IntentObserved',
  'Framed',
  'Generating',
  'Evaluating',
  'Selecting',
  'AwaitingApproval',
  'Merged',
  'Live',
];

export function flowIndex(status: EvolutionStatus): number {
  const i = FLOW.indexOf(status);
  return i < 0 ? 0 : i;
}

export const EVOLUTION_FLOW: ReadonlyArray<EvolutionStatus> = FLOW;
