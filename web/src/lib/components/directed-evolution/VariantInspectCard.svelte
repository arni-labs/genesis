<script lang="ts">
  import { Badge } from '$lib/components/ui';
  import UnifiedDiff from '$lib/components/UnifiedDiff.svelte';
  import EvidenceCard, {
    evidenceDatadogHref,
    evidenceSurface
  } from './EvidenceCard.svelte';
  import type {
    EvolutionEvidenceArtifact,
    EvolutionMeasurement,
    EvolutionMutation,
    EvolutionVariant
  } from '$lib/directedEvolution';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type VariantInspectCardProps = {
    variant: EvolutionVariant;
    measurements: EvolutionMeasurement[];
    evidence: EvolutionEvidenceArtifact[];
    mutation?: EvolutionMutation | null;
    reason: string;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
  };

  let {
    variant,
    measurements,
    evidence,
    mutation = null,
    reason,
    shortId,
    statusTone
  }: VariantInspectCardProps = $props();

  function evidenceRank(artifact: EvolutionEvidenceArtifact): number {
    const provenance = artifact.evidenceProvenance;
    if (provenance === 'datadog-measured' || evidenceDatadogHref(artifact)) return 0;
    if (provenance === 'state-verified' || evidenceSurface(artifact).toLowerCase() === 'state') return 1;
    if (provenance === 'brain-judged') return 2;
    return 3;
  }

  function visibleEvidence(): EvolutionEvidenceArtifact[] {
    return [...evidence].sort((left, right) => evidenceRank(left) - evidenceRank(right)).slice(0, 5);
  }

  function variantStatusLabel(status: string): string {
    if (status === 'NotSelected') return 'Selection-eliminated';
    return status;
  }
</script>

<div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
  <div class="flex items-start justify-between gap-2">
    <div class="min-w-0">
      <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
        {shortId(variant.id, 12)}
      </p>
      <h3 class="mt-1 line-clamp-4 text-[13px] font-semibold leading-snug tracking-tight text-[var(--color-ink)]">
        {variant.summary || variant.appRef || 'Variant'}
      </h3>
    </div>
    <Badge tone={statusTone(variant.status)}>{variantStatusLabel(variant.status)}</Badge>
  </div>
  <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
    {reason}
  </p>
  {#if mutation}
    <div class="mt-3 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-2">
      <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
        App/spec diff
      </p>
      <p class="mt-1 line-clamp-2 text-[11.5px] leading-snug text-[var(--color-ink-soft)]">
        {mutation.summary || mutation.diffRef || 'Mutation recorded without a summary.'}
      </p>
      {#if mutation.changedFiles.length}
        <div class="mt-2 flex flex-wrap gap-1">
          {#each mutation.changedFiles as file (file)}
            <span class="max-w-full truncate rounded-[var(--radius-xs)] bg-white px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-muted)]">
              {file}
            </span>
          {/each}
        </div>
      {/if}
      {#if mutation.diffRef}
        <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-faint)]">{mutation.diffRef}</p>
      {/if}
      {#if mutation.diffPatch}
        <div class="mt-2">
          <UnifiedDiff patch={mutation.diffPatch} maxFiles={7} maxLinesPerFile={28} />
        </div>
      {/if}
    </div>
  {/if}
  <div class="mt-3 grid grid-cols-2 gap-1.5 text-[11px]">
    <div class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1.5">
      <p class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">App Ref</p>
      <p class="mt-0.5 truncate text-[var(--color-ink-soft)]">{variant.appRef || 'pending'}</p>
    </div>
    <div class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1.5">
      <p class="font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">Runtime</p>
      <p class="mt-0.5 truncate text-[var(--color-ink-soft)]">{variant.runtimeRef || 'pending'}</p>
    </div>
  </div>
  {#if measurements.length}
    <div class="mt-3 grid gap-1.5">
      {#each measurements.slice(0, 4) as measurement (`${measurement.metricDefinitionId}-${measurement.value}`)}
        <div class="flex items-center justify-between gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] px-2 py-1 text-[11px]">
          <span class="truncate text-[var(--color-muted)]">{shortId(measurement.metricDefinitionId, 16)}</span>
          <span class="font-mono text-[var(--color-ink)]">{measurement.value} {measurement.unit}</span>
        </div>
      {/each}
    </div>
  {/if}
  {#if evidence.length}
    <div class="mt-3 border-t border-[var(--color-border)] pt-2">
      {#each visibleEvidence() as artifact (artifact.id)}
        <EvidenceCard {artifact} {shortId} />
      {/each}
    </div>
  {/if}
</div>
