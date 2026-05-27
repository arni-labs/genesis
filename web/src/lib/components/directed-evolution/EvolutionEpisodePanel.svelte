<script lang="ts">
  import {
    AlertTriangle,
    CheckCircle2,
    Eye,
    GitCompareArrows,
    LoaderCircle,
    PackageCheck,
    Pause,
    Play,
    ShieldCheck,
    Square
  } from '@lucide/svelte';
  import { Badge, Button } from '$lib/components/ui';
  import type {
    EvolutionAdaptationGoal,
    EvolutionDirection,
    EvolutionEliminationRule,
    EvolutionEpisode,
    EvolutionEpisodeStartRequest,
    EvolutionEvaluationStage,
    EvolutionGeneration,
    EvolutionMetricDefinition,
    EvolutionPromotion,
    EvolutionScoringRule,
    EvolutionSelectionPressure,
    EvolutionStageResult,
    EvolutionVariant,
    EvolutionViabilityConstraint
  } from '$lib/directedEvolution';
  import ConstraintCard from './ConstraintCard.svelte';
  import GenerationTopology from './GenerationTopology.svelte';
  import MetricTile from './MetricTile.svelte';
  import MetricsRulesCard from './MetricsRulesCard.svelte';
  import PanelTitle from './PanelTitle.svelte';
  import StartRequestCard from './StartRequestCard.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    selectedEpisode: EvolutionEpisode | null;
    selectedDirection: EvolutionDirection | null;
    selectedPromotion: EvolutionPromotion | null;
    currentGoal: EvolutionAdaptationGoal | null;
    currentSelectionPressure: EvolutionSelectionPressure | null;
    generations: EvolutionGeneration[];
    stages: EvolutionEvaluationStage[];
    stageResults: EvolutionStageResult[];
    episodeVariants: EvolutionVariant[];
    constraints: EvolutionViabilityConstraint[];
    startRequest: EvolutionEpisodeStartRequest | null;
    metricDefinitions: EvolutionMetricDefinition[];
    eliminationRules: EvolutionEliminationRule[];
    scoringRules: EvolutionScoringRule[];
    comparedVariantIds: string[];
    actionBusy: string;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
    onPauseEpisode: (episode: EvolutionEpisode) => void;
    onResumeEpisode: (episode: EvolutionEpisode) => void;
    onStopEpisode: (episode: EvolutionEpisode) => void;
    onPinConstraint: (constraint: EvolutionViabilityConstraint) => void;
    onInspectVariant: (variantId: string) => void;
    onToggleCompare: (variant: EvolutionVariant) => void;
  };

  let {
    selectedEpisode,
    selectedDirection,
    selectedPromotion,
    currentGoal,
    currentSelectionPressure,
    generations,
    stages,
    stageResults,
    episodeVariants,
    constraints,
    startRequest,
    metricDefinitions,
    eliminationRules,
    scoringRules,
    comparedVariantIds,
    actionBusy,
    shortId,
    statusTone,
    onPauseEpisode,
    onResumeEpisode,
    onStopEpisode,
    onPinConstraint,
    onInspectVariant,
    onToggleCompare
  }: Props = $props();

  const terminalEpisodeStatuses = new Set(['Completed', 'Stopped', 'Failed']);

  function canPause(episode: EvolutionEpisode): boolean {
    return episode.status === 'Running' || episode.status === 'Selecting';
  }

  function canResume(episode: EvolutionEpisode): boolean {
    return episode.status === 'Paused';
  }

  function canStop(episode: EvolutionEpisode): boolean {
    return !terminalEpisodeStatuses.has(episode.status);
  }

  function variantStageResult(variant: EvolutionVariant, stage: EvolutionEvaluationStage) {
    return stageResults.find(
      (result) => result.variantId === variant.id && result.evaluationStageId === stage.id
    );
  }

  function stageResultLabel(result: EvolutionStageResult | undefined): string {
    if (!result) return 'Waiting';
    if (result.summary) return result.summary;
    if (result.failureReason) return result.failureReason;
    if (result.reason) return result.reason;
    return result.status;
  }

  function resultColor(status: string): string {
    if (status === 'Passed') return 'bg-[var(--color-success)]';
    if (status === 'Failed' || status === 'Eliminated') return 'bg-[var(--color-error)]';
    if (status === 'Running') return 'bg-[var(--color-secondary)]';
    return 'bg-[var(--color-faint)]';
  }

  function promotionMaterializationTone(promotion: EvolutionPromotion): StatusTone {
    if (promotion.materializationFailed || promotion.status === 'Failed') return 'danger';
    if (promotion.materialized || promotion.runtimeRef) return 'success';
    return 'warning';
  }

  function promotionMaterializationLabel(promotion: EvolutionPromotion): string {
    if (promotion.materializationFailed || promotion.status === 'Failed') return 'Materialization failed';
    if (promotion.materialized || promotion.runtimeRef) return 'Hot-loaded';
    return 'Hot-load pending';
  }

  function promotionMaterializationNote(promotion: EvolutionPromotion): string {
    if (promotion.materializationFailed || promotion.status === 'Failed') {
      return (
        promotion.failureReason ||
        'The winner was selected, but the canonical app publish or production install failed.'
      );
    }
    if (promotion.materialized || promotion.runtimeRef) {
      return 'The canonical app ref has been published and installed; the episode can be considered complete.';
    }
    return 'Winner selected. Promoter is publishing the canonical app ref and hot-loading it before episode completion.';
  }

  function materializationStepClass(isComplete: boolean, isFailed = false): string {
    if (isFailed) return 'border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] text-[#7a1830]';
    if (isComplete) return 'border-[var(--color-border)] bg-white text-[var(--color-ink)]';
    return 'border-[var(--color-warning)]/30 bg-[rgba(214,166,0,0.10)] text-[#735900]';
  }

