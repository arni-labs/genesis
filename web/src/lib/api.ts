import type {
  AppFilesSnapshot,
  AgentAnswer,
  AgentQuestion,
  ClaimOwnerInput,
  Closure,
  CreateCampaignInput,
  EntityRow,
  EvolutionCampaign,
  EvolutionItem,
  EvolutionSnapshot,
  GitBlob,
  GitCommit,
  GitTree,
  Lineage,
  LoadWarning,
  Owner,
  RepositoryFile,
  RegistryApp,
  RegistrySnapshot
} from './types';

const API_BASE = (import.meta.env.VITE_TEMPER_API_BASE ?? '').replace(/\/$/, '');
const TENANT_ID = import.meta.env.VITE_TEMPER_TENANT_ID ?? 'default';

type CollectionName =
  | 'Apps'
  | 'Owners'
  | 'Lineages'
  | 'Closures'
  | 'Commits'
  | 'Trees'
  | 'Blobs'
  | 'Campaigns'
  | 'SelectionDesigns'
  | 'Generations'
  | 'Candidates'
  | 'Measurements'
  | 'TrafficSources'
  | 'EmergentCapabilities'
  | 'Interventions'
  | 'Questions'
  | 'Answers';

type Principal = {
  id?: string;
  kind?: string;
};

function apiPath(path: string): string {
  return `${API_BASE}${path}`;
}

function snakeKey(key: string): string {
  return key.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase();
}

function baseHeaders(principal: Principal = {}, withBody = false): HeadersInit {
  const headers: Record<string, string> = {
    Accept: 'application/json',
    'X-Tenant-Id': TENANT_ID
  };
  if (withBody) {
    headers['Content-Type'] = 'application/json';
  }
  if (principal.id) {
    headers['X-Temper-Principal-Id'] = principal.id;
  }
  if (principal.kind) {
    headers['X-Temper-Principal-Kind'] = principal.kind;
  }
  return headers;
}

async function responseError(response: Response): Promise<Error> {
  const fallback = `${response.status} ${response.statusText}`;
  try {
    const json = await response.json();
    const message =
      stringValue(json?.error?.message) ??
      stringValue(json?.message) ??
      JSON.stringify(json);
    return new Error(message || fallback);
  } catch {
    try {
      const text = await response.text();
      return new Error(text || fallback);
    } catch {
      return new Error(fallback);
    }
  }
}

async function requestJson<T>(
  path: string,
  init: RequestInit = {},
  principal: Principal = {}
): Promise<T> {
  const response = await fetch(apiPath(path), {
    ...init,
    headers: {
      ...baseHeaders(principal, init.body !== undefined),
      ...(init.headers ?? {})
    }
  });
  if (!response.ok) {
    throw await responseError(response);
  }
  return (await response.json()) as T;
}

async function listCollection(collection: CollectionName, query = ''): Promise<EntityRow[]> {
  const suffix = query ? `?${query}` : '';
  const body = await requestJson<{ value?: EntityRow[] }>(`/tdata/${collection}${suffix}`);
  return Array.isArray(body.value) ? body.value : [];
}

async function loadCollection<T>(
  collection: CollectionName,
  normalizer: (row: EntityRow) => T
): Promise<{ value: T[]; warning?: LoadWarning }> {
  try {
    const rows = await listCollection(collection);
    return { value: rows.map(normalizer) };
  } catch (error) {
    return {
      value: [],
      warning: {
        collection,
        message: error instanceof Error ? error.message : String(error)
      }
    };
  }
}

export async function loadRegistrySnapshot(): Promise<RegistrySnapshot> {
  const [apps, owners, lineages, closures] = await Promise.all([
    loadCollection('Apps', normalizeApp),
    loadCollection('Owners', normalizeOwner),
    loadCollection('Lineages', normalizeLineage),
    loadCollection('Closures', normalizeClosure)
  ]);

  return {
    apps: apps.value.filter((app) => app.ownerId && app.repositoryId),
    owners: owners.value,
    lineages: lineages.value,
    closures: closures.value,
    warnings: [apps.warning, owners.warning, lineages.warning, closures.warning].filter(
      Boolean
    ) as LoadWarning[]
  };
}

