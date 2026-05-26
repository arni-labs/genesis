<script lang="ts">
  import { Badge } from '$lib/components/ui';
  import type {
    EvolutionEvidenceArtifact,
    EvolutionMeasurement,
    EvolutionVariant
  } from '$lib/directedEvolution';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type VariantInspectCardProps = {
    variant: EvolutionVariant;
    measurements: EvolutionMeasurement[];
    evidence: EvolutionEvidenceArtifact[];
    reason: string;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
  };

  let {
    variant,
    measurements,
    evidence,
    reason,
    shortId,
    statusTone
  }: VariantInspectCardProps = $props();
</script>

<div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
  <div class="flex items-start justify-between gap-2">
    <div class="min-w-0">
      <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
        {shortId(variant.id, 12)}
      </p>
      <h3 class="mt-1 text-[13px] font-semibold tracking-tight text-[var(--color-ink)]">
        {variant.summary || variant.appRef || 'Variant'}
      </h3>
    </div>
    <Badge tone={statusTone(variant.status)}>{variant.status}</Badge>
  </div>
  <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
    {reason}
  </p>
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
      {#each evidence.slice(0, 3) as artifact (artifact.id)}
        <p class="truncate text-[11px] text-[var(--color-muted)]">
          <span class="font-semibold text-[var(--color-ink-soft)]">{artifact.artifactKind || 'Evidence'}:</span>
          {artifact.summary || artifact.uri || shortId(artifact.id)}
        </p>
      {/each}
    </div>
  {/if}
</div>
