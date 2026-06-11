<script module lang="ts">
  import type { EvolutionEvidenceArtifact } from '$lib/directedEvolution';

  type EvidenceScope = {
    surface?: string;
    query?: string;
    result_count?: string | number;
    resultCount?: string | number;
    result_summary?: string;
    resultSummary?: string;
    zero_result_meaning?: string;
    zeroResultMeaning?: string;
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

  export function evidenceDatadogHref(artifact: EvolutionEvidenceArtifact): string {
    if (isDatadogHref(artifact.uri)) return artifact.uri;
    const scope = evidenceScopes(artifact).find((entry) => entry.datadog_url || entry.datadogUrl);
    const href = (scope?.datadog_url || scope?.datadogUrl || '').trim();
    return isDatadogHref(href) ? href : '';
  }

  export function evidenceSurface(artifact: EvolutionEvidenceArtifact): string {
    return evidenceScopes(artifact)[0]?.surface || artifact.artifactKind || 'Evidence';
  }

  function evidenceQuery(artifact: EvolutionEvidenceArtifact): string {
    return artifact.query || evidenceScopes(artifact)[0]?.query || '';
  }

  function evidenceTimeWindow(artifact: EvolutionEvidenceArtifact): string {
    return artifact.timeWindow;
  }

  function evidenceResultCount(artifact: EvolutionEvidenceArtifact): string {
    const scope = evidenceScopes(artifact)[0];
    const scoped = scope?.result_count ?? scope?.resultCount;
    if (scoped === undefined || scoped === null || scoped === '') return artifact.resultCount;
    return String(scoped);
  }

  function evidenceZeroResultMeaning(artifact: EvolutionEvidenceArtifact): string {
    const scope = evidenceScopes(artifact)[0];
    return (
      artifact.zeroResultMeaning ||
      scope?.zero_result_meaning ||
      scope?.zeroResultMeaning ||
      ''
    );
  }
</script>

<script lang="ts">
  import { ExternalLink } from '@lucide/svelte';

  type EvidenceCardProps = {
    artifact: EvolutionEvidenceArtifact;
    shortId: (value: string, length?: number) => string;
  };

  let { artifact, shortId }: EvidenceCardProps = $props();

  let href = $derived(evidenceDatadogHref(artifact));
  let query = $derived(evidenceQuery(artifact));
  let timeWindow = $derived(evidenceTimeWindow(artifact));
  let resultCount = $derived(evidenceResultCount(artifact));
  let zeroResultMeaning = $derived(evidenceZeroResultMeaning(artifact));

  function evidenceSummary(): string {
    const scope = evidenceScopes(artifact)[0];
    return (
      artifact.interpretation ||
      scope?.result_summary ||
      scope?.resultSummary ||
      artifact.summary ||
      artifact.uri ||
      shortId(artifact.id)
    );
  }
</script>

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
    {evidenceSummary()}
  </p>
  {#if query}
    <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-faint)]">
      {query}
    </p>
  {/if}
  {#if timeWindow || resultCount || zeroResultMeaning}
    <p class="mt-1 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-faint)]">
      {#if timeWindow}{timeWindow} · {/if}results {resultCount || 'n/a'} · zero={zeroResultMeaning || 'unspecified'}
    </p>
  {/if}
</div>
