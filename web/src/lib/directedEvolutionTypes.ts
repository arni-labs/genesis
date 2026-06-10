import type { EntityRow, LoadWarning } from './types';

export type EntityBase = {
  id: string;
  status: string;
  raw: EntityRow;
};

export type EvolutionOrganism = EntityBase & {
  name: string;
  appRef: string;
  parentVersionId: string;
  organismVersionId: string;
  promotionId: string;
  summary: string;
  baselineEvaluation: string;
};

export type EvolutionOrganismVersion = EntityBase & {
  organismId: string;
  appRef: string;
  commitRef: string;
  promotionId: string;
  summary: string;
  newParentVersionId: string;
};

export type EvolutionLineageEdge = EntityBase & {
  organismId: string;
  parentVersionId: string;
  childVersionId: string;
  episodeId: string;
  promotionId: string;
  summary: string;
};

export type EvolutionSignal = EntityBase & {
  source: string;
  signalKind: string;
  organismId: string;
  summary: string;
  evidenceArtifactId: string;
  correlationJson: string;
  pressureId: string;
};

export type EvolutionPressure = EntityBase & {
  organismId: string;
  pressureClass: string;
  summary: string;
  signalIds: string[];
  evidenceArtifactId: string;
  workerRunId: string;
  directionId: string;
};

export type EvolutionDirection = EntityBase & {
  organismId: string;
  pressureIds: string[];
  pressureClass: string;
  title: string;
  summary: string;
  provenanceJson: string;
  autonomyLane: string;
  proposedAdaptationGoal: string;
  proposedViabilityConstraints: string[];
  workerRunId: string;
  episodeId: string;
  selectionNotes: string;
};

export type EvolutionEpisode = EntityBase & {
  hasSimulatedUserPlan: boolean;
  directionId: string;
  organismId: string;
  parentVersionId: string;
  autonomyLane: string;
  adaptationGoalId: string;
  selectionPressureId: string;
  viabilityConstraintIds: string[];
  metricDefinitionIds: string[];
  evaluationStageIds: string[];
  eliminationRuleIds: string[];
  scoringRuleIds: string[];
  simulatedUserPlanId: string;
  selectionProtocolId: string;
  evaluatorRef: string;
  plannedBy: string;
  planSummary: string;
  generationCount: number;
  startedBy: string;
  reason: string;
  winningVariantId: string;
  promotionId: string;
  organismVersionId: string;
  selectionExplanation: string;
  evidenceArtifactId: string;
  summary: string;
  failureReason: string;
};

export type EvolutionGeneration = EntityBase & {
  episodeId: string;
  parentVersionId: string;
  generationIndex: number;
  variantTargetCount: number;
  winnerVariantId: string;
  summary: string;
  failureReason: string;
};

export type EvolutionVariant = EntityBase & {
  episodeId: string;
  generationId: string;
  mutationId: string;
  appRef: string;
  branchRef: string;
  runtimeRef: string;
  summary: string;
  changedFiles: string[];
  diffPatch: string;
  workerRunId: string;
  workItemId: string;
  eliminationRuleId: string;
  stageResultId: string;
  evidenceArtifactId: string;
  reason: string;
  promotionId: string;
  organismVersionId: string;
  failureReason: string;
};

export type EvolutionPromotion = EntityBase & {
  episodeId: string;
  winningVariantId: string;
  parentVersionId: string;
  newOrganismVersionId: string;
  selectionExplanation: string;
  evidenceArtifactId: string;
  appRef: string;
  canonicalAppRef: string;
  productionTenant: string;
  runtimeRef: string;
  summary: string;
  materialized: boolean;
  materializationFailed: boolean;
  failureReason: string;
};

export type EvolutionAdaptationGoal = EntityBase & {
  episodeId: string;
  goalStatement: string;
  createdByWorkerRunId: string;
  humanNotes: string;
};

export type EvolutionViabilityConstraint = EntityBase & {
  episodeId: string;
  constraintStatement: string;
  constraintKind: string;
  createdByWorkerRunId: string;
  reason: string;
};

export type EvolutionSelectionPressure = EntityBase & {
  episodeId: string;
  selectionStatement: string;
  metricIds: string[];
  eliminationRuleIds: string[];
  scoringRuleIds: string[];
  createdByWorkerRunId: string;
};

export type EvolutionSelectionProtocol = EntityBase & {
  episodeId: string;
  selectionStatement: string;
  metricIds: string[];
  eliminationRuleIds: string[];
  scoringRuleIds: string[];
  evaluatorRef: string;
  decisionPolicy: string;
  createdByWorkerRunId: string;
  frozenBy: string;
  reason: string;
};

export type EvolutionEliminationRule = EntityBase & {
  episodeId: string;
  ruleStatement: string;
  metricIds: string[];
  thresholdJson: string;
  createdByWorkerRunId: string;
  reason: string;
};

export type EvolutionScoringRule = EntityBase & {
  episodeId: string;
  ruleStatement: string;
  metricIds: string[];
  weight: string;
  createdByWorkerRunId: string;
  reason: string;
};

