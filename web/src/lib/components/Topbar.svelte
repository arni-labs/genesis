<script lang="ts">
  import { Loader2, RefreshCw } from '@lucide/svelte';
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
</script>

<header
  class="sticky top-0 z-30 flex flex-wrap items-center justify-between gap-3 border-b border-[var(--color-border)] bg-white/86 px-4 py-2.5 backdrop-blur"
>
  <div class="flex items-center gap-2">
    <BrandMark size={20} />
    <h1 class="text-[20px] font-semibold leading-none tracking-[0.04em] text-[var(--color-ink)] [font-family:var(--font-mono)]">
      Genesis<span class="text-[var(--color-accent)]">.</span>
    </h1>
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
