<script lang="ts">
  type MetricTileProps = {
    label: string;
    value: number;
    compact?: boolean;
  };

  let { label, value, compact = false }: MetricTileProps = $props();
</script>

<div
  class={`min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white ${
    compact ? 'px-2.5 py-2 sm:px-3' : 'px-3 py-2'
  }`}
>
  <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
    {label}
  </p>
  <p class={`mt-1 font-mono leading-none text-[var(--color-ink)] ${compact ? 'text-[19px] sm:text-[22px]' : 'text-[22px]'}`}>
    {value}
  </p>
</div>
