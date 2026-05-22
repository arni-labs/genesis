<script lang="ts">
  import { ArrowUpRight } from '@lucide/svelte';
  import { Badge } from '$lib/components/ui';
  import type { RegistryApp } from '$lib/types';

  type AppCardProps = {
    app: RegistryApp;
    href: string;
    statusTone: (status: string) => 'success' | 'warning' | 'danger' | 'neutral';
    shortHash: (value: string, length?: number) => string;
    displayDate: (value: string) => string;
    onOwnerClick?: (ownerId: string) => void;
  };

  let { app, href, statusTone, shortHash, displayDate, onOwnerClick }: AppCardProps = $props();

  function handleOwnerClick(event: MouseEvent) {
    if (!onOwnerClick) return;
    event.preventDefault();
    event.stopPropagation();
    onOwnerClick(app.ownerId);
  }
</script>

<a
  {href}
  class="group relative flex flex-col gap-2.5 overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white px-3 py-3 transition-all duration-[var(--duration-soft)] ease-[var(--ease)] hover:-translate-y-[1px] hover:border-[var(--color-primary)]/30 hover:shadow-[var(--shadow-md)] focus-visible:outline-none"
>
  <header class="flex items-start justify-between gap-2">
    <div class="min-w-0 flex-1">
      <h3 class="v-display truncate text-[15px] tracking-tight text-[var(--color-ink)]">
        {app.name}
      </h3>
      <p class="truncate font-mono text-[10px] tracking-[0.06em] text-[var(--color-muted)]">
        <button
          type="button"
          onclick={handleOwnerClick}
          title="Filter by owner"
          class="rounded-[3px] hover:text-[var(--color-primary)] hover:underline focus-visible:outline-none"
        >
          {app.ownerId}
        </button>
        <span>/{app.repositoryId}</span>
      </p>
    </div>
    <ArrowUpRight
      size={14}
      class="mt-1 shrink-0 text-[var(--color-faint)] transition-transform duration-[var(--duration-soft)] ease-[var(--ease)] group-hover:-translate-y-[1px] group-hover:translate-x-[1px] group-hover:text-[var(--color-primary)]"
    />
  </header>

  {#if app.description}
    <p class="line-clamp-2 font-sans text-[12px] leading-snug text-[var(--color-ink-soft)]">
      {app.description}
    </p>
  {:else}
    <p class="font-sans text-[12px] italic leading-snug text-[var(--color-faint)]">
      No description recorded.
    </p>
  {/if}

  <footer class="mt-auto flex flex-wrap items-center justify-between gap-1.5 pt-1">
    <div class="flex items-center gap-1">
      <Badge tone={statusTone(app.status)} pixel={statusTone(app.status) === 'success'}>
        {app.status}
      </Badge>
      <Badge tone="neutral">{app.visibility}</Badge>
    </div>
    <div class="flex items-center gap-2 font-mono text-[10px] tracking-[0.06em] uppercase text-[var(--color-muted)]">
      <span>{shortHash(app.latestVersionHash, 8)}</span>
      <span class="text-[var(--color-faint)]">·</span>
      <span>{displayDate(app.updatedAt || app.createdAt)}</span>
    </div>
  </footer>
</a>
