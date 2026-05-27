<script lang="ts">
  import { BrainCircuit, DatabaseZap, Eye, GitPullRequestArrow, RadioTower, X } from '@lucide/svelte';
  import { Badge, Button } from '$lib/components/ui';
  import type {
    EvolutionBrainRun,
    EvolutionDirection,
    EvolutionEvidenceArtifact,
    EvolutionMeasurement,
    EvolutionPressure,
    EvolutionSignal,
    EvolutionVariant,
    EvolutionWorkItem
  } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';
  import VariantInspectCard from './VariantInspectCard.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    activeDirections: EvolutionDirection[];
    signals: EvolutionSignal[];
    pressures: EvolutionPressure[];
    evidenceArtifacts: EvolutionEvidenceArtifact[];
    brainRuns: EvolutionBrainRun[];
    inspectedVariant: EvolutionVariant | null;
    recentWorkItems: EvolutionWorkItem[];
    recentBrainRuns: EvolutionBrainRun[];
    actionBusy: string;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
    jsonEntries: (value: string) => Array<[string, string]>;
    directionPressureSummary: (direction: EvolutionDirection) => string;
    directionPressures: (direction: EvolutionDirection) => EvolutionPressure[];
    directionSignals: (direction: EvolutionDirection) => EvolutionSignal[];
    directionEvidence: (direction: EvolutionDirection) => EvolutionEvidenceArtifact[];
    directionBrainRun: (direction: EvolutionDirection) => EvolutionBrainRun | null;
    variantMeasurements: (variant: EvolutionVariant) => EvolutionMeasurement[];
    variantEvidence: (variant: EvolutionVariant) => EvolutionEvidenceArtifact[];
    variantReason: (variant: EvolutionVariant) => string;
    onDismissDirection: (direction: EvolutionDirection) => void;
  };

  let {
    activeDirections,
    signals,
    pressures,
    evidenceArtifacts,
    brainRuns,
    inspectedVariant,
    recentWorkItems,
    recentBrainRuns,
    actionBusy,
    shortId,
    statusTone,
    jsonEntries,
    directionPressureSummary,
    directionPressures,
    directionSignals,
    directionEvidence,
    directionBrainRun,
    variantMeasurements,
    variantEvidence,
    variantReason,
    onDismissDirection
  }: Props = $props();

  function laneLabel(lane: string): string {
    const normalized = lane.toLowerCase();
    if (normalized.includes('human')) return 'human gate';
    if (normalized.includes('repair') || normalized.includes('auto')) return 'auto lane';
    if (normalized.includes('policy')) return 'policy gate';
    return lane || 'lane pending';
  }

  function laneTone(lane: string): StatusTone {
    const normalized = lane.toLowerCase();
    if (normalized.includes('human') || normalized.includes('approval')) return 'warning';
    if (normalized.includes('repair') || normalized.includes('auto')) return 'success';
    if (normalized.includes('policy')) return 'primary';
    return 'neutral';
  }

  function externalEvidenceHref(artifact: EvolutionEvidenceArtifact): string {
    return artifact.uri.startsWith('https://') ? artifact.uri : '';
  }

  function directionCounts(
    fedPressures: EvolutionPressure[],
    fedSignals: EvolutionSignal[],
    fedEvidence: EvolutionEvidenceArtifact[],
    fedBrainRun: EvolutionBrainRun | null
  ): string {
    return `${fedPressures.length} pressures · ${fedSignals.length} signals · ${fedEvidence.length} evidence · ${fedBrainRun ? 1 : 0} brain runs`;
  }
</script>

