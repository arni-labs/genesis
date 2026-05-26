// Directed Evolution is controlled by the installed campaign operator. The
// app records decisions as entities; deployments can layer stricter identity
// policy without altering the protocol.
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"ApproveSelection", Action::"Start", Action::"RecordRelease", Action::"Pause", Action::"Resume", Action::"Rollback", Action::"Complete", Action::"Fail"],
  resource is Campaign
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Approve", Action::"Freeze", Action::"Retire"],
  resource is SelectionDesign
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Begin", Action::"SelectAndRelease", Action::"Fail"],
  resource is Generation
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"StartTrials", Action::"Assess", Action::"Select", Action::"Eliminate", Action::"Release", Action::"RollBack"],
  resource is Candidate
);

permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Record"], resource is Measurement);
permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Activate", Action::"Pause", Action::"Retire"], resource is TrafficSource);
permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Keep", Action::"Reject"], resource is EmergentCapability);
permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Apply"], resource is Intervention);
