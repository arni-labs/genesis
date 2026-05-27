import {
  listEntityCollection,
  parseJsonList,
  postEntityAction,
  stringField
} from './api';
import type {
  DirectedEvolutionSnapshot,
  EntityBase,
  EvolutionAdaptationGoal,
  EvolutionAutonomyPolicy,
  EvolutionBrainRun,
  EvolutionDirection,
  EvolutionEpisode,
  EvolutionEvaluationStage,
  EvolutionEvidenceArtifact,
  EvolutionGeneration,
  EvolutionLineageEdge,
  EvolutionMeasurement,
  EvolutionMetricDefinition,
  EvolutionOrganism,
  EvolutionOrganismVersion,
  EvolutionPressure,
  EvolutionPromotion,
  EvolutionSelectionPressure,
  EvolutionSignal,
  EvolutionStageResult,
  EvolutionTrial,
  EvolutionVariant,
  EvolutionViabilityConstraint,
  EvolutionWorkItem
} from './directedEvolutionTypes';
import type { EntityRow, LoadWarning } from './types';
export { jsonEntries } from './directedEvolutionFormatting';
export type * from './directedEvolutionTypes';

const DIRECTED_EVOLUTION_NAMESPACE = 'Temper.DirectedEvolution';
const COLLECTION_TOP = '$top=500';

type CollectionResult<T> = {
  value: T[];
  warning?: LoadWarning;
};

