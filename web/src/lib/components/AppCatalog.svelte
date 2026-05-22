<script lang="ts">
  import { AlertCircle, Boxes, Search, X } from '@lucide/svelte';
  import { Card, Input, Select } from '$lib/components/ui';
  import type { SelectOption } from '$lib/components/ui';
  import AppCard from './AppCard.svelte';
  import type { Owner, RegistryApp } from '$lib/types';

  type AppCatalogProps = {
    apps: RegistryApp[];
    owners: Owner[];
    loading: boolean;
    error: string;
    base: string;
    statusTone: (status: string) => 'success' | 'warning' | 'danger' | 'neutral';
    shortHash: (value: string, length?: number) => string;
    displayDate: (value: string) => string;
    isInstallable: (app: RegistryApp) => boolean;
  };

  let {
    apps,
    owners,
    loading,
    error,
    base,
    statusTone,
    shortHash,
    displayDate,
    isInstallable
  }: AppCatalogProps = $props();

  let search = $state('');
  let statusFilter = $state('all');
  let sortMode = $state<'recent' | 'name' | 'owner'>('recent');

  const installable = $derived(apps.filter(isInstallable));

  const statusOptions: SelectOption[] = [
    { value: 'all', label: 'All statuses' },
    { value: 'Active', label: 'Active' },
    { value: 'Deprecated', label: 'Deprecated' },
    { value: 'Deleted', label: 'Deleted' }
  ];

  const sortOptions: SelectOption[] = [
    { value: 'recent', label: 'Recently updated' },
    { value: 'name', label: 'Name (A–Z)' },
    { value: 'owner', label: 'Owner' }
  ];

  const filtered = $derived.by(() => {
    const query = search.trim().toLowerCase();
    return installable
      .filter((app) => {
        if (statusFilter === 'all' ? app.status === 'Deleted' : app.status !== statusFilter) {
          return false;
        }
        if (!query) {
          return true;
        }
        return (
          app.name.toLowerCase().includes(query) ||
          app.ownerId.toLowerCase().includes(query) ||
          app.repositoryId.toLowerCase().includes(query) ||
          (app.description ?? '').toLowerCase().includes(query) ||
          app.latestVersionHash.toLowerCase().includes(query)
        );
      })
      .sort((a, b) => {
        if (sortMode === 'name') {
          return a.name.localeCompare(b.name);
        }
        if (sortMode === 'owner') {
          return `${a.ownerId}/${a.name}`.localeCompare(`${b.ownerId}/${b.name}`);
        }
        const ad = new Date(a.updatedAt || a.createdAt || 0).valueOf();
        const bd = new Date(b.updatedAt || b.createdAt || 0).valueOf();
        return bd - ad;
      });
  });

  const totalCount = $derived(installable.length);
  const visibleCount = $derived(filtered.length);

  function appHref(app: RegistryApp): string {
    return `${base}/app/${encodeURIComponent(app.id)}`;
  }

  function clearFilters() {
    search = '';
    statusFilter = 'all';
  }

  function focusOwner(ownerId: string) {
    search = ownerId;
  }
</script>

<section class="grid gap-3 px-3 py-3 lg:px-4 xl:px-5">
  <Card radius="md" class="px-3 py-2.5">
    <div class="flex items-center justify-between gap-2">
      <p class="v-eyebrow">Catalog · {visibleCount} / {totalCount}</p>
    </div>

    <div class="mt-2 grid grid-cols-2 gap-1.5 sm:grid-cols-[minmax(0,1fr)_auto_auto]">
      <Input
        bind:value={search}
        placeholder="Search apps, owners, hashes"
        aria-label="Search"
        class="col-span-2 sm:col-auto"
      >
        {#snippet leadingIcon()}
          <Search size={12} />
        {/snippet}
      </Input>
      <Select
        bind:value={statusFilter}
        options={statusOptions}
        aria-label="Status filter"
        class="min-w-0"
      />
      <Select
        bind:value={sortMode}
        options={sortOptions}
        aria-label="Sort"
        class="min-w-0 sm:min-w-[170px]"
      />
    </div>

    {#if search || statusFilter !== 'all'}
      <div class="mt-2 flex items-center gap-1.5">
        {#if search}
          <span class="inline-flex h-6 items-center gap-1.5 rounded-[var(--radius-sm)] border border-[var(--color-primary)]/30 bg-[var(--color-primary-soft)] px-2 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-primary)]">
            <span class="v-pixel-square" aria-hidden="true"></span>
            <span>“{search}”</span>
          </span>
        {/if}
        {#if statusFilter !== 'all'}
          <span class="inline-flex h-6 items-center gap-1.5 rounded-[var(--radius-sm)] border border-[var(--color-primary)]/30 bg-[var(--color-primary-soft)] px-2 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-primary)]">
            <span>Status</span>
            <span class="font-semibold">{statusFilter}</span>
          </span>
        {/if}
        <button
          type="button"
          onclick={clearFilters}
          class="ml-auto inline-flex h-6 items-center gap-1 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-2 font-sans text-[11px] font-semibold tracking-tight text-[var(--color-ink-soft)] transition-colors duration-[var(--duration-soft)] hover:bg-[var(--color-surface-soft)]"
        >
          <X size={11} />
          Reset
        </button>
      </div>
    {/if}
  </Card>

  {#if error}
    <Card radius="md" class="px-3 py-3">
      <div class="flex items-start gap-2 text-[12px] text-[#7a1830]">
        <AlertCircle size={14} class="mt-[2px] shrink-0" />
        <span>{error}</span>
      </div>
    </Card>
  {:else if loading && totalCount === 0}
    <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each Array(8) as _, i (i)}
        <div class="h-28 animate-pulse rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white"></div>
      {/each}
    </div>
  {:else if visibleCount === 0}
    <Card radius="md" class="px-4 py-10 text-center">
      <Boxes size={20} class="mx-auto text-[var(--color-faint)]" />
      <p class="mt-1.5 font-sans text-[13px] font-semibold tracking-tight text-[var(--color-ink)]">
        No apps match your filters
      </p>
      <p class="mt-0.5 font-sans text-[12px] text-[var(--color-muted)]">
        Try clearing the search or adjusting the status.
      </p>
    </Card>
  {:else}
    <div class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {#each filtered as app (app.id)}
        <AppCard
          {app}
          href={appHref(app)}
          {statusTone}
          {shortHash}
          {displayDate}
          onOwnerClick={focusOwner}
        />
      {/each}
    </div>
  {/if}

</section>
