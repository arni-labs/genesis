<script lang="ts">
  import {
    Activity,
    Bot,
    Copy,
    ExternalLink,
    FlaskConical,
    GitBranch,
    Minus,
    PlayCircle,
    Plus,
    RefreshCw,
    Settings2
  } from '@lucide/svelte';
  import { Badge, Button, Input } from '$lib/components/ui';
  import type {
    ConfigureOrganismRuntimeInput,
    DirectedEvolutionSnapshot
  } from '$lib/directedEvolution';
  import type { DirectedEvolutionAppContext } from '$lib/directedEvolutionContext';

  type BadgeTone = 'neutral' | 'primary' | 'secondary' | 'accent' | 'success' | 'warning' | 'danger';
  type IconComponent = typeof Activity;

  type Props = {
    context: DirectedEvolutionAppContext;
    snapshot: DirectedEvolutionSnapshot | null;
    loading: boolean;
    error: string;
    missionControlHref: string;
    onRefresh: () => void;
    onCopy: (value: string, label: string) => void;
    onStartSimulatedUsers?: (input: { userCount: number; runsPerUser: number }) => Promise<number>;
    onStartObserver?: () => Promise<string>;
    onConfigureRuntime?: (input: ConfigureOrganismRuntimeInput) => Promise<void>;
  };

  let {
    context,
    snapshot,
    loading,
    error,
    missionControlHref,
    onRefresh,
    onCopy,
    onStartSimulatedUsers,
    onStartObserver,
    onConfigureRuntime
  }: Props = $props();

  let userCount = $state(3);
  let runsPerUser = $state(1);
  let launching = $state(false);
  let launchError = $state('');
  let launchSummary = $state('');
  let observing = $state(false);
  let observeError = $state('');
  let observeSummary = $state('');
  let runtimeBaseUrlInput = $state('');
  let runtimeTenantInput = $state('');
  let datadogServiceInput = $state('');
  let authEnvVarsInput = $state('');
  let evaluatorRefInput = $state('');
  let runtimeFormSeedKey = $state('');
  let configuringRuntime = $state(false);
  let configureError = $state('');
  let configureSummary = $state('');

  const activeWorkStatuses = new Set(['Queued', 'Claimed', 'Running']);
  const terminalRunStatuses = new Set(['Succeeded', 'Failed', 'Cancelled']);

  let organism = $derived(
    snapshot?.organisms.find((item) => item.id === context.organismId) ??
    snapshot?.organisms.find((item) => item.appRef === context.seedAppRef) ??
    snapshot?.organisms[0] ??
    null
  );

  // Seed the runtime form from the organism row once per organism;
  // periodic snapshot refreshes must not clobber in-progress edits.
  $effect(() => {
    const key = `${context.appId}:${organism?.id ?? ''}`;
    if (key === runtimeFormSeedKey) return;
    runtimeFormSeedKey = key;
    runtimeBaseUrlInput = context.runtimeBaseUrl;
    runtimeTenantInput = context.runtimeTenantId;
    datadogServiceInput = context.runtimeDatadogService;
    authEnvVarsInput = context.runtimeAuthEnvVars.join(', ');
    evaluatorRefInput = context.evaluatorRef;
  });
  let activeEpisodes = $derived(
    snapshot?.episodes.filter((episode) => !terminalRunStatuses.has(episode.status)) ?? []
  );
  let latestEpisode = $derived(activeEpisodes[0] ?? snapshot?.episodes[0] ?? null);
  let latestPlan = $derived(
    latestEpisode
      ? snapshot?.simulatedUserPlans.find((plan) => plan.id === latestEpisode.simulatedUserPlanId) ??
        snapshot?.simulatedUserPlans.find((plan) => plan.episodeId === latestEpisode.id) ??
        snapshot?.simulatedUserPlans[0] ??
        null
      : snapshot?.simulatedUserPlans[0] ?? null
  );
  let simulatedUserWorkItems = $derived(
    snapshot?.workItems.filter((item) => item.role === 'simulated_user') ?? []
  );
  let activeSimulatedUserWorkItems = $derived(
    simulatedUserWorkItems.filter((item) => activeWorkStatuses.has(item.status))
  );
  let queuedSimulatedUserWorkItems = $derived(
    simulatedUserWorkItems.filter((item) => item.status === 'Queued')
  );
  let runningSimulatedUserWorkItems = $derived(
    simulatedUserWorkItems.filter((item) => ['Claimed', 'Running'].includes(item.status))
  );
  let completedSimulatedUserWorkItems = $derived(
    simulatedUserWorkItems.filter((item) => item.status === 'Succeeded')
  );
  let failedSimulatedUserWorkItems = $derived(
    simulatedUserWorkItems.filter((item) => ['Failed', 'Cancelled'].includes(item.status))
  );
  let simulatedUserWorkItemIds = $derived(new Set(simulatedUserWorkItems.map((item) => item.id)));
  let observerWorkItems = $derived(snapshot?.workItems.filter((item) => item.role === 'observer') ?? []);
  let activeObserverWorkItems = $derived(
    observerWorkItems.filter((item) => activeWorkStatuses.has(item.status))
  );
  let latestObserverWorkItem = $derived(observerWorkItems.slice(-1)[0] ?? null);
  let simulatedUserWorkerRuns = $derived(
    snapshot?.workerRuns.filter(
      (run) => run.role === 'simulated_user' || simulatedUserWorkItemIds.has(run.workItemId)
    ) ?? []
  );
  let runningSimulatedUserWorkerRuns = $derived(
    simulatedUserWorkerRuns.filter((run) => activeWorkStatuses.has(run.status))
  );
  let simulatedUserWorkerAgents = $derived(
    snapshot?.workerAgents.filter((agent) => {
      const capabilities = agent.capabilities.toLowerCase();
      return agent.status === 'Active' && capabilities.includes('simulated_user');
    }) ?? []
  );
  let hasSimulatedUserWorker = $derived(
    simulatedUserWorkerAgents.length > 0 || runningSimulatedUserWorkerRuns.length > 0
  );
  let journeyQueueLabel = $derived(
    runningSimulatedUserWorkItems.length
      ? 'Running'
      : queuedSimulatedUserWorkItems.length
        ? hasSimulatedUserWorker
          ? 'Queued'
          : 'Waiting for worker'
        : 'Idle'
  );
  let journeyQueueTone: BadgeTone = $derived(
    runningSimulatedUserWorkItems.length
      ? 'success'
      : queuedSimulatedUserWorkItems.length
        ? hasSimulatedUserWorker
          ? 'secondary'
          : 'warning'
        : 'neutral'
  );
  let trials = $derived(snapshot?.trials ?? []);
  let completedTrials = $derived(
    trials.filter((trial) => ['Succeeded', 'Passed', 'Observed'].includes(trial.status))
  );
  let blockedTrials = $derived(
    trials.filter(
      (trial) => trial.blocker || ['Failed', 'Blocked', 'Eliminated'].includes(trial.status)
    )
  );
  let requestedPerVariant = $derived(
    latestPlan ? latestPlan.usersPerVariant * latestPlan.runsPerPersona : 0
  );
  let missionStatus = $derived(organism ? 'Ready' : 'Needs setup');
  let missionTone: BadgeTone = $derived(
    organism ? 'success' : context.configured ? 'warning' : 'neutral'
  );
  let statusTiles = $derived<
    Array<{ icon: IconComponent; label: string; value: string; detail: string }>
  >([
    {
      icon: GitBranch,
      label: 'Organism',
      value: organism?.status ?? 'None',
      detail: organism?.id ?? context.appLabel
    },
    {
      icon: PlayCircle,
      label: 'Episodes',
      value: String(snapshot?.episodes.length ?? 0),
      detail: latestEpisode ? `${latestEpisode.status} · ${latestEpisode.id}` : 'No episode'
    },
    {
      icon: Bot,
      label: 'Seed journeys',
      value: String(simulatedUserWorkItems.length),
      detail: `${queuedSimulatedUserWorkItems.length} queued · ${runningSimulatedUserWorkItems.length} running · ${completedSimulatedUserWorkItems.length} done`
    },
    {
      icon: Activity,
      label: 'Local worker',
      value: hasSimulatedUserWorker ? String(simulatedUserWorkerAgents.length || 1) : 'Offline',
      detail: hasSimulatedUserWorker
        ? simulatedUserWorkerAgents[0]?.id || 'Worker run active'
        : queuedSimulatedUserWorkItems.length
          ? 'Queued journeys are waiting'
          : 'No active worker'
    },
    {
      icon: FlaskConical,
      label: 'Trials',
      value: `${completedTrials.length}/${trials.length}`,
      detail: blockedTrials.length ? `${blockedTrials.length} blocked` : 'No blockers'
    }
  ]);
  let planMetrics = $derived([
    { label: 'Per variant', value: String(requestedPerVariant) },
    { label: 'Personas', value: String(latestPlan?.usersPerVariant ?? 0) },
    { label: 'Journeys each', value: String(latestPlan?.runsPerPersona ?? 0) }
  ]);
  let canLaunchSeedUsers = $derived(
    Boolean(onStartSimulatedUsers && organism && context.configured && context.runtimeBaseUrl)
  );
  let canStartObserver = $derived(
    Boolean(onStartObserver && organism && context.configured && context.runtimeBaseUrl)
  );
  let hasEpisodeUserPlan = $derived(Boolean(latestEpisode || latestPlan));
  let runtimeLogsHref = $derived(
    datadogLogsHref(context.runtimeDatadogService, context.runtimeTenantId)
  );
  let runtimeTracesHref = $derived(
    datadogTracesHref(context.runtimeDatadogService, context.runtimeTenantId)
  );
  let runtimeAuthLabel = $derived(
    context.runtimeAuthEnvVars.length ? context.runtimeAuthEnvVars.join(' -> ') : 'No runtime auth env'
  );

  function valueOrPending(value: string): string {
    return value || 'pending';
  }

  function clampLaunchValue(value: number, min: number, max: number): number {
    if (!Number.isFinite(value)) return min;
    return Math.max(min, Math.min(max, Math.floor(value)));
  }

  function adjustUsers(delta: number) {
    userCount = clampLaunchValue(userCount + delta, 1, 12);
  }

  function adjustRuns(delta: number) {
    runsPerUser = clampLaunchValue(runsPerUser + delta, 1, 8);
  }

  async function startSimulatedUsers() {
    if (!onStartSimulatedUsers || !canLaunchSeedUsers) return;
    launching = true;
    launchError = '';
    launchSummary = '';
    userCount = clampLaunchValue(userCount, 1, 12);
    runsPerUser = clampLaunchValue(runsPerUser, 1, 8);
    try {
      const queued = await onStartSimulatedUsers({ userCount, runsPerUser });
      launchSummary = `Queued ${queued} simulated-user journey${queued === 1 ? '' : 's'}.`;
    } catch (error) {
      launchError = error instanceof Error ? error.message : String(error);
    } finally {
      launching = false;
    }
  }

  async function saveRuntimeTarget() {
    if (!onConfigureRuntime || configuringRuntime) return;
    configuringRuntime = true;
    configureError = '';
    configureSummary = '';
    try {
      await onConfigureRuntime({
        runtimeBaseUrl: runtimeBaseUrlInput.trim(),
        runtimeTenantId: runtimeTenantInput.trim(),
        datadogService: datadogServiceInput.trim(),
        runtimeAuthEnvVars: authEnvVarsInput
          .split(',')
          .map((name) => name.trim())
          .filter(Boolean),
        evaluatorRef: evaluatorRefInput.trim()
      });
      configureSummary = 'Runtime target recorded on the organism.';
    } catch (error) {
      configureError = error instanceof Error ? error.message : String(error);
    } finally {
      configuringRuntime = false;
    }
  }

  async function startObserver() {
    if (!onStartObserver || !canStartObserver) return;
    observing = true;
    observeError = '';
    observeSummary = '';
    try {
      const workItemId = await onStartObserver();
      observeSummary = `Queued observer ${workItemId}.`;
    } catch (error) {
      observeError = error instanceof Error ? error.message : String(error);
    } finally {
      observing = false;
    }
  }

  function openMissionControl(event: MouseEvent) {
    event.preventDefault();
    window.location.assign(missionControlHref);
  }

  function journeyStatusTone(status: string): BadgeTone {
    if (['Claimed', 'Running'].includes(status)) return 'warning';
    if (status === 'Queued') return 'secondary';
    if (status === 'Succeeded') return 'success';
    if (['Failed', 'Cancelled'].includes(status)) return 'danger';
    return 'neutral';
  }

  function datadogLogsHref(service: string, tenant: string): string {
    if (!service || !tenant) return 'https://app.datadoghq.com/logs';
    const query = `service:${service} @tenant:${tenant} "app usage:" @observation_metadata:*de.app_ref*`;
    const columns = [
      'host',
      'service',
      '@tenant',
      '@entity_type',
      '@action',
      '@from_status',
      '@to_status',
      '@entity_id',
      '@observation_metadata',
      '@success'
    ].join(',');
    const params = new URLSearchParams({
      query,
      cols: columns,
      live: 'true',
      messageDisplay: 'inline',
      stream_sort: 'desc'
    });
    return `https://app.datadoghq.com/logs?${params.toString()}`;
  }

  function datadogTracesHref(service: string, tenant: string): string {
    if (!service || !tenant) return 'https://app.datadoghq.com/apm/traces';
    const query = `service:${service} @tenant:${tenant} @temper.observation.de.app_ref:*`;
    const params = new URLSearchParams({
      query,
      live: 'true'
    });
    return `https://app.datadoghq.com/apm/traces?${params.toString()}`;
  }
