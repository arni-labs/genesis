<script lang="ts">
  // One cell in the elimination bracket.
  //
  // - PASS  → green tile with a pixel mark
  // - KILLED → red tile; hovering reveals the cause-of-death (the
  //            counterexample parsed out of StageResult.evidence)
  // - PENDING → faint dashed border
  //
  // The whole cell is a button so clicking opens a detail popover via
  // the `onInspect` callback (the parent owns the detail panel state).

  import { parseEvidence, verdictTone, type StageResult } from '$lib/studio';

  type Props = {
    result: StageResult | null;
    stageId: string;
    variantStatus: string;
    onInspect?: (result: StageResult | null, stageId: string) => void;
  };

  let { result, stageId, variantStatus, onInspect }: Props = $props();

  const verdict = $derived(result?.verdict ?? 'pending');
  const tone = $derived(verdictTone(verdict));
  const evidence = $derived(parseEvidence(result?.evidence ?? ''));

  const tooltip = $derived.by(() => {
    if (verdict === 'pass') {
      return `PASS — ${result?.evaluator ?? stageId}`;
    }
    if (verdict === 'fail') {
      const violation = evidence.violation ?? 'killed';
      const property = evidence.property ? ` [${evidence.property}]` : '';
      return `KILLED at ${stageId}${property}: ${violation}`;
    }
    return variantStatus === 'Killed'
      ? `(skipped — killed earlier)`
      : `pending — not yet evaluated`;
  });

  function handleClick() {
    onInspect?.(result, stageId);
  }
</script>

<button
  type="button"
  onclick={handleClick}
  title={tooltip}
  aria-label={tooltip}
  class={[
    'group relative flex h-12 w-full items-center justify-center rounded-[var(--radius-sm)] border font-mono text-[10px] uppercase tracking-[0.08em] transition-colors duration-[var(--duration-soft)]',
    tone === 'success'
      ? 'border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)] text-[var(--color-ink)] hover:bg-[rgba(183,255,26,0.30)]'
      : tone === 'danger'
        ? 'border-[var(--color-error)]/40 bg-[rgba(217,45,75,0.10)] text-[#7a1830] hover:bg-[rgba(217,45,75,0.16)]'
        : 'border-dashed border-[var(--color-border)] bg-white text-[var(--color-faint)] hover:bg-[var(--color-surface-soft)]',
  ].join(' ')}
>
  {#if tone === 'success'}
    <span class="v-pixel-square" aria-hidden="true"></span>
    <span class="ml-1">PASS</span>
  {:else if tone === 'danger'}
    <span class="text-[#7a1830]">KILLED</span>
  {:else}
    <span>—</span>
  {/if}

  <!-- Hover veil: shows a one-line cause-of-death directly on the cell.
       Kept compact; the inspector panel shows the full counterexample. -->
  {#if verdict === 'fail'}
    <span
      class="pointer-events-none absolute inset-x-0 top-full z-10 mt-1 hidden max-w-[260px] origin-top rounded-[var(--radius-sm)] border border-[var(--color-error)]/30 bg-white px-2 py-1.5 text-[10px] font-normal text-[#7a1830] shadow-[var(--shadow-md)] group-hover:block group-focus-visible:block"
    >
      <span class="block font-semibold">cause of death</span>
      <span class="block whitespace-normal break-words">
        {evidence.violation ?? 'no counterexample recorded'}
      </span>
    </span>
  {/if}
</button>
