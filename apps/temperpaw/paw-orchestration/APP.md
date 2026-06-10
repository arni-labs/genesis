# Paw Orchestration

Shared Paw worker orchestration with providers, concrete worker slots, work items, worker runs, organizations, budget ledgers, and heartbeat-driven execution runs. This app owns generic execution provenance for TemperPaw, Paw Patrol, Directed Evolution, and future worker-backed apps.

## Entity Types

### WorkerProvider

Executor type and capability family, such as `local_codex`, `temperpaw_agent`, or `codex_cloud`.

**States**: Registered -> Active | Disabled

**Key actions**:
- **Register**: Record provider kind, display name, capabilities, risk ceiling, and enabled flag
- **Activate / Disable**: Control provider availability

### WorkerAgent

Concrete worker slot or process. Local Codex concurrency is represented by multiple active WorkerAgents, each claiming one WorkItem at a time.

**States**: Registered -> Active | Unhealthy | Disabled

**Key actions**:
- **Register**: Record worker id, provider, capabilities, host, and worktree root
- **ReportHeartbeat**: Refresh liveness and advertised capabilities
- **Activate / MarkUnhealthy / Disable**: Control claimability

### WorkItem

Queued work requested by another Temper app. WorkItems carry role, target, prompt/context refs, capability requirements, lane, exclusive key, and correlation JSON.

**States**: Queued -> Claimed -> Running -> Succeeded | Failed | Cancelled

**Key actions**:
- **QueueWorkItem**: Create a runnable unit of work
- **ClaimWorkItem**: Claim work for a concrete WorkerAgent
- **StartWorkItem**: Attach a WorkerRun and begin execution
- **SucceedWorkItem / FailWorkItem / CancelWorkItem**: Finish or cancel work

### WorkerRun

One execution attempt for a WorkItem. A WorkerRun records provider/worker provenance, optional TemperPaw session id, output, evidence, timings, and failure.

**States**: Queued -> Running -> Succeeded | Failed | Cancelled

**Key actions**:
- **StartWorkerRun**: Start a bounded worker execution
- **SucceedWorkerRun / FailWorkerRun / CancelWorkerRun**: Record execution outcome

### Organization

Team and budget controls for orchestration.

**States**: Setup → Active → Paused → Archived

**Key actions**:
- **Configure**: Set name, description, and monthly budget
- **Activate**: Enable the organization for scheduling and execution
- **AddMember / RemoveMember**: Manage organization roster
- **RecordCost**: Record budget consumption from orchestration work
- **ResetBudgetCycle**: Roll over the monthly budget counter
- **Pause / Resume**: Temporarily halt execution activity
- **Archive**: Terminal state

### HeartbeatRun

Agent execution heartbeat lifecycle. Tracks a single agent run from scheduling through budget approval, execution, and completion.

**States**: Scheduled → CheckingIn → Working → Completed | Failed | Cancelled

**Key actions**:
- **Schedule**: Initialize run context with agent, org, wake reason, and adapter type
- **ApproveBudget**: Approve execution budget before work can begin
- **CheckIn**: Agent checks in (requires budget approval and agent binding)
- **StartExecution**: Start execution and trigger the configured adapter integration
- **RecordTurn**: Record a completed execution turn with token/cost totals
- **SaveCheckpoint**: Persist resumable session checkpoint
- **RecordResult**: Record execution output payload
- **Complete**: Finish run (requires a result)
- **Fail / Cancel**: Terminal error or cancellation from any active state

### BudgetLedger

Append-only organization cost records for audit and reporting.

**States**: Recorded (single state, append-only)

**Key actions**:
- **Record**: Append a normalized budget event with org, agent, run, amount, tokens, and category

## Setup

```
temper.install_app("<tenant>", "temperpaw/paw-orchestration@<hash>")
```
