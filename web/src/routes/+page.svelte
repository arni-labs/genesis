<script lang="ts">
  import { onMount } from 'svelte';
  import { base } from '$app/paths';
  import { loadRegistry, registryStore } from '$lib/registry';
  import Topbar from '$lib/components/Topbar.svelte';
  import AppCatalog from '$lib/components/AppCatalog.svelte';
  import type { RegistryApp } from '$lib/types';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral';

  $: state = $registryStore;
  $: apps = state.snapshot?.apps ?? [];
  $: owners = state.snapshot?.owners ?? [];
  $: lineages = state.snapshot?.lineages ?? [];
  $: closures = state.snapshot?.closures ?? [];
  $: browsable = apps.filter((app) => app.status !== 'Deleted' && isInstallable(app));

  onMount(() => {
    void loadRegistry();
  });

  function refresh() {
    void loadRegistry(true);
  }

  function statusTone(status: string): StatusTone {
    const normalized = status.toLowerCase();
    if (normalized.includes('verified') || normalized === 'active' || normalized === 'durable') {
      return 'success';
    }
    if (normalized.includes('pending') || normalized.includes('deprecated')) {
      return 'warning';
    }
    if (normalized.includes('suspend') || normalized.includes('delete')) {
      return 'danger';
    }
    return 'neutral';
  }

  function isInstallable(app: RegistryApp): boolean {
    return Boolean(app.ownerId && app.name && app.repositoryId && app.latestVersionHash);
  }

  function shortHash(value: string, length = 12): string {
    if (!value) return 'pending';
    return value.length > length ? `${value.slice(0, length)}...` : value;
  }

  function displayDate(value: string): string {
    if (!value) return '—';
    const date = new Date(value);
    if (Number.isNaN(date.valueOf())) return value;
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric'
    }).format(date);
  }
</script>

<svelte:head>
  <title>Genesis Registry</title>
  <meta
    name="description"
    content="Browse Genesis apps, repository files, lineage, dependency closures, and pinned install commands."
  />
</svelte:head>

<main class="relative z-[1] min-h-screen">
  <Topbar
    appCount={browsable.length}
    lineageCount={lineages.length}
    closureCount={closures.length}
    loading={state.loading}
    onRefresh={refresh}
  />

  <AppCatalog
    {apps}
    {owners}
    loading={state.loading}
    error={state.error}
    {base}
    {statusTone}
    {shortHash}
    {displayDate}
    isInstallable={isInstallable}
  />
</main>
