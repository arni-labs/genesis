<script lang="ts">
  import { onMount } from 'svelte';
  import { base } from '$app/paths';
  import {
    Activity,
    ArrowLeft,
    GitBranch,
    Pause,
    Play,
    RefreshCw,
    RotateCcw,
    ShieldCheck,
    Sparkles,
    Telescope
  } from '@lucide/svelte';
  import { createCampaign, evolutionAction, loadEvolutionSnapshot, recordIntervention } from '$lib/api';
  import type { EvolutionCampaign, EvolutionItem, EvolutionSnapshot } from '$lib/types';

  let snapshot = $state<EvolutionSnapshot | null>(null);
  let loading = $state(true);
  let error = $state('');
  let busy = $state(false);
  let selectedId = $state('');
  let name = $state('Agent Answers: useful answers under evolving usage');
  let targetAppRef = $state('demo/agent-answers@seed');
  let directorBrief = $state('Evolve this app toward genuinely useful agent-to-agent answers while preserving understandable behavior and rollback.');
  let direction = $state('Keep behavior that makes answers easier to verify from evidence.');

  let campaigns = $derived(snapshot?.campaigns ?? []);
  let campaign = $derived(campaigns.find((item) => item.id === selectedId) ?? campaigns[0] ?? null);
  let generations = $derived(filterForCampaign(snapshot?.generations ?? [], campaign?.id));
  let candidates = $derived(filterForCampaign(snapshot?.candidates ?? [], campaign?.id));
  let measurements = $derived(filterForCampaign(snapshot?.measurements ?? [], campaign?.id));
  let capabilities = $derived(filterForCampaign(snapshot?.capabilities ?? [], campaign?.id));
  let interventions = $derived(filterForCampaign(snapshot?.interventions ?? [], campaign?.id));
  let traffic = $derived(filterForCampaign(snapshot?.trafficSources ?? [], campaign?.id));
  let trialSuites = $derived(filterForRecordPrefix(snapshot?.trialSuites ?? [], campaign?.id));
  let metricDefinitions = $derived(filterForRecordPrefix(snapshot?.metricDefinitions ?? [], campaign?.id));
  let validatorRuns = $derived(filterForRecordPrefix(snapshot?.validatorRuns ?? [], campaign?.id));
  let selection = $derived((snapshot?.selectionDesigns ?? []).find((item) => item.id === campaign?.activeSelectionDesignId));
  let proposedSelections = $derived((snapshot?.selectionDesigns ?? []).filter((item) => field(item, 'CampaignId') === campaign?.id && item.status === 'Proposed'));

  onMount(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(false), 5000);
    return () => window.clearInterval(timer);
  });

  function field(item: EvolutionItem | undefined, key: string): string {
    if (!item) return '';
    const snake = key.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
    const value = item.fields[key] ?? item.fields[key.charAt(0).toLowerCase() + key.slice(1)] ?? item.fields[snake];
    return value === undefined || value === null ? '' : String(value);
  }

  function filterForCampaign(items: EvolutionItem[], id: string | undefined): EvolutionItem[] {
    return id ? items.filter((item) => field(item, 'CampaignId') === id) : [];
  }

  function filterForRecordPrefix(items: EvolutionItem[], id: string | undefined): EvolutionItem[] {
    return id ? items.filter((item) => item.id.startsWith(`${id}-`)) : [];
  }

  function tone(status: string): string {
    if (['Running', 'Released', 'Selected', 'Recorded', 'Frozen', 'Kept', 'Active'].includes(status)) return 'bg-emerald-50 text-emerald-700 border-emerald-200';
    if (['Paused', 'Proposed', 'Draft', 'Assessed'].includes(status)) return 'bg-amber-50 text-amber-700 border-amber-200';
    if (['Failed', 'RolledBack', 'Eliminated', 'Rejected'].includes(status)) return 'bg-rose-50 text-rose-700 border-rose-200';
    return 'bg-slate-50 text-slate-600 border-slate-200';
  }

  async function refresh(showLoading = true) {
    if (showLoading) loading = true;
    try {
      snapshot = await loadEvolutionSnapshot();
      if (!selectedId && snapshot.campaigns.length) selectedId = snapshot.campaigns[0].id;
      error = '';
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      loading = false;
    }
  }

  async function newCampaign() {
    busy = true;
    try {
      const id = `campaign-${Date.now()}`;
      await createCampaign({ id, name, directorBrief, targetAppRef });
      selectedId = id;
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  async function control(action: string, params: Record<string, unknown> = {}) {
    if (!campaign) return;
    busy = true;
    try {
      await evolutionAction('Campaigns', campaign.id, action, params);
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  async function addDirection() {
    if (!campaign || !direction.trim()) return;
    busy = true;
    try {
      await recordIntervention(campaign.id, direction.trim());
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }

  async function approveDesign(item: EvolutionItem) {
    if (!campaign) return;
    busy = true;
    try {
      await evolutionAction('SelectionDesigns', item.id, 'Approve', { approved_by: 'human-studio' });
      await evolutionAction('SelectionDesigns', item.id, 'Freeze', { frozen_at: new Date().toISOString() });
      await evolutionAction('Campaigns', campaign.id, 'ApproveSelection', {
        active_selection_design_id: item.id,
        active_evaluator_ref: field(item, 'EvaluatorAppRef')
      });
      await refresh();
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>Evolution Studio | Genesis</title></svelte:head>

<main class="min-h-screen bg-[var(--color-page)] text-[var(--color-ink)]">
  <header class="sticky top-0 z-30 flex h-14 items-center justify-between border-b border-[var(--color-border)] bg-white/92 px-4 backdrop-blur md:px-6">
    <div class="flex items-center gap-4">
      <a href={`${base}/`} class="flex items-center gap-2 text-[12px] font-semibold text-[var(--color-ink-soft)] hover:text-[var(--color-primary)]"><ArrowLeft size={15}/> Registry</a>
      <div class="h-5 w-px bg-[var(--color-border)]"></div>
      <span class="font-mono text-[18px] font-semibold">Evolution Studio<span class="text-[var(--color-secondary)]">.</span></span>
      <a href={`${base}/answers`} class="hidden text-[12px] font-semibold text-[var(--color-ink-soft)] hover:text-[var(--color-primary)] md:block">Open subject app</a>
    </div>
    <button class="flex h-9 items-center gap-2 border border-[var(--color-border)] bg-white px-3 text-[12px] font-semibold" onclick={() => refresh()} disabled={loading}><RefreshCw class={loading ? 'animate-spin' : ''} size={14}/> Sync</button>
  </header>

  <section class="grid min-h-[calc(100vh-3.5rem)] grid-cols-1 lg:grid-cols-[300px_1fr_320px]">
    <aside class="border-b border-[var(--color-border)] bg-white p-4 lg:border-b-0 lg:border-r">
      <div class="mb-3 flex items-center justify-between"><p class="v-eyebrow">Campaigns</p><span class="font-mono text-[11px] text-[var(--color-muted)]">{campaigns.length}</span></div>
      <div class="space-y-2">
        {#each campaigns as item}
          <button class="w-full border p-3 text-left {item.id === campaign?.id ? 'border-[var(--color-primary)] bg-[var(--color-primary-soft)]' : 'border-[var(--color-border)] bg-white'}" onclick={() => selectedId = item.id}>
            <div class="mb-2 flex items-start justify-between gap-2"><span class="text-[13px] font-semibold leading-5">{item.name || item.id}</span><span class="border px-1.5 py-0.5 font-mono text-[9px] uppercase {tone(item.status)}">{item.status}</span></div>
            <p class="line-clamp-2 text-[11px] leading-4 text-[var(--color-muted)]">{item.targetAppRef}</p>
          </button>
        {/each}
      </div>
      <form class="mt-5 space-y-2 border-t border-[var(--color-border)] pt-4" onsubmit={(event) => { event.preventDefault(); void newCampaign(); }}>
        <p class="v-eyebrow">New campaign</p>
        <input class="w-full border border-[var(--color-border)] bg-white p-2 text-[12px]" bind:value={name} aria-label="Campaign name" />
        <input class="w-full border border-[var(--color-border)] bg-white p-2 font-mono text-[11px]" bind:value={targetAppRef} aria-label="Seed app ref" />
        <textarea class="h-24 w-full resize-none border border-[var(--color-border)] bg-white p-2 text-[12px] leading-5" bind:value={directorBrief} aria-label="Director brief"></textarea>
        <button class="btn-primary flex h-9 w-full items-center justify-center gap-2 text-[11px]" disabled={busy}><Sparkles size={14}/> Create</button>
      </form>
    </aside>

    <section class="min-w-0 bg-white/62">
      {#if error}<div class="m-4 border border-rose-200 bg-rose-50 p-3 text-[12px] text-rose-700">{error}</div>{/if}
      {#if campaign}
        <div class="border-b border-[var(--color-border)] px-5 py-5 md:px-7">
          <div class="flex flex-wrap items-start justify-between gap-4">
            <div><p class="v-eyebrow mb-2">Active experiment</p><h1 class="text-[24px] font-semibold leading-8">{campaign.name}</h1><p class="mt-2 max-w-3xl text-[13px] leading-6 text-[var(--color-ink-soft)]">{campaign.directorBrief}</p></div>
            <div class="flex flex-wrap gap-2">
              {#if campaign.status === 'Draft'}<button class="btn-primary flex h-10 items-center gap-2 px-4 text-[11px]" onclick={() => control('Start')} disabled={busy}><Play size={14}/> Start</button>{/if}
              {#if campaign.status === 'Running'}<button class="btn-outline flex h-10 items-center gap-2 px-4 text-[12px]" onclick={() => control('Pause', { pause_reason: 'Paused from Evolution Studio' })} disabled={busy}><Pause size={14}/> Pause</button>{/if}
              {#if campaign.status === 'Paused'}<button class="btn-outline flex h-10 items-center gap-2 px-4 text-[12px]" onclick={() => control('Resume')} disabled={busy}><Play size={14}/> Resume</button>{/if}
              {#if campaign.currentReleaseRef}<button class="btn-outline flex h-10 items-center gap-2 px-4 text-[12px]" onclick={() => control('Rollback', { current_release_ref: campaign.previousReleaseRef, previous_release_ref: campaign.currentReleaseRef, last_release_reason: 'Human rollback from Studio' })} disabled={busy}><RotateCcw size={14}/> Roll back</button>{/if}
            </div>
          </div>
          <div class="mt-5 grid grid-cols-2 gap-px bg-[var(--color-border)] md:grid-cols-4">
            <div class="bg-white p-3"><p class="v-eyebrow">Generation</p><p class="mt-2 font-mono text-[22px]">{campaign.generationCount}</p></div>
            <div class="bg-white p-3"><p class="v-eyebrow">Brain</p><p class="mt-2 text-[14px] font-semibold">{campaign.brainProvider || 'codex'}</p></div>
            <div class="bg-white p-3"><p class="v-eyebrow">Release policy</p><p class="mt-2 text-[13px] font-semibold">Automatic</p></div>
            <div class="bg-white p-3"><p class="v-eyebrow">Signals</p><p class="mt-2 font-mono text-[22px]">{measurements.length}</p></div>
          </div>
        </div>

        <div class="p-5 md:p-7">
          <div class="mb-6 grid gap-4 xl:grid-cols-2">
            <div class="border border-[var(--color-border)] bg-white p-4">
              <div class="mb-4 flex items-center gap-2"><GitBranch size={15}/><h2 class="text-[14px] font-semibold">Subject lineage</h2></div>
              <div class="space-y-3">
                {#each generations as generation}
                  <div class="grid grid-cols-[62px_1fr] gap-3"><span class="font-mono text-[11px] text-[var(--color-muted)]">GEN {field(generation, 'Ordinal')}</span><div class="border-l-2 border-[var(--color-primary)] pl-3"><div class="flex items-center gap-2"><span class="font-mono text-[11px]">{field(generation, 'ReleasedAppRef') || field(generation, 'ParentReleaseRef') || 'evaluating'}</span><span class="border px-1.5 font-mono text-[9px] uppercase {tone(generation.status)}">{generation.status}</span></div><p class="mt-1 text-[11px] text-[var(--color-muted)]">{field(generation, 'SelectionReason')}</p></div></div>
                {/each}
                {#if !generations.length}<p class="text-[12px] text-[var(--color-muted)]">No generations have started.</p>{/if}
              </div>
            </div>
            <div class="border border-[var(--color-border)] bg-white p-4">
              <div class="mb-4 flex items-center gap-2"><ShieldCheck size={15}/><h2 class="text-[14px] font-semibold">Frozen judge</h2></div>
              <p class="font-mono text-[11px] text-[var(--color-primary)]">{campaign.activeEvaluatorRef || 'Awaiting approved selection design'}</p>
              {#if selection}<p class="mt-3 text-[12px] leading-5 text-[var(--color-ink-soft)]">{field(selection, 'Rationale')}</p><p class="mt-3 font-mono text-[10px] uppercase text-[var(--color-muted)]">Selection {selection.status} / {selection.id}</p>{/if}
              {#if trialSuites.length}
                <div class="mt-4 border-t border-[var(--color-border)] pt-3">
                  <p class="v-eyebrow">Native validation</p>
                  <p class="mt-2 text-[12px] font-semibold">{validatorRuns.length} native trial runs / {metricDefinitions.length} frozen measures</p>
                  <div class="mt-3 space-y-2">
                    {#each validatorRuns as run}
                      <div class="flex items-center justify-between gap-3 text-[11px]"><span class="truncate font-mono">{field(run, 'CandidateAppRef') || run.id}</span><span class="border px-1.5 font-mono text-[9px] uppercase {tone(run.status)}">{run.status}</span></div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          </div>

          <div class="border border-[var(--color-border)] bg-white">
            <div class="flex items-center gap-2 border-b border-[var(--color-border)] px-4 py-3"><Activity size={15}/><h2 class="text-[14px] font-semibold">Evidence stream</h2></div>
            <div class="divide-y divide-[var(--color-border)]">
              {#each measurements.slice().reverse() as item}
                <div class="grid gap-2 px-4 py-3 md:grid-cols-[140px_140px_1fr_170px]"><span class="font-mono text-[11px]">{field(item, 'MetricKey')}</span><span class="text-[12px] font-semibold">{field(item, 'MetricValue')}</span><span class="text-[12px] text-[var(--color-ink-soft)]">{field(item, 'Notes')}</span><span class="truncate font-mono text-[10px] text-[var(--color-secondary)]">{field(item, 'SourceKind')} / {field(item, 'EvidenceLocator')}</span></div>
              {/each}
              {#if !measurements.length}<p class="px-4 py-5 text-[12px] text-[var(--color-muted)]">Trial and real-traffic evidence will appear here.</p>{/if}
            </div>
          </div>
        </div>
      {:else if !loading}
        <div class="flex min-h-[70vh] items-center justify-center text-center"><div><Telescope class="mx-auto mb-4 text-[var(--color-secondary)]" size={32}/><h1 class="text-[23px] font-semibold">No campaign selected</h1><p class="mt-2 text-[13px] text-[var(--color-muted)]">Create a campaign from a native app ref to begin.</p></div></div>
      {/if}
    </section>

    <aside class="border-t border-[var(--color-border)] bg-white p-4 lg:border-l lg:border-t-0">
      <p class="v-eyebrow mb-3">Human direction</p>
      <textarea class="h-24 w-full resize-none border border-[var(--color-border)] p-3 text-[12px] leading-5" bind:value={direction} aria-label="New direction"></textarea>
      <button class="btn-outline mt-2 flex h-9 w-full items-center justify-center gap-2 text-[12px]" onclick={() => addDirection()} disabled={!campaign || busy}><Sparkles size={14}/> Record direction</button>
      {#if proposedSelections.length}
        <div class="mt-6 border-t border-[var(--color-border)] pt-4">
          <p class="v-eyebrow mb-3">Selection proposal</p>
          {#each proposedSelections as proposal}
            <div class="mb-3 border border-amber-200 bg-amber-50 p-3">
              <p class="font-mono text-[11px]">{proposal.id}</p>
              <p class="mt-2 text-[11px] leading-4 text-amber-800">{field(proposal, 'Rationale')}</p>
              <button class="btn-primary mt-3 h-8 w-full text-[10px]" onclick={() => approveDesign(proposal)} disabled={busy}>Approve and freeze</button>
            </div>
          {/each}
        </div>
      {/if}
      <div class="mt-6 border-t border-[var(--color-border)] pt-4"><p class="v-eyebrow mb-3">Traffic sources</p>{#each traffic as item}<div class="mb-2 flex items-center justify-between border border-[var(--color-border)] p-2 text-[12px]"><span>{field(item, 'Name')}</span><span class="font-mono text-[10px] uppercase">{field(item, 'Kind')}</span></div>{/each}{#if !traffic.length}<p class="text-[12px] text-[var(--color-muted)]">None active.</p>{/if}</div>
      <div class="mt-6 border-t border-[var(--color-border)] pt-4"><div class="mb-3 flex items-center gap-2"><Sparkles size={14}/><p class="v-eyebrow">Emergent capabilities</p></div>{#each capabilities as item}<div class="mb-2 border border-[var(--color-border)] p-3"><p class="text-[12px] font-semibold">{field(item, 'Title')}</p><p class="mt-1 text-[11px] leading-4 text-[var(--color-muted)]">{field(item, 'Observation')}</p></div>{/each}{#if !capabilities.length}<p class="text-[12px] text-[var(--color-muted)]">Codex has not surfaced a capability yet.</p>{/if}</div>
      <div class="mt-6 border-t border-[var(--color-border)] pt-4">
        <p class="v-eyebrow mb-3">Interventions</p>
        <p class="mb-3 text-[12px] text-[var(--color-muted)]">{interventions.length} recorded / {candidates.length} candidate artifacts</p>
        <div class="space-y-2">
          {#each interventions.slice().reverse().slice(0, 4) as item}
            <div class="border-l-2 border-[var(--color-secondary)] pl-3">
              <p class="text-[12px] leading-5 text-[var(--color-ink-soft)]">{field(item, 'Instruction')}</p>
              <p class="mt-1 font-mono text-[10px] uppercase text-[var(--color-muted)]">{field(item, 'Kind')} / {field(item, 'RequestedBy')}</p>
            </div>
          {/each}
        </div>
      </div>
    </aside>
  </section>
</main>
