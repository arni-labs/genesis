// Studio mutations.
//
// Each control button (Pick winner / Approve / Revert) POSTs a
// bound Evo.DE action against /tdata/Evolutions('<id>')/Evo.DE.<Action>.
//
// Fitness functions are set ELSEWHERE — by the human talking to Claude
// Code (the NL→FitnessSpec compiler). The Studio is deliberately glass
// + yes/no; no fitness editor here.

import type { ActionResult } from './types';

const API_BASE = (import.meta.env.VITE_TEMPER_API_BASE ?? '').replace(/\/$/, '');
const TENANT_ID = import.meta.env.VITE_TEMPER_TENANT_ID ?? 'default';

async function dispatchEvolutionAction(
  evolutionId: string,
  action: string,
  body: Record<string, unknown> = {},
): Promise<ActionResult> {
  if (!API_BASE) {
    return {
      ok: false,
      status: 0,
      message: 'No live API base configured (set VITE_TEMPER_API_BASE).',
    };
  }
  const url = `${API_BASE}/tdata/Evolutions('${encodeURIComponent(evolutionId)}')/Evo.DE.${action}`;
  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
        'X-Tenant-Id': TENANT_ID,
      },
      body: JSON.stringify(body),
    });
    if (!resp.ok) {
      const text = await resp.text();
      return { ok: false, status: resp.status, message: text.slice(0, 200) };
    }
    return { ok: true, status: resp.status };
  } catch (err) {
    return {
      ok: false,
      status: 0,
      message: err instanceof Error ? err.message : String(err),
    };
  }
}

export function approveEvolution(id: string): Promise<ActionResult> {
  return dispatchEvolutionAction(id, 'Approve');
}

export function revertEvolution(id: string, reason: string): Promise<ActionResult> {
  return dispatchEvolutionAction(id, 'Revert', { reason });
}

export function pickWinner(
  id: string,
  winnerVariantId: string,
): Promise<ActionResult> {
  return dispatchEvolutionAction(id, 'Select', {
    WinnerVariantId: winnerVariantId,
  });
}

// There is intentionally no Reject action in the Evolution spec: v1
// surfaces only the spec actions (Frame / Select / Approve / Revert).
// We still export a `rejectEvolution` so the button shape stays open
// for Phase 2 — today it returns a clear 405 so the UI can show the
// message without falsely encouraging a click.
export function rejectEvolution(_id: string): Promise<ActionResult> {
  return Promise.resolve({
    ok: false,
    status: 405,
    message:
      'Reject is not a spec action in v1. Use "Revert" on a Live evolution, or kill individual variants from the bracket.',
  });
}
