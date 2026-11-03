// RateLimit policies.
//
// Buckets are guardrail state. In operator mode the tenant operator manages
// them; commons middleware consumes tokens in commons mode.

permit(
  principal is Agent,
  action in [
    Action::"create", Action::"Create", Action::"read", Action::"list",
    Action::"Consume", Action::"Refill"
  ],
  resource is RateLimit
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};
