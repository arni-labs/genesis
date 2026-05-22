<script lang="ts" module>
  import type { Snippet } from 'svelte';

  export type TabItem = {
    value: string;
    label: string;
    icon?: Snippet;
  };
</script>

<script lang="ts">
  import { Tabs } from 'bits-ui';
  import { cn } from '$lib/utils';

  type TabsProps = {
    value: string;
    items: TabItem[];
    class?: string;
    onchange?: (value: string) => void;
    children?: Snippet;
  };

  let { value = $bindable(), items, class: className, onchange, children }: TabsProps = $props();

  function handleValueChange(next: string | undefined) {
    if (!next) return;
    value = next;
    onchange?.(next);
  }
</script>

<Tabs.Root value={value} onValueChange={handleValueChange} class={cn('flex flex-col', className)}>
  <Tabs.List
    class="flex flex-wrap items-center gap-1 border-b border-[var(--color-border-soft)] bg-white px-3 py-1.5"
  >
    {#each items as item (item.value)}
      <Tabs.Trigger
        value={item.value}
        class={cn(
          'inline-flex items-center gap-1.5 rounded-[var(--radius-sm)] border border-transparent px-2.5 py-1 font-sans text-[12px] tracking-tight text-[var(--color-muted)] transition-colors duration-[var(--duration-soft)] ease-[var(--ease)]',
          'hover:bg-[var(--color-primary-soft)] hover:text-[var(--color-ink)]',
          'data-[state=active]:border-[var(--color-primary)]/22 data-[state=active]:bg-[var(--color-primary-soft)] data-[state=active]:text-[var(--color-primary)]'
        )}
      >
        {#if item.icon}
          <span class="flex items-center text-current">
            {@render item.icon?.()}
          </span>
        {/if}
        <span class="font-semibold">{item.label}</span>
      </Tabs.Trigger>
    {/each}
  </Tabs.List>

  {@render children?.()}
</Tabs.Root>