export async function loadEvolutionSnapshot(): Promise<EvolutionSnapshot> {
  const [campaigns, selectionDesigns, generations, candidates, measurements, trafficSources, capabilities, interventions] =
    await Promise.all([
      loadCollection('Campaigns', normalizeCampaign),
      loadCollection('SelectionDesigns', normalizeEvolutionItem),
      loadCollection('Generations', normalizeEvolutionItem),
      loadCollection('Candidates', normalizeEvolutionItem),
      loadCollection('Measurements', normalizeEvolutionItem),
      loadCollection('TrafficSources', normalizeEvolutionItem),
      loadCollection('EmergentCapabilities', normalizeEvolutionItem),
      loadCollection('Interventions', normalizeEvolutionItem)
    ]);
  const results = [campaigns, selectionDesigns, generations, candidates, measurements, trafficSources, capabilities, interventions];
  return {
    campaigns: campaigns.value,
    selectionDesigns: selectionDesigns.value,
    generations: generations.value,
    candidates: candidates.value,
    measurements: measurements.value,
    trafficSources: trafficSources.value,
    capabilities: capabilities.value,
    interventions: interventions.value,
    warnings: results.flatMap((result) => result.warning ? [result.warning] : [])
  };
}

export async function createCampaign(input: CreateCampaignInput): Promise<void> {
  await requestJson(`/tdata/Campaigns`, { method: 'POST', body: JSON.stringify({ Id: input.id }) });
  await evolutionAction('Campaigns', input.id, 'Configure', {
    name: input.name,
    director_brief: input.directorBrief,
    target_app_ref: input.targetAppRef,
    brain_provider: 'codex',
    automation_mode: 'automatic_release'
  });
}

export async function evolutionAction(
  collection: string,
  id: string,
  action: string,
  params: Record<string, unknown> = {}
): Promise<void> {
  await nativeAction('Genesis.Evolution', collection, id, action, params);
}

async function nativeAction(
  namespace: string,
  collection: string,
  id: string,
  action: string,
  params: Record<string, unknown> = {}
): Promise<void> {
  await requestJson(`/tdata/${collection}('${encodeURIComponent(id)}')/${namespace}.${action}`, {
    method: 'POST',
    body: JSON.stringify(params)
  });
}

export async function recordIntervention(campaignId: string, instruction: string): Promise<void> {
  const id = `intervention-${Date.now()}`;
  await requestJson('/tdata/Interventions', { method: 'POST', body: JSON.stringify({ Id: id }) });
  await evolutionAction('Interventions', id, 'Configure', {
    campaign_id: campaignId,
    kind: 'direction',
    instruction,
    requested_by: 'human'
  });
}

export async function loadAgentAnswers(): Promise<{ questions: AgentQuestion[]; answers: AgentAnswer[] }> {
  const [questions, answers] = await Promise.all([listCollection('Questions' as CollectionName), listCollection('Answers' as CollectionName)]);
  return {
    questions: questions.map((row) => ({
      id: stateStringField(row, 'Id'), status: stateStringField(row, 'Status'), title: stateStringField(row, 'Title'), body: stateStringField(row, 'Body'), askedBy: stateStringField(row, 'AskedBy'), answerCount: numberField(row, 'AnswerCount'), acceptedAnswerId: stateStringField(row, 'AcceptedAnswerId')
    })),
    answers: answers.map((row) => ({
      id: stateStringField(row, 'Id'), status: stateStringField(row, 'Status'), questionId: stateStringField(row, 'QuestionId'), body: stateStringField(row, 'Body'), answeredBy: stateStringField(row, 'AnsweredBy'), evidence: stateStringField(row, 'Evidence')
    }))
  };
}

export async function askQuestion(title: string, body: string, askedBy: string): Promise<void> {
  const id = `question-${Date.now()}`;
  await requestJson('/tdata/Questions', { method: 'POST', body: JSON.stringify({ Id: id }) });
  await nativeAction('Genesis.AgentAnswers', 'Questions', id, 'Configure', { title, body, asked_by: askedBy, created_at: new Date().toISOString() });
}

