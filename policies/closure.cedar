// Closure policies.
//
// Closures are content-addressed lockfile rows. The tenant operator manages
// them; commons mode adds per-owner rate limits.

permit(
  principal is Agent,
  action in [
    Action::"create", Action::"Create", Action::"read", Action::"list",
    Action::"ResolveClosure"
  ],
  resource is Closure
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};