</script>

<section class="grid gap-3 p-3 sm:p-4">
  <div class="flex flex-wrap items-start justify-between gap-3 rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-[var(--color-surface-soft)] p-3">
    <div class="min-w-0">
      <div class="flex flex-wrap items-center gap-1.5">
        <Badge tone={missionTone} pixel={missionTone === 'success'}>{missionStatus}</Badge>
        <Badge tone="secondary">Control tenant {context.controlTenantId}</Badge>
        <Badge tone="neutral">Runtime tenant {context.runtimeTenantId || 'not configured'}</Badge>
        <Badge tone={context.runtimeBaseUrl ? 'success' : 'warning'}>
          {context.runtimeLabel}
        </Badge>
      </div>
      <h3 class="v-display mt-2 text-[18px] text-[var(--color-ink)]">
        Directed Evolution for {context.appLabel}
      </h3>
      <p class="mt-1 max-w-[78ch] font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
        Seed {context.seedAppRef}
      </p>
      <div class="mt-3 grid gap-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-[var(--color-muted)] sm:grid-cols-2 xl:grid-cols-4">
        <span class="truncate">Runtime {context.runtimeBaseUrl || 'not configured'}</span>
        <span class="truncate">Datadog service {context.runtimeDatadogService || 'not configured'}</span>
        <span class="truncate">Auth {runtimeAuthLabel}</span>
        <span class="truncate">Tenant {context.runtimeTenantId || 'not configured'}</span>
      </div>
    </div>
    <div class="flex flex-wrap items-center gap-1.5">
      <Button
        size="md"
        title="Copy control tenant"
        aria-label="Copy control tenant"
        onclick={() => onCopy(context.controlTenantId, 'Control tenant')}
      >
        <Copy size={13} />
        Copy control id
      </Button>
      <Button
        size="md"
        title="Refresh Directed Evolution state"
        aria-label="Refresh Directed Evolution state"
        disabled={loading}
        onclick={onRefresh}
      >
        <RefreshCw size={13} class={loading ? 'opacity-60' : ''} />
        {loading ? 'Refreshing' : 'Refresh state'}
      </Button>
      <a
        href={missionControlHref}
        onclick={openMissionControl}
        class="btn-base btn-primary inline-flex h-8 items-center justify-center gap-1.5 rounded-[var(--radius-sm)] px-3 text-[11px]"
      >
        <ExternalLink size={13} />
        Open Mission Control
      </a>
      {#if context.runtimeBaseUrl}
        <a
          href={context.runtimeBaseUrl}
          target="_blank"
          rel="noreferrer"
          class="btn-base btn-secondary inline-flex h-8 items-center justify-center gap-1.5 rounded-[var(--radius-sm)] px-3 text-[11px]"
        >
          <ExternalLink size={13} />
          Runtime
        </a>
      {/if}
      <a
        href={runtimeLogsHref}
        target="_blank"
        rel="noreferrer"
        class="btn-base btn-secondary inline-flex h-8 items-center justify-center gap-1.5 rounded-[var(--radius-sm)] px-3 text-[11px]"
      >
        <ExternalLink size={13} />
        Logs
      </a>
      <a
        href={runtimeTracesHref}
        target="_blank"
        rel="noreferrer"
        class="btn-base btn-secondary inline-flex h-8 items-center justify-center gap-1.5 rounded-[var(--radius-sm)] px-3 text-[11px]"
      >
        <ExternalLink size={13} />
        Traces
      </a>
    </div>
  </div>

  {#if error}
    <div class="rounded-[var(--radius-sm)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-3 py-2 text-[12px] text-[#7a1830]">
      {error}
    </div>
  {/if}

  <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-5">
    {#each statusTiles as tile (tile.label)}
      {@const TileIcon = tile.icon}
      <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white p-3">
        <div class="flex items-center justify-between gap-2">
          <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
            {tile.label}
          </p>
          <TileIcon size={14} class="text-[var(--color-primary)]" />
        </div>
        <p class="mt-2 truncate font-sans text-[20px] font-semibold tracking-tight text-[var(--color-ink)]">
          {tile.value}
        </p>
        <p class="mt-0.5 truncate text-[11.5px] text-[var(--color-muted)]">{tile.detail}</p>
      </section>
    {/each}
  </div>

  <div class="grid gap-3 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
    <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white">
      <div class="flex items-center justify-between gap-3 border-b border-[var(--color-border)] px-3 py-2">
        <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
          Runtime target
        </p>
        <Badge tone={context.configured ? 'success' : 'warning'}>
          {context.configured ? 'Configured' : 'Not configured'}
        </Badge>
      </div>
      <div class="grid gap-3 p-3">
        <p class="text-[11.5px] leading-relaxed text-[var(--color-muted)]">
          The external runtime this organism is exercised against, recorded on the Organism entity.
          Auth env var names only — secret values never enter Genesis.
        </p>
        <div class="grid gap-2 sm:grid-cols-2">
          <label class="grid gap-1">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Runtime base URL
            </span>
            <Input
              type="url"
              bind:value={runtimeBaseUrlInput}
              placeholder="https://runtime.example.app"
              aria-label="Runtime base URL"
            />
          </label>
          <label class="grid gap-1">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Runtime tenant
            </span>
            <Input
              bind:value={runtimeTenantInput}
              placeholder="app-seed"
              aria-label="Runtime tenant id"
            />
          </label>
          <label class="grid gap-1">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Datadog service
            </span>
            <Input
              bind:value={datadogServiceInput}
              placeholder="temperpaw"
              aria-label="Datadog service"
            />
          </label>
          <label class="grid gap-1">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Auth env vars (comma-separated names)
            </span>
            <Input
              bind:value={authEnvVarsInput}
              placeholder="RUNTIME_API_KEY, TEMPER_API_KEY"
              aria-label="Runtime auth env var names"
            />
          </label>
          <label class="grid gap-1 sm:col-span-2">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Evaluator ref
            </span>
            <Input
              bind:value={evaluatorRefInput}
              placeholder="owner/evaluator-app@hash"
              aria-label="Evaluator ref"
            />
          </label>
        </div>
        <Button
          variant="secondary"
          size="md"
          disabled={!onConfigureRuntime ||
            !organism ||
            configuringRuntime ||
            !runtimeBaseUrlInput.trim() ||
            !runtimeTenantInput.trim()}
          onclick={saveRuntimeTarget}
          class="w-full"
        >
          <Settings2 size={13} />
          {configuringRuntime ? 'Recording runtime target' : 'Save runtime target'}
        </Button>
        {#if !organism}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] px-2 py-1.5 text-[11.5px] text-[var(--color-muted)]">
            No organism is registered in this control tenant yet, so there is nothing to configure.
          </p>
        {/if}
        {#if configureSummary}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-success)]/30 bg-[rgba(34,197,94,0.08)] px-2 py-1.5 text-[11.5px] text-[#166534]">
            {configureSummary}
          </p>
        {/if}
        {#if configureError}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-2 py-1.5 text-[11.5px] text-[#7a1830]">
            {configureError}
          </p>
        {/if}
      </div>
    </section>

    <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white">
      <div class="flex items-center justify-between gap-3 border-b border-[var(--color-border)] px-3 py-2">
        <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
          Simulated users
        </p>
        <Badge tone={canLaunchSeedUsers ? 'success' : 'warning'}>
          {simulatedUserWorkItems.length ? journeyQueueLabel : canLaunchSeedUsers ? 'Ready' : 'Needs seed setup'}
        </Badge>
      </div>
      <div class="grid gap-3 p-3">
        <div class="grid gap-2 sm:grid-cols-2">
          <label class="grid gap-1">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Simulated users
            </span>
            <div class="grid grid-cols-[28px_minmax(0,1fr)_28px] items-center overflow-hidden rounded-[var(--radius-xs)] border border-[var(--color-border)] bg-white">
              <button
                type="button"
                class="flex h-8 items-center justify-center border-r border-[var(--color-border)] text-[var(--color-ink-soft)] hover:bg-[var(--color-surface-soft)]"
                aria-label="Decrease users"
                onclick={() => adjustUsers(-1)}
              >
                <Minus size={13} />
              </button>
              <input
                class="h-8 min-w-0 bg-transparent px-2 text-center font-mono text-[12px] text-[var(--color-ink)] outline-none"
                type="number"
                min="1"
                max="12"
                bind:value={userCount}
                onblur={() => (userCount = clampLaunchValue(userCount, 1, 12))}
              />
              <button
                type="button"
                class="flex h-8 items-center justify-center border-l border-[var(--color-border)] text-[var(--color-ink-soft)] hover:bg-[var(--color-surface-soft)]"
                aria-label="Increase users"
                onclick={() => adjustUsers(1)}
              >
                <Plus size={13} />
              </button>
            </div>
          </label>

          <label class="grid gap-1">
            <span class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
              Journeys per user
            </span>
            <div class="grid grid-cols-[28px_minmax(0,1fr)_28px] items-center overflow-hidden rounded-[var(--radius-xs)] border border-[var(--color-border)] bg-white">
              <button
                type="button"
                class="flex h-8 items-center justify-center border-r border-[var(--color-border)] text-[var(--color-ink-soft)] hover:bg-[var(--color-surface-soft)]"
                aria-label="Decrease journeys per user"
                onclick={() => adjustRuns(-1)}
              >
                <Minus size={13} />
              </button>
              <input
                class="h-8 min-w-0 bg-transparent px-2 text-center font-mono text-[12px] text-[var(--color-ink)] outline-none"
                type="number"
                min="1"
                max="8"
                bind:value={runsPerUser}
                onblur={() => (runsPerUser = clampLaunchValue(runsPerUser, 1, 8))}
              />
              <button
                type="button"
                class="flex h-8 items-center justify-center border-l border-[var(--color-border)] text-[var(--color-ink-soft)] hover:bg-[var(--color-surface-soft)]"
                aria-label="Increase journeys per user"
                onclick={() => adjustRuns(1)}
              >
                <Plus size={13} />
              </button>
            </div>
          </label>
        </div>

        <Button
          variant="primary"
          size="md"
          disabled={!canLaunchSeedUsers || launching}
          onclick={startSimulatedUsers}
          class="w-full"
        >
          <PlayCircle size={13} />
          {launching ? 'Queuing journeys' : `Launch ${userCount * runsPerUser} user journe${userCount * runsPerUser === 1 ? 'y' : 'ys'}`}
        </Button>

        {#if launchSummary}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-success)]/30 bg-[rgba(34,197,94,0.08)] px-2 py-1.5 text-[11.5px] text-[#166534]">
            {launchSummary}
          </p>
        {/if}
        {#if launchError}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-2 py-1.5 text-[11.5px] text-[#7a1830]">
            {launchError}
          </p>
        {/if}
      </div>
    </section>

    <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white">
      <div class="border-b border-[var(--color-border)] px-3 py-2">
        <div class="flex items-center justify-between gap-2">
          <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
            Observer
          </p>
          <Badge tone={activeObserverWorkItems.length ? 'warning' : latestObserverWorkItem?.status === 'Succeeded' ? 'success' : latestObserverWorkItem?.status === 'Failed' ? 'danger' : 'neutral'} pixel={activeObserverWorkItems.length > 0}>
            {activeObserverWorkItems.length ? 'Running' : latestObserverWorkItem?.status || 'Idle'}
          </Badge>
        </div>
      </div>
      <div class="grid gap-3 p-3">
        <Button
          variant="secondary"
          size="md"
          disabled={!canStartObserver || observing || activeObserverWorkItems.length > 0}
          onclick={startObserver}
          class="w-full"
        >
          <Activity size={13} />
          {observing ? 'Queuing observer' : 'Observe available sources'}
        </Button>

        {#if latestObserverWorkItem}
          <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
            <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              {latestObserverWorkItem.id}
            </p>
            <p class="mt-1 line-clamp-3 text-[12px] leading-relaxed text-[var(--color-ink-soft)]">
              {latestObserverWorkItem.summary || latestObserverWorkItem.failureReason || latestObserverWorkItem.status}
            </p>
          </div>
        {/if}

        {#if observeSummary}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-success)]/30 bg-[rgba(34,197,94,0.08)] px-2 py-1.5 text-[11.5px] text-[#166534]">
            {observeSummary}
          </p>
        {/if}
        {#if observeError}
          <p class="rounded-[var(--radius-xs)] border border-[var(--color-error)]/30 bg-[rgba(217,45,75,0.08)] px-2 py-1.5 text-[11.5px] text-[#7a1830]">
            {observeError}
          </p>
        {/if}
      </div>
    </section>

    <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white">
      <div class="border-b border-[var(--color-border)] px-3 py-2">
        <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
          Evolution episode plan
        </p>
      </div>
      {#if hasEpisodeUserPlan}
        <div class="grid gap-2 p-3">
          <div class="grid grid-cols-3 gap-2">
            {#each planMetrics as metric (metric.label)}
              <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white p-2">
                <p class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
                  {metric.label}
                </p>
                <p class="mt-1 text-[18px] font-semibold tracking-tight text-[var(--color-ink)]">
                  {metric.value}
                </p>
              </div>
            {/each}
          </div>
          <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
            <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              {valueOrPending(latestPlan?.status ?? '')}
            </p>
            <p class="mt-1 line-clamp-2 text-[12px] leading-relaxed text-[var(--color-ink-soft)]">
              {latestPlan?.humanDecisionSummary || 'No simulated-user plan recorded yet.'}
            </p>
          </div>
        </div>
      {:else}
        <div class="grid gap-2 p-3">
          <div class="grid grid-cols-3 gap-2">
            <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white p-2">
              <p class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
                Episode
              </p>
              <p class="mt-1 text-[18px] font-semibold tracking-tight text-[var(--color-ink)]">
                0
              </p>
            </div>
            <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white p-2">
              <p class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
                Variants
              </p>
              <p class="mt-1 text-[18px] font-semibold tracking-tight text-[var(--color-ink)]">
                0
              </p>
            </div>
            <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-white p-2">
              <p class="font-mono text-[9.5px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
                Trials
              </p>
              <p class="mt-1 text-[18px] font-semibold tracking-tight text-[var(--color-ink)]">
                0
              </p>
            </div>
          </div>
          <div class="rounded-[var(--radius-xs)] border border-[var(--color-border-soft)] bg-[var(--color-surface-soft)] p-2">
            <p class="truncate font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-muted)]">
              Not started
            </p>
            <p class="mt-1 text-[12px] leading-relaxed text-[var(--color-ink-soft)]">
              This appears after observation produces an evolution direction and an episode is started.
            </p>
          </div>
        </div>
      {/if}
    </section>

    <section class="rounded-[var(--radius-sm)] border border-[var(--color-border)] bg-white">
      <div class="flex items-center justify-between gap-3 border-b border-[var(--color-border)] px-3 py-2">
        <p class="font-mono text-[10px] uppercase tracking-[0.10em] text-[var(--color-faint)]">
          Journey activity
        </p>
        <Badge tone={journeyQueueTone} pixel={runningSimulatedUserWorkItems.length > 0}>
          {journeyQueueLabel}
        </Badge>
      </div>
      <div class="divide-y divide-[var(--color-border-soft)]">
        {#each simulatedUserWorkItems.slice(-8).reverse() as item (item.id)}
          <div class="grid gap-1 px-3 py-2 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center">
            <div class="min-w-0">
              <p class="truncate font-mono text-[11px] text-[var(--color-ink)]">{item.id}</p>
              <p class="truncate text-[11.5px] text-[var(--color-muted)]">
                AI user journey · {valueOrPending(item.contextRef)}
              </p>
            </div>
            <Badge tone={journeyStatusTone(item.status)}>
              {item.status}
            </Badge>
          </div>
        {:else}
          <div class="flex items-center gap-2 px-3 py-5 text-[12px] text-[var(--color-muted)]">
            <Activity size={14} />
            No simulated-user journeys yet.
          </div>
        {/each}
      </div>
    </section>
  </div>
</section>