export type EvolutionEvaluationStage = EntityBase & {
  episodeId: string;
  stageName: string;
  stageKind: string;
  sequenceIndex: number;
  requiredEvidence: string[];
  executorKind: string;
  measurementProvenance: string;
  evaluatorRef: string;
  evaluatorModule: string;
  decisionAuthority: string;
};

export type EvolutionStageResult = EntityBase & {
  episodeId: string;
  generationId: string;
  variantId: string;
  evaluationStageId: string;
  workItemId: string;
  metricsJson: string;
  evidenceArtifactId: string;
  summary: string;
  failureReason: string;
  evaluatorRole: string;
  provenanceKind: string;
  decisionBasisJson: string;
  inputsJson: string;
  eliminationRuleId: string;
  reason: string;
};

export type EvolutionSimulatedUserPlan = EntityBase & {
  episodeId: string;
  usersPerVariant: number;
  runsPerPersona: number;
  personasJson: string;
  goalsJson: string;
  createdBy: string;
  humanDecisionSummary: string;
  frozenBy: string;
  reason: string;
};

export type EvolutionMetricDefinition = EntityBase & {
  episodeId: string;
  metricName: string;
  unit: string;
  metricKind: string;
  source: string;
  desiredDirection: string;
  higherIsBetter: string;
  description: string;
  provenanceKind: string;
  evaluatorRef: string;
  evaluatorModule: string;
  interpretation: string;
  hardConstraint: string;
};

export type EvolutionMeasurement = EntityBase & {
  metricDefinitionId: string;
  stageResultId: string;
  trialId: string;
  variantId: string;
  value: string;
  unit: string;
  evidenceArtifactId: string;
  provenanceKind: string;
  measurementKind: string;
  sourceRunId: string;
  computedByRef: string;
  interpretation: string;
};

export type EvolutionMutation = EntityBase & {
  variantId: string;
  summary: string;
  changedFiles: string[];
  diffRef: string;
  diffPatch: string;
  workerRunId: string;
  reason: string;
};

export type EvolutionEvidenceArtifact = EntityBase & {
  artifactKind: string;
  uri: string;
  summary: string;
  correlationJson: string;
  digest: string;
  query: string;
  timeWindow: string;
  resultCount: string;
  interpretation: string;
  zeroResultMeaning: string;
  evidenceProvenance: string;
  targetEntityType: string;
  targetEntityId: string;
};

export type EvolutionTrial = EntityBase & {
  episodeId: string;
  generationId: string;
  variantId: string;
  evaluationStageId: string;
  stageResultId: string;
  simulatedUserPlanId: string;
  simulatedUserId: string;
  personaJson: string;
  goal: string;
  workItemId: string;
  journeyJson: string;
  observationJson: string;
  intentSatisfied: string;
  frictionJson: string;
  blocker: string;
  summary: string;
  measurementsJson: string;
  failureReason: string;
  evidenceArtifactId: string;
};

export type EvolutionAutonomyPolicy = EntityBase & {
  organismId: string;
  policyJson: string;
  createdBy: string;
  updatedBy: string;
  summary: string;
};

export type EvolutionWorkItem = EntityBase & {
  role: string;
  targetEntityType: string;
  targetEntityId: string;
  promptRef: string;
  contextRef: string;
  outputSchemaRef: string;
  correlationJson: string;
  workerId: string;
  workerRunId: string;
  resultJson: string;
  summary: string;
  failureReason: string;
};

export type EvolutionWorkerRun = EntityBase & {
  role: string;
  workItemId: string;
  agentKind: string;
  model: string;
  outputJson: string;
  summary: string;
  failureReason: string;
  correlationJson: string;
};

export type EvolutionWorkerAgent = EntityBase & {
  capabilities: string;
  lastSeenAt: string;
  statusSummary: string;
};

export type DirectedEvolutionSnapshot = {
  organisms: EvolutionOrganism[];
  organismVersions: EvolutionOrganismVersion[];
  lineageEdges: EvolutionLineageEdge[];
  signals: EvolutionSignal[];
  pressures: EvolutionPressure[];
  directions: EvolutionDirection[];
  episodes: EvolutionEpisode[];
  generations: EvolutionGeneration[];
  variants: EvolutionVariant[];
  promotions: EvolutionPromotion[];
  adaptationGoals: EvolutionAdaptationGoal[];
  viabilityConstraints: EvolutionViabilityConstraint[];
  selectionPressures: EvolutionSelectionPressure[];
  selectionProtocols: EvolutionSelectionProtocol[];
  eliminationRules: EvolutionEliminationRule[];
  scoringRules: EvolutionScoringRule[];
  evaluationStages: EvolutionEvaluationStage[];
  stageResults: EvolutionStageResult[];
  simulatedUserPlans: EvolutionSimulatedUserPlan[];
  metricDefinitions: EvolutionMetricDefinition[];
  measurements: EvolutionMeasurement[];
  mutations: EvolutionMutation[];
  evidenceArtifacts: EvolutionEvidenceArtifact[];
  trials: EvolutionTrial[];
  autonomyPolicies: EvolutionAutonomyPolicy[];
  workItems: EvolutionWorkItem[];
  workerRuns: EvolutionWorkerRun[];
  workerAgents: EvolutionWorkerAgent[];
  warnings: LoadWarning[];
};
