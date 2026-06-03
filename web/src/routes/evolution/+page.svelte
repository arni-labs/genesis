<script lang="ts">
  import { browser } from '$app/environment';
  import { onDestroy, onMount } from 'svelte';
  import {
    Activity,
    AlertCircle,
    Compass,
    Dna,
    FileDiff,
    GitCompareArrows,
    ListChecks,
    RefreshCw,
    ShieldCheck,
    X
  } from '@lucide/svelte';
  import Topbar from '$lib/components/Topbar.svelte';
  import EvolutionEpisodePanel from '$lib/components/directed-evolution/EvolutionEpisodePanel.svelte';
  import EvolutionLineagePolicy from '$lib/components/directed-evolution/EvolutionLineagePolicy.svelte';
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
    type EvolutionEliminationRule,
    type EvolutionEpisode,
    type EvolutionEvidenceArtifact,
    type EvolutionMetricDefinition,
    type EvolutionMutation,
    type EvolutionScoringRule,
    type EvolutionSelectionProtocol,
    type EvolutionSimulatedUserPlan,
    type EvolutionVariant
  } from '$lib/directedEvolution';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';
  type EvolutionView = 'directions' | 'detail' | 'genealogy';
  const evolutionViews = [
    { id: 'directions', label: 'Directions', icon: Compass },
    { id: 'detail', label: 'Direction Detail', icon: ListChecks },
    { id: 'genealogy', label: 'Organism Genealogy', icon: Dna }
  ] satisfies { id: EvolutionView; label: string; icon: typeof Compass }[];

  let snapshot: DirectedEvolutionSnapshot | null = null;
  let loading = false;
  let error = '';
  let actionBusy = '';
  let selectedEpisodeId = '';
  let activeView: EvolutionView = 'detail';
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
  $: pendingMaterializations = promotions.filter(
    (promotion) => !promotionHotLoaded(promotion) && !promotionFailed(promotion)
  );
  $: failedMaterializations = promotions.filter((promotion) => promotionFailed(promotion));
  $: currentParentVersion = organism
    ? organismVersions.find((version) => version.id === (organism.organismVersionId || organism.parentVersionId)) ??
      null
    : null;
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
  $: organismHeaderAppRef =
    currentParentVersion?.appRef ||
    selectedPromotion?.canonicalAppRef ||
    selectedPromotion?.appRef ||
    organism?.appRef ||
    '';
  $: currentGoal = selectedEpisode
    ? snapshot?.adaptationGoals.find((goal) => goal.id === selectedEpisode.adaptationGoalId) ?? null
    : null;
  $: currentSelectionPressure = selectedEpisode
    ? snapshot?.selectionPressures.find((pressure) => pressure.id === selectedEpisode.selectionPressureId) ??
      null
    : null;
  $: currentSelectionProtocol = selectedEpisode ? episodeSelectionProtocol(selectedEpisode) : null;
  $: currentSimulatedUserPlan = selectedEpisode ? episodeSimulatedUserPlan(selectedEpisode) : null;
  $: episodeVariants = selectedEpisode
    ? (snapshot?.variants ?? []).filter((variant) => variant.episodeId === selectedEpisode.id)
    : [];
  $: episodeGenerations = selectedEpisode
    ? (snapshot?.generations ?? [])
        .filter((generation) => generation.episodeId === selectedEpisode.id)
        .sort((a, b) => a.generationIndex - b.generationIndex)
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
  $: metrics = selectedEpisode
    ? episodeMetrics(selectedEpisode, currentSelectionPressure, currentSelectionProtocol)
    : [];
  $: eliminationRules = selectedEpisode
    ? episodeEliminationRules(selectedEpisode, currentSelectionPressure)
    : [];
  $: scoringRules = selectedEpisode
    ? episodeScoringRules(selectedEpisode, currentSelectionPressure)
    : [];
  $: stageResults = selectedEpisode
    ? (snapshot?.stageResults ?? []).filter((result) => result.episodeId === selectedEpisode.id)
    : [];
  $: episodeTrials = selectedEpisode
    ? (snapshot?.trials ?? []).filter((trial) => trial.episodeId === selectedEpisode.id)
    : [];
  $: completedEpisodeTrials = episodeTrials.filter((trial) =>
    ['Succeeded', 'Passed', 'Observed'].includes(trial.status)
  );
  $: blockedEpisodeTrials = episodeTrials.filter(
    (trial) => trial.blocker || ['Failed', 'Blocked', 'Eliminated'].includes(trial.status)
  );
  $: inspectedVariant =
    episodeVariants.find((variant) => variant.id === inspectedVariantId) ??
    episodeVariants.find((variant) => variant.id === selectedEpisode?.winningVariantId) ??
    episodeVariants.find((variant) => variant.status === 'Promoted' || variant.status === 'Selected') ??
    episodeVariants.find((variant) => variant.status !== 'Eliminated' && variant.status !== 'Failed') ??
    episodeVariants.find((variant) => variant.status === 'Eliminated' || variant.status === 'Failed') ??
    episodeVariants[0] ??
    null;
  $: comparedVariants = comparedVariantIds
    .map((id) => episodeVariants.find((variant) => variant.id === id))
    .filter((variant): variant is EvolutionVariant => Boolean(variant));
  $: recentWorkerRuns = (snapshot?.workerRuns ?? []).slice(-8).reverse();
  $: recentWorkItems = (snapshot?.workItems ?? []).slice(-8).reverse();
  $: selectedEpisodeWorkItems = selectedEpisode ? workItemsForEpisode(selectedEpisode) : [];
  $: selectedEpisodeWorkerRuns = selectedEpisode ? workerRunsForEpisode(selectedEpisode) : [];
  $: selectedEpisodeEvidence = selectedEpisode ? evidenceForEpisode(selectedEpisode) : [];
  $: selectedEpisodeDatadogEvidence = selectedEpisodeEvidence.filter(isStructuredDatadogEvidence);
  $: selectedEpisodeProofGates = selectedEpisode
    ? proofGatesForEpisode(
        selectedEpisode,
        selectedEpisodeWorkItems.length,
        selectedEpisodeWorkerRuns.length,
        selectedEpisodeEvidence.length,
        selectedEpisodeDatadogEvidence.length
      )
    : [];
  $: selectedEpisodeProofReady = selectedEpisodeProofGates.every((gate) => gate.tone === 'success');
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

  function selectEpisode(episodeId: string) {
    selectedEpisodeId = episodeId;
    activeView = 'detail';
  }

  function selectDirection(direction: EvolutionDirection) {
    const episode =
      (snapshot?.episodes ?? []).find((item) => item.id === direction.episodeId) ??
      (snapshot?.episodes ?? []).find((item) => item.directionId === direction.id);
    if (episode) {
      selectEpisode(episode.id);
    }
  }

  function shortId(value: string, length = 10): string {
    if (!value) return 'pending';
    return value.length > length ? `${value.slice(0, length)}...` : value;
  }

  function statusTone(status: string): StatusTone {
    if (['Active', 'Running', 'Passed', 'Selected', 'Promoted', 'Completed', 'Succeeded', 'Parent'].includes(status)) {
      return 'success';
    }
    if (['Queued', 'Draft', 'Negotiating', 'Planned', 'Generating', 'Evaluating', 'Selecting', 'Promoting', 'Paused', 'Claimed'].includes(status)) {
      return 'warning';
    }
    if (['Failed', 'Stopped', 'Eliminated', 'NotSelected', 'Dismissed', 'Cancelled'].includes(status)) {
      return 'danger';
    }
    if (['Proposed', 'Framed', 'Linked', 'Pinned'].includes(status)) {
      return 'primary';
    }
    return 'neutral';
  }

  function episodeDirection(episode: EvolutionEpisode): EvolutionDirection | null {
    return activeDirections.find((direction) => direction.id === episode.directionId) ?? null;
  }

  function episodePromotion(episode: EvolutionEpisode) {
    return (
      promotions.find((promotion) => promotion.id === episode.promotionId) ??
      promotions.find((promotion) => promotion.episodeId === episode.id) ??
      promotions.find((promotion) => promotion.winningVariantId === episode.winningVariantId) ??
      null
    );
  }

  function promotionHotLoaded(promotion: { materialized: boolean; runtimeRef: string }): boolean {
    return promotion.materialized || Boolean(promotion.runtimeRef);
  }

  function promotionFailed(promotion: { materializationFailed: boolean; status: string }): boolean {
    return promotion.materializationFailed || promotion.status === 'Failed';
  }

  function episodeSelectionProtocol(episode: EvolutionEpisode): EvolutionSelectionProtocol | null {
    return (
      (snapshot?.selectionProtocols ?? []).find((protocol) => protocol.id === episode.selectionProtocolId) ??
      (snapshot?.selectionProtocols ?? []).find((protocol) => protocol.episodeId === episode.id) ??
      null
    );
  }

  function episodeSimulatedUserPlan(episode: EvolutionEpisode): EvolutionSimulatedUserPlan | null {
    return (
      (snapshot?.simulatedUserPlans ?? []).find((plan) => plan.id === episode.simulatedUserPlanId) ??
      (snapshot?.simulatedUserPlans ?? []).find((plan) => plan.episodeId === episode.id) ??
      null
    );
  }

  function episodeMetrics(
    episode: EvolutionEpisode,
    pressure: DirectedEvolutionSnapshot['selectionPressures'][number] | null,
    protocol: DirectedEvolutionSnapshot['selectionProtocols'][number] | null
  ): EvolutionMetricDefinition[] {
    const ids = new Set([
      ...episode.metricDefinitionIds,
      ...(pressure?.metricIds ?? []),
      ...(protocol?.metricIds ?? [])
    ]);
    const directMatches = (snapshot?.metricDefinitions ?? []).filter(
      (metric) => metric.episodeId === episode.id || ids.has(metric.id)
    );
    if (directMatches.length) return directMatches;
    return (snapshot?.metricDefinitions ?? []).filter((metric) => metric.status !== 'Archived');
  }

  function episodeEliminationRules(
    episode: EvolutionEpisode,
    pressure: DirectedEvolutionSnapshot['selectionPressures'][number] | null
  ): EvolutionEliminationRule[] {
    const ids = new Set([...episode.eliminationRuleIds, ...(pressure?.eliminationRuleIds ?? [])]);
    return (snapshot?.eliminationRules ?? []).filter(
      (rule) => rule.episodeId === episode.id || ids.has(rule.id)
    );
  }

  function episodeScoringRules(
    episode: EvolutionEpisode,
    pressure: DirectedEvolutionSnapshot['selectionPressures'][number] | null
  ): EvolutionScoringRule[] {
    const ids = new Set([...episode.scoringRuleIds, ...(pressure?.scoringRuleIds ?? [])]);
    return (snapshot?.scoringRules ?? []).filter(
      (rule) => rule.episodeId === episode.id || ids.has(rule.id)
    );
  }

  function episodeMaterializationTone(episode: EvolutionEpisode): StatusTone {
    const promotion = episodePromotion(episode);
    if (promotion && promotionFailed(promotion)) return 'danger';
    if (promotion && promotionHotLoaded(promotion)) return 'success';
    if (episode.status === 'Promoting' || promotion) return 'warning';
    return 'neutral';
  }

  function episodeMaterializationLabel(episode: EvolutionEpisode): string {
    const promotion = episodePromotion(episode);
    if (promotion && promotionFailed(promotion)) return 'install failed';
    if (promotion && promotionHotLoaded(promotion)) return 'hot-loaded';
    if (episode.status === 'Promoting' || promotion) return 'install pending';
    return 'no promotion';
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

  function variantMutation(variant: EvolutionVariant): EvolutionMutation | null {
    return (
      (snapshot?.mutations ?? []).find((mutation) => mutation.id === variant.mutationId) ??
      (snapshot?.mutations ?? []).find((mutation) => mutation.variantId === variant.id) ??
      (variant.changedFiles.length || variant.diffPatch
        ? {
            id: `${variant.id}:inline-mutation`,
            status: variant.status,
            variantId: variant.id,
            summary: variant.summary,
            changedFiles: variant.changedFiles,
            diffRef: variant.branchRef || variant.appRef,
            diffPatch: variant.diffPatch,
            workerRunId: variant.workerRunId,
            reason: variant.reason,
            raw: variant.raw
          }
        : null)
    );
  }

  function variantMeasurements(variant: EvolutionVariant) {
    return (snapshot?.measurements ?? []).filter((measurement) => measurement.variantId === variant.id);
  }

  function variantTrials(variant: EvolutionVariant) {
    return (snapshot?.trials ?? []).filter((trial) => trial.variantId === variant.id);
  }

  function variantStageResults(variant: EvolutionVariant) {
    return stageResults.filter((result) => result.variantId === variant.id);
  }

  function directionEpisodes(direction: EvolutionDirection) {
    return (snapshot?.episodes ?? []).filter(
      (episode) => episode.directionId === direction.id || episode.id === direction.episodeId
    );
  }

  function directionStatus(direction: EvolutionDirection): string {
    const episodes = directionEpisodes(direction);
    if (episodes.some((episode) => ['Running', 'Selecting', 'Promoting'].includes(episode.status))) {
      return 'active';
    }
    if (episodes.some((episode) => episode.status === 'Completed')) return 'completed';
    if (direction.status === 'Dismissed') return 'dismissed';
    return direction.status || 'suggested';
  }

  function directionAutonomyLabel(direction: EvolutionDirection): string {
    const episode = directionEpisodes(direction)[0];
    return episode?.autonomyLane || direction.autonomyLane || 'human-gated';
  }

  function prettyJsonList(raw: string, fallback = 'None recorded'): string[] {
    if (!raw) return [fallback];
    try {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        return parsed.map((item) =>
          typeof item === 'string' ? item : JSON.stringify(item)
        );
      }
      return [typeof parsed === 'string' ? parsed : JSON.stringify(parsed)];
    } catch {
      return [raw];
    }
  }

  function directionPressureSummary(direction: EvolutionDirection): string {
    const pressure =
      snapshot?.pressures.find((item) => item.directionId === direction.id) ??
      snapshot?.pressures.find((item) => direction.pressureIds.includes(item.id));
    return pressure?.summary || direction.summary;
  }

  function directionPressures(direction: EvolutionDirection) {
    return (snapshot?.pressures ?? []).filter(
      (pressure) => pressure.directionId === direction.id || direction.pressureIds.includes(pressure.id)
    );
  }

  function directionSignals(direction: EvolutionDirection) {
    const pressures = directionPressures(direction);
    const pressureIds = new Set(pressures.map((pressure) => pressure.id));
    const signalIds = new Set(pressures.flatMap((pressure) => pressure.signalIds));
    return (snapshot?.signals ?? []).filter(
      (signal) => signalIds.has(signal.id) || pressureIds.has(signal.pressureId)
    );
  }

  function directionEvidence(direction: EvolutionDirection) {
    const pressures = directionPressures(direction);
    const signals = directionSignals(direction);
    const ids = new Set(
      [
        ...pressures.map((pressure) => pressure.evidenceArtifactId),
        ...signals.map((signal) => signal.evidenceArtifactId)
      ].filter(Boolean)
    );
    const targetPairs = new Set([
      `Direction:${direction.id}`,
      ...pressures.map((pressure) => `Pressure:${pressure.id}`),
      ...signals.map((signal) => `Signal:${signal.id}`)
    ]);
    return (snapshot?.evidenceArtifacts ?? []).filter(
      (artifact) =>
        ids.has(artifact.id) || targetPairs.has(`${artifact.targetEntityType}:${artifact.targetEntityId}`)
    );
  }

  function directionWorkerRun(direction: EvolutionDirection) {
    const pressures = directionPressures(direction);
    const workerRunIds = [direction.workerRunId, ...pressures.map((pressure) => pressure.workerRunId)].filter(
      Boolean
    );
    return (snapshot?.workerRuns ?? []).find((run) => workerRunIds.includes(run.id)) ?? null;
  }

  function selectedEpisodeEntityIds(episode: EvolutionEpisode): Set<string> {
    const ids = new Set([
      episode.id,
      episode.directionId,
      episode.promotionId,
      episode.winningVariantId,
      ...((snapshot?.generations ?? [])
        .filter((generation) => generation.episodeId === episode.id)
        .map((generation) => generation.id)),
      ...((snapshot?.variants ?? [])
        .filter((variant) => variant.episodeId === episode.id)
        .flatMap((variant) => [variant.id, variant.generationId, variant.mutationId, variant.workItemId])),
      ...((snapshot?.stageResults ?? [])
        .filter((result) => result.episodeId === episode.id)
        .flatMap((result) => [result.id, result.workItemId, result.evidenceArtifactId])),
      ...((snapshot?.trials ?? [])
        .filter((trial) => trial.episodeId === episode.id)
        .flatMap((trial) => [trial.id, trial.workItemId, trial.evidenceArtifactId]))
    ].filter(Boolean));
    return ids;
  }

  function parsedRecord(raw: string): Record<string, unknown> {
    if (!raw) return {};
    try {
      const parsed = JSON.parse(raw);
      return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
        ? (parsed as Record<string, unknown>)
        : {};
    } catch {
      return {};
    }
  }

  function nestedString(record: Record<string, unknown>, path: string[]): string {
    let current: unknown = record;
    for (const key of path) {
      if (!current || typeof current !== 'object' || Array.isArray(current)) return '';
      current = (current as Record<string, unknown>)[key];
    }
    return typeof current === 'string' ? current : '';
  }

  function correlationEpisodeId(raw: string): string {
    const correlation = parsedRecord(raw);
    return (
      nestedString(correlation, ['episode_id']) ||
      nestedString(correlation, ['episodeId']) ||
      nestedString(correlation, ['datadog', 'join_fields', 'episode_id']) ||
      nestedString(correlation, ['output', 'episode_id']) ||
      nestedString(correlation, ['output', 'episodeId'])
    );
  }

  function workItemsForEpisode(episode: EvolutionEpisode) {
    const ids = selectedEpisodeEntityIds(episode);
    return (snapshot?.workItems ?? []).filter(
      (item) => ids.has(item.targetEntityId) || correlationEpisodeId(item.correlationJson) === episode.id
    );
  }

  function workerRunsForEpisode(episode: EvolutionEpisode) {
    const workItemIds = new Set(workItemsForEpisode(episode).map((item) => item.id));
    return (snapshot?.workerRuns ?? []).filter(
      (run) =>
        workItemIds.has(run.workItemId) ||
        correlationEpisodeId(run.correlationJson) === episode.id ||
        run.summary.includes(episode.id)
    );
  }

  function evidenceForEpisode(episode: EvolutionEpisode) {
    const ids = selectedEpisodeEntityIds(episode);
    return (snapshot?.evidenceArtifacts ?? []).filter(
      (artifact) =>
        (artifact.targetEntityType === 'Episode' && artifact.targetEntityId === episode.id) ||
        ids.has(artifact.targetEntityId) ||
        ids.has(artifact.id) ||
        correlationEpisodeId(artifact.correlationJson) === episode.id
    );
  }

  function evidenceScopeDatadogUrl(raw: string): string {
    const correlation = parsedRecord(raw);
    const directOutput = correlation.output;
    const scope =
      directOutput && typeof directOutput === 'object' && !Array.isArray(directOutput)
        ? ((directOutput as Record<string, unknown>).evidence_scope ??
            (directOutput as Record<string, unknown>).evidenceScope)
        : undefined;
    if (!Array.isArray(scope)) return '';
    const first = scope.find((item) => {
      const url =
        item && typeof item === 'object'
          ? ((item as Record<string, unknown>).datadog_url ??
              (item as Record<string, unknown>).datadogUrl)
          : '';
      return typeof url === 'string' && url.startsWith('https://app.');
    });
    if (!first || typeof first !== 'object') return '';
    const url =
      (first as Record<string, unknown>).datadog_url ??
      (first as Record<string, unknown>).datadogUrl;
    return typeof url === 'string' ? url : '';
  }

  function isStructuredDatadogEvidence(artifact: EvolutionEvidenceArtifact): boolean {
    const datadogUrl = artifact.uri.startsWith('https://app.') ? artifact.uri : evidenceScopeDatadogUrl(artifact.correlationJson);
    return (
      artifact.evidenceProvenance === 'datadog-measured' &&
      Boolean(artifact.query) &&
      Boolean(artifact.timeWindow) &&
      artifact.resultCount !== '' &&
      Boolean(artifact.interpretation) &&
      Boolean(artifact.zeroResultMeaning) &&
      Boolean(datadogUrl)
    );
  }

  function proofGatesForEpisode(
    episode: EvolutionEpisode,
    workItemCount: number,
    workerRunCount: number,
    evidenceCount: number,
    datadogEvidenceCount: number
  ): { label: string; value: string; tone: StatusTone }[] {
    const terminalSuccess = ['Completed', 'Complete', 'Succeeded', 'Promoted', 'NoPromotion'].includes(episode.status);
    const terminalFailure = ['Failed', 'Stopped', 'Cancelled', 'Abandoned'].includes(episode.status);
    return [
      {
        label: 'WorkItems',
        value: `${workItemCount}`,
        tone: workItemCount ? 'success' : 'warning'
      },
      {
        label: 'WorkerRuns',
        value: `${workerRunCount}`,
        tone: workerRunCount ? 'success' : 'warning'
      },
      {
        label: 'EvidenceArtifacts',
        value: `${evidenceCount}`,
        tone: evidenceCount ? 'success' : 'warning'
      },
      {
        label: 'Datadog measured evidence',
        value: `${datadogEvidenceCount}`,
        tone: datadogEvidenceCount ? 'success' : 'danger'
      },
      {
        label: 'Terminal success',
        value: terminalSuccess ? episode.status : terminalFailure ? episode.status : 'pending',
        tone: terminalSuccess ? 'success' : terminalFailure ? 'danger' : 'warning'
      }
    ];
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
        <section class="overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white">
          <div class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--color-border)] px-3 py-3 sm:px-4">
            <div class="min-w-0">
              <p class="v-eyebrow">Directed Evolution · Mission Control</p>
              <h2 class="v-display mt-1 text-[24px] text-[var(--color-ink)]">
                {organism?.name || 'No organism online'}
              </h2>
              <p class="mt-1 max-w-[78ch] text-[12.5px] leading-relaxed text-[var(--color-muted)]">
                {organismHeaderAppRef ||
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
              <Button
                size="icon"
                title="Refresh evolution state"
                aria-label="Refresh evolution state"
                onclick={() => void refreshAll()}
                disabled={loading}
              >
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
            <div class="flex flex-wrap items-center gap-1.5 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-1">
              {#each evolutionViews as item (item.id)}
                <button
                  type="button"
                  class={`inline-flex min-h-9 flex-1 items-center justify-center gap-2 rounded-[var(--radius-xs)] px-3 py-2 text-[12px] font-semibold transition-colors sm:flex-none ${
                    activeView === item.id
                      ? 'bg-white text-[var(--color-ink)] shadow-[var(--shadow-xs)]'
                      : 'text-[var(--color-muted)] hover:bg-white/65 hover:text-[var(--color-ink)]'
                  }`}
                  onclick={() => (activeView = item.id)}
                >
                  <svelte:component this={item.icon} size={14} />
                  {item.label}
                </button>
              {/each}
            </div>

            <div class="min-w-0">
              <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-9">
                <MetricTile compact label="Directions" value={activeDirections.length} />
                <MetricTile compact label="Plans" value={snapshot?.simulatedUserPlans.length ?? 0} />
                <MetricTile compact label="Episodes" value={activeEpisodes.length} />
                <MetricTile compact label="Variants" value={episodeVariants.length} />
                <MetricTile compact label="Promotions" value={promotions.length} />
                <MetricTile compact label="Materialized" value={promotions.filter((item) => promotionHotLoaded(item)).length} />
                <MetricTile compact label="Materializing" value={pendingMaterializations.length} />
                <MetricTile compact label="Failed Installs" value={failedMaterializations.length} />
                <MetricTile compact label="Worker Runs" value={snapshot?.workerRuns.length ?? 0} />
              </div>
            </div>

            {#if activeView === 'directions'}
              <section class="grid gap-3 lg:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
                <div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-3">
                  <PanelTitle icon={Compass} title="Directions View" />
                  <p class="mt-2 max-w-[78ch] text-[12px] leading-relaxed text-[var(--color-muted)]">
                    Suggested, active, completed, dismissed, auto-started, and human-gated directions. Each card shows what fed it and whether it is allowed to move without human approval.
                  </p>
                  <div class="mt-3 grid gap-2">
                    {#each activeDirections as direction (direction.id)}
                      {@const episodesForDirection = directionEpisodes(direction)}
                      {@const pressuresForDirection = directionPressures(direction)}
                      {@const signalsForDirection = directionSignals(direction)}
                      {@const evidenceForDirection = directionEvidence(direction)}
                      <button
                        type="button"
                        class={`min-w-0 rounded-[var(--radius-sm)] border bg-white p-3 text-left transition-colors ${
                          selectedDirection?.id === direction.id
                            ? 'border-[var(--color-primary)]/35 shadow-[var(--shadow-xs)]'
                            : 'border-[var(--color-border-soft)] hover:border-[var(--color-primary)]/25'
                        }`}
                        onclick={() => selectDirection(direction)}
                      >
                        <div class="flex flex-wrap items-center gap-1.5">
                          <Badge tone={statusTone(directionStatus(direction))}>{directionStatus(direction)}</Badge>
                          <Badge tone={directionAutonomyLabel(direction).includes('auto') ? 'warning' : 'primary'}>
                            {directionAutonomyLabel(direction)}
                          </Badge>
                          <Badge tone="neutral">{direction.pressureClass || 'pressure pending'}</Badge>
                        </div>
                        <h3 class="mt-2 text-[14px] font-semibold tracking-tight text-[var(--color-ink)]">
                          {direction.title || direction.proposedAdaptationGoal || shortId(direction.id)}
                        </h3>
                        <p class="mt-1 line-clamp-3 text-[12px] leading-relaxed text-[var(--color-muted)]">
                          {directionPressureSummary(direction) || direction.summary || 'No direction rationale recorded yet.'}
                        </p>
                        <div class="mt-3 grid grid-cols-4 gap-1.5 text-center font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
                          <span class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1">{pressuresForDirection.length} pressures</span>
                          <span class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1">{signalsForDirection.length} signals</span>
                          <span class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1">{evidenceForDirection.length} evidence</span>
                          <span class="rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1">{episodesForDirection.length} episodes</span>
                        </div>
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="grid gap-3">
                  <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
                    <PanelTitle icon={Activity} title="Basis Drill-In" />
                    {#if selectedDirection}
                      {@const pressuresForDirection = directionPressures(selectedDirection)}
                      {@const signalsForDirection = directionSignals(selectedDirection)}
                      {@const evidenceForDirection = directionEvidence(selectedDirection)}
                      <h3 class="mt-3 text-[16px] font-semibold tracking-tight text-[var(--color-ink)]">
                        {selectedDirection.title || shortId(selectedDirection.id)}
                      </h3>
                      <p class="mt-1 text-[12px] leading-relaxed text-[var(--color-muted)]">
                        {selectedDirection.proposedAdaptationGoal || selectedDirection.summary || 'No proposed Adaptation Goal recorded.'}
                      </p>
                      <div class="mt-3 grid gap-2 md:grid-cols-3">
                        <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] p-2">
                          <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">Signals</p>
                          {#each signalsForDirection.slice(0, 4) as signal (signal.id)}
                            <p class="mt-1 line-clamp-2 text-[11px] text-[var(--color-ink-soft)]">{signal.summary}</p>
                          {:else}
                            <p class="mt-1 text-[11px] text-[var(--color-faint)]">No linked signals.</p>
                          {/each}
                        </div>
                        <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] p-2">
                          <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">Pressures</p>
                          {#each pressuresForDirection.slice(0, 4) as pressure (pressure.id)}
                            <p class="mt-1 line-clamp-2 text-[11px] text-[var(--color-ink-soft)]">{pressure.summary}</p>
                          {:else}
                            <p class="mt-1 text-[11px] text-[var(--color-faint)]">No linked pressures.</p>
                          {/each}
                        </div>
                        <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] p-2">
                          <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">Evidence</p>
                          {#each evidenceForDirection.slice(0, 4) as artifact (artifact.id)}
                            <p class="mt-1 line-clamp-2 text-[11px] text-[var(--color-ink-soft)]">{artifact.summary || artifact.interpretation || artifact.uri}</p>
                          {:else}
                            <p class="mt-1 text-[11px] text-[var(--color-faint)]">No evidence summaries.</p>
                          {/each}
                        </div>
                      </div>
                    {:else}
                      <p class="mt-3 rounded-[var(--radius-sm)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-3 py-3 text-[12px] text-[var(--color-muted)]">
                        No direction is available in this tenant yet.
                      </p>
                    {/if}
                  </section>
                </div>
              </section>
            {:else if activeView === 'detail'}
              {#if activeEpisodes.length > 1}
                <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-2">
                  <div class="flex flex-wrap items-center justify-between gap-2 px-1 pb-2">
                    <PanelTitle icon={GitCompareArrows} title="Episode Track" />
                    <span class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
                      {activeEpisodes.length} live records
                    </span>
                  </div>
                  <div class="grid gap-2 lg:grid-cols-3">
                    {#each activeEpisodes as episode (episode.id)}
                      {@const direction = episodeDirection(episode)}
                      <button
                        type="button"
                        aria-label={`Open episode ${direction?.title || shortId(episode.id)}`}
                        class={`min-w-0 rounded-[var(--radius-sm)] border px-2.5 py-2 text-left transition-colors duration-[var(--duration-soft)] ${
                          selectedEpisode?.id === episode.id
                            ? 'border-[var(--color-primary)]/30 bg-white shadow-[var(--shadow-xs)]'
                            : 'border-[var(--color-border-soft)] bg-white/70 hover:border-[var(--color-primary)]/24 hover:bg-white'
                        }`}
                        onclick={() => selectEpisode(episode.id)}
                      >
                        <div class="flex flex-wrap items-center gap-1.5">
                          <Badge tone={statusTone(episode.status)}>{episode.status}</Badge>
                          <Badge tone={episodeMaterializationTone(episode)}>
                            {episodeMaterializationLabel(episode)}
                          </Badge>
                        </div>
                        <p class="mt-1.5 truncate text-[12px] font-semibold tracking-tight text-[var(--color-ink)]">
                          {direction?.title || episode.summary || shortId(episode.id)}
                        </p>
                        <p class="mt-0.5 truncate font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
                          {episode.autonomyLane || 'lane pending'} · {shortId(episode.id, 12)}
                        </p>
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}

              <section class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.42fr)]">
                <div class="grid gap-3">
                  <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-3">
                    <div class="flex flex-wrap items-center justify-between gap-2">
                      <PanelTitle icon={FileDiff} title="Protocol And Lab" />
                      <Badge tone={currentSimulatedUserPlan ? 'success' : 'warning'}>
                        {currentSimulatedUserPlan
                          ? `${currentSimulatedUserPlan.usersPerVariant} personas × ${currentSimulatedUserPlan.runsPerPersona} runs`
                          : 'sim plan missing'}
                      </Badge>
                    </div>
                    <div class="mt-3 grid gap-2 md:grid-cols-3">
                      <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-2">
                        <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">Selection protocol</p>
                        <p class="mt-1 line-clamp-3 text-[11.5px] leading-snug text-[var(--color-ink-soft)]">
                          {currentSelectionProtocol?.selectionStatement || currentSelectionPressure?.selectionStatement || 'No selection protocol linked.'}
                        </p>
                      </div>
                      <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-2">
                        <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">Evaluator lineage</p>
                        <p class="mt-1 line-clamp-3 text-[11.5px] leading-snug text-[var(--color-ink-soft)]">
                          {currentSelectionProtocol?.evaluatorRef || selectedEpisode?.evaluatorRef || 'No frozen evaluator ref recorded.'}
                        </p>
                      </div>
                      <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-2">
                        <p class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">Simulated-user lab</p>
                        <p class="mt-1 text-[11.5px] leading-snug text-[var(--color-ink-soft)]">
                          {episodeTrials.length} trial rows · {completedEpisodeTrials.length} completed · {blockedEpisodeTrials.length} blockers
                        </p>
                      </div>
                    </div>
                  </section>

              <EvolutionEpisodePanel
                {selectedEpisode}
                {selectedDirection}
                selectedPromotion={selectedPromotion}
                {currentGoal}
                {currentSelectionPressure}
                generations={episodeGenerations}
                {stages}
                {stageResults}
                {episodeVariants}
                mutations={snapshot?.mutations ?? []}
                {constraints}
                metricDefinitions={metrics}
                {eliminationRules}
                {scoringRules}
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

                <aside class="grid gap-3">
                  {#if inspectedVariant}
                    <VariantInspectCard
                      variant={inspectedVariant}
                      mutation={variantMutation(inspectedVariant)}
                      measurements={variantMeasurements(inspectedVariant)}
                      evidence={variantEvidence(inspectedVariant)}
                      reason={variantReason(inspectedVariant)}
                      shortId={shortId}
                      statusTone={statusTone}
                    />
                    <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
                      <PanelTitle icon={Activity} title="Variant Lab Runs" />
                      <div class="mt-2 grid gap-1.5">
                        {#each variantTrials(inspectedVariant).slice(0, 6) as trial (trial.id)}
                          <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] px-2 py-1.5">
                            <div class="flex items-center justify-between gap-2">
                              <Badge tone={statusTone(trial.status)}>{trial.status}</Badge>
                              <span class="font-mono text-[10px] text-[var(--color-faint)]">{shortId(trial.simulatedUserId, 16)}</span>
                            </div>
                            <p class="mt-1 line-clamp-2 text-[11px] text-[var(--color-muted)]">
                              {trial.summary || trial.blocker || trial.goal}
                            </p>
                          </div>
                        {:else}
                          <p class="text-[12px] text-[var(--color-muted)]">No simulated-user trials recorded for this variant.</p>
                        {/each}
                      </div>
                    </section>
                  {/if}
                  {#if selectedEpisode}
                    <section
                      aria-label="Agent Answers live proof gate"
                      class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3"
                    >
                      <div class="flex flex-wrap items-center justify-between gap-2">
                        <PanelTitle icon={ShieldCheck} title="Agent Answers Proof Gate" />
                        <Badge tone={selectedEpisodeProofReady ? 'success' : 'warning'}>
                          {selectedEpisodeProofReady ? 'ready' : 'pending'}
                        </Badge>
                      </div>
                      <p class="mt-2 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-faint)]">
                        directed-evolution-agent-answers-live-proof.sh
                      </p>
                      <div class="mt-3 grid gap-1.5">
                        {#each selectedEpisodeProofGates as gate (gate.label)}
                          <div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] px-2 py-1.5">
                            <span class="truncate text-[11px] font-medium text-[var(--color-ink-soft)]">
                              {gate.label}
                            </span>
                            <Badge tone={gate.tone}>{gate.value}</Badge>
                          </div>
                        {/each}
                      </div>
                      {#if selectedEpisodeDatadogEvidence[0]}
                        <div class="mt-3 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
                          <p class="line-clamp-2 text-[11px] font-medium leading-snug text-[var(--color-ink-soft)]">
                            {selectedEpisodeDatadogEvidence[0].interpretation || selectedEpisodeDatadogEvidence[0].summary}
                          </p>
                          <p class="mt-1 line-clamp-2 font-mono text-[10px] text-[var(--color-muted)]">
                            {selectedEpisodeDatadogEvidence[0].query}
                          </p>
                          <p class="mt-1 text-[10px] text-[var(--color-faint)]">
                            {selectedEpisodeDatadogEvidence[0].timeWindow} · zero means {selectedEpisodeDatadogEvidence[0].zeroResultMeaning}
                          </p>
                        </div>
                      {/if}
                    </section>
                  {/if}
                  <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
                    <PanelTitle icon={Activity} title="Recent Worker Work" />
                    <div class="mt-2 grid gap-1.5">
                      {#each recentWorkItems.slice(0, 5) as item (item.id)}
                        <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] px-2 py-1.5 text-[11px]">
                          <div class="flex items-center justify-between gap-2">
                            <Badge tone={statusTone(item.status)}>{item.status}</Badge>
                            <span class="font-mono text-[10px] text-[var(--color-faint)]">{item.role}</span>
                          </div>
                          <p class="mt-1 line-clamp-2 text-[var(--color-muted)]">{item.summary || item.failureReason || shortId(item.targetEntityId)}</p>
                        </div>
                      {/each}
                    </div>
                  </section>
                </aside>
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
                        mutation={variantMutation(variant)}
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
            {:else}
              <EvolutionLineagePolicy
                {organism}
                {currentParentVersion}
                {organismVersions}
                {lineageEdges}
                episodes={activeEpisodes}
                directions={activeDirections}
                {promotions}
                variants={snapshot?.variants ?? []}
                mutations={snapshot?.mutations ?? []}
                {activePolicy}
                {shortId}
                {statusTone}
                {jsonEntries}
              />
            {/if}
          </div>
        </section>
  </section>
</main>
