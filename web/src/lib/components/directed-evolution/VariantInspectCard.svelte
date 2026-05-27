<script lang="ts">
  import { ExternalLink } from '@lucide/svelte';
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

  type EvidenceScope = {
    surface?: string;
    query?: string;
    result_summary?: string;
    resultSummary?: string;
    datadog_url?: string;
    datadogUrl?: string;
  };

  function parsedCorrelation(artifact: EvolutionEvidenceArtifact): Record<string, unknown> {
    if (!artifact.correlationJson) return {};
    try {
      const value = JSON.parse(artifact.correlationJson);
      return value && typeof value === 'object' && !Array.isArray(value)
        ? (value as Record<string, unknown>)
        : {};
    } catch {
      return {};
    }
  }

  function evidenceScopes(artifact: EvolutionEvidenceArtifact): EvidenceScope[] {
    const correlation = parsedCorrelation(artifact);
    const output = correlation.output;
    const candidates = [
      correlation.evidence_scope,
      correlation.evidenceScope,
      output && typeof output === 'object' && !Array.isArray(output)
        ? (output as Record<string, unknown>).evidence_scope
        : undefined,
      output && typeof output === 'object' && !Array.isArray(output)
        ? (output as Record<string, unknown>).evidenceScope
        : undefined
    ];

    for (const candidate of candidates) {
      if (Array.isArray(candidate)) {
        return candidate.filter(
          (item): item is EvidenceScope => item && typeof item === 'object' && !Array.isArray(item)
        );
      }
    }
    return [];
  }

  function datadogHref(artifact: EvolutionEvidenceArtifact): string {
    if (isDatadogHref(artifact.uri)) return artifact.uri;
    const scope = evidenceScopes(artifact).find((entry) => entry.datadog_url || entry.datadogUrl);
    const href = (scope?.datadog_url || scope?.datadogUrl || '').trim();
    return isDatadogHref(href) ? href : '';
  }

  function isDatadogHref(value: string): boolean {
    return [
      'https://app.datadoghq.com',
      'https://app.us3.datadoghq.com',
      'https://app.us5.datadoghq.com',
      'https://app.datadoghq.eu',
      'https://app.ap1.datadoghq.com',
      'https://app.ap2.datadoghq.com',
      'https://app.ddog-gov.com'
    ].some((prefix) => value.startsWith(prefix));
  }

  function evidenceSummary(artifact: EvolutionEvidenceArtifact): string {
    const scope = evidenceScopes(artifact)[0];
    return (
      scope?.result_summary ||
      scope?.resultSummary ||
      artifact.summary ||
      artifact.uri ||
      shortId(artifact.id)
    );
  }

  function evidenceSurface(artifact: EvolutionEvidenceArtifact): string {
    return evidenceScopes(artifact)[0]?.surface || artifact.artifactKind || 'Evidence';
  }

  function evidenceQuery(artifact: EvolutionEvidenceArtifact): string {
    return evidenceScopes(artifact)[0]?.query || '';
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
        {@const href = datadogHref(artifact)}
        {@const query = evidenceQuery(artifact)}
        <div class="mb-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5 last:mb-0">
          <div class="flex items-center justify-between gap-2">
            <p class="truncate font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
              {evidenceSurface(artifact)}
            </p>
            {#if href}
              <a
                class="inline-flex shrink-0 items-center gap-1 rounded-[var(--radius-xs)] px-1.5 py-0.5 text-[10px] font-semibold text-[var(--color-primary)] hover:bg-white"
                href={href}
                target="_blank"
                rel="noreferrer"
              >
                <ExternalLink size={10} />
                Datadog
              </a>
            {/if}
          </div>
          <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-muted)]">
            {evidenceSummary(artifact)}
          </p>
          {#if query}
            <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-faint)]">
              {query}
            </p>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
