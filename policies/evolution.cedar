// Directed-evolution engine — authorization (see ADR-0011).
//
// Phase 1 is permissive: the engine (principal "system") and human director
// drive the loop freely. The per-episode autonomy slider (Cedar gates on
// Frame/Select/Approve keyed on resource.Autonomy) lands in Phase 2.

permit(principal, action, resource is Evolution);
permit(principal, action, resource is Variant);
permit(principal, action, resource is FitnessSpec);
permit(principal, action, resource is StageResult);
