<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';

  export const buttonVariants = tv({
    base: 'btn-base inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-[var(--radius-sm)] transition-colors duration-[var(--duration-soft)] ease-[var(--ease)] disabled:cursor-not-allowed disabled:opacity-55',
    variants: {
      variant: {
        primary: 'btn-primary',
        secondary: 'btn-secondary',
        accent: 'btn-accent',
        outline: 'btn-outline',
        ghost: 'btn-ghost'
      },
      size: {
        xs: 'h-6 px-2 text-[10px]',
        sm: 'h-7 px-3 text-[10.5px]',
        md: 'h-8 px-3.5 text-[11px]',
        lg: 'h-9 px-4 text-[11.5px]',
        icon: 'h-7 w-7 p-0'
      }
    },
    defaultVariants: {
      variant: 'outline',
      size: 'md'
    }
  });

  export type ButtonVariant = VariantProps<typeof buttonVariants>['variant'];
  export type ButtonSize = VariantProps<typeof buttonVariants>['size'];
</script>

<script lang="ts">
  import { cn } from '$lib/utils';

  type ButtonProps = {
    variant?: ButtonVariant;
    size?: ButtonSize;
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
    class?: string;
    title?: string;
    'aria-label'?: string;
    onclick?: (event: MouseEvent) => void;
    children?: import('svelte').Snippet;
  };

  let {
    variant = 'outline',
    size = 'md',
    type = 'button',
    disabled = false,
    class: className,
    title,
    'aria-label': ariaLabel,
    onclick,
    children
  }: ButtonProps = $props();
</script>

<button
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  class={cn(buttonVariants({ variant, size }), className)}
  {onclick}
>
  {@render children?.()}
</button>
