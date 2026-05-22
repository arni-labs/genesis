<script lang="ts">
  import { GitFork } from '@lucide/svelte';
  import type { Lineage, RegistryApp } from '$lib/types';

  type LineageTabProps = {
    app: RegistryApp;
    lineage: Lineage | null;
    parentApp: RegistryApp | null;
    childApps: RegistryApp[];
    mutationList: string[];
    shortHash: (value: string, length?: number) => string;
  };

  let { app, lineage, parentApp, childApps, mutationList, shortHash }: LineageTabProps = $props();

  const visibleChildren = $derived(childApps.slice(0, 2));
</script>

<div class="grid gap-3 px-3 pb-3 pt-3">
  <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-3">
    <svg viewBox="0 0 760 220" role="img" aria-label="Lineage graph" class="h-56 w-full">
      <defs>
        <marker id="lin-arrow" markerWidth="6" markerHeight="6" refX="5" refY="2.5" orient="auto">
          <path d="M0,0 L0,5 L6,2.5 z" fill="#3D4FE3" />
        </marker>
      </defs>

      {#if parentApp}
        <line
          x1="206"
          y1="110"
          x2="318"
          y2="110"
          stroke="#3D4FE3"
          stroke-width="1.2"
          stroke-dasharray="3 4"
          marker-end="url(#lin-arrow)"
        />
      {/if}
      {#each visibleChildren as _child, index}
        <line
          x1="460"
          y1="110"
          x2="552"
          y2={index === 0 ? 70 : 150}
          stroke="#3D4FE3"
          stroke-width="1.2"
          stroke-dasharray="3 4"
          marker-end="url(#lin-arrow)"
        />
      {/each}

      {#if parentApp}
        <g transform="translate(34 80)">
          <rect width="172" height="60" rx="10" fill="#ffffff" stroke="#D8E1EA" />
          <rect x="12" y="14" width="4" height="4" fill="#16BCE8" />
          <text x="22" y="22" font-family="Familjen Grotesk, sans-serif" font-size="12" font-weight="600" fill="#111827">
            {parentApp.name}
          </text>
          <text x="12" y="42" font-family="Kode Mono, monospace" font-size="10" fill="#657080">
            {shortHash(lineage?.parentCommit ?? parentApp.latestVersionHash, 18)}
          </text>
        </g>
      {/if}

      <g transform="translate(318 70)">
        <rect width="142" height="80" rx="10" fill="#ffffff" stroke="#3D4FE3" stroke-width="1.2" />
        <rect x="12" y="14" width="4" height="4" fill="#B7FF1A" />
        <text x="22" y="22" font-family="Familjen Grotesk, sans-serif" font-size="12" font-weight="700" fill="#111827">
          {app.name}
        </text>
        <text x="12" y="42" font-family="Kode Mono, monospace" font-size="10" fill="#657080">
          {app.ownerId}
        </text>
        <text x="12" y="58" font-family="Kode Mono, monospace" font-size="10" fill="#657080">
          {shortHash(app.latestVersionHash, 18)}
        </text>
      </g>

      {#each visibleChildren as child, index (child.id)}
        <g transform={`translate(552 ${index === 0 ? 40 : 120})`}>
          <rect width="172" height="60" rx="10" fill="#ffffff" stroke="#D8E1EA" />
          <rect x="12" y="14" width="4" height="4" fill="#B7FF1A" />
          <text x="22" y="22" font-family="Familjen Grotesk, sans-serif" font-size="12" font-weight="600" fill="#111827">
            {child.name}
          </text>
          <text x="12" y="42" font-family="Kode Mono, monospace" font-size="10" fill="#657080">
            {child.ownerId}
          </text>
        </g>
      {/each}

      {#if !parentApp && childApps.length === 0}
        <text x="304" y="194" font-family="Kode Mono, monospace" font-size="10" fill="#94a3b8">
          No fork links recorded
        </text>
      {/if}
    </svg>
  </div>

  <div class="grid gap-2 sm:grid-cols-2">
    <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-2">
      <p class="v-eyebrow">Parent</p>
      {#if parentApp}
        <p class="mt-0.5 font-sans text-[12.5px] font-semibold tracking-tight text-[var(--color-ink)]">
          {parentApp.ownerId}/{parentApp.name}
        </p>
        <code class="mt-0.5 block break-words font-mono text-[11px] text-[var(--color-muted)]">{parentApp.repositoryId}</code>
      {:else}
        <p class="mt-0.5 font-sans text-[12.5px] tracking-tight text-[var(--color-ink-soft)]">no parent</p>
        <code class="mt-0.5 block break-words font-mono text-[11px] text-[var(--color-muted)]">{lineage?.parentRepositoryId || 'root app'}</code>
      {/if}
    </div>
    <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-2">
      <p class="v-eyebrow">Selected</p>
      <p class="mt-0.5 font-sans text-[12.5px] font-semibold tracking-tight text-[var(--color-ink)]">
        {app.ownerId}/{app.name}
      </p>
      <code class="mt-0.5 block break-words font-mono text-[11px] text-[var(--color-muted)]">{app.repositoryId}</code>
    </div>
  </div>

  {#if mutationList.length}
    <ul class="grid gap-1">
      {#each mutationList as mutation, index (`${mutation}-${index}`)}
        <li class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-1.5 font-sans text-[12px] text-[var(--color-ink-soft)]">
          {mutation}
        </li>
      {/each}
    </ul>
  {:else}
    <div class="flex items-center gap-2 rounded-[var(--radius-md)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] px-3 py-1.5 text-[11.5px] text-[var(--color-muted)]">
      <GitFork size={12} class="shrink-0 text-[var(--color-primary)]" />
      <span>No mutation records on this lineage row.</span>
    </div>
  {/if}
</div>
