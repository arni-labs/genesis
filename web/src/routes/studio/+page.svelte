<script lang="ts">
  // Evolution Studio.
  //
  // Single-page view of every directed-evolution episode this genesis
  // instance has seen. Left rail = list of Evolutions, with selection
  // state in the URL. Right pane = the bracket, director controls,
  // cause-of-death inspector, and (when Live) the celebration banner.
  //
  // Data path: `loadStudio()` from $lib/studio fetches the three Evo.DE
  // entity sets and degrades to a fixture when the data plane isn't
  // available. A toggle in the topbar forces the fixture for design
  // work without standing up the server.

  import { onMount } from 'svelte';
  import { Loader2, RefreshCw, Sparkles, ToggleLeft, ToggleRight } from '@lucide/svelte';
  import { base } from '$app/paths';
  import { Badge, Button, Card, IconButton, Toast } from '$lib/components/ui';
  import BrandMark from '$lib/components/BrandMark.svelte';
  import EvolutionFlow from '$lib/components/studio/EvolutionFlow.svelte';
  import EvolutionListItem from '$lib/components/studio/EvolutionListItem.svelte';
  import Bracket from '$lib/components/studio/Bracket.svelte';
  import EvidenceInspector from '$lib/components/studio/EvidenceInspector.svelte';
  import DirectorControls from '$lib/components/studio/DirectorControls.svelte';
  import LiveBanner from '$lib/components/studio/LiveBanner.svelte';
  import {
    loadStudio,
    selectVariantsForEvolution,
    approveEvolution,
    revertEvolution,
    pickWinner,
    isEvolutionLive,
    evolutionStatusTone,
    type EvolutionStudioSnapshot,
    type Evolution,
    type Variant,
    type StageResult,
  } from '$lib/studio';

  let snapshot = $state<EvolutionStudioSnapshot | null>(null);
  let loading = $state(false);
  let error = $state('');
  let useFixture = $state(false);

  let selectedId = $state('');
  let selectedWinnerId = $state('');
  let inspectorResult = $state<StageResult | null>(null);
  let inspectorVariant = $state<Variant | null>(null);
  let inspectorStage = $state('');

  let toast = $state('');
  let toastTimer: number | undefined;
  let busy = $state(false);

  const selected = $derived<Evolution | null>(
    snapshot?.evolutions.find((e) => e.id === selectedId) ?? null,
  );
  const selectedVariants = $derived<Variant[]>(
    snapshot && selected
      ? selectVariantsForEvolution(snapshot, selected.id)
      : [],
  );
  const liveEpisode = $derived<Evolution | null>(
    snapshot?.evolutions.find(isEvolutionLive) ?? null,
  );

  onMount(() => {
    void refresh();
  });

  async function refresh() {
    loading = true;
    error = '';
    try {
      const data = await loadStudio({ forceFixture: useFixture });
      snapshot = data;
      // Pick a default selection: prefer a Live evolution, else the
      // most recent one. Keep an existing selection if still valid.
      if (selectedId && data.evolutions.some((e) => e.id === selectedId)) {
        return;
      }
      const live = data.evolutions.find(isEvolutionLive);
      const recent = [...data.evolutions].sort((a, b) =>
        b.createdAt.localeCompare(a.createdAt),
      )[0];
      selectedId = live?.id ?? recent?.id ?? '';
      selectedWinnerId = selected?.winnerVariantId ?? '';
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  function showToast(msg: string) {
    toast = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => (toast = ''), 3500);
  }

  function handleSelect(id: string) {
    selectedId = id;
    inspectorResult = null;
    inspectorVariant = null;
    inspectorStage = '';
    selectedWinnerId = snapshot?.evolutions.find((e) => e.id === id)?.winnerVariantId ?? '';
  }

  function handleToggleFixture() {
    useFixture = !useFixture;
    void refresh();
  }

  function handlePickWinner(variantId: string) {
    selectedWinnerId = variantId;
    showToast(`Pre-picked winner: ${variantId.slice(0, 12)}… — click "Pick winner" to confirm.`);
  }

  function handleInspectCell(r: StageResult | null, sid: string, v: Variant) {
    inspectorResult = r;
    inspectorVariant = v;
    inspectorStage = sid;
  }

  async function handleConfirmWinner() {
    if (!selected || !selectedWinnerId) return;
    busy = true;
    try {
      const r = await pickWinner(selected.id, selectedWinnerId);
      if (!r.ok) {
        showToast(`Pick failed (${r.status}): ${r.message ?? 'unknown'}`);
      } else {
        showToast(`Winner recorded — Evolution → AwaitingApproval.`);
        await refresh();
      }
    } finally {
      busy = false;
    }
  }

  async function handleApprove() {
    if (!selected) return;
    busy = true;
    try {
      const r = await approveEvolution(selected.id);
      if (!r.ok) {
        showToast(`Approve failed (${r.status}): ${r.message ?? 'unknown'}`);
      } else {
        showToast(`Approved — merge_variant pipeline kicked off.`);
        await refresh();
      }
    } finally {
      busy = false;
    }
  }

  async function handleRevert() {
    if (!selected) return;
    const reason = window.prompt('Revert reason?', 'manual revert');
    if (reason === null) return;
    busy = true;
    try {
      const r = await revertEvolution(selected.id, reason);
      if (!r.ok) {
        showToast(`Revert failed (${r.status}): ${r.message ?? 'unknown'}`);
      } else {
        showToast(`Reverted — tenant rolled back.`);
        await refresh();
      }
    } finally {
      busy = false;
    }
  }

  function variantsFor(evolutionId: string): number {
    return snapshot?.variants.filter((v) => v.evolutionId === evolutionId).length ?? 0;
  }
</script>

<svelte:head>
  <title>Evolution Studio · Genesis</title>
  <meta name="description" content="Live elimination bracket for directed-evolution episodes." />
</svelte:head>

<main class="relative z-[1] min-h-screen pb-12">
  <!-- Studio-specific topbar (deliberately not the registry's; this
       view has its own controls — fixture toggle, refresh, source
       indicator). -->
  <header
    class="sticky top-0 z-30 flex flex-wrap items-center justify-between gap-3 border-b border-[var(--color-border)] bg-white/86 px-4 py-2.5 backdrop-blur"
  >
    <div class="flex items-center gap-2">
      <BrandMark size={20} />
      <h1
        class="text-[20px] font-semibold leading-none tracking-[0.04em] text-[var(--color-ink)] [font-family:var(--font-mono)]"
      >
        Evolution<span class="text-[var(--color-accent)]">.</span>Studio
      </h1>
      <a
        href="{base}/"
        class="ml-2 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)] hover:text-[var(--color-primary)] hover:underline"
      >
        ← registry
      </a>
    </div>

    <div class="flex items-center gap-3 font-mono text-[10px] tracking-[0.10em] uppercase text-[var(--color-muted)]">
      <span class="hidden items-center gap-1.5 sm:inline-flex">
        <span>Episodes</span>
        <span class="font-semibold text-[var(--color-ink)]">{snapshot?.evolutions.length ?? 0}</span>
      </span>
      <span class="hidden items-center gap-1.5 sm:inline-flex">
        <span>Variants</span>
        <span class="font-semibold text-[var(--color-ink)]">{snapshot?.variants.length ?? 0}</span>
      </span>
      <Badge tone={snapshot?.source === 'live' ? 'success' : snapshot?.source === 'fixture' ? 'warning' : 'neutral'} pixel={snapshot?.source === 'live'}>
        {snapshot?.source === 'live' ? 'Live data' : snapshot?.source === 'fixture' ? 'Fixture' : 'Loading'}
      </Badge>
      <button
        type="button"
        onclick={handleToggleFixture}
        class="inline-flex h-6 items-center gap-1.5 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white px-2 font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-ink-soft)] hover:bg-[var(--color-surface-soft)]"
        title={useFixture ? 'Switch to live data' : 'Force fixture (for design)'}
      >
        {#if useFixture}
          <ToggleRight size={13} />
        {:else}
          <ToggleLeft size={13} />
        {/if}
        <span>Fixture</span>
      </button>
      <IconButton aria-label="Refresh studio data" disabled={loading} onclick={refresh}>
        {#if loading}
          <Loader2 size={13} class="animate-spin" />
        {:else}
          <RefreshCw size={13} />
        {/if}
      </IconButton>
    </div>
  </header>

  {#if liveEpisode && (!selected || selected.id !== liveEpisode.id)}
    <!-- Persistent reminder when *any* episode is Live but the user
         is currently looking at a different one. Clicking jumps to it. -->
    <div class="px-3 py-2 lg:px-4">
      <button
        type="button"
        onclick={() => handleSelect(liveEpisode.id)}
        class="flex w-full items-center gap-2 rounded-[var(--radius-md)] border border-[var(--color-accent-strong)] bg-[var(--color-accent-soft)] px-3 py-1.5 text-left font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-ink)] hover:bg-[rgba(183,255,26,0.30)]"
      >
        <Sparkles size={12} />
        <span class="font-semibold">Live now:</span>
        <span>{liveEpisode.intent}</span>
        <span class="ml-auto">View →</span>
      </button>
    </div>
  {/if}

  <section class="grid gap-3 px-3 py-3 lg:grid-cols-[320px_minmax(0,1fr)] lg:px-4 xl:px-5">
    <!-- ───── Left rail: Evolutions list ───── -->
    <aside class="flex flex-col gap-2">
      <Card radius="md" class="px-3 py-2.5">
        <p class="v-eyebrow">Episodes</p>
        <p class="mt-0.5 font-sans text-[11px] text-[var(--color-muted)]">
          Each row is one directed-evolution run — its intent, current state, and variant count.
        </p>
      </Card>

      {#if !snapshot}
        <div class="flex h-32 items-center justify-center rounded-[var(--radius-md)] border border-dashed border-[var(--color-border)] bg-white">
          <Loader2 size={16} class="animate-spin text-[var(--color-faint)]" />
        </div>
      {:else if snapshot.evolutions.length === 0}
        <Card radius="md" class="px-3 py-6 text-center">
          <p class="font-sans text-[12px] text-[var(--color-muted)]">No evolutions yet.</p>
        </Card>
      {:else}
        <div class="flex flex-col gap-1.5">
          {#each snapshot.evolutions as e (e.id)}
            <EvolutionListItem
              evolution={e}
              selected={e.id === selectedId}
              variantCount={variantsFor(e.id)}
              onSelect={handleSelect}
            />
          {/each}
        </div>
      {/if}

      {#if snapshot && snapshot.warnings.length > 0}
        <Card radius="md" class="px-2.5 py-2">
          <p class="v-eyebrow">Notes</p>
          <ul class="mt-1 list-disc space-y-0.5 pl-4 font-sans text-[11px] leading-snug text-[var(--color-muted)]">
            {#each snapshot.warnings as w (w)}
              <li>{w}</li>
            {/each}
          </ul>
        </Card>
      {/if}
    </aside>

    <!-- ───── Right pane: selected episode detail ───── -->
    <section class="flex min-w-0 flex-col gap-3">
      {#if error}
        <Card radius="md" class="px-3 py-3">
          <p class="font-sans text-[12px] text-[#7a1830]">{error}</p>
        </Card>
      {/if}

      {#if !selected}
        <Card radius="md" class="px-3 py-10 text-center">
          <p class="font-sans text-[13px] font-semibold text-[var(--color-ink)]">
            Select an Evolution to explore.
          </p>
          <p class="mt-1 font-sans text-[12px] text-[var(--color-muted)]">
            The bracket, cause-of-death, and director controls show up here.
          </p>
        </Card>
      {:else}
        {#if isEvolutionLive(selected)}
          <LiveBanner evolution={selected} />
        {/if}

        <!-- Episode header card -->
        <Card radius="md" class="px-3 py-3">
          <header class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <p class="v-eyebrow">Episode</p>
              <h2 class="v-display mt-0.5 text-[18px] tracking-tight text-[var(--color-ink)]">
                {selected.intent || '(no intent recorded)'}
              </h2>
              <p class="mt-1 font-sans text-[12px] text-[var(--color-ink-soft)]">
                {selected.problemStatement || '(no problem statement yet)'}
              </p>
            </div>
            <Badge tone={evolutionStatusTone(selected.status)} pixel={selected.status === 'Live'}>
              {selected.status}
            </Badge>
          </header>

          <dl class="mt-3 grid grid-cols-2 gap-x-3 gap-y-1 font-mono text-[10px] uppercase tracking-[0.08em] sm:grid-cols-4">
            <dt class="text-[var(--color-muted)]">Target app</dt>
            <dd class="truncate text-[var(--color-ink)]">{selected.targetApp || '—'}</dd>
            <dt class="text-[var(--color-muted)]">Tenant</dt>
            <dd class="truncate text-[var(--color-ink)]">{selected.targetTenant || '—'}</dd>
            <dt class="text-[var(--color-muted)]">Variants</dt>
            <dd class="truncate text-[var(--color-ink)]">{selectedVariants.length}</dd>
            <dt class="text-[var(--color-muted)]">Autonomy</dt>
            <dd class="truncate text-[var(--color-ink)]">{selected.autonomy}</dd>
            {#if selected.winnerVariantId}
              <dt class="text-[var(--color-muted)]">Winner</dt>
              <dd class="truncate text-[var(--color-ink)]">{selected.winnerVariantId}</dd>
            {/if}
            {#if selected.mergedRef}
              <dt class="text-[var(--color-muted)]">Merged ref</dt>
              <dd class="truncate text-[var(--color-ink)]">{selected.mergedRef}</dd>
            {/if}
          </dl>

          <div class="mt-3">
            <EvolutionFlow status={selected.status} />
          </div>
        </Card>

        <!-- Bracket + controls + inspector. On wide screens the
             inspector floats to the right; on narrow screens it
             stacks below the bracket. -->
        <div class="grid gap-3 xl:grid-cols-[minmax(0,1fr)_320px]">
          <Card radius="md" class="px-3 py-3">
            <header class="flex flex-wrap items-center justify-between gap-2">
              <div>
                <p class="v-eyebrow">Elimination bracket</p>
                <p class="font-sans text-[11px] text-[var(--color-muted)]">
                  Columns = variants, rows = fitness stages. Click cells for evidence; click variant headers to pre-pick a winner.
                </p>
              </div>
              <DirectorControls
                evolution={selected}
                selectedWinnerId={selectedWinnerId}
                busy={busy}
                onApprove={handleApprove}
                onRevert={handleRevert}
                onConfirmWinner={handleConfirmWinner}
              />
            </header>
            <div class="mt-3">
              {#if snapshot}
                <Bracket
                  snapshot={snapshot}
                  variants={selectedVariants}
                  selectedWinnerId={selectedWinnerId}
                  canSelectWinner={selected.status === 'Selecting'}
                  onPickWinner={handlePickWinner}
                  onInspectCell={handleInspectCell}
                />
              {/if}
            </div>
          </Card>

          <div class="flex flex-col gap-3">
            <EvidenceInspector
              result={inspectorResult}
              variant={inspectorVariant}
              stageId={inspectorStage}
            />
            <Card radius="md" class="px-3 py-3">
              <p class="v-eyebrow">Fitness function</p>
              <p class="mt-1 font-sans text-[12px] text-[var(--color-ink-soft)]">
                v1 stages (lexicographic): <span class="v-mono">{(snapshot?.stageOrder ?? []).join(' → ')}</span>.
              </p>
              <p class="mt-1.5 font-sans text-[11px] text-[var(--color-muted)]">
                Fitness is compiled from natural language via Claude Code — not edited here. This panel is read-only by design.
              </p>
            </Card>
          </div>
        </div>
      {/if}
    </section>
  </section>

  <Toast message={toast} />
</main>
