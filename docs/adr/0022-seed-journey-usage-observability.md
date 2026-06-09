# ADR-0022: Seed Journey Usage Observability

## Status

Accepted

## Context

The Agent Answers seed walkthrough queues simulated-user work items before a full
Directed Evolution episode exists. The current seed prompt prescribes specific
OData actions, which makes independent Codex users converge on the same path.
It also only requires `X-Tenant-Id`, so Temper records entity transition logs
but cannot correlate those logs with a seed journey through Directed Evolution
runtime-request headers.

The existing transition telemetry is real app usage, but the log message
`trajectory.entry` is too internal for humans trying to inspect application
activity in Datadog.

## Decision

Seed simulated users should first read the app description and OData metadata,
then decide how to use the app without being handed a scripted action sequence.
The prompt must require stable seed-journey correlation headers on runtime
requests.

Temper should also emit app-usage telemetry for every entity action dispatch,
alongside the existing trajectory entry. The visible log content must describe
the usage event itself, for example tenant, `Entity.Action`, entity ID, status
transition, success, session, and workflow run. The telemetry must also carry
structured fields and a named span so Datadog can group usage by tenant,
entity, action, session, and journey.

## Consequences

Seed walkthrough traffic becomes less scripted while still queryable by
journey. Datadog users and observer agents should prefer readable
`app usage: ...` records and spans scoped by `observation_metadata`; the older
`trajectory.entry` records remain lower-level platform evidence. Full
app-specific product metrics can still be layered on top later.
