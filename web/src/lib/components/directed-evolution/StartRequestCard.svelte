<script lang="ts">
  import { ClipboardCheck } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type { EvolutionEpisode, EvolutionEpisodeStartRequest } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    selectedEpisode: EvolutionEpisode;
    startRequest: EvolutionEpisodeStartRequest | null;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
  };

  let { selectedEpisode, startRequest, shortId, statusTone }: Props = $props();
</script>

<div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
  <div class="flex items-center justify-between gap-2">
    <PanelTitle icon={ClipboardCheck} title="Start Request" />
    {#if startRequest}
      <Badge tone={statusTone(startRequest.status)}>{startRequest.status}</Badge>
    {:else}
      <Badge tone="neutral">not recorded</Badge>
    {/if}
  </div>
  {#if startRequest}
    <div class="mt-3 grid gap-1.5 text-[11px]">
      <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
        <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">lane</span>
        <span class="min-w-0 truncate text-[var(--color-ink-soft)]">
          {startRequest.autonomyLane || selectedEpisode.autonomyLane || 'lane pending'}
        </span>
      </div>
      <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
        <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">request</span>
        <span class="min-w-0 truncate text-[var(--color-ink-soft)]">{shortId(startRequest.id, 22)}</span>
      </div>
      <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
        <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">brain</span>
        <span class="min-w-0 truncate text-[var(--color-ink-soft)]">
          {startRequest.requestedBy || startRequest.startedBy || selectedEpisode.startedBy || 'brain pending'}
        </span>
      </div>
      <div class="grid grid-cols-[84px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
        <span class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">contract</span>
        <span class="min-w-0 truncate text-[var(--color-ink-soft)]">
          {startRequest.hasContract ? 'present' : 'missing'}
        </span>
      </div>
    </div>
    {#if startRequest.summary || startRequest.reason}
      <p class="mt-2 line-clamp-3 text-[11.5px] leading-snug text-[var(--color-muted)]">
        {startRequest.summary || startRequest.reason}
      </p>
    {/if}
  {:else}
    <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
      No start request is linked to this episode.
    </p>
  {/if}
</div>
