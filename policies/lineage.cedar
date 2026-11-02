// Lineage policies.
//
// Lineage rows are metadata sidecars for forks and mutation summaries. App.Fork
// writes them as its declared sub-write; the tenant operator manages them.

permit(principal, action == Action::"Create", resource is Lineage) when {
  principal has action_context &&
  principal.action_context == "composite:App.Fork"
};

permit(
  principal is Agent,
  action in [
    Action::"create", Action::"Create", Action::"read", Action::"list",
    Action::"AddMutation"
  ],
  resource is Lineage
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};
