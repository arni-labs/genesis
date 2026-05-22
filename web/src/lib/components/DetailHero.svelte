<script lang="ts">
  import { Copy, GitFork } from '@lucide/svelte';
  import { Badge, Button } from '$lib/components/ui';
  import type { Lineage, RegistryApp } from '$lib/types';

  type DetailHeroProps = {
    app: RegistryApp;
    lineage: Lineage | null;
    statusTone: (status: string) => 'success' | 'warning' | 'danger' | 'neutral';
    onCopyClone: () => void;
  };

  let { app, lineage, statusTone, onCopyClone }: DetailHeroProps = $props();
</script>

<section
  class="flex flex-wrap items-start justify-between gap-3 border-b border-[var(--color-border)] bg-white px-3 py-3 sm:gap-4 sm:px-4"
>
  <div class="min-w-0 flex-1">
    <div class="flex flex-wrap items-center gap-1.5">
      <Badge tone={statusTone(app.status)} pixel={statusTone(app.status) === 'success'}>
        {app.status}
      </Badge>
      <Badge tone="neutral">{app.visibility}</Badge>
      {#if lineage}
        <Badge tone="primary">
          <GitFork size={9} />
          {lineage.type}
        </Badge>
      {/if}
      <span class="truncate font-mono text-[10px] tracking-[0.10em] uppercase text-[var(--color-muted)]">
        {app.ownerId}/{app.repositoryId}
      </span>
    </div>
    <h2 class="v-display mt-1.5 break-words text-[20px] tracking-tight text-[var(--color-ink)]">
      {app.name}
    </h2>
    {#if app.description}
      <p class="mt-0.5 max-w-[72ch] font-sans text-[12.5px] leading-relaxed text-[var(--color-muted)]">
        {app.description}
      </p>
    {/if}
  </div>

  <Button variant="outline" size="sm" onclick={onCopyClone} class="shrink-0">
    <Copy size={12} />
    Copy clone
  </Button>
</section>
