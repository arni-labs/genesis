<script lang="ts">
  import { Copy, GitCommitHorizontal, PackageCheck } from '@lucide/svelte';
  import { IconButton } from '$lib/components/ui';
  import type { GitCommit, RegistryApp } from '$lib/types';

  type VersionInstallCommands = {
    appRef: string;
    odata: string;
    cli: string;
    paw: string;
  };

  type VersionsTabProps = {
    app: RegistryApp;
    versions: GitCommit[];
    selectedHash: string;
    loading: boolean;
    error: string;
    installCommands: VersionInstallCommands;
    shortHash: (value: string, length?: number) => string;
    displayDate: (value: string) => string;
    onSelect: (hash: string) => void;
    onCopy: (value: string, label: string) => void;
  };

  let {
    app,
    versions,
    selectedHash,
    loading,
    error,
    installCommands,
    shortHash,
    displayDate,
    onSelect,
    onCopy
  }: VersionsTabProps = $props();

  const selectedVersion = $derived(
    versions.find((version) => version.id === selectedHash) ?? versions[0] ?? null
  );

  function parents(commit: GitCommit): string[] {
    const value = commit.parentShas.trim();
    if (!value) return [];
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        return parsed.filter((item): item is string => typeof item === 'string' && item.length > 0);
      }
    } catch {
      // Older rows store parents as a plain comma/space separated string.
    }
    return value
      .split(/[,\s]+/)
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function subject(commit: GitCommit): string {
    return commit.message.split('\n').find(Boolean) ?? 'untitled commit';
  }

  function installCards(commands: VersionInstallCommands) {
    return [
      { title: 'Pinned ref', value: commands.appRef, label: 'Pinned app ref' },
      { title: 'CLI', value: commands.cli, label: 'Version CLI install command' },
      { title: 'TemperPaw', value: commands.paw, label: 'Version TemperPaw install call' },
      { title: 'OData', value: commands.odata, label: 'Version OData install command' }
    ];
  }
</script>

<div class="grid gap-3 px-3 pb-3 pt-3">
  <div
    class="grid grid-cols-1 gap-0 divide-y divide-[var(--color-border)] rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white sm:grid-cols-3 sm:divide-x sm:divide-y-0"
  >
    <div class="px-3 py-2">
      <p class="v-eyebrow">Latest</p>
      <p class="mt-0.5 truncate font-mono text-[12px] text-[var(--color-ink)]">
        {shortHash(app.latestVersionHash, 18)}
      </p>
    </div>
    <div class="px-3 py-2">
      <p class="v-eyebrow">Selected</p>
      <p class="mt-0.5 truncate font-mono text-[12px] text-[var(--color-ink)]">
        {shortHash(selectedVersion?.id ?? selectedHash, 18)}
      </p>
    </div>
    <div class="px-3 py-2">
      <p class="v-eyebrow">Commits</p>
      <p class="mt-0.5 font-mono text-[12px] text-[var(--color-ink)]">
        {versions.length}
      </p>
    </div>
  </div>

  <div class="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(280px,0.82fr)]">
    <div class="overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white">
      <div class="flex items-center justify-between gap-2 border-b border-[var(--color-border)] px-3 py-2">
        <div class="flex min-w-0 items-center gap-2">
          <GitCommitHorizontal size={13} class="shrink-0 text-[var(--color-primary)]" />
          <p class="truncate font-sans text-[12px] font-semibold tracking-tight text-[var(--color-ink)]">
            Version Chain
          </p>
        </div>
        <code class="truncate font-mono text-[10.5px] text-[var(--color-muted)]">
          {app.ownerId}/{app.name}
        </code>
      </div>

      {#if loading}
        <div class="px-3 py-8 text-center font-sans text-[12px] text-[var(--color-muted)]">
          Loading commits
        </div>
      {:else if error}
        <div class="m-2 rounded-[var(--radius-md)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-3 py-2 font-sans text-[12px] text-[#7a1830]">
          {error}
        </div>
      {:else if !versions.length}
        <div class="px-3 py-8 text-center font-sans text-[12px] text-[var(--color-muted)]">
          No commits projected
        </div>
      {:else}
        <div class="grid divide-y divide-[var(--color-border)]">
          {#each versions as version, index (version.id)}
            {@const isLatest = version.id === app.latestVersionHash}
            {@const isSelected = version.id === selectedHash}
            {@const parentList = parents(version)}
            <button
              type="button"
              onclick={() => onSelect(version.id)}
              class={[
                'grid gap-1 px-3 py-2 text-left transition-colors duration-150',
                isSelected ? 'bg-[var(--color-primary-soft)]' : 'hover:bg-[var(--color-surface-soft)]'
              ].join(' ')}
            >
              <span class="flex min-w-0 items-center justify-between gap-2">
                <span class="flex min-w-0 items-center gap-2">
                  <GitCommitHorizontal size={13} class="shrink-0 text-[var(--color-primary)]" />
                  <strong class="truncate font-sans text-[12.5px] font-semibold tracking-tight text-[var(--color-ink)]">
                    {subject(version)}
                  </strong>
                </span>
                {#if isLatest}
                  <span class="shrink-0 rounded-[var(--radius-sm)] bg-[var(--color-accent)] px-1.5 py-0.5 font-mono text-[9.5px] font-semibold uppercase text-[#1f2a00]">
                    Latest
                  </span>
                {:else}
                  <span class="shrink-0 font-mono text-[10px] text-[var(--color-faint)]">
                    v{versions.length - index}
                  </span>
                {/if}
              </span>
              <span class="grid gap-1 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_96px]">
                <code class="truncate font-mono text-[10.5px] text-[var(--color-muted)]">
                  {shortHash(version.id, 20)}
                </code>
                <code class="truncate font-mono text-[10.5px] text-[var(--color-muted)]">
                  parent {parentList.length ? shortHash(parentList[0], 14) : 'root'}
                </code>
                <span class="font-mono text-[10.5px] text-[var(--color-muted)]">
                  {displayDate(version.createdAt)}
                </span>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-3">
      <div class="flex items-center justify-between gap-2">
        <p class="v-eyebrow">Install Selected</p>
        <PackageCheck size={13} class="text-[var(--color-primary)]" />
      </div>
      <p class="mt-1 truncate font-sans text-[12.5px] font-semibold tracking-tight text-[var(--color-ink)]">
        {selectedVersion ? subject(selectedVersion) : app.name}
      </p>
      <div class="mt-2 grid gap-2">
        {#each installCards(installCommands) as card (card.title)}
          <div
            class="flex items-center justify-between gap-2 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] py-1 pl-2.5 pr-1"
          >
            <div class="min-w-0">
              <p class="v-eyebrow">{card.title}</p>
              <code class="block truncate font-mono text-[10.5px] text-[var(--color-ink-soft)]">
                {card.value}
              </code>
            </div>
            <IconButton
              aria-label={`Copy ${card.label}`}
              class="h-6 w-6 shrink-0"
              onclick={() => onCopy(card.value, card.label)}
            >
              <Copy size={11} />
            </IconButton>
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>
