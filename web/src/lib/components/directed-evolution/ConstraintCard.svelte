<script lang="ts">
  import { Pin } from '@lucide/svelte';
  import { Badge, Button } from '$lib/components/ui';
  import type { EvolutionViabilityConstraint } from '$lib/directedEvolution';

  type StatusTone = 'success' | 'warning' | 'danger' | 'neutral' | 'primary';

  type ConstraintCardProps = {
    constraint: EvolutionViabilityConstraint;
    busy: boolean;
    tone: StatusTone;
    onPin: () => void;
  };

  let { constraint, busy, tone, onPin }: ConstraintCardProps = $props();
</script>

<div class="relative z-[2] min-w-0 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-2.5">
  <div class="flex flex-wrap items-start justify-between gap-2">
    <Badge {tone}>{constraint.status}</Badge>
    {#if constraint.status !== 'Pinned' && constraint.status !== 'Archived'}
      <Button size="xs" onclick={onPin} disabled={busy}>
        <Pin size={11} />
        Pin
      </Button>
    {/if}
  </div>
  <p class="mt-2 text-[12px] leading-relaxed text-[var(--color-ink)]">
    {constraint.constraintStatement || constraint.id}
  </p>
  {#if constraint.constraintKind}
    <p class="mt-1 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
      {constraint.constraintKind}
    </p>
  {/if}
</div>
