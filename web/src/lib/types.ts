export type EntityRow = Record<string, unknown> & {
  fields?: Record<string, unknown>;
  entity_id?: string;
  status?: string;
};

export type LoadWarning = {
  collection: string;
  message: string;
};

export type RegistryApp = {
  id: string;
  ownerId: string;
  name: string;
  repositoryId: string;
  latestVersionHash: string;
  exports: string;
  description: string;
  visibility: string;
  status: string;
  createdAt: string;
  updatedAt: string;
  raw: EntityRow;
};

export type GitCommit = {
  id: string;
  repositoryId: string;
  treeSha: string;
  parentShas: string;
  author: string;
  committer: string;
  message: string;
  createdAt: string;
  raw: EntityRow;
};

export type GitTree = {
  id: string;
  repositoryId: string;
  canonicalBytes: string;
  raw: EntityRow;
};

export type GitBlob = {
  id: string;
  repositoryId: string;
  content: string;
  size: number;
  createdAt: string;
  raw: EntityRow;
};

export type RepositoryFile = {
  path: string;
  name: string;
  parentPath: string;
  kind: 'directory' | 'file' | 'symlink' | 'submodule';
  mode: string;
  objectSha: string;
  size: number;
  preview: string;
  isBinary: boolean;
};

export type AppFilesSnapshot = {
  appId: string;
  repositoryId: string;
  commitHash: string;
  commit: GitCommit | null;
  versions: GitCommit[];
  files: RepositoryFile[];
};

export type Owner = {
  id: string;
  accountId: string;
  displayName: string;
  contact: string;
  storageCapBytes: number;
  rateLimitTier: string;
  verificationProvider: string;
  verificationSubject: string;
  verifiedAt: string;
  status: string;
  raw: EntityRow;
};

export type Lineage = {
  id: string;
  childRepositoryId: string;
  parentRepositoryId: string;
  parentCommit: string;
  type: string;
  createdBy: string;
  mutations: string;
  status: string;
  createdAt: string;
  raw: EntityRow;
};

export type Closure = {
  id: string;
  root: string;
  resolved: string;
  resolverVersion: string;
  resolvedAt: string;
  resolvedBy: string;
  status: string;
  raw: EntityRow;
};

export type RegistrySnapshot = {
  apps: RegistryApp[];
  owners: Owner[];
  lineages: Lineage[];
  closures: Closure[];
  warnings: LoadWarning[];
};

export type ClaimOwnerInput = {
  accountId: string;
  displayName: string;
  contact: string;
  verificationProvider: string;
  verificationSubject: string;
};

export type EvolutionCampaign = {
  id: string;
  status: string;
  name: string;
  directorBrief: string;
  targetAppRef: string;
  activeSelectionDesignId: string;
  activeEvaluatorRef: string;
  currentReleaseRef: string;
  previousReleaseRef: string;
  generationCount: number;
  automationMode: string;
  brainProvider: string;
  pauseReason: string;
  lastReleaseReason: string;
};

export type EvolutionItem = {
  id: string;
  status: string;
  fields: Record<string, unknown>;
};

export type EvolutionSnapshot = {
  campaigns: EvolutionCampaign[];
  selectionDesigns: EvolutionItem[];
  generations: EvolutionItem[];
  candidates: EvolutionItem[];
  measurements: EvolutionItem[];
  trafficSources: EvolutionItem[];
  capabilities: EvolutionItem[];
  interventions: EvolutionItem[];
  warnings: LoadWarning[];
};

export type CreateCampaignInput = {
  id: string;
  name: string;
  directorBrief: string;
  targetAppRef: string;
};

export type AgentQuestion = {
  id: string;
  status: string;
  title: string;
  body: string;
  askedBy: string;
  answerCount: number;
  acceptedAnswerId: string;
};

export type AgentAnswer = {
  id: string;
  status: string;
  questionId: string;
  body: string;
  answeredBy: string;
  evidence: string;
};
