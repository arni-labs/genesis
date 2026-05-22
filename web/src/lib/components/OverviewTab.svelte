<script lang="ts">
  import { Boxes, Clipboard } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type { Closure, RegistryApp } from '$lib/types';

  type OverviewTabProps = {
    app: RegistryApp;
    ownerLabel: (id: string) => string;
    exportsList: string[];
    closures: Closure[];
    closureEntries: (closure: Closure) => Array<[string, string]>;
    shortHash: (value: string, length?: number) => string;
    displayDate: (value: string) => string;
  };

  let {
    app,
    ownerLabel,
    exportsList,
    closures,
    closureEntries,
    shortHash,
    displayDate
  }: OverviewTabProps = $props();

  const metrics = $derived([
    { label: 'Owner', value: ownerLabel(app.ownerId) },
    { label: 'Repository', value: app.repositoryId },
    { label: 'Latest hash', value: shortHash(app.latestVersionHash) },
    { label: 'Updated', value: displayDate(app.updatedAt || app.createdAt) }
  ]);
</script>

<div class="grid gap-3 px-3 pb-3 pt-3">
  <div
    class="grid grid-cols-2 divide-y divide-[var(--color-border)] rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white sm:grid-cols-4 sm:divide-x sm:divide-y-0"
  >
    {#each metrics as metric (metric.label)}
      <div class="px-3 py-2 min-w-0">
        <p class="v-eyebrow">{metric.label}</p>
        <p class="mt-0.5 truncate font-sans text-[12.5px] font-medium tracking-tight text-[var(--color-ink)]">
          {metric.value}
        </p>
      </div>
    {/each}
  </div>

  <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(260px,0.85fr)]">
    <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-3">
      <div class="mb-2 flex items-center justify-between">
        <p class="v-eyebrow">App detail</p>
        <Clipboard size={12} class="text-[var(--color-primary)]" />
      </div>
      <dl class="grid grid-cols-[110px_minmax(0,1fr)] gap-y-1.5 gap-x-3 font-sans text-[12px]">
        <dt class="text-[var(--color-muted)]">App ID</dt>
        <dd class="break-words font-mono text-[11px] text-[var(--color-ink-soft)]">{app.id}</dd>
        <dt class="text-[var(--color-muted)]">Owner</dt>
        <dd class="text-[var(--color-ink)]">{app.ownerId}</dd>
        <dt class="text-[var(--color-muted)]">Exports</dt>
        <dd class="text-[var(--color-ink)]">
          {exportsList.length ? `${exportsList.length} entries` : 'none recorded'}
        </dd>
        <dt class="text-[var(--color-muted)]">Created</dt>
        <dd class="text-[var(--color-ink)]">{displayDate(app.createdAt)}</dd>
      </dl>
      {#if exportsList.length}
        <div class="mt-2 flex flex-wrap gap-1">
          {#each exportsList as item}
            <Badge tone="primary">{item}</Badge>
          {/each}
        </div>
      {/if}
    </div>

    <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-3">
      <div class="mb-2 flex items-center justify-between">
        <p class="v-eyebrow">Closures</p>
        <Boxes size={12} class="text-[var(--color-primary)]" />
      </div>
      {#if closures.length}
        <ul class="grid gap-1.5">
          {#each closures as closure (closure.id)}
            <li class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] px-2.5 py-1.5">
              <p class="truncate font-mono text-[11px] font-semibold tracking-tight text-[var(--color-ink)]">
                {closure.id}
              </p>
              <p class="mt-0.5 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
                {closure.resolverVersion} · {displayDate(closure.resolvedAt)}
              </p>
              <div class="mt-1 grid gap-0.5">
                {#each closureEntries(closure).slice(0, 3) as [name, hash]}
                  <code class="break-words rounded-[var(--radius-sm)] bg-white px-1.5 py-0.5 font-mono text-[10px] text-[var(--color-ink-soft)]">
                    {name}: {shortHash(hash, 16)}
                  </code>
                {/each}
              </div>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="font-sans text-[11.5px] text-[var(--color-muted)]">
          No closure rows matched the selected app.
        </p>
      {/if}
    </div>
  </div>
</div>