export async function submitAnswer(questionId: string, body: string, answeredBy: string, evidence: string): Promise<void> {
  const id = `answer-${Date.now()}`;
  await requestJson('/tdata/Answers', { method: 'POST', body: JSON.stringify({ Id: id }) });
  await nativeAction('Genesis.AgentAnswers', 'Answers', id, 'Submit', { question_id: questionId, body, answered_by: answeredBy, evidence, created_at: new Date().toISOString() });
  await nativeAction('Genesis.AgentAnswers', 'Questions', questionId, 'RecordAnswer');
}

export async function acceptAnswer(questionId: string, answerId: string): Promise<void> {
  await nativeAction('Genesis.AgentAnswers', 'Answers', answerId, 'Accept');
  await nativeAction('Genesis.AgentAnswers', 'Questions', questionId, 'Accept', { accepted_answer_id: answerId });
}

function normalizeCampaign(row: EntityRow): EvolutionCampaign {
  return {
    id: stateStringField(row, 'Id'),
    status: stateStringField(row, 'Status') || 'Draft',
    name: stateStringField(row, 'Name'),
    directorBrief: stateStringField(row, 'DirectorBrief'),
    targetAppRef: stateStringField(row, 'TargetAppRef'),
    activeSelectionDesignId: stateStringField(row, 'ActiveSelectionDesignId'),
    activeEvaluatorRef: stateStringField(row, 'ActiveEvaluatorRef'),
    currentReleaseRef: stateStringField(row, 'CurrentReleaseRef'),
    previousReleaseRef: stateStringField(row, 'PreviousReleaseRef'),
    generationCount: numberField(row, 'GenerationCount'),
    automationMode: stateStringField(row, 'AutomationMode'),
    brainProvider: stateStringField(row, 'BrainProvider'),
    pauseReason: stateStringField(row, 'PauseReason'),
    lastReleaseReason: stateStringField(row, 'LastReleaseReason')
  };
}

function normalizeEvolutionItem(row: EntityRow): EvolutionItem {
  return {
    id: stateStringField(row, 'Id'),
    status: stateStringField(row, 'Status'),
    fields: (row.fields ?? row) as Record<string, unknown>
  };
}

export async function createOwner(input: ClaimOwnerInput): Promise<Owner> {
  const accountId = input.accountId.trim();
  const verificationProvider = input.verificationProvider.trim() || 'email_magic_link';
  const verificationSubject =
    input.verificationSubject.trim() || input.contact.trim() || accountId;
  const now = new Date().toISOString();
  const body = {
    Id: accountId,
    AccountId: accountId,
    DisplayName: input.displayName.trim() || accountId,
    Contact: input.contact.trim(),
    StorageCapBytes: 104_857_600,
    RateLimitTier: 'free',
    PublicKey: '',
    VerificationProvider: verificationProvider,
    VerificationSubject: verificationSubject,
    VerificationRequestedAt: now
  };

  const row = await requestJson<EntityRow>(
    '/tdata/Owners',
    {
      method: 'POST',
      body: JSON.stringify(body)
    },
    { id: accountId, kind: 'customer' }
  );
  return normalizeOwner(row);
}

export async function loadAppFiles(app: RegistryApp): Promise<AppFilesSnapshot> {
  if (!app.repositoryId || !app.latestVersionHash) {
    return {
      appId: app.id,
      repositoryId: app.repositoryId,
      commitHash: app.latestVersionHash,
      commit: null,
      versions: [],
      files: []
    };
  }

  const repositoryFilter = encodeURIComponent(
    `RepositoryId eq '${app.repositoryId.replace(/'/g, "''")}'`
  );
  const query = `$filter=${repositoryFilter}&$top=5000`;
  const [commitRows, treeRows, blobRows] = await Promise.all([
    listCollection('Commits', query),
    listCollection('Trees', query),
    listCollection('Blobs', query)
  ]);

  const commits = commitRows.map(normalizeCommit);
  const versions = orderCommitsForLatest(commits, app.latestVersionHash);
  const trees = treeRows.map(normalizeTree);
  const blobs = blobRows.map(normalizeBlob);
  const commit =
    commits.find((item) => item.id === app.latestVersionHash) ??
    commits.find((item) => item.treeSha) ??
    null;

  return {
    appId: app.id,
    repositoryId: app.repositoryId,
    commitHash: app.latestVersionHash,
    commit,
    versions,
    files: commit ? buildRepositoryFiles(commit.treeSha, trees, blobs) : []
  };
}

