import type { RegistryApp } from './types';

export type DirectedEvolutionAppContext = {
  appId: string;
  appLabel: string;
  controlTenantId: string;
  runtimeTenantId: string;
  runtimeBaseUrl: string;
  runtimeLabel: string;
  runtimeDatadogService: string;
  runtimeAuthEnvVars: string[];
  runtimeTraceResourceNames: string[];
  seedAppRef: string;
  seedHash: string;
  configured: boolean;
};

const agentAnswersSeedHash = '4001f5fce94de5557af3b31c17d160cc0e69fbed';
const agentAnswersRuntimeBaseUrl = (
  import.meta.env.VITE_AGENT_ANSWERS_RUNTIME_BASE_URL ??
  import.meta.env.VITE_TEMPERPAW_RUNTIME_BASE_URL ??
  'https://openpaw-production.up.railway.app'
).replace(/\/$/, '');

function slug(value: string): string {
  return (
    value
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'app'
  );
}

export function directedEvolutionContextForApp(
  app: RegistryApp
): DirectedEvolutionAppContext {
  if (app.id === 'app-nerdsane-agent-answers') {
    return {
      appId: app.id,
      appLabel: 'Agent Answers',
      controlTenantId: 'de-agent-answers',
      runtimeTenantId: 'agent-answers-seed',
      runtimeBaseUrl: agentAnswersRuntimeBaseUrl,
      runtimeLabel: 'TemperPaw production',
      runtimeDatadogService: 'temperpaw',
      runtimeAuthEnvVars: ['TEMPERPAW_RUNTIME_API_KEY', 'TEMPER_API_KEY'],
      runtimeTraceResourceNames: [
        'Question.Configure',
        'Answer.Submit',
        'Question.RecordAnswer',
        'Question.Accept'
      ],
      seedAppRef: `nerdsane/agent-answers@${agentAnswersSeedHash}`,
      seedHash: agentAnswersSeedHash,
      configured: true
    };
  }

  const owner = slug(app.ownerId);
  const name = slug(app.name || app.id);
  const seedHash = app.latestVersionHash || app.id;
  return {
    appId: app.id,
    appLabel: app.name || app.id,
    controlTenantId: `de-${owner}-${name}`,
    runtimeTenantId: `${owner}-${name}-seed`,
    runtimeBaseUrl: '',
    runtimeLabel: 'Unconfigured runtime',
    runtimeDatadogService: '',
    runtimeAuthEnvVars: [],
    runtimeTraceResourceNames: [],
    seedAppRef: `${app.ownerId}/${app.name}@${seedHash}`,
    seedHash,
    configured: false
  };
}

export function directedEvolutionHref(
  context: DirectedEvolutionAppContext,
  basePath: string
): string {
  const params = new URLSearchParams({
    tenant: context.controlTenantId,
    app: context.appId,
    runtimeTenant: context.runtimeTenantId,
    runtimeBase: context.runtimeBaseUrl,
    runtimeService: context.runtimeDatadogService
  });
  return `${basePath}/evolution?${params.toString()}`;
}
