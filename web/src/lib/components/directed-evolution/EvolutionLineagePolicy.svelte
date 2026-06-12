<script lang="ts">
  import { Activity, GitBranch, GitCommit, Rocket, Trophy } from '@lucide/svelte';
  import { Badge, Card } from '$lib/components/ui';
  import type {
    EvolutionAutonomyPolicy,
    EvolutionDirection,
    EvolutionEpisode,
    EvolutionLineageEdge,
    EvolutionMutation,
    EvolutionOrganism,
    EvolutionOrganismVersion,
    EvolutionPromotion,
    EvolutionVariant
  } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';
  import UnifiedDiff from '$lib/components/UnifiedDiff.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    organism: EvolutionOrganism | null;
    currentParentVersion: EvolutionOrganismVersion | null;
    organismVersions: EvolutionOrganismVersion[];
    lineageEdges: EvolutionLineageEdge[];
    episodes: EvolutionEpisode[];
    directions: EvolutionDirection[];
    promotions: EvolutionPromotion[];
    variants: EvolutionVariant[];
    mutations: EvolutionMutation[];
    activePolicy: EvolutionAutonomyPolicy | null;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
    jsonEntries: (value: string) => Array<[string, string]>;
  };

  let {
    organism,
    currentParentVersion,
    organismVersions,
    lineageEdges,
    episodes,
    directions,
    promotions,
    variants,
    mutations,
    activePolicy,
    shortId,
    statusTone,
    jsonEntries
  }: Props = $props();

  function scopedVersions(): EvolutionOrganismVersion[] {
    if (!organism) return organismVersions;
    return organismVersions.filter(
      (version) => !version.organismId || version.organismId === organism.id
    );
  }

  function scopedEdges(): EvolutionLineageEdge[] {
    if (!organism) return lineageEdges;
    return lineageEdges.filter((edge) => !edge.organismId || edge.organismId === organism.id);
  }

  function orderedVersions(): EvolutionOrganismVersion[] {
    const versions = scopedVersions();
    const versionById = new Map(versions.map((version) => [version.id, version]));
    const ordered: EvolutionOrganismVersion[] = [];
    const seen = new Set<string>();

    const append = (versionId: string) => {
      const version = versionById.get(versionId);
      if (!version || seen.has(version.id)) return;
      ordered.push(version);
      seen.add(version.id);
    };

    for (const edge of scopedEdges()) {
      append(edge.parentVersionId);
      append(edge.childVersionId);
    }

    for (const version of versions) {
      append(version.id);
    }

    return ordered;
  }

  function incomingEdge(versionId: string): EvolutionLineageEdge | null {
    return scopedEdges().find((edge) => edge.childVersionId === versionId) ?? null;
  }

  function edgeEpisode(edge: EvolutionLineageEdge): EvolutionEpisode | null {
    return (
      episodes.find((episode) => episode.id === edge.episodeId) ??
      episodes.find((episode) => episode.promotionId === edge.promotionId) ??
      episodes.find((episode) => episode.organismVersionId === edge.childVersionId) ??
      null
    );
  }

  function edgeDirection(edge: EvolutionLineageEdge): EvolutionDirection | null {
    const episode = edgeEpisode(edge);
    const promotion = edgePromotion(edge);
    return (
      directions.find((direction) => direction.episodeId === edge.episodeId) ??
      directions.find((direction) => direction.episodeId === episode?.id) ??
      directions.find((direction) => direction.id === episode?.directionId) ??
      directions.find((direction) => direction.episodeId === promotion?.episodeId) ??
      null
    );
  }

  function edgePromotion(edge: EvolutionLineageEdge): EvolutionPromotion | null {
    return (
      promotions.find((promotion) => promotion.id === edge.promotionId) ??
      promotions.find((promotion) => promotion.episodeId === edge.episodeId) ??
      promotions.find((promotion) => promotion.newOrganismVersionId === edge.childVersionId) ??
      null
    );
  }

  function edgeWinner(edge: EvolutionLineageEdge): EvolutionVariant | null {
    const episode = edgeEpisode(edge);
    const promotion = edgePromotion(edge);
    const winnerIds = [promotion?.winningVariantId, episode?.winningVariantId].filter(Boolean);
    return variants.find((variant) => winnerIds.includes(variant.id)) ?? null;
  }

  function winnerMutation(winner: EvolutionVariant | null): EvolutionMutation | null {
    if (!winner) return null;
    return (
      mutations.find((mutation) => mutation.variantId === winner.id) ??
      mutations.find((mutation) => mutation.id === winner.mutationId) ??
      null
    );
  }

  function versionLabel(versionId: string): string {
    const version = organismVersions.find((item) => item.id === versionId);
    return version?.summary || version?.appRef || shortId(versionId, 14);
  }

  function policyLaneTone(value: string): StatusTone {
    const normalized = value.toLowerCase();
    if (normalized.includes('auto') || normalized.includes('repair')) return 'success';
    if (normalized.includes('human') || normalized.includes('approval')) return 'warning';
    if (normalized.includes('blocked') || normalized.includes('never')) return 'danger';
    return 'neutral';
  }

  function policyLaneLabel(value: string): string {
    const normalized = value.toLowerCase();
    if (normalized.includes('auto')) return 'auto';
    if (normalized.includes('human') || normalized.includes('approval')) return 'human gate';
    if (normalized.includes('blocked') || normalized.includes('never')) return 'blocked';
    return 'declared';
  }

  function parentRefAligned(): boolean | null {
    if (!organism || !currentParentVersion) return null;
    if (!organism.appRef || !currentParentVersion.appRef) return null;
    return organism.appRef === currentParentVersion.appRef;
  }

  function currentParentAppRef(): string {
    return currentParentVersion?.appRef || organism?.appRef || '';
  }

  function isCurrentVersion(version: EvolutionOrganismVersion): boolean {
    if (!organism) return false;
    const currentId = organism.organismVersionId || organism.parentVersionId;
    return currentId === version.id;
  }

  function versionRole(version: EvolutionOrganismVersion): string {
    if (isCurrentVersion(version)) return 'current parent';
    if (incomingEdge(version.id)) return 'promoted child';
    return 'seed parent';
  }

  function versionRoleTone(version: EvolutionOrganismVersion): StatusTone {
    if (isCurrentVersion(version)) return 'success';
    if (incomingEdge(version.id)) return 'primary';
    return 'neutral';
  }

  function promotionTone(promotion: EvolutionPromotion | null): StatusTone {
    if (!promotion) return 'neutral';
    if (promotion.materializationFailed || promotion.status === 'Failed') return 'danger';
    if (promotion.materialized || promotion.runtimeRef) return 'success';
    return 'warning';
  }

  function promotionLabel(promotion: EvolutionPromotion | null): string {
    if (!promotion) return 'no promotion record';
    if (promotion.materializationFailed || promotion.status === 'Failed') return 'hot-load failed';
    if (promotion.materialized || promotion.runtimeRef) return 'hot-loaded';
    return 'promotion pending';
  }

  function versionRef(version: EvolutionOrganismVersion): string {
    return version.appRef || version.commitRef || 'version ref pending';
  }

  function edgeStory(
    edge: EvolutionLineageEdge,
    version: EvolutionOrganismVersion,
    direction: EvolutionDirection | null,
    episode: EvolutionEpisode | null,
    promotion: EvolutionPromotion | null
  ): string {
    return (
      direction?.title ||
      edge.summary ||
      episode?.summary ||
      promotion?.summary ||
      version.summary ||
      'Lineage edge recorded without direction text.'
    );
  }
