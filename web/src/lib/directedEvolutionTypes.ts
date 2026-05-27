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
  brainRunId: string;
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
  brainRunId: string;
  episodeId: string;
  selectionNotes: string;
};

export type EvolutionEpisode = EntityBase & {
  directionId: string;
  organismId: string;
  parentVersionId: string;
  autonomyLane: string;
  adaptationGoalId: string;
  selectionPressureId: string;
  viabilityConstraintIds: string[];
  evaluationStageIds: string[];
  generationCount: number;
  winningVariantId: string;
  promotionId: string;
  organismVersionId: string;
  selectionExplanation: string;
  summary: string;
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
  brainRunId: string;
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
  createdByBrainRunId: string;
  humanNotes: string;
};

export type EvolutionViabilityConstraint = EntityBase & {
  episodeId: string;
  constraintStatement: string;
  constraintKind: string;
  createdByBrainRunId: string;
  reason: string;
};

export type EvolutionSelectionPressure = EntityBase & {
  episodeId: string;
  selectionStatement: string;
  metricIds: string[];
  eliminationRuleIds: string[];
  scoringRuleIds: string[];
  createdByBrainRunId: string;
};

export type EvolutionEvaluationStage = EntityBase & {
  episodeId: string;
  stageName: string;
  stageKind: string;
  sequenceIndex: number;
  requiredEvidence: string[];
  executorKind: string;
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
  eliminationRuleId: string;
  reason: string;
};

export type EvolutionMetricDefinition = EntityBase & {
  episodeId: string;
  metricName: string;
  unit: string;
  metricKind: string;
  source: string;
  desiredDirection: string;
};

export type EvolutionMeasurement = EntityBase & {
  metricDefinitionId: string;
  stageResultId: string;
  trialId: string;
  variantId: string;
  value: string;
  unit: string;
  evidenceArtifactId: string;
};

export type EvolutionEvidenceArtifact = EntityBase & {
  artifactKind: string;
  uri: string;
  summary: string;
  correlationJson: string;
  digest: string;
  targetEntityType: string;
  targetEntityId: string;
};

export type EvolutionTrial = EntityBase & {
  episodeId: string;
  generationId: string;
  variantId: string;
  simulatedUserBrainRunId: string;
  runtimeRef: string;
  goalJson: string;
  resultJson: string;
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
  brainRunId: string;
  resultJson: string;
  summary: string;
  failureReason: string;
};

export type EvolutionBrainRun = EntityBase & {
  role: string;
  workItemId: string;
  agentKind: string;
  model: string;
  outputJson: string;
  summary: string;
  failureReason: string;
  correlationJson: string;
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
  evaluationStages: EvolutionEvaluationStage[];
  stageResults: EvolutionStageResult[];
  metricDefinitions: EvolutionMetricDefinition[];
  measurements: EvolutionMeasurement[];
  evidenceArtifacts: EvolutionEvidenceArtifact[];
  trials: EvolutionTrial[];
  autonomyPolicies: EvolutionAutonomyPolicy[];
  workItems: EvolutionWorkItem[];
  brainRuns: EvolutionBrainRun[];
  warnings: LoadWarning[];
};
