<script lang="ts">
  import { AlertCircle, File, Files as FilesIcon, Folder, ListTree, Loader2 } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import CodePreview from './CodePreview.svelte';
  import type { AppFilesSnapshot, RegistryApp, RepositoryFile } from '$lib/types';

  type FilesTabProps = {
    app: RegistryApp;
    snapshot: AppFilesSnapshot | null;
    loading: boolean;
    error: string;
    fileEntries: RepositoryFile[];
    visibleEntries: RepositoryFile[];
    fileCount: number;
    directoryCount: number;
    repositorySize: number;
    selectedFile: RepositoryFile | null;
    breadcrumbs: Array<{ label: string; path: string }>;
    onCrumb: (path: string) => void;
    onSelect: (entry: RepositoryFile) => void;
    shortHash: (value: string, length?: number) => string;
    formatBytes: (value: number) => string;
    fileKindLabel: (entry: RepositoryFile) => string;
  };

  let {
    app,
    snapshot,
    loading,
    error,
    fileEntries,
    visibleEntries,
    fileCount,
    directoryCount,
    repositorySize,
    selectedFile,
    breadcrumbs,
    onCrumb,
    onSelect,
    shortHash,
    formatBytes,
    fileKindLabel
  }: FilesTabProps = $props();
</script>

<div class="grid gap-3 px-3 pb-3 pt-3">
  <div
    class="grid grid-cols-1 gap-0 divide-y divide-[var(--color-border)] rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white sm:grid-cols-3 sm:divide-x sm:divide-y-0"
  >
    <div class="px-3 py-2">
      <p class="v-eyebrow">Commit</p>
      <p class="mt-0.5 truncate font-mono text-[12px] text-[var(--color-ink)]">
        {shortHash(snapshot?.commit?.id ?? app.latestVersionHash, 18)}
      </p>
    </div>
    <div class="px-3 py-2">
      <p class="v-eyebrow">Contents</p>
      <p class="mt-0.5 font-sans text-[12px] tracking-tight text-[var(--color-ink)]">
        <span class="font-medium">{fileCount}</span> files ·
        <span class="font-medium">{directoryCount}</span> folders
      </p>
    </div>
    <div class="px-3 py-2">
      <p class="v-eyebrow">Size</p>
      <p class="mt-0.5 font-mono text-[12px] tracking-tight text-[var(--color-ink)]">
        {formatBytes(repositorySize)}
      </p>
    </div>
  </div>

  <div class="grid gap-3 lg:grid-cols-[minmax(0,1.1fr)_minmax(280px,0.9fr)]">
    <div class="overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white">
      <div
        class="flex flex-wrap items-center gap-0.5 border-b border-[var(--color-border)] px-3 py-1.5 text-[11px] text-[var(--color-muted)]"
      >
        {#each breadcrumbs as crumb, index (crumb.path)}
          {#if index > 0}
            <span class="px-1 text-[var(--color-faint)]">/</span>
          {/if}
          <button
            type="button"
            onclick={() => onCrumb(crumb.path)}
            class="rounded-[var(--radius-sm)] px-1 py-0.5 font-mono text-[11px] tracking-[0.04em] text-[var(--color-primary)] transition-colors duration-150 hover:bg-[var(--color-primary-soft)]"
          >
            {crumb.label}
          </button>
        {/each}
      </div>

      {#if loading}
        <div class="flex flex-col items-center gap-1.5 px-4 py-8 text-center">
          <Loader2 size={18} class="animate-spin text-[var(--color-primary)]" />
          <h3 class="font-sans text-[12px] font-medium tracking-tight text-[var(--color-ink)]">Loading files</h3>
          <p class="font-sans text-[11px] text-[var(--color-muted)]">Reading commit, tree, and blob rows.</p>
        </div>
      {:else if error}
        <div
          class="m-2 flex items-start gap-1.5 rounded-[var(--radius-md)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-2.5 py-1.5 text-[11.5px] text-[#7a1830]"
        >
          <AlertCircle size={12} class="mt-[1px] shrink-0" />
          <span>{error}</span>
        </div>
      {:else if !fileEntries.length}
        <div class="flex flex-col items-center gap-1.5 px-4 py-8 text-center">
          <FilesIcon size={20} class="text-[var(--color-faint)]" />
          <h3 class="font-sans text-[12px] font-medium tracking-tight text-[var(--color-ink)]">No files projected</h3>
        </div>
      {:else}
        <div class="grid divide-y divide-[var(--color-border)]">
          {#each visibleEntries as entry (entry.path)}
            {@const isSelected = selectedFile?.path === entry.path}
            <button
              type="button"
              onclick={() => onSelect(entry)}
              class={[
                'grid grid-cols-[minmax(0,1fr)_60px] items-center gap-2 px-3 py-1.5 text-left text-[11.5px] transition-colors duration-150',
                'sm:grid-cols-[minmax(140px,1fr)_70px_92px_60px]',
                isSelected
                  ? 'bg-[var(--color-primary-soft)] text-[var(--color-ink)]'
                  : 'hover:bg-[var(--color-surface-soft)]'
              ].join(' ')}
            >
              <span class="flex min-w-0 items-center gap-2">
                {#if entry.kind === 'directory'}
                  <Folder size={13} class="shrink-0 text-[var(--color-primary)]" />
                {:else}
                  <File size={13} class="shrink-0 text-[var(--color-muted)]" />
                {/if}
                <strong class="truncate font-sans text-[12px] font-medium tracking-tight text-[var(--color-ink)]">
                  {entry.name}
                </strong>
              </span>
              <span class="hidden font-mono text-[10px] tracking-[0.04em] uppercase text-[var(--color-muted)] sm:block">
                {fileKindLabel(entry)}
              </span>
              <code class="hidden truncate font-mono text-[10.5px] tracking-[0.04em] text-[var(--color-muted)] sm:block">
                {shortHash(entry.objectSha, 8)}
              </code>
              <span class="text-right font-mono text-[10px] tracking-[0.04em] uppercase text-[var(--color-muted)]">
                {entry.kind === 'directory' ? '' : formatBytes(entry.size)}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="flex min-h-[420px] flex-col overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white">
      {#if selectedFile}
        <div class="flex items-center justify-between gap-2 border-b border-[var(--color-border)] px-3 py-1.5">
          <div class="min-w-0">
            <p class="v-eyebrow">{selectedFile.mode}</p>
            <h3 class="truncate font-mono text-[12px] font-medium tracking-tight text-[var(--color-ink)]">
              {selectedFile.path}
            </h3>
          </div>
          <Badge tone="neutral">{formatBytes(selectedFile.size)}</Badge>
        </div>
        {#if selectedFile.isBinary}
          <div class="flex flex-1 flex-col items-center justify-center gap-1.5 px-4 py-8 text-center">
            <File size={20} class="text-[var(--color-faint)]" />
            <h3 class="font-sans text-[12px] font-medium tracking-tight text-[var(--color-ink)]">Binary file</h3>
          </div>
        {:else}
          <div class="min-h-0 flex-1">
            <CodePreview code={selectedFile.preview || 'Empty file'} path={selectedFile.path} />
          </div>
        {/if}
      {:else}
        <div class="flex flex-1 flex-col items-center justify-center gap-1.5 px-4 py-8 text-center">
          <ListTree size={20} class="text-[var(--color-faint)]" />
          <h3 class="font-sans text-[12px] font-medium tracking-tight text-[var(--color-ink)]">Select a file</h3>
        </div>
      {/if}
    </div>
  </div>
</div>
