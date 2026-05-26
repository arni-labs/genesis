<script lang="ts">
  // The right-side inspector: when the user clicks a stage cell, this
  // shows the verdict, evaluator, and the parsed counterexample. For
  // PASS cells it shows the objective_scores; for FAIL it shows the
  // violation + the raw evidence JSON (pretty-printed) so the human
  // director can see exactly why the variant was killed.

  import { Badge, Card } from '$lib/components/ui';
  import {
    parseEvidence,
    verdictTone,
    type StageResult,
    type Variant,
  } from '$lib/studio';

  type Props = {
    result: StageResult | null;
    variant: Variant | null;
    stageId: string;
  };

  let { result, variant, stageId }: Props = $props();

  const evidence = $derived(parseEvidence(result?.evidence ?? ''));
  const tone = $derived(verdictTone(result?.verdict ?? 'pending'));
  const evidencePretty = $derived.by(() => {
    if (!result?.evidence) return '';
    try {
      const parsed = JSON.parse(result.evidence);
      return JSON.stringify(parsed, null, 2);
    } catch (_) {
      return result.evidence;
    }
  });
  const scoresPretty = $derived.by(() => {
    if (!result?.objectiveScores) return '';
    try {
      const parsed = JSON.parse(result.objectiveScores);
      return JSON.stringify(parsed, null, 2);
    } catch (_) {
      return result.objectiveScores;
    }
  });
</script>

<Card radius="md" class="px-3 py-3">
  <p class="v-eyebrow">Inspector</p>
  {#if !result && !variant}
    <p class="mt-1 font-sans text-[12px] text-[var(--color-muted)]">
      Click a cell in the bracket to inspect a stage result.
    </p>
  {:else}
    <header class="mt-1.5 flex items-start justify-between gap-2">
      <div class="min-w-0">
        <h3 class="v-display truncate text-[14px] tracking-tight text-[var(--color-ink)]">
          {stageId || result?.stageId || '—'}
        </h3>
        <p class="truncate font-mono text-[10px] tracking-[0.06em] text-[var(--color-muted)]">
          {variant?.branchRef || variant?.id || ''}
        </p>
      </div>
      <Badge tone={tone}>
        {(result?.verdict ?? 'pending').toUpperCase()}
      </Badge>
    </header>

    <dl class="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 font-mono text-[10px] uppercase tracking-[0.08em]">
      <dt class="text-[var(--color-muted)]">Evaluator</dt>
      <dd class="truncate text-[var(--color-ink)]">{result?.evaluator || stageId || '—'}</dd>
      {#if variant}
        <dt class="text-[var(--color-muted)]">Variant</dt>
        <dd class="truncate text-[var(--color-ink)]">{variant.id}</dd>
        <dt class="text-[var(--color-muted)]">Commit</dt>
        <dd class="truncate text-[var(--color-ink)]">{variant.commitSha || '(none)'}</dd>
      {/if}
      {#if result?.createdAt}
        <dt class="text-[var(--color-muted)]">At</dt>
        <dd class="truncate text-[var(--color-ink)]">{result.createdAt}</dd>
      {/if}
    </dl>

    {#if result?.verdict === 'fail'}
      <section class="mt-3 rounded-[var(--radius-sm)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.05)] px-2.5 py-2">
        <p class="v-eyebrow text-[#7a1830]">Cause of death</p>
        {#if evidence.property}
          <p class="mt-0.5 font-mono text-[11px] text-[#7a1830]">
            <strong>property:</strong> {evidence.property}
          </p>
        {/if}
        {#if evidence.violation}
          <p class="mt-0.5 font-sans text-[12px] text-[#7a1830]">
            {evidence.violation}
          </p>
        {/if}
        {#if evidencePretty}
          <details class="mt-1.5">
            <summary class="cursor-pointer font-mono text-[10px] uppercase tracking-[0.08em] text-[#7a1830]">
              counterexample (raw)
            </summary>
            <pre class="mt-1 max-h-64 overflow-auto rounded-[var(--radius-xs)] border border-[var(--color-border)] bg-white px-2 py-1.5 font-mono text-[10px] leading-snug text-[var(--color-ink)]">{evidencePretty}</pre>
          </details>
        {/if}
      </section>
    {:else if result?.verdict === 'pass'}
      <section class="mt-3 rounded-[var(--radius-sm)] border border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)] px-2.5 py-2">
        <p class="v-eyebrow text-[var(--color-ink)]">Verdict</p>
        <p class="mt-0.5 font-sans text-[12px] text-[var(--color-ink)]">
          Passed {result.evaluator || stageId}.
        </p>
        {#if scoresPretty}
          <details class="mt-1.5">
            <summary class="cursor-pointer font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-ink-soft)]">
              objective scores
            </summary>
            <pre class="mt-1 max-h-48 overflow-auto rounded-[var(--radius-xs)] border border-[var(--color-border)] bg-white px-2 py-1.5 font-mono text-[10px] leading-snug text-[var(--color-ink)]">{scoresPretty}</pre>
          </details>
        {/if}
      </section>
    {:else}
      <p class="mt-2 font-sans text-[12px] text-[var(--color-muted)]">
        No result yet. Either the variant hasn't reached this stage, or it
        was killed at a previous stage so this stage was skipped.
      </p>
    {/if}
  {/if}
</Card>
