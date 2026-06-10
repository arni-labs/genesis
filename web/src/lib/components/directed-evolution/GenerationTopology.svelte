<script lang="ts">
  import {
    AlertTriangle,
    CheckCircle2,
    Eye,
    GitBranch,
    GitCompareArrows,
    Trophy
  } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type {
    EvolutionGeneration,
    EvolutionStageResult,
    EvolutionVariant
  } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    generations: EvolutionGeneration[];
    variants: EvolutionVariant[];
    stageResults: EvolutionStageResult[];
    comparedVariantIds: string[];
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
    onInspectVariant: (variantId: string) => void;
    onToggleCompare: (variant: EvolutionVariant) => void;
  };

  let {
    generations,
    variants,
    stageResults,
    comparedVariantIds,
    shortId,
    statusTone,
    onInspectVariant,
    onToggleCompare
  }: Props = $props();

  const deadStatuses = new Set(['Eliminated', 'Failed', 'Cancelled', 'NotSelected']);
  const viableStatuses = new Set(['Active', 'Selected', 'Promoted']);

  function generationVariants(generation: EvolutionGeneration): EvolutionVariant[] {
    return variants.filter((variant) => variant.generationId === generation.id);
  }

  function generationSurvivors(generation: EvolutionGeneration): EvolutionVariant[] {
    return generationVariants(generation).filter((variant) => viableStatuses.has(variant.status));
  }

  function generationWinner(generation: EvolutionGeneration): EvolutionVariant | null {
    return (
      variants.find((variant) => variant.id === generation.winnerVariantId) ??
      generationVariants(generation).find((variant) => variant.status === 'Promoted') ??
      null
    );
  }

  function generationLabel(generation: EvolutionGeneration, index: number): string {
    const generationIndex = generation.generationIndex || index + 1;
    return `Generation ${generationIndex}`;
  }

  function generationNote(generation: EvolutionGeneration): string {
    return (
      generation.failureReason ||
      generation.summary ||
      `${generationSurvivors(generation).length} of ${generationVariants(generation).length} variants remain viable.`
    );
  }

  function followUpLabel(generation: EvolutionGeneration, index: number): string {
    if (index >= generations.length - 1) return '';
    if (generation.failureReason.toLowerCase().includes('follow-up')) return 'evidence-fed follow-up';
    if (generation.status === 'Failed') return 'follow-up generated';
    return 'next generation';
  }

  function resultForVariant(variant: EvolutionVariant): EvolutionStageResult[] {
    return stageResults.filter((result) => result.variantId === variant.id);
  }

  function resultSummary(variant: EvolutionVariant): string {
    const results = resultForVariant(variant);
    if (!results.length) return 'No stage results yet';
    const passed = results.filter((result) => result.status === 'Passed').length;
    const eliminated = results.filter((result) => result.status === 'Eliminated').length;
    const running = results.filter((result) => result.status === 'Running').length;
    if (eliminated) return `${eliminated} elimination signal${eliminated === 1 ? '' : 's'}`;
    if (variant.status === 'NotSelected') return `selection-eliminated after ${passed}/${results.length} stages`;
    if (running) return `${running} stage${running === 1 ? '' : 's'} still running`;
    return `${passed}/${results.length} stages passed`;
  }

  function resultStatusColor(status: string): string {
    if (status === 'Passed') return 'bg-[var(--color-success)]';
    if (status === 'Eliminated' || status === 'Failed') return 'bg-[var(--color-error)]';
    if (status === 'Running') return 'bg-[var(--color-secondary)]';
    return 'bg-[var(--color-faint)]';
  }

  function variantReason(variant: EvolutionVariant): string {
    return (
      variant.reason ||
      variant.failureReason ||
      resultForVariant(variant).find((result) => result.reason || result.failureReason)?.reason ||
      resultForVariant(variant).find((result) => result.reason || result.failureReason)?.failureReason ||
      variant.summary ||
      'No variant note recorded yet.'
    );
  }

  function variantStatusLabel(status: string): string {
    if (status === 'NotSelected') return 'Selection-eliminated';
    return status;
  }
</script>

