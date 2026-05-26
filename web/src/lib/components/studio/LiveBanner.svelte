<script lang="ts">
  // Loud, satisfying banner that surfaces ONLY when an Evolution is
  // in the Live state. The whole demo lands on this moment — the
  // verification cascade said yes, the WASM merge_variant succeeded,
  // the hot-deploy happened, and the missing action is now live in
  // the seed app.

  import { Sparkles } from '@lucide/svelte';
  import type { Evolution } from '$lib/studio';

  let { evolution }: { evolution: Evolution } = $props();
</script>

<aside
  class="relative overflow-hidden rounded-[var(--radius-md)] border border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)] px-4 py-3"
  aria-live="polite"
>
  <span class="v-pixel-corners" aria-hidden="true"></span>
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div class="flex items-center gap-2">
      <Sparkles size={18} class="shrink-0 text-[var(--color-ink)]" />
      <div>
        <p class="v-display text-[15px] tracking-tight text-[var(--color-ink)]">
          Downvote is LIVE.
        </p>
        <p class="font-sans text-[12px] text-[var(--color-ink-soft)]">
          Merged from <span class="v-mono">{evolution.mergedRef || 'main'}</span>
          {' '}into <span class="v-mono">{evolution.targetTenant}</span>; the missing action grew into the running app — no human code edit.
        </p>
      </div>
    </div>
    <div class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-ink-soft)]">
      Episode {evolution.id.slice(0, 8)}…
    </div>
  </div>
</aside>
