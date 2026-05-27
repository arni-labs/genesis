<script lang="ts">
  import { Activity, GitBranch } from '@lucide/svelte';
  import { Badge, Card } from '$lib/components/ui';
  import type {
    EvolutionAutonomyPolicy,
    EvolutionLineageEdge,
    EvolutionOrganism,
    EvolutionOrganismVersion
  } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    organism: EvolutionOrganism | null;
    currentParentVersion: EvolutionOrganismVersion | null;
    organismVersions: EvolutionOrganismVersion[];
    lineageEdges: EvolutionLineageEdge[];
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
    activePolicy,
    shortId,
    statusTone,
    jsonEntries
  }: Props = $props();

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
</script>

<aside class="grid gap-3 lg:grid-cols-2">
  <Card radius="md" class="p-3">
    <div class="flex items-center justify-between gap-2">
      <PanelTitle icon={GitBranch} title="Organism Lineage" />
      <Badge tone={organism?.status ? statusTone(organism.status) : 'neutral'}>
        {organism?.status || 'offline'}
      </Badge>
    </div>
    <div class="mt-3 grid gap-2">
      {#if organism}
        <div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-2">
          <div class="flex items-center justify-between gap-2">
            <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              Current Parent
            </p>
            <Badge tone={parentRefAligned() === false ? 'danger' : parentRefAligned() ? 'success' : 'neutral'}>
              {parentRefAligned() === false ? 'Ref Mismatch' : parentRefAligned() ? 'Ref Aligned' : 'Ref Pending'}
            </Badge>
          </div>
          <p class="mt-1 truncate text-[12px] font-semibold tracking-tight text-[var(--color-ink)]">
            {organism.appRef || 'organism app ref pending'}
          </p>
          <p class="mt-1 truncate font-mono text-[10px] text-[var(--color-muted)]">
            version {shortId(organism.organismVersionId || organism.parentVersionId, 16)}
          </p>
          {#if currentParentVersion?.appRef && currentParentVersion.appRef !== organism.appRef}
            <p class="mt-1 truncate text-[11px] text-[var(--color-error)]">
              Parent version reports {currentParentVersion.appRef}
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
        <div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-2">
          <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
            Evolution Edges
          </p>
          <div class="mt-2 grid gap-1.5">
            {#each lineageEdges.slice(-4) as edge (edge.id)}
              <div class="grid grid-cols-[minmax(0,1fr)_18px_minmax(0,1fr)] items-center gap-1.5 text-[11px]">
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
      {#if organismVersions.length}
        {#each organismVersions as version (version.id)}
          <div class="relative rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-2">
            <div class="flex items-center justify-between gap-2">
              <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
                {shortId(version.id, 12)}
              </p>
              <Badge tone={statusTone(version.status)}>{version.status}</Badge>
            </div>
            <p class="mt-1 truncate text-[12px] font-semibold tracking-tight text-[var(--color-ink)]">
              {version.appRef || version.commitRef || 'version ref pending'}
            </p>
            {#if version.summary}
              <p class="mt-1 line-clamp-2 text-[11px] leading-snug text-[var(--color-muted)]">
                {version.summary}
              </p>
            {/if}
          </div>
        {/each}
      {:else}
        <p class="text-[12px] text-[var(--color-muted)]">No organism versions recorded yet.</p>
      {/if}
    </div>
  </Card>

  <Card radius="md" class="p-3">
    <PanelTitle icon={Activity} title="Autonomy Policy" />
    {#if activePolicy}
      <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-ink)]">
        {activePolicy.summary || 'Active policy'}
      </p>
      <div class="mt-2 grid gap-1.5 sm:grid-cols-3">
        {#each jsonEntries(activePolicy.policyJson).slice(0, 6) as [key, value] (key)}
          <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5 text-[11px]">
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
