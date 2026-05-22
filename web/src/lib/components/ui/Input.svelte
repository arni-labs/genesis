<script lang="ts">
  import { cn } from '$lib/utils';

  type InputProps = {
    value?: string;
    placeholder?: string;
    type?: 'text' | 'search' | 'email' | 'url';
    class?: string;
    'aria-label'?: string;
    leadingIcon?: import('svelte').Snippet;
  };

  let {
    value = $bindable(''),
    placeholder,
    type = 'text',
    class: className,
    'aria-label': ariaLabel,
    leadingIcon
  }: InputProps = $props();
</script>

<label class={cn('relative block w-full', className)}>
  {#if leadingIcon}
    <span
      class="pointer-events-none absolute inset-y-0 left-2.5 flex items-center text-[var(--color-faint)]"
    >
      {@render leadingIcon?.()}
    </span>
  {/if}
  <input
    {type}
    {placeholder}
    aria-label={ariaLabel}
    bind:value
    class={cn(
      'h-7 w-full rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white pr-3 font-sans text-[12px] tracking-tight text-[var(--color-ink)] placeholder:text-[var(--color-faint)] transition-colors duration-[var(--duration-soft)] ease-[var(--ease)] focus-visible:border-[var(--color-secondary)]/45 focus-visible:outline-none',
      leadingIcon ? 'pl-7' : 'pl-3'
    )}
  />
</label>
