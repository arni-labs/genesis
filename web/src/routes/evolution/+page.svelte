<script lang="ts">
  import { browser } from '$app/environment';
  import { onDestroy, onMount } from 'svelte';
  import {
    AlertCircle,
    GitCompareArrows,
    RefreshCw,
    X
  } from '@lucide/svelte';
  import Topbar from '$lib/components/Topbar.svelte';
  import EvolutionEpisodePanel from '$lib/components/directed-evolution/EvolutionEpisodePanel.svelte';
  import EvolutionLineagePolicy from '$lib/components/directed-evolution/EvolutionLineagePolicy.svelte';
  import EvolutionSideRail from '$lib/components/directed-evolution/EvolutionSideRail.svelte';
  import MetricTile from '$lib/components/directed-evolution/MetricTile.svelte';
  import PanelTitle from '$lib/components/directed-evolution/PanelTitle.svelte';
  import VariantInspectCard from '$lib/components/directed-evolution/VariantInspectCard.svelte';
  import { Badge, Button } from '$lib/components/ui';
  import { loadRegistry, registryStore } from '$lib/registry';
  import {
    dismissDirection,
    jsonEntries,
    loadDirectedEvolutionSnapshot,
    pauseEpisode,
    pinViabilityConstraint,
    resumeEpisode,
    stopEpisode,
    type DirectedEvolutionSnapshot,
    type EvolutionDirection,
    type EvolutionEpisode,
    type EvolutionEvidenceArtifact,
    type EvolutionVariant
  } from '$lib/directedEvolution';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  let snapshot: DirectedEvolutionSnapshot | null = null;
  let loading = false;
  let error = '';
  let actionBusy = '';
  let selectedEpisodeId = '';
  let inspectedVariantId = '';
  let comparedVariantIds: string[] = [];
  let refreshTimer: number | undefined;
  const defaultDirectedEvolutionTenant =
    import.meta.env.VITE_DIRECTED_EVOLUTION_TENANT_ID ??
    import.meta.env.VITE_TEMPER_TENANT_ID ??
    'default';
  let directedEvolutionTenantId = defaultDirectedEvolutionTenant;

  $: registryState = $registryStore;
  $: registrySnapshot = registryState.snapshot;
  $: organisms = snapshot?.organisms ?? [];
  $: organism = organisms.find((item) => item.status === 'Active') ?? organisms[0] ?? null;
  $: organismVersions = snapshot?.organismVersions ?? [];
  $: lineageEdges = snapshot?.lineageEdges ?? [];
  $: promotions = snapshot?.promotions ?? [];
  $: activeDirections = (snapshot?.directions ?? []).filter((direction) => direction.status !== 'Archived');
  $: activeEpisodes = snapshot?.episodes ?? [];
  $: selectedEpisode =
    activeEpisodes.find((episode) => episode.id === selectedEpisodeId) ??
    activeEpisodes.find((episode) => !terminalEpisodeStatuses.has(episode.status)) ??
    activeEpisodes[0] ??
    null;
  $: selectedDirection = selectedEpisode
    ? activeDirections.find((direction) => direction.id === selectedEpisode.directionId) ?? null
    : null;
  $: selectedPromotion = selectedEpisode
    ? promotions.find((promotion) => promotion.id === selectedEpisode.promotionId) ??
      promotions.find((promotion) => promotion.episodeId === selectedEpisode.id) ??
      promotions.find((promotion) => promotion.winningVariantId === selectedEpisode.winningVariantId) ??
      null
    : null;
  $: currentGoal = selectedEpisode
    ? snapshot?.adaptationGoals.find((goal) => goal.id === selectedEpisode.adaptationGoalId) ?? null
    : null;
  $: currentSelectionPressure = selectedEpisode
    ? snapshot?.selectionPressures.find((pressure) => pressure.id === selectedEpisode.selectionPressureId) ??
      null
    : null;
  $: episodeVariants = selectedEpisode
    ? (snapshot?.variants ?? []).filter((variant) => variant.episodeId === selectedEpisode.id)
    : [];
  $: activePolicy =
    (snapshot?.autonomyPolicies ?? []).find(
      (policy) => policy.status === 'Active' && (!organism || policy.organismId === organism.id)
    ) ??
    (snapshot?.autonomyPolicies ?? []).find((policy) => policy.status === 'Active') ??
    null;
  $: constraints = selectedEpisode
    ? (snapshot?.viabilityConstraints ?? []).filter(
        (constraint) =>
          constraint.episodeId === selectedEpisode.id ||
          selectedEpisode.viabilityConstraintIds.includes(constraint.id)
      )
    : [];
  $: stages = selectedEpisode
    ? (snapshot?.evaluationStages ?? [])
        .filter(
          (stage) =>
            stage.episodeId === selectedEpisode.id ||
            selectedEpisode.evaluationStageIds.includes(stage.id)
        )
        .sort((a, b) => a.sequenceIndex - b.sequenceIndex)
    : [];
  $: stageResults = selectedEpisode
    ? (snapshot?.stageResults ?? []).filter((result) => result.episodeId === selectedEpisode.id)
    : [];
  $: inspectedVariant =
    episodeVariants.find((variant) => variant.id === inspectedVariantId) ??
    episodeVariants.find((variant) => variant.status === 'Eliminated' || variant.status === 'Failed') ??
    episodeVariants[0] ??
    null;
  $: comparedVariants = comparedVariantIds
    .map((id) => episodeVariants.find((variant) => variant.id === id))
    .filter((variant): variant is EvolutionVariant => Boolean(variant));
  $: recentBrainRuns = (snapshot?.brainRuns ?? []).slice(-8).reverse();
  $: recentWorkItems = (snapshot?.workItems ?? []).slice(-8).reverse();
  $: totalWarnings = snapshot?.warnings ?? [];

  const terminalEpisodeStatuses = new Set(['Completed', 'Stopped', 'Failed']);

  onMount(() => {
    directedEvolutionTenantId = resolveDirectedEvolutionTenant();
    void loadRegistry();
    void loadEvolution();
    refreshTimer = window.setInterval(() => void loadEvolution(), 12_000);
  });

  onDestroy(() => {
    if (refreshTimer !== undefined) {
      window.clearInterval(refreshTimer);
    }
  });

  async function refreshAll() {
    await Promise.all([loadRegistry(true), loadEvolution(true)]);
  }

  async function loadEvolution(force = false) {
    if (loading && !force) return;
    loading = true;
    error = '';
    try {
      snapshot = await loadDirectedEvolutionSnapshot(directedEvolutionTenantId);
    } catch (loadError) {
      error = loadError instanceof Error ? loadError.message : String(loadError);
    } finally {
      loading = false;
    }
  }

  async function runControl(label: string, action: () => Promise<unknown>) {
    actionBusy = label;
    error = '';
    try {
      await action();
      await loadEvolution(true);
    } catch (actionError) {
      error = actionError instanceof Error ? actionError.message : String(actionError);
    } finally {
      actionBusy = '';
    }
  }

  function toggleCompare(variant: EvolutionVariant) {
    if (comparedVariantIds.includes(variant.id)) {
      comparedVariantIds = comparedVariantIds.filter((id) => id !== variant.id);
      return;
    }
    comparedVariantIds = [...comparedVariantIds.slice(-2), variant.id];
  }

  function shortId(value: string, length = 10): string {
    if (!value) return 'pending';
    return value.length > length ? `${value.slice(0, length)}...` : value;
  }

  function statusTone(status: string): StatusTone {
    if (['Active', 'Running', 'Passed', 'Selected', 'Promoted', 'Completed', 'Succeeded', 'Parent'].includes(status)) {
      return 'success';
    }
    if (['Queued', 'Draft', 'Negotiating', 'Planned', 'Generating', 'Evaluating', 'Selecting', 'Paused', 'Claimed'].includes(status)) {
      return 'warning';
    }
    if (['Failed', 'Stopped', 'Eliminated', 'Dismissed', 'Cancelled'].includes(status)) {
      return 'danger';
    }
    if (['Proposed', 'Framed', 'Linked', 'Pinned'].includes(status)) {
      return 'primary';
    }
    return 'neutral';
  }

  function variantReason(variant: EvolutionVariant): string {
    return (
      variant.reason ||
      variant.failureReason ||
      stageResults.find((result) => result.variantId === variant.id && (result.reason || result.failureReason))
        ?.reason ||
      stageResults.find((result) => result.variantId === variant.id && (result.reason || result.failureReason))
        ?.failureReason ||
      'No elimination reason has been recorded yet.'
    );
  }

  function variantEvidence(variant: EvolutionVariant): EvolutionEvidenceArtifact[] {
    const ids = new Set(
      [
        variant.evidenceArtifactId,
        ...stageResults
          .filter((result) => result.variantId === variant.id)
          .map((result) => result.evidenceArtifactId),
        ...(snapshot?.trials ?? [])
          .filter((trial) => trial.variantId === variant.id)
          .map((trial) => trial.evidenceArtifactId)
      ].filter(Boolean)
    );
    return (snapshot?.evidenceArtifacts ?? []).filter(
      (artifact) =>
        ids.has(artifact.id) ||
        (artifact.targetEntityType === 'Variant' && artifact.targetEntityId === variant.id)
    );
  }

  function variantMeasurements(variant: EvolutionVariant) {
    return (snapshot?.measurements ?? []).filter((measurement) => measurement.variantId === variant.id);
  }

  function directionPressureSummary(direction: EvolutionDirection): string {
    const pressure =
      snapshot?.pressures.find((item) => item.directionId === direction.id) ??
      snapshot?.pressures.find((item) => direction.pressureIds.includes(item.id));
    return pressure?.summary || direction.summary;
  }

  function resolveDirectedEvolutionTenant(): string {
    if (!browser) return defaultDirectedEvolutionTenant;
    const params = new URLSearchParams(window.location.search);
    const fromUrl = params.get('tenant')?.trim();
    if (fromUrl) {
      window.localStorage.setItem('genesis-directed-evolution-tenant', fromUrl);
      return fromUrl;
    }
    return (
      window.localStorage.getItem('genesis-directed-evolution-tenant')?.trim() ||
      defaultDirectedEvolutionTenant
    );
  }

