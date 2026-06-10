# ADR-0023: External Directed Evolution Runtime Targets

## Status

Accepted

## Context

Genesis is the registry and Directed Evolution control plane for Temper apps.
It should own app source, content-addressed versions, lineage, and approval
state. It should not imply that every app being evolved runs inside the Genesis
server process.

The Agent Answers seed walkthrough originally used a Genesis tenant as the
runtime. That blurred source-of-truth state with application runtime state and
made observability appear under `service:temper-platform`. For the user's own
apps, TemperPaw is the natural runtime plane: it already hosts installed Temper
apps and exports APM traces through the Railway Datadog runtime agent as
`service:temperpaw`.

## Decision

Directed Evolution app context must carry an explicit runtime target:
runtime base URL, runtime tenant, Datadog service, and runtime auth secret
names. Genesis remains the control plane and queues work items there, but
simulated users exercise the app through the configured runtime base URL.

Agent Answers seed runtime points at the user's TemperPaw production runtime
with tenant `agent-answers-seed` and Datadog service `temperpaw`. Simulated-user
prompts must use the runtime base URL for `/tdata` calls and resolve a bearer
token from runtime-specific environment variables without writing the secret
into Genesis entities.

## Consequences

Directed Evolution can evolve apps whose live runtime is outside Genesis while
keeping Genesis as the source of truth. Observability links and observer
queries can target the runtime service that actually executes app transitions.
For Agent Answers, good traces should appear under `service:temperpaw` once the
app is installed in the TemperPaw runtime tenant and the worker has the runtime
API credential.
