import { writable, get } from 'svelte/store';
import { loadAppFiles, loadRegistrySnapshot } from './api';
import type { AppFilesSnapshot, RegistryApp, RegistrySnapshot } from './types';

type RegistryState = {
  snapshot: RegistrySnapshot | null;
  loading: boolean;
  error: string;
  loadedAt: number;
};

const STALE_MS = 5 * 60 * 1000;

export const registryStore = writable<RegistryState>({
  snapshot: null,
  loading: false,
  error: '',
  loadedAt: 0
});

let inflight: Promise<void> | null = null;

export async function loadRegistry(force = false): Promise<void> {
  const state = get(registryStore);
  const fresh = state.snapshot && Date.now() - state.loadedAt < STALE_MS;
  if (fresh && !force) {
    return;
  }
  if (inflight && !force) {
    return inflight;
  }

  inflight = (async () => {
    registryStore.update((s) => ({ ...s, loading: true, error: '' }));
    try {
      const snapshot = await loadRegistrySnapshot();
      registryStore.set({
        snapshot,
        loading: false,
        error: '',
        loadedAt: Date.now()
      });
    } catch (error) {
      registryStore.update((s) => ({
        ...s,
        loading: false,
        error: error instanceof Error ? error.message : String(error)
      }));
    } finally {
      inflight = null;
    }
  })();

  return inflight;
}

const filesCache = new Map<string, AppFilesSnapshot>();

export async function loadAppFilesCached(app: RegistryApp): Promise<AppFilesSnapshot> {
  const key = `${app.id}:${app.repositoryId}:${app.latestVersionHash}`;
  const cached = filesCache.get(key);
  if (cached) {
    return cached;
  }
  const snapshot = await loadAppFiles(app);
  filesCache.set(key, snapshot);
  return snapshot;
}

export function findAppById(
  snapshot: RegistrySnapshot | null,
  id: string
): RegistryApp | null {
  if (!snapshot) {
    return null;
  }
  return snapshot.apps.find((app) => app.id === id) ?? null;
}
