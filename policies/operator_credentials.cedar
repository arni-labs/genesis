// The tenant operator may revoke and rotate agent credentials, next to the
// kernel's operator permit to create, read, list, define and issue them.

permit(
  principal is Agent,
  action in [Action::"Revoke", Action::"Rotate"],
  resource is AgentCredential
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};
