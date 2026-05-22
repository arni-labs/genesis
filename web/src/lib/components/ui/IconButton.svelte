<script lang="ts">
  import { cn } from '$lib/utils';

  type IconButtonProps = {
    'aria-label': string;
    title?: string;
    disabled?: boolean;
    class?: string;
    onclick?: (event: MouseEvent) => void;
    children?: import('svelte').Snippet;
  };

  let {
    'aria-label': ariaLabel,
    title,
    disabled = false,
    class: className,
    onclick,
    children
  }: IconButtonProps = $props();
</script>

<button
  type="button"
  aria-label={ariaLabel}
  {title}
  {disabled}
  class={cn(
    'inline-flex h-7 w-7 items-center justify-center rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white text-[var(--color-ink-soft)] transition-colors duration-[var(--duration-soft)] ease-[var(--ease)] hover:border-[var(--color-primary)]/40 hover:bg-[var(--color-primary-soft)] hover:text-[var(--color-primary)] disabled:cursor-not-allowed disabled:opacity-55',
    className
  )}
  {onclick}
>
  {@render children?.()}
</button>
