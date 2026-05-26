<script lang="ts">
  import { Loader2, RefreshCw } from '@lucide/svelte';
  import { base } from '$app/paths';
  import { page } from '$app/stores';
  import { Badge, IconButton } from '$lib/components/ui';
  import BrandMark from './BrandMark.svelte';

  type TopbarProps = {
    appCount: number;
    lineageCount: number;
    closureCount: number;
    loading: boolean;
    onRefresh: () => void;
  };

  let { appCount, lineageCount, closureCount, loading, onRefresh }: TopbarProps = $props();

  const evolutionHref = `${base}/evolution`;
  const catalogHref = `${base}/`;
  let pathname = $derived($page.url.pathname);
</script>

<header
  class="sticky top-0 z-30 flex flex-wrap items-center justify-between gap-3 border-b border-[var(--color-border)] bg-white/86 px-4 py-2.5 backdrop-blur"
>
  <div class="flex items-center gap-2">
    <BrandMark size={20} />
    <h1 class="text-[20px] font-semibold leading-none tracking-[0.04em] text-[var(--color-ink)] [font-family:var(--font-mono)]">
      Genesis<span class="text-[var(--color-accent)]">.</span>
    </h1>
    <nav class="ml-1 hidden items-center gap-1 rounded-[var(--radius-sm)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-0.5 sm:flex">
      <a
        href={catalogHref}
        class={`inline-flex h-6 items-center rounded-[var(--radius-xs)] px-2 font-mono text-[10px] uppercase tracking-[0.10em] transition-colors duration-[var(--duration-soft)] ${
          pathname === catalogHref || pathname === `${catalogHref}/`
            ? 'bg-white text-[var(--color-ink)] shadow-[var(--shadow-xs)]'
            : 'text-[var(--color-muted)] hover:text-[var(--color-ink)]'
        }`}
      >
        Catalog
      </a>
      <a
        href={evolutionHref}
        class={`inline-flex h-6 items-center rounded-[var(--radius-xs)] px-2 font-mono text-[10px] uppercase tracking-[0.10em] transition-colors duration-[var(--duration-soft)] ${
          pathname === evolutionHref
            ? 'bg-white text-[var(--color-ink)] shadow-[var(--shadow-xs)]'
            : 'text-[var(--color-muted)] hover:text-[var(--color-ink)]'
        }`}
      >
        Evolution
      </a>
    </nav>
  </div>

  <div class="flex items-center gap-3 font-mono text-[10px] tracking-[0.10em] uppercase text-[var(--color-muted)]">
    <span class="hidden items-center gap-1.5 sm:inline-flex">
      <span>Apps</span>
      <span class="font-semibold text-[var(--color-ink)]">{appCount}</span>
    </span>
    <span class="hidden items-center gap-1.5 sm:inline-flex">
      <span>Lineage</span>
      <span class="font-semibold text-[var(--color-ink)]">{lineageCount}</span>
    </span>
    <span class="hidden items-center gap-1.5 sm:inline-flex">
      <span>Closures</span>
      <span class="font-semibold text-[var(--color-ink)]">{closureCount}</span>
    </span>
    <Badge tone={loading ? 'warning' : 'neutral'} pixel={!loading}>
      {loading ? 'Sync' : 'Live'}
    </Badge>
    <IconButton aria-label="Refresh registry data" disabled={loading} onclick={onRefresh}>
      {#if loading}
        <Loader2 size={13} class="animate-spin" />
      {:else}
        <RefreshCw size={13} />
      {/if}
    </IconButton>
  </div>
</header>