<aside class="grid min-w-0 content-start gap-3">
  <section class="min-w-0 rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white p-3">
    <PanelTitle icon={BrainCircuit} title="Suggested Directions" />
    <p class="mt-1 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
      {pressures.length} pressures · {signals.length} signals · {evidenceArtifacts.length} evidence · {brainRuns.length} brain runs
    </p>
    <div class="mt-3 grid gap-2">
      {#if activeDirections.length}
        {#each activeDirections as direction (direction.id)}
          {@const fedPressures = directionPressures(direction)}
          {@const fedSignals = directionSignals(direction)}
          {@const fedEvidence = directionEvidence(direction)}
          {@const fedBrainRun = directionBrainRun(direction)}
          <div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-3">
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <div class="flex flex-wrap items-center gap-1.5">
                  <Badge tone={statusTone(direction.status)}>{direction.status}</Badge>
                  <Badge tone={laneTone(direction.autonomyLane)}>
                    {laneLabel(direction.autonomyLane)}
                  </Badge>
                </div>
                <h3 class="mt-2 text-[13px] font-semibold tracking-tight text-[var(--color-ink)]">
                  {direction.title || shortId(direction.id)}
                </h3>
              </div>
              {#if direction.status === 'Proposed'}
                <Button
                  size="xs"
                  onclick={() => onDismissDirection(direction)}
                  disabled={actionBusy === `dismiss-${direction.id}`}
                >
                  <X size={11} />
                  Dismiss
                </Button>
              {/if}
            </div>
            <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
              {directionPressureSummary(direction) || 'No direction summary recorded yet.'}
            </p>
            <div class="mt-2 grid gap-1.5">
              {#if fedPressures.length}
                {#each fedPressures.slice(0, 2) as pressure (pressure.id)}
                  <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-1.5">
                    <div class="flex items-center justify-between gap-2">
                      <span class="inline-flex min-w-0 items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
                        <GitPullRequestArrow size={10} />
                        {pressure.pressureClass || 'pressure'}
                      </span>
                      <Badge tone={statusTone(pressure.status)}>{pressure.status}</Badge>
                    </div>
                    <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-ink-soft)]">
                      {pressure.summary || shortId(pressure.id)}
                    </p>
                  </div>
                {/each}
              {/if}
              {#if fedSignals.length}
                {#each fedSignals.slice(0, 2) as signal (signal.id)}
                  <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-1.5">
                    <div class="flex items-center justify-between gap-2">
                      <span class="inline-flex min-w-0 items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
                        <RadioTower size={10} />
                        {signal.signalKind || signal.source || 'signal'}
                      </span>
                      <Badge tone={statusTone(signal.status)}>{signal.status}</Badge>
                    </div>
                    <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-ink-soft)]">
                      {signal.summary || shortId(signal.id)}
                    </p>
                  </div>
                {/each}
              {/if}
              {#if fedBrainRun}
                <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-1.5">
                  <div class="flex items-center justify-between gap-2">
                    <span class="inline-flex min-w-0 items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
                      <BrainCircuit size={10} />
                      {fedBrainRun.role || fedBrainRun.agentKind || 'brain'}
                    </span>
                    <Badge tone={statusTone(fedBrainRun.status)}>{fedBrainRun.status}</Badge>
                  </div>
                  <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-ink-soft)]">
                    {fedBrainRun.summary || fedBrainRun.model || shortId(fedBrainRun.id)}
                  </p>
                </div>
              {/if}
              {#if fedEvidence.length}
                <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-1.5">
                  <div class="flex items-center justify-between gap-2">
                    <span class="inline-flex min-w-0 items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
                      <DatabaseZap size={10} />
                      evidence
                    </span>
                    <Badge tone="neutral">{fedEvidence.length}</Badge>
                  </div>
                  <div class="mt-1 grid gap-1">
                    {#each fedEvidence.slice(0, 2) as artifact (artifact.id)}
                      {@const href = externalEvidenceHref(artifact)}
                      {#if href}
                        <a
                          href={href}
                          target="_blank"
                          rel="noreferrer"
                          class="truncate text-[11px] text-[var(--color-primary)] hover:text-[var(--color-primary-strong)]"
                        >
                          {artifact.summary || artifact.artifactKind || artifact.uri}
                        </a>
                      {:else}
                        <p class="truncate text-[11px] text-[var(--color-ink-soft)]">
                          {artifact.summary || artifact.artifactKind || shortId(artifact.id)}
                        </p>
                      {/if}
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
            {#if direction.proposedAdaptationGoal}
              <p class="mt-2 text-[11px] leading-relaxed text-[var(--color-ink-soft)]">
                <span class="font-semibold">Goal:</span>
                {direction.proposedAdaptationGoal}
              </p>
            {/if}
            {#if direction.provenanceJson}
              <details class="mt-2 rounded-[var(--radius-xs)] border border-[var(--color-border)] bg-white px-2 py-1.5">
                <summary class="cursor-pointer font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
                  Basis · {directionCounts(fedPressures, fedSignals, fedEvidence, fedBrainRun)}
                </summary>
                <div class="mt-1 grid gap-1">
                  {#each jsonEntries(direction.provenanceJson).slice(0, 5) as [key, value] (key)}
                    <p class="text-[11px] leading-snug text-[var(--color-muted)]">
                      <span class="font-semibold text-[var(--color-ink-soft)]">{key}:</span>
                      {value}
                    </p>
                  {/each}
                </div>
              </details>
            {/if}
          </div>
        {/each}
      {:else}
        <p class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] px-3 py-3 text-[12px] text-[var(--color-muted)]">
          No suggested directions are present in the live API response.
        </p>
      {/if}
    </div>
  </section>

  <section class="min-w-0 rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white p-3">
    <PanelTitle icon={Eye} title="Variant Inspector" />
    {#if inspectedVariant}
      <div class="mt-3">
        <VariantInspectCard
          variant={inspectedVariant}
          measurements={variantMeasurements(inspectedVariant)}
          evidence={variantEvidence(inspectedVariant)}
          reason={variantReason(inspectedVariant)}
          {shortId}
          {statusTone}
        />
      </div>
    {:else}
      <p class="mt-2 text-[12px] text-[var(--color-muted)]">
        No variant is available to inspect.
      </p>
    {/if}
  </section>

  <section class="min-w-0 rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white p-3">
    <PanelTitle icon={BrainCircuit} title="Brain Queue" />
    <div class="mt-3 grid gap-2">
      {#each recentWorkItems as item (item.id)}
        <div class="rounded-[var(--radius-sm)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-2">
          <div class="flex items-center justify-between gap-2">
            <span class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              {item.role || 'role pending'}
            </span>
            <Badge tone={statusTone(item.status)}>{item.status}</Badge>
          </div>
          <p class="mt-1 truncate text-[11px] text-[var(--color-muted)]">
            {item.targetEntityType}:{shortId(item.targetEntityId)}
          </p>
        </div>
      {:else}
        <p class="text-[12px] text-[var(--color-muted)]">
          No queued or completed work items recorded yet.
        </p>
      {/each}
    </div>
    {#if recentBrainRuns.length}
      <div class="mt-3 border-t border-[var(--color-border)] pt-3">
        {#each recentBrainRuns.slice(0, 4) as run (run.id)}
          <div class="mb-1.5 flex items-center justify-between gap-2 text-[11px] last:mb-0">
            <span class="truncate text-[var(--color-muted)]">{run.role || shortId(run.id)}</span>
            <Badge tone={statusTone(run.status)}>{run.status}</Badge>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</aside>