async function loadDirectedCollection<T>(
  collection: string,
  normalizer: (row: EntityRow) => T,
  tenantId?: string
): Promise<CollectionResult<T>> {
  try {
    const rows = await listEntityCollection(collection, COLLECTION_TOP, tenantId);
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

export async function loadDirectedEvolutionSnapshot(
  tenantId?: string
): Promise<DirectedEvolutionSnapshot> {
  const [
    organisms,
    organismVersions,
    lineageEdges,
    signals,
    pressures,
    directions,
    episodes,
    generations,
    variants,
    promotions,
    adaptationGoals,
    viabilityConstraints,
    selectionPressures,
    evaluationStages,
    stageResults,
    metricDefinitions,
    measurements,
    evidenceArtifacts,
    trials,
    autonomyPolicies,
    workItems,
    brainRuns
  ] = await Promise.all([
    loadDirectedCollection('Organisms', normalizeOrganism, tenantId),
    loadDirectedCollection('OrganismVersions', normalizeOrganismVersion, tenantId),
    loadDirectedCollection('LineageEdges', normalizeLineageEdge, tenantId),
    loadDirectedCollection('Signals', normalizeSignal, tenantId),
    loadDirectedCollection('Pressures', normalizePressure, tenantId),
    loadDirectedCollection('Directions', normalizeDirection, tenantId),
    loadDirectedCollection('Episodes', normalizeEpisode, tenantId),
    loadDirectedCollection('Generations', normalizeGeneration, tenantId),
    loadDirectedCollection('Variants', normalizeVariant, tenantId),
    loadDirectedCollection('Promotions', normalizePromotion, tenantId),
    loadDirectedCollection('AdaptationGoals', normalizeAdaptationGoal, tenantId),
    loadDirectedCollection('ViabilityConstraints', normalizeViabilityConstraint, tenantId),
    loadDirectedCollection('SelectionPressures', normalizeSelectionPressure, tenantId),
    loadDirectedCollection('EvaluationStages', normalizeEvaluationStage, tenantId),
    loadDirectedCollection('StageResults', normalizeStageResult, tenantId),
    loadDirectedCollection('MetricDefinitions', normalizeMetricDefinition, tenantId),
    loadDirectedCollection('Measurements', normalizeMeasurement, tenantId),
    loadDirectedCollection('EvidenceArtifacts', normalizeEvidenceArtifact, tenantId),
    loadDirectedCollection('Trials', normalizeTrial, tenantId),
    loadDirectedCollection('AutonomyPolicies', normalizeAutonomyPolicy, tenantId),
    loadDirectedCollection('WorkItems', normalizeWorkItem, tenantId),
    loadDirectedCollection('BrainRuns', normalizeBrainRun, tenantId)
  ]);

  return {
    organisms: organisms.value,
    organismVersions: organismVersions.value,
    lineageEdges: lineageEdges.value,
    signals: signals.value,
    pressures: pressures.value,
    directions: directions.value,
    episodes: episodes.value,
    generations: generations.value,
    variants: variants.value,
    promotions: promotions.value,
    adaptationGoals: adaptationGoals.value,
    viabilityConstraints: viabilityConstraints.value,
    selectionPressures: selectionPressures.value,
    evaluationStages: evaluationStages.value,
    stageResults: stageResults.value,
    metricDefinitions: metricDefinitions.value,
    measurements: measurements.value,
    evidenceArtifacts: evidenceArtifacts.value,
    trials: trials.value,
    autonomyPolicies: autonomyPolicies.value,
    workItems: workItems.value,
    brainRuns: brainRuns.value,
    warnings: [
      organisms.warning,
      organismVersions.warning,
      lineageEdges.warning,
      signals.warning,
      pressures.warning,
      directions.warning,
      episodes.warning,
      generations.warning,
      variants.warning,
      promotions.warning,
      adaptationGoals.warning,
      viabilityConstraints.warning,
      selectionPressures.warning,
      evaluationStages.warning,
      stageResults.warning,
      metricDefinitions.warning,
      measurements.warning,
      evidenceArtifacts.warning,
      trials.warning,
      autonomyPolicies.warning,
      workItems.warning,
      brainRuns.warning
    ].filter(Boolean) as LoadWarning[]
  };
}

export function resumeEpisode(
  id: string,
  reason = 'Resumed from Genesis Mission Control',
  tenantId?: string
) {
  return directedAction('Episodes', id, 'ResumeEpisode', { Reason: reason }, tenantId);
}

export function pauseEpisode(
  id: string,
  reason = 'Paused from Genesis Mission Control',
  tenantId?: string
) {
  return directedAction('Episodes', id, 'PauseEpisode', { Reason: reason }, tenantId);
}

export function stopEpisode(
  id: string,
  reason = 'Stopped from Genesis Mission Control',
  tenantId?: string
) {
  return directedAction('Episodes', id, 'StopEpisode', { Reason: reason }, tenantId);
}

export function dismissDirection(
  id: string,
  reason = 'Dismissed from Genesis Mission Control',
  tenantId?: string
) {
  return directedAction('Directions', id, 'DismissDirection', { Reason: reason }, tenantId);
}

export function pinViabilityConstraint(
  id: string,
  reason = 'Pinned from Genesis Mission Control',
  tenantId?: string
) {
  return directedAction(
    'ViabilityConstraints',
    id,
    'PinViabilityConstraint',
    {
      PinnedBy: 'genesis-mission-control',
      Reason: reason
    },
    tenantId
  );
}

function directedAction(
  collection: string,
  id: string,
  action: string,
  body: Record<string, unknown>,
  tenantId?: string
) {
  return postEntityAction(
    collection,
    id,
    DIRECTED_EVOLUTION_NAMESPACE,
    action,
    body,
    { id: 'genesis-mission-control', kind: 'agent', agentType: 'human' },
    tenantId
  );
}

function base(row: EntityRow): EntityBase {
  return {
    id: stringField(row, 'Id'),
    status: stringField(row, 'Status') || 'Recorded',
    raw: row
  };
}

function normalizeOrganism(row: EntityRow): EvolutionOrganism {
  return {
    ...base(row),
    name: stringField(row, 'Name'),
    appRef: stringField(row, 'AppRef'),
    parentVersionId: stringField(row, 'ParentVersionId'),
    baselineEvaluation: stringField(row, 'BaselineEvaluationJson')
  };
}

function normalizeOrganismVersion(row: EntityRow): EvolutionOrganismVersion {
  return {
    ...base(row),
    organismId: stringField(row, 'OrganismId'),
    appRef: stringField(row, 'AppRef'),
    commitRef: stringField(row, 'CommitRef'),
    promotionId: stringField(row, 'PromotionId'),
    summary: stringField(row, 'Summary'),
    newParentVersionId: stringField(row, 'NewParentVersionId')
  };
}

function normalizeLineageEdge(row: EntityRow): EvolutionLineageEdge {
  return {
    ...base(row),
    organismId: stringField(row, 'OrganismId'),
    parentVersionId: stringField(row, 'ParentVersionId'),
    childVersionId: stringField(row, 'ChildVersionId'),
    episodeId: stringField(row, 'EpisodeId'),
    promotionId: stringField(row, 'PromotionId'),
    summary: stringField(row, 'Summary')
  };
}

function normalizeSignal(row: EntityRow): EvolutionSignal {
  return {
    ...base(row),
    source: stringField(row, 'Source'),
    signalKind: stringField(row, 'SignalKind'),
    organismId: stringField(row, 'OrganismId'),
    summary: stringField(row, 'Summary'),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId'),
    correlationJson: stringField(row, 'CorrelationJson'),
    pressureId: stringField(row, 'PressureId')
  };
}

function normalizePressure(row: EntityRow): EvolutionPressure {
  return {
    ...base(row),
    organismId: stringField(row, 'OrganismId'),
    pressureClass: stringField(row, 'PressureClass'),
    summary: stringField(row, 'Summary'),
    signalIds: parseJsonList(stringField(row, 'SignalIdsJson')),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId'),
    brainRunId: stringField(row, 'BrainRunId'),
    directionId: stringField(row, 'DirectionId')
  };
}

function normalizeDirection(row: EntityRow): EvolutionDirection {
  return {
    ...base(row),
    organismId: stringField(row, 'OrganismId'),
    pressureIds: parseJsonList(stringField(row, 'PressureIdsJson')),
    pressureClass: stringField(row, 'PressureClass'),
    title: stringField(row, 'Title'),
    summary: stringField(row, 'Summary'),
    provenanceJson: stringField(row, 'ProvenanceJson'),
    autonomyLane: stringField(row, 'AutonomyLane'),
    proposedAdaptationGoal: stringField(row, 'ProposedAdaptationGoal'),
    proposedViabilityConstraints: parseJsonList(
      stringField(row, 'ProposedViabilityConstraintsJson')
    ),
    brainRunId: stringField(row, 'BrainRunId'),
    episodeId: stringField(row, 'EpisodeId'),
    selectionNotes: stringField(row, 'SelectionNotes')
  };
}

function normalizeEpisode(row: EntityRow): EvolutionEpisode {
  return {
    ...base(row),
    directionId: stringField(row, 'DirectionId'),
    organismId: stringField(row, 'OrganismId'),
    parentVersionId: stringField(row, 'ParentVersionId'),
    autonomyLane: stringField(row, 'AutonomyLane'),
    adaptationGoalId: stringField(row, 'AdaptationGoalId'),
    selectionPressureId: stringField(row, 'SelectionPressureId'),
    viabilityConstraintIds: parseJsonList(stringField(row, 'ViabilityConstraintIdsJson')),
    evaluationStageIds: parseJsonList(stringField(row, 'EvaluationStageIdsJson')),
    generationCount: numberField(row, 'generation_count', 'GenerationCount'),
    winningVariantId: stringField(row, 'WinningVariantId'),
    promotionId: stringField(row, 'PromotionId'),
    organismVersionId: stringField(row, 'OrganismVersionId'),
    selectionExplanation: stringField(row, 'SelectionExplanation'),
    summary: stringField(row, 'Summary')
  };
}

function normalizeGeneration(row: EntityRow): EvolutionGeneration {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    parentVersionId: stringField(row, 'ParentVersionId'),
    generationIndex: numberField(row, 'GenerationIndex'),
    variantTargetCount: numberField(row, 'VariantTargetCount'),
    winnerVariantId: stringField(row, 'WinnerVariantId'),
    summary: stringField(row, 'Summary'),
    failureReason: stringField(row, 'FailureReason')
  };
}

function normalizeVariant(row: EntityRow): EvolutionVariant {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    generationId: stringField(row, 'GenerationId'),
    mutationId: stringField(row, 'MutationId'),
    appRef: stringField(row, 'AppRef'),
    branchRef: stringField(row, 'BranchRef'),
    runtimeRef: stringField(row, 'RuntimeRef'),
    summary: stringField(row, 'Summary'),
    brainRunId: stringField(row, 'BrainRunId'),
    workItemId: stringField(row, 'WorkItemId'),
    eliminationRuleId: stringField(row, 'EliminationRuleId'),
    stageResultId: stringField(row, 'StageResultId'),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId'),
    reason: stringField(row, 'Reason'),
    promotionId: stringField(row, 'PromotionId'),
    organismVersionId: stringField(row, 'OrganismVersionId'),
    failureReason: stringField(row, 'FailureReason')
  };
}

