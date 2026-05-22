<script lang="ts" module>
  export type SelectOption = {
    value: string;
    label: string;
  };
</script>

<script lang="ts">
  import { Select } from 'bits-ui';
  import { ChevronDown, Check } from '@lucide/svelte';
  import { cn } from '$lib/utils';

  type SelectProps = {
    value: string;
    options: SelectOption[];
    placeholder?: string;
    'aria-label'?: string;
    class?: string;
    onchange?: (value: string) => void;
  };

  let {
    value = $bindable(),
    options,
    placeholder,
    'aria-label': ariaLabel,
    class: className,
    onchange
  }: SelectProps = $props();

  function handleValueChange(next: string) {
    value = next;
    onchange?.(next);
  }

  const selectedLabel = $derived(
    options.find((option) => option.value === value)?.label ?? placeholder ?? ''
  );
</script>

<Select.Root type="single" {value} onValueChange={handleValueChange}>
  <Select.Trigger
    aria-label={ariaLabel}
    class={cn(
      'inline-flex h-7 min-w-[120px] items-center justify-between gap-1.5 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-3 font-sans text-[12px] tracking-tight text-[var(--color-ink)] transition-colors duration-[var(--duration-soft)] ease-[var(--ease)] hover:border-[var(--color-primary)]/40 hover:bg-[var(--color-primary-soft)] focus-visible:outline-none',
      className
    )}
  >
    <span class="truncate">{selectedLabel}</span>
    <ChevronDown size={12} class="text-[var(--color-faint)]" />
  </Select.Trigger>
  <Select.Portal>
    <Select.Content
      sideOffset={4}
      class="z-50 min-w-[160px] overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-border)] bg-white p-1 shadow-[var(--shadow-md)]"
    >
      {#each options as option (option.value)}
        <Select.Item
          value={option.value}
          label={option.label}
          class="flex cursor-default items-center justify-between gap-2 rounded-[var(--radius-sm)] px-2 py-1 font-sans text-[12px] tracking-tight text-[var(--color-ink-soft)] outline-none data-[highlighted]:bg-[var(--color-primary-soft)] data-[highlighted]:text-[var(--color-primary)]"
        >
          <span>{option.label}</span>
          {#if option.value === value}
            <Check size={12} class="text-[var(--color-primary)]" />
          {/if}
        </Select.Item>
      {/each}
    </Select.Content>
  </Select.Portal>
</Select.Root>
