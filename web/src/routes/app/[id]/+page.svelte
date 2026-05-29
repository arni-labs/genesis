<script lang="ts">
  import { onMount } from 'svelte';
  import { Tabs as BitsTabs } from 'bits-ui';
  import { ArrowLeft, AlertCircle, PackageCheck } from '@lucide/svelte';
  import { base } from '$app/paths';
  import { page } from '$app/stores';
  import { Card, Tabs, Toast, type TabItem } from '$lib/components/ui';
  import Topbar from '$lib/components/Topbar.svelte';
  import DetailHero from '$lib/components/DetailHero.svelte';
  import FilesTab from '$lib/components/FilesTab.svelte';
  import VersionsTab from '$lib/components/VersionsTab.svelte';
  import OverviewTab from '$lib/components/OverviewTab.svelte';
  import LineageTab from '$lib/components/LineageTab.svelte';
  import InstallTab from '$lib/components/InstallTab.svelte';
  import {
    loadRegistry,
    loadAppFilesCached,
    findAppById,
    registryStore
  } from '$lib/registry';
  import { parseJsonList, parseJsonMap } from '$lib/api';
  import type {
    AppFilesSnapshot,
    Closure,
    Lineage,
    Owner,
    RegistryApp,
    RepositoryFile
  } from '$lib/types';

  type WorkbenchTab = 'files' | 'versions' | 'overview' | 'lineage' | 'install';
  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral';

  const configuredApiBase = (import.meta.env.VITE_TEMPER_API_BASE ?? '').replace(/\/$/, '');

  let activeTab: WorkbenchTab = 'files';
  let toast = '';
  let toastTimer: number | undefined;

  let fileSnapshot: AppFilesSnapshot | null = null;
  let filesLoading = false;
  let filesError = '';
  let filesLoadKey = '';
  let currentPath = '';
  let selectedFilePath = '';
  let selectedVersionHash = '';

  const tabItems: TabItem[] = [
    { value: 'files', label: 'Files' },
    { value: 'versions', label: 'Versions' },
    { value: 'overview', label: 'Overview' },
    { value: 'lineage', label: 'Lineage' },
    { value: 'install', label: 'Install' }
  ];

  $: state = $registryStore;
  $: apps = (state.snapshot?.apps ?? []) as RegistryApp[];
  $: owners = (state.snapshot?.owners ?? []) as Owner[];
  $: lineages = (state.snapshot?.lineages ?? []) as Lineage[];
  $: closures = (state.snapshot?.closures ?? []) as Closure[];

  $: appId = decodeURIComponent($page.params.id ?? '');
  $: selectedApp = findAppById(state.snapshot, appId);
  $: ownersById = new Map(owners.map((owner) => [owner.id || owner.accountId, owner]));

  $: selectedLineage = selectedApp
    ? lineages.find((lineage) => lineage.childRepositoryId === selectedApp.repositoryId) ?? null
    : null;
  $: parentApp = selectedLineage
    ? apps.find((app) => app.repositoryId === selectedLineage?.parentRepositoryId) ?? null
    : null;
  $: childLineages = selectedApp
    ? lineages.filter((lineage) => lineage.parentRepositoryId === selectedApp.repositoryId)
    : [];
  $: childApps = childLineages
    .map((lineage) => apps.find((app) => app.repositoryId === lineage.childRepositoryId))
    .filter((app): app is RegistryApp => Boolean(app));
  $: selectedClosures = selectedApp
    ? closures.filter(
        (closure) =>
          closure.root === selectedApp.id ||
          closure.root === selectedApp.latestVersionHash ||
          closure.root === selectedApp.repositoryId
      )
    : [];
  $: exportsList = selectedApp ? parseJsonList(selectedApp.exports) : [];
  $: mutationList = selectedLineage ? parseJsonList(selectedLineage.mutations) : [];
  $: fileEntries = fileSnapshot?.files ?? [];
  $: versionEntries = fileSnapshot?.versions ?? [];
  $: visibleEntries = entriesForPath(fileEntries, currentPath);
  $: selectedFile =
    fileEntries.find((entry) => entry.path === selectedFilePath && entry.kind !== 'directory') ??
    null;
  $: fileCount = fileEntries.filter((entry) => entry.kind !== 'directory').length;
  $: directoryCount = fileEntries.filter((entry) => entry.kind === 'directory').length;
  $: repositorySize = fileEntries.reduce((total, entry) => total + entry.size, 0);
  $: currentBreadcrumbs = breadcrumbs(currentPath);
  $: warnings = (state.snapshot?.warnings ?? []).map(
    (warning) => `${warning.collection}: ${warning.message}`
  );

  $: if (selectedApp) {
    const key = `${selectedApp.id}:${selectedApp.repositoryId}:${selectedApp.latestVersionHash}`;
    if (key !== filesLoadKey) {
      void loadFilesFor(selectedApp, key);
    }
  }

  $: if (!selectedApp && filesLoadKey) {
    filesLoadKey = '';
    fileSnapshot = null;
    selectedFilePath = '';
    currentPath = '';
    selectedVersionHash = '';
  }

  onMount(() => {
    void loadRegistry();
  });

  async function loadFilesFor(app: RegistryApp, key: string) {
    filesLoadKey = key;
    filesLoading = true;
    filesError = '';
    fileSnapshot = null;
    currentPath = '';
    selectedFilePath = '';

    try {
      const snapshot = await loadAppFilesCached(app);
      if (filesLoadKey !== key) return;
      fileSnapshot = snapshot;
      selectedVersionHash = snapshot.commitHash;
      currentPath = initialBrowserPath(snapshot.files);
      selectedFilePath =
        entriesForPath(snapshot.files, currentPath).find((entry) => entry.kind !== 'directory')
          ?.path ?? '';
    } catch (error) {
      if (filesLoadKey === key) {
        filesError = error instanceof Error ? error.message : String(error);
      }
    } finally {
      if (filesLoadKey === key) {
        filesLoading = false;
      }
    }
  }

  function refresh() {
    void loadRegistry(true);
  }

  function selectEntry(entry: RepositoryFile) {
    if (entry.kind === 'directory') {
      currentPath = entry.path;
      selectedFilePath = '';
      return;
    }
    selectedFilePath = entry.path;
  }

  async function copyText(value: string, label: string) {
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
      showToast(`${label} copied`);
    } catch (error) {
      showToast(error instanceof Error ? error.message : 'Copy failed');
    }
  }

  function showToast(message: string) {
    toast = message;
    if (toastTimer !== undefined) {
      window.clearTimeout(toastTimer);
    }
    toastTimer = window.setTimeout(() => {
      toast = '';
    }, 2400);
  }

  function ownerLabel(ownerId: string): string {
    const owner = ownersById.get(ownerId);
    return owner?.displayName || owner?.accountId || ownerId || 'unowned';
  }

  function shortHash(value: string, length = 12): string {
    if (!value) return 'pending';
    return value.length > length ? `${value.slice(0, length)}...` : value;
  }

  function displayDate(value: string): string {
    if (!value) return 'not recorded';
    const date = new Date(value);
    if (Number.isNaN(date.valueOf())) return value;
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    }).format(date);
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

  function appInitial(app: RegistryApp): string {
    return (app.name || app.id || 'G').slice(0, 1).toUpperCase();
  }

  function formatBytes(value: number): string {
    if (!value) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = value;
    let unit = 0;
    while (size >= 1024 && unit < units.length - 1) {
      size /= 1024;
      unit += 1;
    }
    return `${size.toFixed(size >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
  }

  function installHash(app: RegistryApp, hash = ''): string {
    return hash || app.latestVersionHash || app.id;
  }

  function appRef(app: RegistryApp, hash = ''): string {
    return `${app.ownerId}/${app.name}@${installHash(app, hash)}`;
  }

  function escapedODataId(value: string): string {
    return value.replace(/'/g, "''");
  }

  function registryApiBase(): string {
    return configuredApiBase || (typeof location !== 'undefined' ? location.origin : '');
  }

  function odataInstallCommand(app: RegistryApp, hash = ''): string {
    const ref = appRef(app, hash);
    const body = JSON.stringify({
      TargetTenant: 'default',
      AppRef: ref,
      Installer: 'manual'
    });
    return `curl -sS -X POST "${registryApiBase()}/tdata/Apps('${escapedODataId(app.id)}')/App.Install" -H "Content-Type: application/json" -H "X-Tenant-Id: default" -d '${body}'`;
  }

  function cliInstallCommand(app: RegistryApp, hash = ''): string {
    return `temper install ${appRef(app, hash)} --tenant default --url ${registryApiBase()}`;
  }

  function temperPawInstallCommand(app: RegistryApp, hash = ''): string {
    return `temper.install_app({"app_ref":"${appRef(app, hash)}","tenant":"default","registry_url":"${registryApiBase()}"})`;
  }

  function cloneCommand(app: RegistryApp): string {
    return `git clone ${registryApiBase()}/${app.ownerId}/${app.name}.git`;
  }

  function closureEntries(closure: Closure): Array<[string, string]> {
    return parseJsonMap(closure.resolved);
  }

  function entriesForPath(entries: RepositoryFile[], path: string): RepositoryFile[] {
    return entries
      .filter((entry) => entry.parentPath === path)
      .sort((a, b) => {
        if (a.kind === 'directory' && b.kind !== 'directory') return -1;
        if (a.kind !== 'directory' && b.kind === 'directory') return 1;
        return a.name.localeCompare(b.name);
      });
  }

  function initialBrowserPath(entries: RepositoryFile[]): string {
    const rootEntries = entriesForPath(entries, '');
    const rootFiles = rootEntries.filter((entry) => entry.kind !== 'directory');
    const rootDirectories = rootEntries.filter((entry) => entry.kind === 'directory');
    if (rootFiles.length === 0 && rootDirectories.length === 1) {
      return rootDirectories[0].path;
    }
    return '';
  }

  function breadcrumbs(path: string): Array<{ label: string; path: string }> {
    const parts = path.split('/').filter(Boolean);
    const crumbs = [{ label: 'root', path: '' }];
    let cursor = '';
    for (const part of parts) {
      cursor = cursor ? `${cursor}/${part}` : part;
      crumbs.push({ label: part, path: cursor });
    }
    return crumbs;
  }

  function fileKindLabel(entry: RepositoryFile): string {
    if (entry.kind === 'directory') return 'Directory';
    if (entry.kind === 'symlink') return 'Symlink';
    if (entry.kind === 'submodule') return 'Submodule';
    return 'File';
  }
</script>

<svelte:head>
  <title>{selectedApp?.name ? `${selectedApp.name} · Genesis` : 'Genesis Registry'}</title>
  <meta
    name="description"
    content={selectedApp?.description ||
      'Inspect Genesis app files, versions, lineage, dependency closures, and pinned install commands.'}
  />
</svelte:head>

<main class="relative z-[1] min-h-screen">
  <Topbar
    appCount={apps.filter((app) => app.status !== 'Deleted').length}
    lineageCount={lineages.length}
    closureCount={closures.length}
    loading={state.loading}
    onRefresh={refresh}
  />

  <div class="flex items-center gap-2 overflow-x-auto border-b border-[var(--color-border)] bg-white px-3 py-1.5 sm:px-4">
    <a
      href={`${base}/`}
      class="inline-flex h-7 shrink-0 items-center gap-1.5 rounded-[var(--radius-sm)] border border-transparent px-2 font-sans text-[12px] font-semibold tracking-tight text-[var(--color-ink-soft)] transition-colors duration-[var(--duration-soft)] hover:border-[var(--color-border)] hover:bg-white hover:text-[var(--color-ink)]"
    >
      <ArrowLeft size={13} />
      Catalog
    </a>
    <span class="shrink-0 font-mono text-[10px] tracking-[0.10em] uppercase text-[var(--color-faint)]">
      /
    </span>
    {#if selectedApp}
      <span class="truncate font-mono text-[10px] tracking-[0.10em] uppercase text-[var(--color-muted)]">
        {selectedApp.ownerId}/{selectedApp.name}
      </span>
    {/if}
  </div>

  <div class="grid gap-3 px-3 py-3 lg:px-4 xl:px-5">
    {#if state.loading && !selectedApp}
      <div class="flex flex-col items-center gap-1.5 px-4 py-16 text-center">
        <PackageCheck size={20} class="animate-pulse text-[var(--color-faint)]" />
        <p class="font-sans text-[12.5px] font-medium tracking-tight text-[var(--color-ink)]">
          Loading registry…
        </p>
      </div>
    {:else if !selectedApp}
      <Card radius="md" class="px-4 py-12 text-center">
        <AlertCircle size={20} class="mx-auto text-[var(--color-faint)]" />
        <p class="mt-1.5 font-sans text-[13px] font-semibold tracking-tight text-[var(--color-ink)]">
          App not found
        </p>
        <p class="mt-0.5 font-sans text-[12px] text-[var(--color-muted)]">
          The id <code class="font-mono">{appId}</code> is not in the registry.
        </p>
        <a
          href={`${base}/`}
          class="mt-3 inline-flex h-7 items-center gap-1.5 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-2.5 font-sans text-[12px] font-semibold tracking-tight text-[var(--color-ink)] hover:border-[var(--color-primary)]/40 hover:bg-[var(--color-primary-soft)]"
        >
          <ArrowLeft size={12} />
          Back to catalog
        </a>
      </Card>
    {:else}
      <Card radius="md" class="overflow-hidden">
        <DetailHero
          app={selectedApp}
          lineage={selectedLineage}
          {statusTone}
          onCopyClone={() => copyText(cloneCommand(selectedApp), 'Clone command')}
        />

        <Tabs items={tabItems} bind:value={activeTab} class="w-full">
          <BitsTabs.Content value="files">
            <FilesTab
              app={selectedApp}
              snapshot={fileSnapshot}
              loading={filesLoading}
              error={filesError}
              {fileEntries}
              {visibleEntries}
              {fileCount}
              {directoryCount}
              {repositorySize}
              {selectedFile}
              breadcrumbs={currentBreadcrumbs}
              onCrumb={(path) => (currentPath = path)}
              onSelect={selectEntry}
              {shortHash}
              {formatBytes}
              {fileKindLabel}
            />
          </BitsTabs.Content>
          <BitsTabs.Content value="versions">
            <VersionsTab
              app={selectedApp}
              versions={versionEntries}
              diffs={fileSnapshot?.diffs ?? []}
              selectedHash={selectedVersionHash || selectedApp.latestVersionHash}
              loading={filesLoading}
              error={filesError}
              installCommands={{
                appRef: appRef(selectedApp, selectedVersionHash),
                odata: odataInstallCommand(selectedApp, selectedVersionHash),
                cli: cliInstallCommand(selectedApp, selectedVersionHash),
                paw: temperPawInstallCommand(selectedApp, selectedVersionHash)
              }}
              {shortHash}
              {displayDate}
              onSelect={(hash) => (selectedVersionHash = hash)}
              onCopy={copyText}
            />
          </BitsTabs.Content>
          <BitsTabs.Content value="overview">
            <OverviewTab
              app={selectedApp}
              {ownerLabel}
              {exportsList}
              closures={selectedClosures}
              {closureEntries}
              {shortHash}
              {displayDate}
            />
          </BitsTabs.Content>
          <BitsTabs.Content value="lineage">
            <LineageTab
              app={selectedApp}
              lineage={selectedLineage}
              {parentApp}
              {childApps}
              {mutationList}
              {shortHash}
            />
          </BitsTabs.Content>
          <BitsTabs.Content value="install">
            <InstallTab
              app={selectedApp}
              odata={odataInstallCommand(selectedApp)}
              cli={cliInstallCommand(selectedApp)}
              paw={temperPawInstallCommand(selectedApp)}
              clone={cloneCommand(selectedApp)}
              {warnings}
              onCopy={copyText}
            />
          </BitsTabs.Content>
        </Tabs>
      </Card>
    {/if}
  </div>

  <Toast message={toast} />
</main>