function normalizePromotion(row: EntityRow): EvolutionPromotion {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    winningVariantId: stringField(row, 'WinningVariantId'),
    parentVersionId: stringField(row, 'ParentVersionId'),
    newOrganismVersionId: stringField(row, 'NewOrganismVersionId'),
    selectionExplanation: stringField(row, 'SelectionExplanation'),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId'),
    appRef: stringField(row, 'AppRef'),
    canonicalAppRef: stringField(row, 'CanonicalAppRef'),
    productionTenant: stringField(row, 'ProductionTenant'),
    runtimeRef: stringField(row, 'RuntimeRef'),
    summary: stringField(row, 'Summary'),
    materialized: booleanField(row, 'materialized', 'Materialized'),
    materializationFailed: booleanField(row, 'materialization_failed', 'MaterializationFailed'),
    failureReason: stringField(row, 'FailureReason')
  };
}

function normalizeAdaptationGoal(row: EntityRow): EvolutionAdaptationGoal {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    goalStatement: stringField(row, 'GoalStatement'),
    createdByBrainRunId: stringField(row, 'CreatedByBrainRunId'),
    humanNotes: stringField(row, 'HumanNotes')
  };
}

function normalizeViabilityConstraint(row: EntityRow): EvolutionViabilityConstraint {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    constraintStatement: stringField(row, 'ConstraintStatement'),
    constraintKind: stringField(row, 'ConstraintKind'),
    createdByBrainRunId: stringField(row, 'CreatedByBrainRunId'),
    reason: stringField(row, 'Reason')
  };
}