</script>

<svelte:head>
  <title>Directed Evolution · Genesis</title>
  <meta
    name="description"
    content="Observe Directed Evolution episodes, variants, evaluation stages, evidence, autonomy policy, and organism lineage."
  />
</svelte:head>

<main class="relative z-[1] min-h-screen">
  <Topbar
    appCount={(registrySnapshot?.apps ?? []).filter((app) => app.status !== 'Deleted').length}
    lineageCount={registrySnapshot?.lineages.length ?? 0}
    closureCount={registrySnapshot?.closures.length ?? 0}
    loading={registryState.loading || loading}
    onRefresh={refreshAll}
  />

  <section class="grid gap-3 px-3 py-3 lg:px-4 xl:px-5">
    <div class="grid gap-3 xl:grid-cols-[minmax(0,1.45fr)_minmax(340px,0.55fr)]">
      <div class="grid min-w-0 gap-3">
        <section class="overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white">
          <div class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--color-border)] px-3 py-3 sm:px-4">
            <div class="min-w-0">
              <p class="v-eyebrow">Directed Evolution · Mission Control</p>
              <h2 class="v-display mt-1 text-[24px] text-[var(--color-ink)]">
                {organism?.name || 'No organism online'}
              </h2>
              <p class="mt-1 max-w-[78ch] text-[12.5px] leading-relaxed text-[var(--color-muted)]">
                {organism?.appRef ||
                  'Install and activate the Directed Evolution app to stream live organism, episode, variant, and evidence state here.'}
              </p>
              <p class="mt-1 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
                Tenant {directedEvolutionTenantId}
              </p>
            </div>
            <div class="flex items-center gap-1.5">
              <Badge tone={loading ? 'warning' : 'neutral'} pixel={!loading}>
                {loading ? 'Syncing' : 'Live state'}
              </Badge>
              <Button size="icon" title="Refresh evolution state" onclick={() => void refreshAll()} disabled={loading}>
                <RefreshCw size={13} class={loading ? 'animate-spin' : ''} />
              </Button>
            </div>
          </div>

          {#if error}
            <div class="border-b border-[var(--color-border)] bg-[#fff5f7] px-3 py-2 text-[12px] text-[#7a1830] sm:px-4">
              <div class="flex items-start gap-2">
                <AlertCircle size={14} class="mt-[2px] shrink-0" />
                <span>{error}</span>
              </div>
            </div>
          {/if}

          {#if totalWarnings.length}
            <div class="border-b border-[var(--color-border)] bg-[var(--color-surface-soft)] px-3 py-2 sm:px-4">
              <div class="flex flex-wrap gap-1.5 text-[11px] text-[var(--color-muted)]">
                {#each totalWarnings.slice(0, 4) as warning (warning.collection)}
                  <span class="rounded-[var(--radius-xs)] border border-[var(--color-border)] bg-white px-2 py-1">
                    {warning.collection}: {warning.message}
                  </span>
                {/each}
              </div>
            </div>
          {/if}

          <div class="grid gap-3 p-3 sm:p-4">
            <div class="min-w-0">
              <div class="grid gap-2 sm:grid-cols-3 xl:grid-cols-6">
                <MetricTile label="Directions" value={activeDirections.length} />
                <MetricTile label="Episodes" value={activeEpisodes.length} />
                <MetricTile label="Variants" value={episodeVariants.length} />
                <MetricTile label="Promotions" value={promotions.length} />
                <MetricTile label="Materialized" value={promotions.filter((item) => item.materialized).length} />
                <MetricTile label="Brain Runs" value={snapshot?.brainRuns.length ?? 0} />
              </div>

              <EvolutionEpisodePanel
                {selectedEpisode}
                {selectedDirection}
                selectedPromotion={selectedPromotion}
                {currentGoal}
                {currentSelectionPressure}
                {stages}
                {stageResults}
                {episodeVariants}
                {constraints}
                {comparedVariantIds}
                {actionBusy}
                {shortId}
                {statusTone}
                onPauseEpisode={(episode) =>
                  void runControl(`pause-${episode.id}`, () =>
                    pauseEpisode(episode.id, 'Paused from Genesis Mission Control', directedEvolutionTenantId)
                  )}
                onResumeEpisode={(episode) =>
                  void runControl(`resume-${episode.id}`, () =>
                    resumeEpisode(episode.id, 'Resumed from Genesis Mission Control', directedEvolutionTenantId)
                  )}
                onStopEpisode={(episode) =>
                  void runControl(`stop-${episode.id}`, () =>
                    stopEpisode(episode.id, 'Stopped from Genesis Mission Control', directedEvolutionTenantId)
                  )}
                onPinConstraint={(constraint) =>
                  void runControl(`pin-${constraint.id}`, () =>
                    pinViabilityConstraint(constraint.id, 'Pinned from Genesis Mission Control', directedEvolutionTenantId)
                  )}
                onInspectVariant={(id) => (inspectedVariantId = id)}
                onToggleCompare={toggleCompare}
              />
            </div>

            <EvolutionLineagePolicy
              {organism}
              {organismVersions}
              {lineageEdges}
              {activePolicy}
              {shortId}
              {statusTone}
              {jsonEntries}
            />
          </div>
        </section>

        {#if comparedVariants.length}
          <section class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white p-3 sm:p-4">
            <div class="flex items-center justify-between gap-2">
              <PanelTitle icon={GitCompareArrows} title="Variant Compare" />
              <Button size="xs" onclick={() => (comparedVariantIds = [])}>
                <X size={11} />
                Clear
              </Button>
            </div>
            <div class="mt-3 grid gap-2 md:grid-cols-2 xl:grid-cols-3">
              {#each comparedVariants as variant (variant.id)}
                <VariantInspectCard
                  variant={variant}
                  measurements={variantMeasurements(variant)}
                  evidence={variantEvidence(variant)}
                  reason={variantReason(variant)}
                  shortId={shortId}
                  statusTone={statusTone}
                />
              {/each}
            </div>
          </section>
        {/if}
      </div>

      <EvolutionSideRail
        {activeDirections}
        {inspectedVariant}
        {recentWorkItems}
        {recentBrainRuns}
        {actionBusy}
        {shortId}
        {statusTone}
        {jsonEntries}
        {directionPressureSummary}
        {variantMeasurements}
        {variantEvidence}
        {variantReason}
        onDismissDirection={(direction) =>
          void runControl(`dismiss-${direction.id}`, () =>
            dismissDirection(direction.id, 'Dismissed from Genesis Mission Control', directedEvolutionTenantId)
          )}
      />
    </div>
  </section>
</main>
