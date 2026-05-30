<script lang="ts">
  // One row in the evolution list (left rail). Renders title, intent,
  // status badge, and a tiny flow indicator. Selected state styles
  // with the accent border.

  import { Badge } from '$lib/components/ui';
  import { evolutionStatusTone, type Evolution } from '$lib/studio';

  type Props = {
    evolution: Evolution;
    selected: boolean;
    variantCount: number;
    onSelect: (id: string) => void;
  };

  let { evolution, selected, variantCount, onSelect }: Props = $props();
  const tone = $derived(evolutionStatusTone(evolution.status));
</script>

<button
  type="button"
  onclick={() => onSelect(evolution.id)}
  class={[
    'group flex w-full flex-col gap-2 rounded-[var(--radius-md)] border bg-white px-3 py-2.5 text-left transition-all duration-[var(--duration-soft)] ease-[var(--ease)]',
    selected
      ? 'border-[var(--color-primary)] shadow-[var(--shadow-md)]'
      : 'border-[var(--color-border)] hover:-translate-y-[1px] hover:border-[var(--color-primary)]/30 hover:shadow-[var(--shadow-sm)]',
  ].join(' ')}
>
  <header class="flex items-start justify-between gap-2">
    <div class="min-w-0 flex-1">
      <p class="truncate font-mono text-[10px] tracking-[0.08em] text-[var(--color-muted)]">
        {evolution.targetApp}
      </p>
      <h3 class="line-clamp-2 v-display text-[13px] tracking-tight text-[var(--color-ink)]">
        {evolution.intent || '(no intent recorded)'}
      </h3>
    </div>
    <Badge tone={tone} pixel={tone === 'success'}>
      {evolution.status}
    </Badge>
  </header>

  <footer class="flex items-center justify-between gap-2 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)]">
    <span>Variants {variantCount}</span>
    <span>Autonomy {evolution.autonomy}</span>
  </footer>
</button>
