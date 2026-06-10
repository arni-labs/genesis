// Paw Orchestration OS App Cedar policies.

// --- WorkerProvider / WorkerAgent ---

permit(principal is Admin, action, resource);
permit(principal is System, action, resource);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource
) when {
  resource is WorkerProvider || resource is WorkerAgent
};

permit(
  principal,
  action in [Action::"Register", Action::"Activate", Action::"Disable", Action::"MarkUnhealthy"],
  resource
) when {
  (resource is WorkerProvider || resource is WorkerAgent) &&
  ["supervisor", "human"].contains(principal.agent_type) &&
  context.agentTypeVerified == true
};

permit(
  principal is Agent,
  action,
  resource
) when {
  (resource is WorkerAgent || resource is WorkItem || resource is WorkerRun) &&
  (
    principal.agent_type == "worker" ||
    (["supervisor", "human"].contains(principal.agent_type) && context.agentTypeVerified == true)
  )
};

// --- WorkItem / WorkerRun ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource
) when {
  resource is WorkItem || resource is WorkerRun
};

permit(
  principal,
  action in [Action::"QueueWorkItem", Action::"CancelWorkItem"],
  resource is WorkItem
);

permit(
  principal is Agent,
  action in [Action::"ClaimWorkItem", Action::"StartWorkItem", Action::"SucceedWorkItem", Action::"FailWorkItem"],
  resource is WorkItem
) when {
  principal.agent_type == "worker" ||
  (["supervisor", "human"].contains(principal.agent_type) && context.agentTypeVerified == true)
};

permit(
  principal is Agent,
  action in [Action::"StartWorkerRun", Action::"SucceedWorkerRun", Action::"FailWorkerRun", Action::"CancelWorkerRun"],
  resource is WorkerRun
) when {
  principal.agent_type == "worker" ||
  (["supervisor", "human"].contains(principal.agent_type) && context.agentTypeVerified == true)
};

// --- HeartbeatRun ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is HeartbeatRun
);

permit(
  principal,
  action in [Action::"Schedule", Action::"ApproveBudget"],
  resource is HeartbeatRun
) when {
  ["supervisor", "human"].contains(principal.agent_type) &&
  context.agentTypeVerified == true
};

permit(
  principal,
  action in [Action::"CheckIn", Action::"StartExecution", Action::"RecordTurn", Action::"SaveCheckpoint", Action::"RecordResult", Action::"Complete"],
  resource is HeartbeatRun
) when {
  resource.AgentId == principal.id
};

permit(
  principal,
  action in [Action::"Fail", Action::"Cancel"],
  resource is HeartbeatRun
) when {
  resource.AgentId == principal.id || (["supervisor", "human"].contains(principal.agent_type) && context.agentTypeVerified == true)
};

// --- Organization ---

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Organization
);

permit(
  principal,
  action in [Action::"Configure", Action::"Activate", Action::"AddMember", Action::"RemoveMember", Action::"ResetBudgetCycle", Action::"Pause", Action::"Resume", Action::"Archive"],
  resource is Organization
) when {
  ["supervisor", "human"].contains(principal.agent_type) &&
  context.agentTypeVerified == true
};

permit(
  principal,
  action == Action::"RecordCost",
  resource is Organization
);

// --- BudgetLedger ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Record"],
  resource is BudgetLedger
);
