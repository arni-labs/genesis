import type { EvolutionOrganism } from './directedEvolutionTypes';
import type { RegistryApp } from './types';

export type DirectedEvolutionAppContext = {
  appId: string;
  appLabel: string;
  controlTenantId: string;
  organismId: string;
  runtimeTenantId: string;
  runtimeBaseUrl: string;
  runtimeLabel: string;
  runtimeDatadogService: string;
  runtimeAuthEnvVars: string[];
  evaluatorRef: string;
  seedAppRef: string;
  configured: boolean;
};

function slug(value: string): string {
  return (
    value
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'app'
  );
}

export function organismRuntimeConfigured(organism: EvolutionOrganism | null): boolean {
  return Boolean(organism?.runtimeBaseUrl && organism?.runtimeTenantId);
}

export function organismForApp(
  organisms: EvolutionOrganism[],
  app: RegistryApp
): EvolutionOrganism | null {
  const refPrefix = `${app.ownerId}/${app.name}@`;
  return organisms.find((organism) => organism.appRef.startsWith(refPrefix)) ?? organisms[0] ?? null;
}

// ADR-0026: the runtime target lives on the Organism entity. The app
// only determines labels and the deterministic control tenant; runtime
// base URL, tenant, Datadog service, auth env var names, and the
// evaluator ref come from the loaded organism row.
export function directedEvolutionContextForApp(
  app: RegistryApp,
  organism: EvolutionOrganism | null = null
): DirectedEvolutionAppContext {
  const owner = slug(app.ownerId);
  const name = slug(app.name || app.id);
  const seedHash = app.latestVersionHash || app.id;
  const configured = organismRuntimeConfigured(organism);
  return {
    appId: app.id,
    appLabel: app.name || app.id,
    controlTenantId: `de-${owner}-${name}`,
    organismId: organism?.id ?? '',
    runtimeTenantId: organism?.runtimeTenantId ?? '',
    runtimeBaseUrl: organism?.runtimeBaseUrl ?? '',
    runtimeLabel: configured ? 'Runtime configured' : 'Unconfigured runtime',
    runtimeDatadogService: organism?.datadogService ?? '',
    runtimeAuthEnvVars: organism?.runtimeAuthEnvVars ?? [],
    evaluatorRef: organism?.evaluatorRef ?? '',
    seedAppRef: organism?.appRef || `${app.ownerId}/${app.name}@${seedHash}`,
    configured
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
