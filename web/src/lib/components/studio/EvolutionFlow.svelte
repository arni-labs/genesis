<script lang="ts">
  // Horizontal step indicator: IntentObserved -> ... -> Live.
  // Highlights the current state in lime; passed states get a
  // primary tint; future states stay faint.

  import {
    EVOLUTION_FLOW,
    flowIndex,
    type EvolutionStatus,
  } from '$lib/studio';

  let { status }: { status: EvolutionStatus } = $props();

  const reached = $derived(flowIndex(status));
</script>

<ol
  class="flex flex-wrap items-center gap-1 font-mono text-[10px] uppercase tracking-[0.10em]"
  aria-label="Evolution flow"
>
  {#each EVOLUTION_FLOW as state, i (state)}
    {@const isCurrent = state === status}
    {@const isPast = i < reached}
    <li class="flex items-center gap-1">
      <span
        class={[
          'inline-flex h-5 items-center gap-1 rounded-full border px-1.5 transition-colors duration-[var(--duration-soft)]',
          isCurrent
            ? 'border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)] text-[var(--color-ink)]'
            : isPast
              ? 'border-[var(--color-primary)]/30 bg-[var(--color-primary-soft)] text-[var(--color-primary)]'
              : 'border-[var(--color-border)] bg-white text-[var(--color-faint)]',
        ].join(' ')}
      >
        {#if isCurrent}
          <span class="v-pixel-square" aria-hidden="true"></span>
        {/if}
        <span>{state}</span>
      </span>
      {#if i < EVOLUTION_FLOW.length - 1}
        <span class="text-[var(--color-faint)]">·</span>
      {/if}
    </li>
  {/each}
</ol>
