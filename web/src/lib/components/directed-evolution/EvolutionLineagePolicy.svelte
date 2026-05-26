<script lang="ts">
  import { Activity, GitBranch } from '@lucide/svelte';
  import { Badge, Card } from '$lib/components/ui';
  import type {
    EvolutionAutonomyPolicy,
    EvolutionOrganism,
    EvolutionOrganismVersion
  } from '$lib/directedEvolution';
  import PanelTitle from './PanelTitle.svelte';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type Props = {
    organism: EvolutionOrganism | null;
    organismVersions: EvolutionOrganismVersion[];
    activePolicy: EvolutionAutonomyPolicy | null;
    shortId: (value: string, length?: number) => string;
    statusTone: (status: string) => StatusTone;
    jsonEntries: (value: string) => Array<[string, string]>;
  };

  let { organism, organismVersions, activePolicy, shortId, statusTone, jsonEntries }: Props =
    $props();
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
    {:else}
      <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-muted)]">
        No active policy is recorded, so the UI cannot show what repair or growth pressure is allowed
        to proceed automatically.
      </p>
    {/if}
  </Card>
</aside>