<section class="min-w-0 overflow-hidden rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <PanelTitle icon={GitBranch} title="Generation Topology" />
    <span class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
      {generations.length} live generation{generations.length === 1 ? '' : 's'}
    </span>
  </div>

  {#if generations.length}
    <div class="mt-3 grid gap-2">
      {#each generations as generation, index (generation.id)}
        {@const variantsForGeneration = generationVariants(generation)}
        {@const survivors = generationSurvivors(generation)}
        {@const winner = generationWinner(generation)}
        {@const followUp = followUpLabel(generation, index)}
        <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
          <div class="flex flex-wrap items-start justify-between gap-2">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-1.5">
                <Badge tone={statusTone(generation.status)}>{generation.status}</Badge>
                {#if followUp}
                  <Badge tone="primary">{followUp}</Badge>
                {/if}
                {#if winner}
                  <Badge tone="success">winner {shortId(winner.id, 10)}</Badge>
                {/if}
              </div>
              <h4 class="mt-1.5 text-[13px] font-semibold tracking-tight text-[var(--color-ink)]">
                {generationLabel(generation, index)}
              </h4>
            </div>
            <div class="grid grid-cols-3 gap-1 text-center font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
              <span class="rounded-[var(--radius-xs)] bg-white px-2 py-1">{variantsForGeneration.length} variants</span>
              <span class="rounded-[var(--radius-xs)] bg-white px-2 py-1">{survivors.length} viable</span>
              <span class="rounded-[var(--radius-xs)] bg-white px-2 py-1">
                target {generation.variantTargetCount || variantsForGeneration.length || '-'}
              </span>
            </div>
          </div>

          <p class="mt-2 line-clamp-2 text-[11.5px] leading-snug text-[var(--color-muted)]">
            {generationNote(generation)}
          </p>

          <div class="mt-2 grid gap-1.5 md:grid-cols-2 xl:grid-cols-3">
            {#each variantsForGeneration as variant (variant.id)}
              {@const results = resultForVariant(variant)}
              {@const isWinner = winner?.id === variant.id || variant.status === 'Promoted'}
              <div
                class={`min-w-0 overflow-hidden rounded-[var(--radius-xs)] border bg-white px-2 py-2 ${
                  isWinner
                    ? 'border-[var(--color-success)]/35 shadow-[inset_0_0_0_1px_rgba(40,150,90,0.12)]'
                    : 'border-[var(--color-border-soft)]'
                }`}
              >
                <div class="flex items-start justify-between gap-2">
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-1.5">
                      <Badge tone={statusTone(variant.status)}>{variantStatusLabel(variant.status)}</Badge>
                      {#if isWinner}
                        <span class="inline-flex items-center gap-1 rounded-[var(--radius-xs)] bg-[rgba(40,150,90,0.10)] px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-success)]">
                          <Trophy size={10} />
                          selected
                        </span>
                      {/if}
                    </div>
                    <p class="mt-1 line-clamp-2 text-[12px] font-semibold leading-snug text-[var(--color-ink)]">
                      {variant.summary || variant.appRef || shortId(variant.id)}
                    </p>
                  </div>
                </div>

                <div class="mt-2 flex flex-wrap items-center gap-1.5">
                  <span class="inline-flex items-center gap-1 rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-1.5 py-0.5 text-[10.5px] text-[var(--color-muted)]">
                    {#if deadStatuses.has(variant.status)}
                      <AlertTriangle size={10} />
                    {:else}
                      <CheckCircle2 size={10} />
                    {/if}
                    {resultSummary(variant)}
                  </span>
                  {#each results.slice(0, 3) as result (result.id)}
                    <span class={`h-2 w-2 rounded-[2px] ${resultStatusColor(result.status)}`} title={result.status}></span>
                  {/each}
                </div>

                <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-muted)]">
                  {variantReason(variant)}
                </p>

                <div class="mt-2 flex items-center gap-1.5">
                  <button
                    type="button"
                    class="inline-flex items-center gap-1 rounded-[var(--radius-xs)] px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)] hover:bg-[var(--color-primary-soft)] hover:text-[var(--color-primary)]"
                    onclick={() => onInspectVariant(variant.id)}
                  >
                    <Eye size={10} />
                    Inspect
                  </button>
                  <button
                    type="button"
                    class={`inline-flex items-center gap-1 rounded-[var(--radius-xs)] px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.08em] ${
                      comparedVariantIds.includes(variant.id)
                        ? 'bg-[var(--color-primary-soft)] text-[var(--color-primary)]'
                        : 'text-[var(--color-muted)] hover:bg-[var(--color-surface-soft)]'
                    }`}
                    onclick={() => onToggleCompare(variant)}
                  >
                    <GitCompareArrows size={10} />
                    Compare
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <p class="mt-3 rounded-[var(--radius-sm)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-3 py-3 text-[12px] text-[var(--color-muted)]">
      No Generation rows are present in the live API response, so Mission Control cannot infer round boundaries.
    </p>
  {/if}
</section>
