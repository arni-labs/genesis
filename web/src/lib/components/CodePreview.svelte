<script lang="ts">
  import { highlight, detectLanguage } from '$lib/highlight';
  import { cn } from '$lib/utils';

  type CodePreviewProps = {
    code: string;
    path: string;
    class?: string;
  };

  let { code, path, class: className }: CodePreviewProps = $props();

  const language = $derived(detectLanguage(path));

  let html = $state('');
  let pending = $state(true);

  $effect(() => {
    let cancelled = false;
    pending = true;
    highlight(code, language).then((next) => {
      if (cancelled) return;
      html = next;
      pending = false;
    });
    return () => {
      cancelled = true;
    };
  });
</script>

<div class={cn('relative h-full overflow-hidden', className)}>
  <div class="absolute right-2 top-1.5 z-10">
    <span class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white/86 px-1.5 py-[1px] font-mono text-[9.5px] font-semibold uppercase tracking-[0.10em] text-[var(--color-muted)] backdrop-blur">
      {language}
    </span>
  </div>
  <div class="v-scrollbar shiki-frame h-full overflow-auto bg-white">
    {#if pending}
      <pre class="m-0 px-4 py-3 font-mono text-[11.5px] leading-[1.55] text-[var(--color-muted)]">{code}</pre>
    {:else}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      {@html html}
    {/if}
  </div>
</div>

<style>
  :global(.shiki-frame pre.shiki) {
    margin: 0;
    padding: 14px 16px;
    background: transparent !important;
    font-family: var(--font-mono, 'Geist Mono', monospace);
    font-size: 11.5px;
    line-height: 1.55;
    counter-reset: line;
  }

  :global(.shiki-frame pre.shiki code) {
    display: grid;
    background: transparent !important;
  }

  :global(.shiki-frame .line) {
    display: inline-block;
    width: 100%;
    padding-left: 2.75rem;
    position: relative;
  }

  :global(.shiki-frame .line::before) {
    counter-increment: line;
    content: counter(line);
    position: absolute;
    left: 0;
    width: 2rem;
    text-align: right;
    color: var(--color-faint, #8b93a7);
    font-size: 10px;
    user-select: none;
    padding-right: 0.5rem;
  }
</style>
