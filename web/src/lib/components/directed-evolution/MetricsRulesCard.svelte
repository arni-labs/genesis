<script lang="ts">
  import { Gauge, ListX } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type {
    EvolutionEliminationRule,
    EvolutionMetricDefinition,
    EvolutionScoringRule
  } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';

  type Props = {
    metricDefinitions: EvolutionMetricDefinition[];
    eliminationRules: EvolutionEliminationRule[];
    scoringRules: EvolutionScoringRule[];
    shortId: (value: string, length?: number) => string;
  };

  let { metricDefinitions, eliminationRules, scoringRules, shortId }: Props = $props();

  function metricDirection(metric: EvolutionMetricDefinition): string {
    if (metric.higherIsBetter === 'true') return 'higher wins';
    if (metric.higherIsBetter === 'false') return 'lower wins';
    return metric.desiredDirection || 'direction pending';
  }
</script>

<div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
  <div class="flex flex-wrap items-center gap-2">
    <PanelTitle icon={Gauge} title="Metrics & Rules" />
    <div class="flex w-full max-w-full flex-wrap items-center gap-1.5 sm:w-auto">
      <Badge tone="neutral">{metricDefinitions.length} metrics</Badge>
      <Badge tone="danger">{eliminationRules.length} hard rules</Badge>
      <Badge tone="primary">{scoringRules.length} scores</Badge>
    </div>
  </div>
  <div class="mt-3 grid gap-2 lg:grid-cols-3">
    <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
      <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
        Metrics
      </p>
      <div class="mt-1.5 grid gap-1.5">
        {#each metricDefinitions.slice(0, 4) as metric (metric.id)}
          <div class="rounded-[var(--radius-xs)] bg-white px-2 py-1.5">
            <div class="flex items-center justify-between gap-2">
              <span class="truncate text-[11.5px] font-semibold text-[var(--color-ink)]">
                {metric.metricName || shortId(metric.id)}
              </span>
              <Badge tone="neutral">{metric.unit || metric.metricKind || 'metric'}</Badge>
            </div>
            <p class="mt-0.5 truncate text-[10.5px] text-[var(--color-muted)]">
              {metricDirection(metric)}
            </p>
            {#if metric.description}
              <p class="mt-1 line-clamp-2 text-[10.5px] leading-snug text-[var(--color-muted)]">
                {metric.description}
              </p>
            {/if}
          </div>
        {:else}
          <p class="text-[11px] text-[var(--color-muted)]">No metrics recorded.</p>
        {/each}
      </div>
    </div>

    <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
      <p class="inline-flex items-center gap-1 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
        <ListX size={10} />
        Elimination
      </p>
      <div class="mt-1.5 grid gap-1.5">
        {#each eliminationRules.slice(0, 4) as rule (rule.id)}
          <div class="rounded-[var(--radius-xs)] bg-white px-2 py-1.5">
            <p class="line-clamp-3 text-[11px] leading-snug text-[var(--color-ink-soft)]">
              {rule.ruleStatement || shortId(rule.id)}
            </p>
            {#if rule.thresholdJson}
              <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-muted)]">
                {rule.thresholdJson}
              </p>
            {/if}
          </div>
        {:else}
          <p class="text-[11px] text-[var(--color-muted)]">No hard rules recorded.</p>
        {/each}
      </div>
    </div>

    <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
      <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
        Scoring
      </p>
      <div class="mt-1.5 grid gap-1.5">
        {#each scoringRules.slice(0, 4) as rule (rule.id)}
          <div class="rounded-[var(--radius-xs)] bg-white px-2 py-1.5">
            <div class="flex items-start justify-between gap-2">
              <p class="line-clamp-3 min-w-0 text-[11px] leading-snug text-[var(--color-ink-soft)]">
                {rule.ruleStatement || shortId(rule.id)}
              </p>
              {#if rule.weight}
                <Badge tone="primary">Weight {rule.weight}</Badge>
              {/if}
            </div>
          </div>
        {:else}
          <p class="text-[11px] text-[var(--color-muted)]">No scoring rules recorded.</p>
        {/each}
      </div>
    </div>
  </div>
</div>
