// Owner policies.
//
// In operator mode the tenant operator manages registry owner accounts.
// Commons mode layers the ownership and verification overlays in
// policies/commons/owner.cedar.

permit(
  principal is Agent,
  action in [
    Action::"create", Action::"Create", Action::"read", Action::"list",
    Action::"RequestVerification", Action::"MarkVerified", Action::"UpdateProfile",
    Action::"SetQuota", Action::"Suspend", Action::"Unsuspend"
  ],
  resource is Owner
) when {
  principal has agent_type && principal.agent_type == "operator" &&
  principal has agentTypeVerified && principal.agentTypeVerified == true
};
