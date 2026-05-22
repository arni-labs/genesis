<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';

  export const badgeVariants = tv({
    base: 'inline-flex items-center gap-1.5 rounded-full border px-2 py-[2px] font-mono text-[10px] uppercase tracking-[0.10em] font-semibold',
    variants: {
      tone: {
        neutral:
          'border-[var(--color-border)] bg-white text-[var(--color-ink-soft)]',
        primary:
          'border-[var(--color-primary)]/22 bg-[var(--color-primary-soft)] text-[var(--color-primary)]',
        secondary:
          'border-[var(--color-secondary)]/24 bg-[var(--color-secondary-soft)] text-[var(--color-secondary-strong)]',
        accent:
          'border-[var(--color-primary)]/22 bg-white text-[var(--color-primary)]',
        success:
          'border-[var(--color-border)] bg-white text-[var(--color-ink)]',
        warning:
          'border-[var(--color-warning)]/30 bg-[rgba(214,166,0,0.10)] text-[#735900]',
        danger:
          'border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] text-[#7a1830]'
      }
    },
    defaultVariants: {
      tone: 'neutral'
    }
  });

  export type BadgeTone = VariantProps<typeof badgeVariants>['tone'];
</script>

<script lang="ts">
  import { cn } from '$lib/utils';

  type BadgeProps = {
    tone?: BadgeTone;
    pixel?: boolean;
    class?: string;
    children?: import('svelte').Snippet;
  };

  let { tone = 'neutral', pixel = false, class: className, children }: BadgeProps = $props();
</script>

<span class={cn(badgeVariants({ tone }), className)}>
  {#if pixel}
    <span class="v-pixel-square" aria-hidden="true"></span>
  {/if}
  {@render children?.()}
</span>