</script>

<aside class="grid min-w-0 gap-3 lg:grid-cols-2">
  <Card radius="md" class="min-w-0 p-3">
    <div class="flex items-center justify-between gap-2">
      <PanelTitle icon={GitBranch} title="Organism Lineage" />
      <Badge tone={organism?.status ? statusTone(organism.status) : 'neutral'}>
        {organism?.status || 'offline'}
      </Badge>
    </div>
    <div class="mt-3 grid gap-2">
      {#if organism}
        <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-2">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              Current Parent
            </p>
            <Badge tone={currentParentVersion?.appRef ? 'success' : parentRefAligned() === false ? 'warning' : 'neutral'}>
              {currentParentVersion?.appRef ? 'Version Ref' : parentRefAligned() === false ? 'Entity Ref Differs' : 'Ref Pending'}
            </Badge>
          </div>
          <p class="mt-1 break-all text-[12px] font-semibold leading-snug tracking-tight text-[var(--color-ink)]">
            {currentParentAppRef() || 'organism app ref pending'}
          </p>
          <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-muted)]">
            version {shortId(organism.organismVersionId || organism.parentVersionId, 16)}
          </p>
          {#if currentParentVersion?.appRef && currentParentVersion.appRef !== organism.appRef}
            <p class="mt-1 truncate text-[11px] text-[var(--color-muted)]">
              Organism entity still reports {organism.appRef || 'no app ref'}
            </p>
          {/if}
          {#if organism.summary}
            <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-muted)]">
              {organism.summary}
            </p>
          {/if}
        </div>
      {/if}

      {#if lineageEdges.length}
        <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-2">
          <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
            Evolution Edges
          </p>
          <div class="mt-2 grid gap-1.5">
            {#each lineageEdges.slice(-4) as edge (edge.id)}
              <div class="grid min-w-0 grid-cols-[minmax(0,1fr)_18px_minmax(0,1fr)] items-center gap-1.5 text-[11px]">
                <span class="truncate rounded-[var(--radius-xs)] bg-white px-2 py-1 text-[var(--color-ink-soft)]">
                  {versionLabel(edge.parentVersionId)}
                </span>
                <span class="text-center font-mono text-[var(--color-primary)]">&gt;</span>
                <span class="truncate rounded-[var(--radius-xs)] bg-white px-2 py-1 text-[var(--color-ink-soft)]">
                  {versionLabel(edge.childVersionId)}
                </span>
              </div>
              {#if edge.summary}
                <p class="truncate text-[10.5px] text-[var(--color-muted)]">{edge.summary}</p>
              {/if}
            {/each}
          </div>
        </div>
      {/if}
      {#if scopedVersions().length}
        <div class="min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-2">
          <div class="flex flex-wrap items-center justify-between gap-2">
            <PanelTitle icon={GitCommit} title="Specimen History" />
            <span class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              {orderedVersions().length} version{orderedVersions().length === 1 ? '' : 's'}
            </span>
          </div>

          <div class="mt-3 grid gap-2" aria-label="Organism version genealogy">
            {#each orderedVersions() as version, index (version.id)}
              {@const edge = incomingEdge(version.id)}
              {@const direction = edge ? edgeDirection(edge) : null}
              {@const episode = edge ? edgeEpisode(edge) : null}
              {@const promotion = edge ? edgePromotion(edge) : null}
              {@const winner = edge ? edgeWinner(edge) : null}
              <div class="relative min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border-soft)] bg-white p-2">
                {#if index > 0}
                  <div class="absolute -top-2 left-4 h-2 border-l border-[var(--color-border)]" aria-hidden="true"></div>
                {/if}
                <div class="flex flex-wrap items-start justify-between gap-2">
                  <div class="min-w-0 flex-1">
                    <div class="flex flex-wrap items-center gap-1.5">
                      <Badge tone={versionRoleTone(version)}>{versionRole(version)}</Badge>
                      <Badge tone={statusTone(version.status)}>{version.status}</Badge>
                      {#if promotion}
                        <Badge tone={promotionTone(promotion)}>
                          <Rocket size={10} />
                          {promotionLabel(promotion)}
                        </Badge>
                      {/if}
                    </div>
                    <p class="mt-1.5 line-clamp-2 text-[12.5px] font-semibold leading-snug tracking-tight text-[var(--color-ink)]">
                      {version.summary || versionRef(version)}
                    </p>
                    <p class="mt-1 break-all font-mono text-[10px] leading-snug text-[var(--color-muted)]">
                      {versionRef(version)}
                    </p>
                  </div>
                  <span class="shrink-0 rounded-[var(--radius-xs)] bg-[var(--color-surface-soft)] px-2 py-1 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-faint)]">
                    v{index + 1}
                  </span>
                </div>

                {#if edge}
                  {@const mutation = winnerMutation(winner)}
                  {@const promotedDiffPatch = edge.diffPatch || mutation?.diffPatch || ''}
                  <div class="mt-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5">
                    <div class="flex flex-wrap items-center gap-1.5">
                      <Badge tone="primary">promoted change</Badge>
                      {#if direction}
                        <Badge tone={statusTone(direction.status)}>{direction.pressureClass || direction.status}</Badge>
                      {/if}
                      {#if winner}
                        <Badge tone={statusTone(winner.status)}>
                          <Trophy size={10} />
                          winner variant
                        </Badge>
                      {/if}
                    </div>
                    <p class="mt-1 line-clamp-2 text-[11.5px] leading-snug text-[var(--color-ink-soft)]">
                      {edgeStory(edge, version, direction, episode, promotion)}
                    </p>
                    {#if winner}
                      <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-muted)]">
                        Winner: {winner.summary || winner.appRef || shortId(winner.id, 14)}
                      </p>
                    {/if}
                    {#if promotion?.runtimeRef}
                      <p class="mt-1 break-all font-mono text-[10px] leading-snug text-[var(--color-muted)]">
                        Runtime: {promotion.runtimeRef}
                      </p>
                    {/if}
                    {#if promotedDiffPatch}
                      <div class="mt-2">
                        <UnifiedDiff patch={promotedDiffPatch} maxFiles={5} maxLinesPerFile={22} />
                      </div>
                    {/if}
                    <details class="mt-1.5">
                      <summary class="cursor-pointer font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
                        Evidence ids
                      </summary>
                      <div class="mt-1 grid gap-1 font-mono text-[10px] text-[var(--color-muted)]">
                        <span class="truncate">parent {shortId(edge.parentVersionId, 18)}</span>
                        <span class="truncate">child {shortId(edge.childVersionId, 18)}</span>
                        <span class="truncate">episode {shortId(edge.episodeId, 18)}</span>
                        <span class="truncate">promotion {shortId(edge.promotionId, 18)}</span>
                      </div>
                    </details>
                  </div>
                {:else}
                  <p class="mt-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5 text-[11px] leading-snug text-[var(--color-muted)]">
                    Seed version. No incoming lineage edge is recorded for this parent.
                  </p>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <p class="text-[12px] text-[var(--color-muted)]">No organism versions recorded yet.</p>
      {/if}
    </div>
  </Card>

  <Card radius="md" class="min-w-0 p-3">
    <PanelTitle icon={Activity} title="Autonomy Policy" />
    {#if activePolicy}
      <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-ink)]">
        {activePolicy.summary || 'Active policy'}
      </p>
      <div class="mt-2 grid gap-1.5 sm:grid-cols-3">
        {#each jsonEntries(activePolicy.policyJson).slice(0, 6) as [key, value] (key)}
          <div class="min-w-0 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5 text-[11px]">
            <div class="flex items-center justify-between gap-2">
              <span class="truncate font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">
                {key}
              </span>
              <Badge tone={policyLaneTone(`${key} ${value}`)}>
                {policyLaneLabel(`${key} ${value}`)}
              </Badge>
            </div>
            <p class="mt-1 line-clamp-3 text-[var(--color-ink-soft)]">{value}</p>
          </div>
        {/each}
      </div>
      <details class="mt-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white px-2 py-1.5">
        <summary class="cursor-pointer font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
          Policy Payload
        </summary>
        <div class="mt-2 grid gap-1.5">
          {#each jsonEntries(activePolicy.policyJson).slice(0, 6) as [key, value] (key)}
            <div class="grid grid-cols-[94px_minmax(0,1fr)] gap-2 rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5 text-[11px]">
              <span class="truncate font-mono uppercase tracking-[0.08em] text-[var(--color-muted)]">
                {key}
              </span>
              <span class="min-w-0 truncate text-[var(--color-ink-soft)]">{value}</span>
            </div>
          {/each}
        </div>
      </details>
    {:else}
      <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
        No active policy is recorded, so the UI cannot show what repair or growth pressure is allowed
        to proceed automatically.
      </p>
    {/if}
  </Card>
</aside>
