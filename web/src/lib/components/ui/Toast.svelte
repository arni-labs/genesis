<script lang="ts">
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { cn } from '$lib/utils';

  type ToastProps = {
    message: string;
    class?: string;
  };

  let { message, class: className }: ToastProps = $props();
</script>

{#if message}
  <div
    role="status"
    aria-live="polite"
    in:fly={{ y: 8, duration: 200, easing: cubicOut }}
    out:fly={{ y: 8, duration: 160, easing: cubicOut }}
    class={cn(
      'pointer-events-none fixed bottom-4 right-4 z-50 flex max-w-[320px] items-center gap-2 rounded-[var(--radius-md)] border border-[var(--color-border)] bg-[var(--color-primary)] px-3 py-2 font-sans text-[12px] font-semibold text-white shadow-[var(--shadow-md)]',
      className
    )}
  >
    <span class="v-pixel-square" aria-hidden="true"></span>
    <span class="font-mono uppercase tracking-[0.10em] text-[10.5px]">{message}</span>
  </div>
{/if}