export function field(row: EntityRow | undefined, ...keys: string[]): unknown {
  if (!row) {
    return undefined;
  }
  const sources = [row, row.fields].filter((source): source is Record<string, unknown> => {
    return Boolean(source && typeof source === 'object');
  });

  for (const key of keys) {
    if (key === 'Id' && typeof row.entity_id === 'string') {
      return row.entity_id;
    }
    if (key === 'Status' && typeof row.status === 'string') {
      return row.status;
    }
    for (const source of sources) {
      if (Object.prototype.hasOwnProperty.call(source, key)) {
        return source[key];
      }
      const lowerKey = key.charAt(0).toLowerCase() + key.slice(1);
      if (Object.prototype.hasOwnProperty.call(source, lowerKey)) {
        return source[lowerKey];
      }
      const snake = snakeKey(key);
      if (Object.prototype.hasOwnProperty.call(source, snake)) {
        return source[snake];
      }
    }
  }

  return undefined;
}

export function stringField(row: EntityRow | undefined, ...keys: string[]): string {
  return stringValue(field(row, ...keys)) ?? '';
}

export function stringValue(value: unknown): string | undefined {
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  if (value === null || value === undefined) {
    return undefined;
  }
  return JSON.stringify(value);
}

function stateStringField(row: EntityRow | undefined, ...keys: string[]): string {
  if (!row) {
    return '';
  }

  const sources = [row.fields, row].filter((source): source is Record<string, unknown> => {
    return Boolean(source && typeof source === 'object');
  });

  for (const key of keys) {
    for (const source of sources) {
      if (Object.prototype.hasOwnProperty.call(source, key)) {
        return stringValue(source[key]) ?? '';
      }
      const lowerKey = key.charAt(0).toLowerCase() + key.slice(1);
      if (Object.prototype.hasOwnProperty.call(source, lowerKey)) {
        return stringValue(source[lowerKey]) ?? '';
      }
      const snake = snakeKey(key);
      if (Object.prototype.hasOwnProperty.call(source, snake)) {
        return stringValue(source[snake]) ?? '';
      }
    }
  }

  return '';
}

