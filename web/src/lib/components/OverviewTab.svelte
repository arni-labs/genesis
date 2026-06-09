<script lang="ts">
  import { Boxes, Clipboard, GitBranch, PackageCheck } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type { AppFilesSnapshot, AppInstallation, Closure, RegistryApp } from '$lib/types';

  type OverviewTabProps = {
    app: RegistryApp;
    snapshot: AppFilesSnapshot | null;
    installations: AppInstallation[];
    ownerLabel: (id: string) => string;
    exportsList: string[];
    closures: Closure[];
    closureEntries: (closure: Closure) => Array<[string, string]>;
    shortHash: (value: string, length?: number) => string;
    displayDate: (value: string) => string;
  };

  let {
    app,
    snapshot,
    installations,
    ownerLabel,
    exportsList,
    closures,
    closureEntries,
    shortHash,
    displayDate
  }: OverviewTabProps = $props();

  const repoHeadHash = $derived(snapshot?.repoHeadHash ?? '');
  const repoHeadRef = $derived(snapshot?.repoHeadRef ?? 'refs/heads/main');
  const promotionPending = $derived(Boolean(repoHeadHash && repoHeadHash !== app.latestVersionHash));
  const latestInstallation = $derived(installations[0] ?? null);
  const installedHash = $derived(latestInstallation?.versionHash ?? '');
  const rolloutState = $derived(
    latestInstallation
      ? `${latestInstallation.status} · ${latestInstallation.followPolicy || 'pinned'}`
      : 'no installs'
  );
  const metrics = $derived([
    { label: 'Owner', value: ownerLabel(app.ownerId) },
    { label: 'Latest hash', value: shortHash(app.latestVersionHash) },
    { label: 'Repo head', value: repoHeadHash ? shortHash(repoHeadHash) : 'not projected' },
    { label: 'Promotion', value: promotionPending ? 'pending' : 'current' },
    { label: 'Installed', value: installedHash ? shortHash(installedHash) : 'none' },
    { label: 'Rollout', value: rolloutState }
  ]);
</script>

<div class="grid gap-3 px-3 pb-3 pt-3">
  <div
    class="grid grid-cols-2 divide-y divide-[var(--color-border)] rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white sm:grid-cols-3 lg:grid-cols-6 lg:divide-x lg:divide-y-0"
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

  {#if promotionPending}
    <div class="flex items-start gap-2 rounded-[var(--radius-md)] border border-[var(--color-warning)]/40 bg-[rgba(235,180,32,0.12)] px-3 py-2 font-sans text-[12px] text-[#604000]">
      <GitBranch size={13} class="mt-[1px] shrink-0" />
      <span>
        Repo head <code class="font-mono">{repoHeadRef}@{shortHash(repoHeadHash, 16)}</code> is newer than Genesis latest <code class="font-mono">{shortHash(app.latestVersionHash, 16)}</code>. Run <code class="font-mono">Temper.Git.PublishNewVersion</code> to promote it.
      </span>
    </div>
  {/if}

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
        <dt class="text-[var(--color-muted)]">Repository</dt>
        <dd class="break-words font-mono text-[11px] text-[var(--color-ink-soft)]">{app.repositoryId}</dd>
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
        <p class="v-eyebrow">Runtime provenance</p>
        <PackageCheck size={12} class="text-[var(--color-primary)]" />
      </div>
      {#if latestInstallation}
        <dl class="grid grid-cols-[96px_minmax(0,1fr)] gap-y-1.5 gap-x-3 font-sans text-[12px]">
          <dt class="text-[var(--color-muted)]">Tenant</dt>
          <dd class="text-[var(--color-ink)]">{latestInstallation.targetTenant || 'default'}</dd>
          <dt class="text-[var(--color-muted)]">Hash</dt>
          <dd class="break-words font-mono text-[11px] text-[var(--color-ink-soft)]">{latestInstallation.versionHash}</dd>
          <dt class="text-[var(--color-muted)]">Policy</dt>
          <dd class="text-[var(--color-ink)]">{latestInstallation.followPolicy || 'pinned'}</dd>
          <dt class="text-[var(--color-muted)]">Status</dt>
          <dd class="text-[var(--color-ink)]">{latestInstallation.status}</dd>
          <dt class="text-[var(--color-muted)]">Installed</dt>
          <dd class="text-[var(--color-ink)]">{displayDate(latestInstallation.installedAt || latestInstallation.createdAt)}</dd>
        </dl>
      {:else}
        <p class="font-sans text-[11.5px] text-[var(--color-muted)]">
          No AppInstallation rows matched this app.
        </p>
      {/if}
    </div>
  </div>

  <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)]">
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