</script>

<div class="mt-3 min-w-0 overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-[var(--color-surface-soft)]">
  <div class="flex flex-wrap items-start justify-between gap-2 border-b border-[var(--color-border)] bg-white px-3 py-2">
    <div class="min-w-0">
      <p class="v-eyebrow">Current Episode</p>
      <h3 class="mt-1 line-clamp-2 font-sans text-[14px] font-semibold leading-snug tracking-tight text-[var(--color-ink)]">
        {selectedDirection?.title || selectedEpisode?.id || 'No episode selected'}
      </h3>
    </div>
    {#if selectedEpisode}
      <div class="flex flex-wrap items-center gap-1.5">
        <Badge tone={statusTone(selectedEpisode.status)}>{selectedEpisode.status}</Badge>
        {#if selectedPromotion}
          <Badge tone={promotionMaterializationTone(selectedPromotion)}>
            {promotionMaterializationLabel(selectedPromotion)}
          </Badge>
        {/if}
        {#if canPause(selectedEpisode)}
          <Button
            size="xs"
            onclick={() => onPauseEpisode(selectedEpisode)}
            disabled={actionBusy === `pause-${selectedEpisode.id}`}
          >
            <Pause size={11} />
            Pause
          </Button>
        {/if}
        {#if canResume(selectedEpisode)}
          <Button
            size="xs"
            onclick={() => onResumeEpisode(selectedEpisode)}
            disabled={actionBusy === `resume-${selectedEpisode.id}`}
          >
            <Play size={11} />
            Resume
          </Button>
        {/if}
        {#if canStop(selectedEpisode)}
          <Button
            size="xs"
            onclick={() => onStopEpisode(selectedEpisode)}
            disabled={actionBusy === `stop-${selectedEpisode.id}`}
          >
            <Square size={10} />
            Stop
          </Button>
        {/if}
      </div>
    {/if}
  </div>

  {#if selectedEpisode}
    <div class="grid min-w-0 gap-3 p-3">
      <div class="grid min-w-0 gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.42fr)]">
        <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
          <p class="v-eyebrow">Adaptation Goal</p>
          <p class="mt-1 text-[13px] leading-relaxed text-[var(--color-ink)]">
            {currentGoal?.goalStatement ||
              selectedDirection?.proposedAdaptationGoal ||
              'No goal recorded yet.'}
          </p>
          {#if currentSelectionPressure}
            <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
              <span class="font-semibold text-[var(--color-ink-soft)]">Selection Pressure:</span>
              {currentSelectionPressure.selectionStatement}
            </p>
          {/if}
        </div>

        <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
          <p class="v-eyebrow">Generation State</p>
          <div class="mt-2 grid grid-cols-3 gap-2">
            <MetricTile label="Stage Results" value={stageResults.length} />
            <MetricTile
              label="Survivors"
              value={episodeVariants.filter((variant) => variant.status !== 'Eliminated' && variant.status !== 'Failed').length}
            />
            <MetricTile label="Winner" value={selectedEpisode.winningVariantId ? 1 : 0} />
          </div>
          {#if selectedPromotion}
            <div class="mt-3 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
              <div class="flex items-center justify-between gap-2">
                <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
                  Promotion
                </p>
                <Badge tone={promotionMaterializationTone(selectedPromotion)}>
                  {promotionMaterializationLabel(selectedPromotion)}
                </Badge>
              </div>
              <p class="mt-1 text-[11px] leading-snug text-[var(--color-ink-soft)]">
                {promotionMaterializationNote(selectedPromotion)}
              </p>
              <div class="mt-2 grid gap-1.5">
                <div
                  class={`grid grid-cols-[18px_minmax(0,1fr)] items-center gap-1.5 rounded-[var(--radius-xs)] border px-2 py-1.5 text-[11px] ${materializationStepClass(Boolean(selectedPromotion.winningVariantId || selectedEpisode.winningVariantId))}`}
                >
                  <CheckCircle2 size={13} class="shrink-0" />
                  <span class="min-w-0 truncate">Winner selected: {shortId(selectedPromotion.winningVariantId || selectedEpisode.winningVariantId, 18)}</span>
                </div>
                <div
                  class={`grid grid-cols-[18px_minmax(0,1fr)] items-center gap-1.5 rounded-[var(--radius-xs)] border px-2 py-1.5 text-[11px] ${materializationStepClass(Boolean(selectedPromotion.canonicalAppRef), selectedPromotion.materializationFailed && !selectedPromotion.canonicalAppRef)}`}
                >
                  {#if selectedPromotion.materializationFailed && !selectedPromotion.canonicalAppRef}
                    <AlertTriangle size={13} class="shrink-0" />
                  {:else if selectedPromotion.canonicalAppRef}
                    <PackageCheck size={13} class="shrink-0" />
                  {:else}
                    <LoaderCircle size={13} class="shrink-0 animate-spin" />
                  {/if}
                  <span class="min-w-0 truncate">
                    Canonical ref: {selectedPromotion.canonicalAppRef || 'publish pending'}
                  </span>
                </div>
                <div
                  class={`grid grid-cols-[18px_minmax(0,1fr)] items-center gap-1.5 rounded-[var(--radius-xs)] border px-2 py-1.5 text-[11px] ${materializationStepClass(Boolean(selectedPromotion.materialized || selectedPromotion.runtimeRef), selectedPromotion.materializationFailed || selectedPromotion.status === 'Failed')}`}
                >
                  {#if selectedPromotion.materializationFailed || selectedPromotion.status === 'Failed'}
                    <AlertTriangle size={13} class="shrink-0" />
                  {:else if selectedPromotion.materialized || selectedPromotion.runtimeRef}
                    <PackageCheck size={13} class="shrink-0" />
                  {:else}
                    <LoaderCircle size={13} class="shrink-0 animate-spin" />
                  {/if}
                  <span class="min-w-0 truncate">
                    Runtime: {selectedPromotion.runtimeRef || selectedPromotion.productionTenant || 'install pending'}
                  </span>
                </div>
              </div>
              <p class="mt-2 truncate text-[11px] text-[var(--color-ink-soft)]">
                {selectedPromotion.canonicalAppRef || selectedPromotion.appRef || 'canonical app pending'}
              </p>
              <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-muted)]">
                {selectedPromotion.runtimeRef || selectedPromotion.productionTenant || 'runtime pending'}
              </p>
            </div>
          {:else if selectedEpisode.status === 'Promoting'}
            <div class="mt-3 rounded-[var(--radius-xs)] border border-[var(--color-warning)]/30 bg-[rgba(214,166,0,0.10)] p-2 text-[#735900]">
              <div class="flex items-center justify-between gap-2">
                <p class="font-mono text-[10px] uppercase tracking-[0.10em]">Promotion</p>
                <Badge tone="warning">Promoter pending</Badge>
              </div>
              <p class="mt-1 text-[11px] leading-snug">
                Winner selected; the promotion row has not landed in the live read model yet.
              </p>
            </div>
          {/if}
        </div>
      </div>

      <div class="grid min-w-0 gap-3 xl:grid-cols-[minmax(280px,0.42fr)_minmax(0,1fr)]">
        <StartRequestCard {selectedEpisode} {startRequest} {shortId} {statusTone} />
        <MetricsRulesCard {metricDefinitions} {eliminationRules} {scoringRules} {shortId} />
      </div>

      <GenerationTopology
        {generations}
        variants={episodeVariants}
        {stageResults}
        {comparedVariantIds}
        {shortId}
        {statusTone}
        {onInspectVariant}
        {onToggleCompare}
      />

      <aside class="relative z-20 grid content-start gap-3">
        <PanelTitle icon={ShieldCheck} title="Viability Constraints" />
        <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
          {#if constraints.length}
            {#each constraints as constraint (constraint.id)}
              <ConstraintCard
                constraint={constraint}
                busy={actionBusy === `pin-${constraint.id}`}
                tone={statusTone(constraint.status)}
                onPin={() => onPinConstraint(constraint)}
              />
            {/each}
          {:else}
            <p class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-3 py-3 text-[12px] text-[var(--color-muted)]">
              No constraints recorded for this episode.
            </p>
          {/if}
        </div>
      </aside>

      <div class="grid gap-2 lg:hidden">
        <PanelTitle icon={GitCompareArrows} title="Evaluation Ladder" />
        {#if episodeVariants.length}
          {#each episodeVariants as variant (variant.id)}
            {@const results = stages.map((stage) => ({ stage, result: variantStageResult(variant, stage) }))}
            <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-2.5">
              <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-1.5">
                    <Badge tone={statusTone(variant.status)}>{variant.status}</Badge>
                    <span class="font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-faint)]">
                      {shortId(variant.id, 12)}
                    </span>
                  </div>
                  <p class="mt-1 line-clamp-2 text-[12px] font-semibold leading-snug text-[var(--color-ink)]">
                    {variant.summary || variant.appRef || shortId(variant.id)}
                  </p>
                </div>
                <div class="flex flex-wrap items-center gap-1.5 sm:shrink-0">
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

              <div class="mt-2 grid gap-1.5">
                {#if stages.length}
                  {#each results as item (item.stage.id)}
                    <button
                      type="button"
                      class="grid min-w-0 grid-cols-[8px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-2 text-left"
                      onclick={() => onInspectVariant(variant.id)}
                    >
                      <span class={`mt-1 h-2 w-2 rounded-[2px] ${resultColor(item.result?.status ?? 'Waiting')}`}></span>
                      <span class="min-w-0">
                        <span class="flex flex-wrap items-center gap-1.5">
                          <span class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-ink-soft)]">
                            {item.stage.stageName || item.stage.stageKind || shortId(item.stage.id)}
                          </span>
                          <Badge tone={statusTone(item.result?.status ?? 'Waiting')}>
                            {item.result?.status ?? 'Waiting'}
                          </Badge>
                        </span>
                        <span class="mt-1 line-clamp-2 block text-[11px] leading-snug text-[var(--color-muted)]">
                          {stageResultLabel(item.result)}
                        </span>
                      </span>
                    </button>
                  {/each}
                {:else}
                  <p class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-2 text-[11px] text-[var(--color-muted)]">
                    Stage ladder pending.
                  </p>
                {/if}
              </div>
            </div>
          {/each}
        {:else}
          <p class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-3 py-3 text-[12px] text-[var(--color-muted)]">
            No variants have been generated for this episode yet.
          </p>
        {/if}
      </div>

      <div class="hidden overflow-x-auto v-scrollbar lg:block">
        <div class="min-w-[760px] rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white">
          <div
            class="grid border-b border-[var(--color-border)] bg-[var(--color-surface-soft)]"
            style={`grid-template-columns: minmax(240px, 0.9fr) repeat(${Math.max(stages.length, 1)}, minmax(220px, 1fr));`}
          >
            <div class="px-3 py-2 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              Variant
            </div>
            {#if stages.length}
              {#each stages as stage (stage.id)}
                <div class="border-l border-[var(--color-border)] px-3 py-2">
                  <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
                    {stage.stageName || stage.stageKind || shortId(stage.id)}
                  </p>
                  <p class="mt-0.5 truncate text-[11px] text-[var(--color-faint)]">
                    {stage.executorKind || 'executor pending'}
                  </p>
                </div>
              {/each}
            {:else}
              <div class="border-l border-[var(--color-border)] px-3 py-2 text-[11px] text-[var(--color-muted)]">
                No evaluation stages recorded.
              </div>
            {/if}
          </div>

          {#if episodeVariants.length}
            {#each episodeVariants as variant (variant.id)}
              <div
                class="grid border-b border-[var(--color-border-soft)] last:border-b-0"
                style={`grid-template-columns: minmax(240px, 0.9fr) repeat(${Math.max(stages.length, 1)}, minmax(220px, 1fr));`}
              >
                <div class="flex min-w-0 flex-col justify-center gap-1 px-3 py-2">
                  <div class="flex items-center gap-1.5">
                    <Badge tone={statusTone(variant.status)}>{variant.status}</Badge>
                    <button
                      type="button"
                      class="truncate text-left font-sans text-[12px] font-semibold tracking-tight text-[var(--color-ink)] hover:text-[var(--color-primary)]"
                      onclick={() => onInspectVariant(variant.id)}
                    >
                      {variant.summary || shortId(variant.id)}
                    </button>
                  </div>
                  <div class="flex items-center gap-1.5">
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
                {#if stages.length}
                  {#each stages as stage (stage.id)}
                    {@const result = variantStageResult(variant, stage)}
                    <button
                      type="button"
                      class="min-w-0 border-l border-[var(--color-border-soft)] px-3 py-2 text-left transition-colors duration-[var(--duration-soft)] hover:bg-[var(--color-surface-soft)]"
                      onclick={() => onInspectVariant(variant.id)}
                    >
                      <div class="flex items-center gap-1.5">
                        <span class={`h-2 w-2 rounded-[2px] ${resultColor(result?.status ?? 'Waiting')}`}></span>
                        <span class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-ink-soft)]">
                          {result?.status ?? 'Waiting'}
                        </span>
                      </div>
                      <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-muted)]">
                        {stageResultLabel(result)}
                      </p>
                    </button>
                  {/each}
                {:else}
                  <div class="border-l border-[var(--color-border-soft)] px-3 py-2 text-[11px] text-[var(--color-muted)]">
                    Stage ladder pending.
                  </div>
                {/if}
              </div>
            {/each}
          {:else}
            <div class="px-3 py-8 text-center text-[12px] text-[var(--color-muted)]">
              No variants have been generated for this episode yet.
            </div>
          {/if}
        </div>
      </div>
    </div>
  {:else}
    <div class="px-3 py-12 text-center text-[12px] text-[var(--color-muted)]">
      No Directed Evolution episodes are present in the live API response.
    </div>
  {/if}
</div>
