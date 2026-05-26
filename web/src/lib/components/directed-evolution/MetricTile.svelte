<script lang="ts">
  type MetricTileProps = {
    label: string;
    value: number;
  };

  let { label, value }: MetricTileProps = $props();
</script>

<div class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-3 py-2">
  <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
    {label}
  </p>
  <p class="mt-1 font-mono text-[22px] leading-none text-[var(--color-ink)]">{value}</p>
</div>
