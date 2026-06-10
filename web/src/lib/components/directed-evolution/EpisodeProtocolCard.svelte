<script lang="ts">
  import { ClipboardCheck } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type { EvolutionEpisode } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    selectedEpisode: EvolutionEpisode;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
  };

  let { selectedEpisode, shortId, statusTone }: Props = $props();

  const protocolGraphReady = $derived(
    Boolean(selectedEpisode.adaptationGoalId) &&
    Boolean(selectedEpisode.selectionProtocolId) &&
      selectedEpisode.hasSimulatedUserPlan
  );
</script>

<div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
  <div class="flex items-center justify-between gap-2">
    <PanelTitle icon={ClipboardCheck} title="Authored Protocol" />
    <Badge tone={statusTone(selectedEpisode.status)}>{selectedEpisode.status}</Badge>
  </div>
  <div class="mt-3 grid gap-1.5 text-[11px]">
    <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
      <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">lane</span>
      <span class="min-w-0 truncate text-[var(--color-ink-soft)]">
        {selectedEpisode.autonomyLane || 'lane pending'}
      </span>
    </div>
    <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
      <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">episode</span>
      <span class="min-w-0 truncate text-[var(--color-ink-soft)]">{shortId(selectedEpisode.id, 22)}</span>
    </div>
    <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
      <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">director</span>
      <span class="min-w-0 truncate text-[var(--color-ink-soft)]">
        {selectedEpisode.plannedBy || selectedEpisode.startedBy || 'worker pending'}
      </span>
    </div>
    <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
      <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">graph</span>
      <span class="min-w-0 truncate text-[var(--color-ink-soft)]">
        {protocolGraphReady ? 'goal, constraints, stages, lab, and selection linked' : 'protocol graph incomplete'}
      </span>
    </div>
  </div>
  {#if selectedEpisode.planSummary || selectedEpisode.reason}
    <p class="mt-2 line-clamp-3 text-[11.5px] leading-snug text-[var(--color-muted)]">
      {selectedEpisode.planSummary || selectedEpisode.reason}
    </p>
  {/if}
</div>
