// Studio loader. Fetches the three Evo.DE entity sets from genesis's
// OData surface and degrades to the fixture when the data plane isn't
// reachable. Adds a derived `stageOrder` so the Bracket can render
// rows in a consistent order.

import type { EntityRow } from '../types';
import type { EvolutionStudioSnapshot, StageResult } from './types';
import { STAGE_DEFAULT } from './types';
import {
  normalizeEvolution,
  normalizeStageResult,
  normalizeVariant,
} from './normalize';
import { fixtureSnapshot } from './fixture';

const API_BASE = (import.meta.env.VITE_TEMPER_API_BASE ?? '').replace(/\/$/, '');
const TENANT_ID = import.meta.env.VITE_TEMPER_TENANT_ID ?? 'default';

async function fetchCollection(name: string): Promise<EntityRow[]> {
  if (
    !API_BASE &&
    typeof window !== 'undefined' &&
    window.location?.protocol === 'file:'
  ) {
    // No backend in static preview mode — force fixture.
    throw new Error('no API base configured');
  }
  const url = `${API_BASE}/tdata/${name}`;
  const resp = await fetch(url, {
    headers: {
      Accept: 'application/json',
      'X-Tenant-Id': TENANT_ID,
    },
  });
  if (!resp.ok) {
    throw new Error(`${name}: ${resp.status} ${resp.statusText}`);
  }
  const json = await resp.json();
  return Array.isArray(json?.value) ? json.value : [];
}

function describeErr(collection: string, err: unknown): string {
  if (err instanceof Error) return `${collection}: ${err.message}`;
  return `${collection}: ${String(err)}`;
}

function deriveStageOrder(results: StageResult[]): string[] {
  // We use the first variant's stage_id sequence (sorted by createdAt)
  // as the canonical order; fall back to STAGE_DEFAULT.
  const byVariant = new Map<string, StageResult[]>();
  for (const r of results) {
    const list = byVariant.get(r.variantId) ?? [];
    list.push(r);
    byVariant.set(r.variantId, list);
  }
  for (const list of byVariant.values()) {
    list.sort((a, b) => a.createdAt.localeCompare(b.createdAt));
    if (list.length === 0) continue;
    const order: string[] = [];
    for (const r of list) {
      if (!order.includes(r.stageId)) order.push(r.stageId);
    }
    if (order.length === 0) continue;
    // Pad with defaults so the bracket is consistent across episodes.
    for (const s of STAGE_DEFAULT) {
      if (!order.includes(s)) order.push(s);
    }
    return order;
  }
  return STAGE_DEFAULT.slice();
}

export async function loadStudio(
  opts: { forceFixture?: boolean } = {},
): Promise<EvolutionStudioSnapshot> {
  if (opts.forceFixture) {
    return { ...fixtureSnapshot(), source: 'fixture' };
  }

  const warnings: string[] = [];
  let liveCount = 0;
  let evolutions = [] as ReturnType<typeof normalizeEvolution>[];
  let variants = [] as ReturnType<typeof normalizeVariant>[];
  let stageResults = [] as ReturnType<typeof normalizeStageResult>[];

  try {
    const rows = await fetchCollection('Evolutions');
    evolutions = rows.map(normalizeEvolution);
    liveCount += rows.length;
  } catch (err) {
    warnings.push(describeErr('Evolutions', err));
  }
  try {
    const rows = await fetchCollection('Variants');
    variants = rows.map(normalizeVariant);
    liveCount += rows.length;
  } catch (err) {
    warnings.push(describeErr('Variants', err));
  }
  try {
    const rows = await fetchCollection('StageResults');
    stageResults = rows.map(normalizeStageResult);
    liveCount += rows.length;
  } catch (err) {
    warnings.push(describeErr('StageResults', err));
  }

  if (liveCount === 0) {
    const fx = fixtureSnapshot();
    return { ...fx, source: 'fixture', warnings: warnings.concat(fx.warnings) };
  }

  return {
    evolutions,
    variants,
    stageResults,
    stageOrder: deriveStageOrder(stageResults),
    source: 'live',
    warnings,
  };
}