function normalizeSelectionPressure(row: EntityRow): EvolutionSelectionPressure {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    selectionStatement: stringField(row, 'SelectionStatement'),
    metricIds: parseJsonList(stringField(row, 'MetricIdsJson')),
    eliminationRuleIds: parseJsonList(stringField(row, 'EliminationRuleIdsJson')),
    scoringRuleIds: parseJsonList(stringField(row, 'ScoringRuleIdsJson')),
    createdByBrainRunId: stringField(row, 'CreatedByBrainRunId')
  };
}

function normalizeEvaluationStage(row: EntityRow): EvolutionEvaluationStage {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    stageName: stringField(row, 'StageName'),
    stageKind: stringField(row, 'StageKind'),
    sequenceIndex: numberField(row, 'SequenceIndex'),
    requiredEvidence: parseJsonList(stringField(row, 'RequiredEvidenceJson')),
    executorKind: stringField(row, 'ExecutorKind')
  };
}

function normalizeStageResult(row: EntityRow): EvolutionStageResult {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    generationId: stringField(row, 'GenerationId'),
    variantId: stringField(row, 'VariantId'),
    evaluationStageId: stringField(row, 'EvaluationStageId'),
    workItemId: stringField(row, 'WorkItemId'),
    metricsJson: stringField(row, 'MetricsJson'),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId'),
    summary: stringField(row, 'Summary'),
    failureReason: stringField(row, 'FailureReason'),
    eliminationRuleId: stringField(row, 'EliminationRuleId'),
    reason: stringField(row, 'Reason')
  };
}

function normalizeMetricDefinition(row: EntityRow): EvolutionMetricDefinition {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    metricName: stringField(row, 'MetricName'),
    unit: stringField(row, 'Unit'),
    metricKind: stringField(row, 'MetricKind'),
    source: stringField(row, 'Source'),
    desiredDirection: stringField(row, 'DesiredDirection')
  };
}