function numberField(row: EntityRow, ...keys: string[]): number {
  const value = field(row, ...keys);
  if (typeof value === 'number') {
    return value;
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function normalizeApp(row: EntityRow): RegistryApp {
  return {
    id: stringField(row, 'Id'),
    ownerId: stringField(row, 'OwnerId'),
    name: stringField(row, 'Name') || stringField(row, 'Id'),
    repositoryId: stringField(row, 'RepositoryId'),
    latestVersionHash: stringField(row, 'LatestVersionHash'),
    exports: stringField(row, 'Exports'),
    description: stringField(row, 'Description'),
    visibility: stringField(row, 'Visibility') || 'public',
    status: stringField(row, 'Status') || 'Active',
    createdAt: stringField(row, 'CreatedAt'),
    updatedAt: stringField(row, 'UpdatedAt'),
    raw: row
  };
}

function normalizeOwner(row: EntityRow): Owner {
  return {
    id: stringField(row, 'Id'),
    accountId: stringField(row, 'AccountId'),
    displayName: stringField(row, 'DisplayName') || stringField(row, 'Id'),
    contact: stringField(row, 'Contact'),
    storageCapBytes: numberField(row, 'StorageCapBytes'),
    rateLimitTier: stringField(row, 'RateLimitTier') || 'free',
    verificationProvider: stringField(row, 'VerificationProvider'),
    verificationSubject: stringField(row, 'VerificationSubject'),
    verifiedAt: stringField(row, 'VerifiedAt'),
    status: stringField(row, 'Status') || 'PendingVerification',
    raw: row
  };
}

function normalizeLineage(row: EntityRow): Lineage {
  return {
    id: stringField(row, 'Id'),
    childRepositoryId: stringField(row, 'ChildRepositoryId'),
    parentRepositoryId: stringField(row, 'ParentRepositoryId'),
    parentCommit: stringField(row, 'ParentCommit'),
    type: stringField(row, 'Type') || 'fork',
    createdBy: stringField(row, 'CreatedBy'),
    mutations: stringField(row, 'Mutations'),
    status: stringField(row, 'Status') || 'Active',
    createdAt: stringField(row, 'CreatedAt'),
    raw: row
  };
}

function normalizeClosure(row: EntityRow): Closure {
  return {
    id: stringField(row, 'Id'),
    root: stringField(row, 'Root'),
    resolved: stringField(row, 'Resolved'),
    resolverVersion: stringField(row, 'ResolverVersion'),
    resolvedAt: stringField(row, 'ResolvedAt'),
    resolvedBy: stringField(row, 'ResolvedBy'),
    status: stringField(row, 'Status') || 'Durable',
    raw: row
  };
}

function normalizeCommit(row: EntityRow): GitCommit {
  return {
    id: stateStringField(row, 'Id'),
    repositoryId: stateStringField(row, 'RepositoryId'),
    treeSha: stateStringField(row, 'TreeSha'),
    parentShas: stateStringField(row, 'ParentShas'),
    author: stateStringField(row, 'Author'),
    committer: stateStringField(row, 'Committer'),
    message: stateStringField(row, 'Message'),
    createdAt: stateStringField(row, 'CreatedAt'),
    raw: row
  };
}

function normalizeTree(row: EntityRow): GitTree {
  return {
    id: stateStringField(row, 'Id'),
    repositoryId: stateStringField(row, 'RepositoryId'),
    canonicalBytes: stateStringField(row, 'CanonicalBytes'),
    raw: row
  };
}

function normalizeBlob(row: EntityRow): GitBlob {
  return {
    id: stateStringField(row, 'Id'),
    repositoryId: stateStringField(row, 'RepositoryId'),
    content: stateStringField(row, 'Content'),
    size: numberField(row, 'Size'),
    createdAt: stateStringField(row, 'CreatedAt'),
    raw: row
  };
}

function orderCommitsForLatest(commits: GitCommit[], latestHash: string): GitCommit[] {
  const byId = new Map(commits.map((commit) => [commit.id, commit]));
  const visited = new Set<string>();
  const ordered: GitCommit[] = [];

  let cursor = latestHash;
  while (cursor && byId.has(cursor) && !visited.has(cursor)) {
    const commit = byId.get(cursor)!;
    ordered.push(commit);
    visited.add(cursor);
    cursor = parseParentShas(commit.parentShas)[0] ?? '';
  }

  const remaining = commits
    .filter((commit) => !visited.has(commit.id))
    .sort((a, b) => {
      const bDate = Date.parse(b.createdAt);
      const aDate = Date.parse(a.createdAt);
      if (Number.isFinite(bDate) && Number.isFinite(aDate) && bDate !== aDate) {
        return bDate - aDate;
      }
      return b.id.localeCompare(a.id);
    });

  return [...ordered, ...remaining];
}

function parseParentShas(value: string): string[] {
  const trimmed = value.trim();
  if (!trimmed) {
    return [];
  }

  try {
    const parsed = JSON.parse(trimmed);
    if (Array.isArray(parsed)) {
      return parsed.filter((item): item is string => typeof item === 'string' && item.length > 0);
    }
  } catch {
    // Git parent lists from older projections are stored as plain strings.
  }

  return trimmed
    .split(/[,\s]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function buildRepositoryFiles(
  rootTreeSha: string,
  trees: GitTree[],
  blobs: GitBlob[]
): RepositoryFile[] {
  const treeById = new Map(trees.map((tree) => [tree.id, tree]));
  const blobById = new Map(blobs.map((blob) => [blob.id, blob]));
  const files: RepositoryFile[] = [];
  const visitedTrees = new Set<string>();

  function walk(treeSha: string, parentPath: string) {
    if (!treeSha || visitedTrees.has(treeSha)) {
      return;
    }
    visitedTrees.add(treeSha);

    for (const entry of parseCanonicalTree(treeById.get(treeSha)?.canonicalBytes ?? '')) {
      const path = parentPath ? `${parentPath}/${entry.name}` : entry.name;
      const kind = treeEntryKind(entry.mode);
      const parent = parentPath;
      const blob = blobById.get(entry.objectSha);
      const decoded = blob ? decodeBlobPreview(blob.content) : { preview: '', isBinary: false };
      files.push({
        path,
        name: entry.name,
        parentPath: parent,
        kind,
        mode: entry.mode,
        objectSha: entry.objectSha,
        size: blob?.size ?? 0,
        preview: decoded.preview,
        isBinary: decoded.isBinary
      });

      if (kind === 'directory') {
        walk(entry.objectSha, path);
      }
    }
  }

  walk(rootTreeSha, '');
  return files.sort((a, b) => a.path.localeCompare(b.path));
}

function parseCanonicalTree(
  canonicalBytes: string
): Array<{ mode: string; name: string; objectSha: string }> {
  const bytes = decodeBase64Bytes(canonicalBytes);
  if (!bytes.length) {
    return [];
  }
  const bodyStart = bytes.indexOf(0) + 1;
  if (bodyStart <= 0 || bodyStart >= bytes.length) {
    return [];
  }

  const decoder = new TextDecoder();
  const entries: Array<{ mode: string; name: string; objectSha: string }> = [];
  let offset = bodyStart;

  while (offset < bytes.length) {
    const modeStart = offset;
    while (offset < bytes.length && bytes[offset] !== 32) {
      offset += 1;
    }
    if (offset >= bytes.length) {
      break;
    }
    const mode = decoder.decode(bytes.slice(modeStart, offset));
    offset += 1;

    const nameStart = offset;
    while (offset < bytes.length && bytes[offset] !== 0) {
      offset += 1;
    }
    if (offset >= bytes.length) {
      break;
    }
    const name = decoder.decode(bytes.slice(nameStart, offset));
    offset += 1;

    if (offset + 20 > bytes.length) {
      break;
    }
    const objectSha = [...bytes.slice(offset, offset + 20)]
      .map((byte) => byte.toString(16).padStart(2, '0'))
      .join('');
    offset += 20;
    entries.push({ mode, name, objectSha });
  }

  return entries;
}

function treeEntryKind(mode: string): RepositoryFile['kind'] {
  if (mode === '40000' || mode === '040000') {
    return 'directory';
  }
  if (mode === '120000') {
    return 'symlink';
  }
  if (mode === '160000') {
    return 'submodule';
  }
  return 'file';
}

function decodeBlobPreview(content: string): { preview: string; isBinary: boolean } {
  const bytes = decodeBase64Bytes(content);
  if (!bytes.length) {
    return { preview: '', isBinary: false };
  }
  const isBinary = bytes.some((byte) => byte === 0);
  if (isBinary) {
    return { preview: '', isBinary: true };
  }
  return {
    preview: new TextDecoder('utf-8').decode(bytes.slice(0, 32_000)),
    isBinary: false
  };
}

function decodeBase64Bytes(value: string): Uint8Array {
  if (!value) {
    return new Uint8Array();
  }
  try {
    const binary = atob(value);
    const bytes = new Uint8Array(binary.length);
    for (let index = 0; index < binary.length; index += 1) {
      bytes[index] = binary.charCodeAt(index);
    }
    return bytes;
  } catch {
    return new Uint8Array();
  }
}

export function parseJsonList(value: string): string[] {
  if (!value.trim()) {
    return [];
  }
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) {
      return parsed.map((item) => {
        if (typeof item === 'string') {
          return item;
        }
        if (item && typeof item === 'object') {
          const record = item as Record<string, unknown>;
          const type = stringValue(record.type);
          const summary = stringValue(record.summary);
          if (type && summary) {
            return `${type}: ${summary}`;
          }
        }
        return JSON.stringify(item);
      });
    }
    if (parsed && typeof parsed === 'object') {
      return Object.entries(parsed).map(([key, item]) => `${key}: ${stringValue(item) ?? ''}`);
    }
  } catch {
    return value
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean);
  }
  return [value];
}

export function parseJsonMap(value: string): Array<[string, string]> {
  if (!value.trim()) {
    return [];
  }
  try {
    const parsed = JSON.parse(value);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return Object.entries(parsed).map(([key, item]) => [key, stringValue(item) ?? '']);
    }
  } catch {
    return [];
  }
  return [];
}
