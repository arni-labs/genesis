permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Freeze", Action::"Retire"], resource is TrialSuite);
permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Freeze", Action::"Retire"], resource is MetricDefinition);
permit(principal, action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Pass", Action::"Fail"], resource is ValidatorRun);