function normalizeMeasurement(row: EntityRow): EvolutionMeasurement {
  return {
    ...base(row),
    metricDefinitionId: stringField(row, 'MetricDefinitionId'),
    stageResultId: stringField(row, 'StageResultId'),
    trialId: stringField(row, 'TrialId'),
    variantId: stringField(row, 'VariantId'),
    value: stringField(row, 'Value'),
    unit: stringField(row, 'Unit'),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId')
  };
}

function normalizeEvidenceArtifact(row: EntityRow): EvolutionEvidenceArtifact {
  return {
    ...base(row),
    artifactKind: stringField(row, 'ArtifactKind'),
    uri: stringField(row, 'Uri'),
    summary: stringField(row, 'Summary'),
    correlationJson: stringField(row, 'CorrelationJson'),
    digest: stringField(row, 'Digest'),
    targetEntityType: stringField(row, 'TargetEntityType'),
    targetEntityId: stringField(row, 'TargetEntityId')
  };
}

function normalizeTrial(row: EntityRow): EvolutionTrial {
  return {
    ...base(row),
    episodeId: stringField(row, 'EpisodeId'),
    generationId: stringField(row, 'GenerationId'),
    variantId: stringField(row, 'VariantId'),
    simulatedUserBrainRunId: stringField(row, 'SimulatedUserBrainRunId'),
    runtimeRef: stringField(row, 'RuntimeRef'),
    goalJson: stringField(row, 'GoalJson'),
    resultJson: stringField(row, 'ResultJson'),
    evidenceArtifactId: stringField(row, 'EvidenceArtifactId')
  };
}

function normalizeAutonomyPolicy(row: EntityRow): EvolutionAutonomyPolicy {
  return {
    ...base(row),
    organismId: stringField(row, 'OrganismId'),
    policyJson: stringField(row, 'PolicyJson'),
    createdBy: stringField(row, 'CreatedBy'),
    updatedBy: stringField(row, 'UpdatedBy'),
    summary: stringField(row, 'Summary')
  };
}

function normalizeWorkItem(row: EntityRow): EvolutionWorkItem {
  return {
    ...base(row),
    role: stringField(row, 'Role'),
    targetEntityType: stringField(row, 'TargetEntityType'),
    targetEntityId: stringField(row, 'TargetEntityId'),
    promptRef: stringField(row, 'PromptRef'),
    contextRef: stringField(row, 'ContextRef'),
    outputSchemaRef: stringField(row, 'OutputSchemaRef'),
    correlationJson: stringField(row, 'CorrelationJson'),
    workerId: stringField(row, 'WorkerId'),
    brainRunId: stringField(row, 'BrainRunId'),
    resultJson: stringField(row, 'ResultJson'),
    summary: stringField(row, 'Summary'),
    failureReason: stringField(row, 'FailureReason')
  };
}

function normalizeBrainRun(row: EntityRow): EvolutionBrainRun {
  return {
    ...base(row),
    role: stringField(row, 'Role'),
    workItemId: stringField(row, 'WorkItemId'),
    agentKind: stringField(row, 'AgentKind'),
    model: stringField(row, 'Model'),
    outputJson: stringField(row, 'OutputJson'),
    summary: stringField(row, 'Summary'),
    failureReason: stringField(row, 'FailureReason'),
    correlationJson: stringField(row, 'CorrelationJson')
  };
}

function numberField(row: EntityRow, ...keys: string[]): number {
  for (const key of keys) {
    const value = stringField(row, key);
    if (value) {
      const parsed = Number(value);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return 0;
}

function booleanField(row: EntityRow, ...keys: string[]): boolean {
  for (const key of keys) {
    const value = stringField(row, key);
    if (value) {
      return value === 'true' || value === '1';
    }
    const booleans = row.booleans;
    const raw =
      (booleans && typeof booleans === 'object'
        ? (booleans as Record<string, unknown>)[key]
        : undefined) ?? row.fields?.[key];
    if (typeof raw === 'boolean') {
      return raw;
    }
  }
  return false;
}
