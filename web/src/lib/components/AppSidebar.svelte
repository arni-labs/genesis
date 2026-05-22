<script lang="ts">
  import { AlertCircle, PackageCheck, Search } from '@lucide/svelte';
  import { Badge, Card, Input, Select } from '$lib/components/ui';
  import type { SelectOption } from '$lib/components/ui';
  import type { RegistryApp } from '$lib/types';

  type AppSidebarProps = {
    apps: RegistryApp[];
    search: string;
    statusFilter: string;
    selectedAppId: string;
    loading: boolean;
    loadError: string;
    onSelect: (app: RegistryApp) => void;
    statusTone: (status: string) => 'success' | 'warning' | 'danger' | 'neutral';
    initial: (app: RegistryApp) => string;
  };

  let {
    apps,
    search = $bindable(),
    statusFilter = $bindable(),
    selectedAppId,
    loading,
    loadError,
    onSelect,
    statusTone,
    initial
  }: AppSidebarProps = $props();

  const statusOptions: SelectOption[] = [
    { value: 'all', label: 'All' },
    { value: 'Active', label: 'Active' },
    { value: 'Deprecated', label: 'Deprecated' },
    { value: 'Deleted', label: 'Deleted' }
  ];
</script>

<Card radius="md" class="flex h-full min-h-[calc(100vh-90px)] flex-col">
  <header class="flex items-center justify-between gap-2 border-b border-[var(--color-border-soft)] px-3 py-2">
    <div class="flex items-baseline gap-1.5">
      <span class="v-eyebrow">Apps</span>
      <span class="font-mono text-[10px] text-[var(--color-faint)]">·</span>
      <span class="font-mono text-[10.5px] tracking-[0.10em] uppercase text-[var(--color-faint)]">
        {apps.length}
      </span>
    </div>
    <span
      class="grid h-5 w-5 place-items-center rounded-[4px] border border-[var(--color-border)] bg-[var(--color-surface-soft)] text-[var(--color-primary)]"
    >
      <PackageCheck size={11} />
    </span>
  </header>

  <div class="grid grid-cols-[1fr_auto] gap-1.5 border-b border-[var(--color-border-soft)] px-3 py-2">
    <Input bind:value={search} placeholder="Search" aria-label="Search apps">
      {#snippet leadingIcon()}
        <Search size={12} />
      {/snippet}
    </Input>
    <Select bind:value={statusFilter} options={statusOptions} aria-label="Status filter" />
  </div>

  {#if loadError}
    <div class="px-3 py-2.5">
      <div
        class="flex items-start gap-1.5 rounded-[5px] border border-[var(--color-error)]/30 bg-[rgba(239,68,68,0.06)] px-2.5 py-1.5 text-[11.5px] text-[#991b1b]"
      >
        <AlertCircle size={13} class="mt-[1px] shrink-0" />
        <span class="leading-snug">{loadError}</span>
      </div>
    </div>
  {:else if apps.length}
    <ul class="v-scrollbar flex-1 overflow-y-auto py-1">
      {#each apps as app (app.id)}
        {@const isSelected = selectedAppId === app.id}
        <li>
          <button
            type="button"
            onclick={() => onSelect(app)}
            class={[
              'group relative grid w-full grid-cols-[24px_minmax(0,1fr)_auto] items-center gap-2 border-l-2 px-3 py-1.5 text-left transition-colors duration-[var(--duration-soft)]',
              isSelected
                ? 'border-l-[var(--color-primary)] bg-[var(--color-primary-soft)]'
                : 'border-l-transparent hover:bg-[var(--color-surface-soft)]'
            ].join(' ')}
          >
            <span
              class={[
                'grid h-6 w-6 place-items-center rounded-[4px] font-mono text-[11px] font-semibold',
                isSelected
                  ? 'bg-[var(--color-primary)] text-white'
                  : 'border border-[var(--color-border)] bg-white text-[var(--color-primary-strong)]'
              ].join(' ')}
            >
              {initial(app)}
            </span>
            <span class="min-w-0">
              <span class="block truncate font-sans text-[12.5px] font-medium tracking-tight text-[var(--color-ink)]">
                {app.name}
              </span>
              <span class="block truncate font-mono text-[10px] tracking-[0.04em] text-[var(--color-muted)]">
                {app.ownerId}/{app.repositoryId}
              </span>
            </span>
            <span class="flex flex-col items-end gap-[2px]">
              <Badge tone={statusTone(app.status)}>{app.status}</Badge>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    <div class="flex flex-1 flex-col items-center justify-center gap-1.5 px-4 py-6 text-center">
      <PackageCheck size={20} class="text-[var(--color-faint)]" />
      <h3 class="font-sans text-[12.5px] font-medium tracking-tight text-[var(--color-ink)]">No apps</h3>
      <p class="font-sans text-[11px] text-[var(--color-muted)]">
        {loading ? 'Reading rows…' : 'No App rows matched the filter.'}
      </p>
    </div>
  {/if}
</Card>
