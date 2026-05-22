<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';

  export const cardVariants = tv({
    base: 'relative overflow-hidden border border-[var(--color-border)] bg-white',
    variants: {
      tone: {
        plain: '',
        soft: 'bg-[var(--color-surface-soft)]',
        raised: 'shadow-[var(--shadow-sm)]',
        bloom: 'shadow-[var(--shadow-md)]'
      },
      radius: {
        sm: 'rounded-[var(--radius-sm)]',
        md: 'rounded-[var(--radius-md)]',
        lg: 'rounded-[var(--radius-lg)]'
      }
    },
    defaultVariants: {
      tone: 'plain',
      radius: 'md'
    }
  });

  export type CardTone = VariantProps<typeof cardVariants>['tone'];
  export type CardRadius = VariantProps<typeof cardVariants>['radius'];
</script>

<script lang="ts">
  import { cn } from '$lib/utils';

  type CardProps = {
    tone?: CardTone;
    radius?: CardRadius;
    pixel?: boolean;
    class?: string;
    children?: import('svelte').Snippet;
  };

  let {
    tone = 'plain',
    radius = 'md',
    pixel = false,
    class: className,
    children
  }: CardProps = $props();
</script>

<div class={cn(cardVariants({ tone, radius }), pixel && 'v-pixel-corners', className)}>
  {@render children?.()}
</div>
