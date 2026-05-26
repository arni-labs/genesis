<script lang="ts">
  // The elimination bracket.
  //
  // Variants are columns, FitnessSpec stages are rows. Each cell is a
  // StageCell showing PASS/KILLED/pending. The header row labels each
  // variant; clicking a variant header makes it the selected winner
  // (Phase 2 wiring — surfaced now to keep the visual hierarchy).
  //
  // Visual: a single CSS grid; columns sized 1fr, rows ~14 / 48px.

  import { Badge } from '$lib/components/ui';
  import StageCell from './StageCell.svelte';
  import {
    selectStageResultForCell,
    variantStatusTone,
    type EvolutionStudioSnapshot,
    type Variant,
    type StageResult,
  } from '$lib/studio';

  type Props = {
    snapshot: EvolutionStudioSnapshot;
    variants: Variant[];
    selectedWinnerId: string;
    canSelectWinner: boolean;
    onPickWinner?: (variantId: string) => void;
    onInspectCell?: (result: StageResult | null, stageId: string, variant: Variant) => void;
  };

  let {
    snapshot,
    variants,
    selectedWinnerId,
    canSelectWinner,
    onPickWinner,
    onInspectCell,
  }: Props = $props();

  const stages = $derived(snapshot.stageOrder);

  function shortRef(v: Variant): string {
    if (v.commitSha) return v.commitSha.slice(0, 8);
    if (v.branchRef) return v.branchRef.replace(/^evolver\//, '');
    return v.id.slice(0, 8);
  }
</script>

{#if variants.length === 0}
  <div class="rounded-[var(--radius-md)] border border-dashed border-[var(--color-border)] bg-white px-4 py-8 text-center font-sans text-[12px] text-[var(--color-muted)]">
    No variants for this evolution yet. (Generator may still be running.)
  </div>
{:else}
  <div
    class="grid gap-1.5 overflow-x-auto pb-2 v-scrollbar"
    style="grid-template-columns: 90px repeat({variants.length}, minmax(120px, 1fr));"
  >
    <!-- Header row: stage label cell (empty) + each variant -->
    <div></div>
    {#each variants as v (v.id)}
      <button
        type="button"
        onclick={() => canSelectWinner && onPickWinner?.(v.id)}
        disabled={!canSelectWinner}
        class={[
          'flex h-14 flex-col items-start justify-between rounded-[var(--radius-sm)] border bg-white px-2 py-1.5 text-left transition-colors duration-[var(--duration-soft)]',
          v.id === selectedWinnerId
            ? 'border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)]'
            : canSelectWinner
              ? 'border-[var(--color-border)] hover:border-[var(--color-primary)]/40 hover:bg-[var(--color-primary-soft)]'
              : 'border-[var(--color-border)] opacity-100',
          !canSelectWinner ? 'cursor-default' : 'cursor-pointer',
        ].join(' ')}
        title={canSelectWinner ? `Pick ${shortRef(v)} as winner` : `Variant ${shortRef(v)}`}
      >
        <span class="truncate font-mono text-[10px] tracking-[0.08em] text-[var(--color-muted)]">
          {shortRef(v)}
        </span>
        <Badge tone={variantStatusTone(v.status)} pixel={v.status === 'Survived'}>
          {v.status}
        </Badge>
      </button>
    {/each}

    <!-- Body: one row per stage -->
    {#each stages as stage (stage)}
      <div class="flex h-12 items-center justify-end pr-2 font-mono text-[11px] uppercase tracking-[0.08em] text-[var(--color-ink-soft)]">
        {stage}
      </div>
      {#each variants as v (v.id + ':' + stage)}
        {@const result = selectStageResultForCell(snapshot, v.id, stage)}
        <StageCell
          result={result}
          stageId={stage}
          variantStatus={v.status}
          onInspect={(r, sid) => onInspectCell?.(r, sid, v)}
        />
      {/each}
    {/each}
  </div>
{/if}
