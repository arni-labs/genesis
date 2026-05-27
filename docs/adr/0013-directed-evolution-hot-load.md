# ADR-0013: Directed Evolution Hot Load

## Status

Proposed.

## Context

Mission Control is a production UI, not a fixture showcase. The deployed web app
already points at the public Genesis backend, but Directed Evolution and the
Agent Answers organism must arrive there as Temper-native app bundles. Railway
deploys are costly and should not be used just to install or update app specs,
CSDL, Cedar policies, or packaged WASM artifacts.

Genesis already provides the intended runtime path:

1. Publish app bundle bytes to a Genesis repository.
2. Register the bundle as a pinned `owner/name@hash` App.
3. Install the pinned ref into a target tenant through `App.Install`.
4. Let the running Temper server materialize the Genesis bundle closure and
   reconcile specs, policies, WASM modules, ADRs, and seed data live.

## Decision

Directed Evolution and its dependencies are installed into a live control
tenant by pinned Genesis refs. Agent Answers variants are installed into
isolated variant tenants while they are evaluated, then the promoted winner is
materialized into the configured production tenant. The Genesis Railway image
continues to boot only the `temper-git` bootstrap app.

Railway deploys are reserved for runtime or web changes: new server code, new
WASM host behavior, bootstrap app changes, or Mission Control UI builds. App
bundle iteration uses Genesis publish/install hot load.

The default operating rule is:

- If the change is a Temper-native app artifact - IOA specs, CSDL, Cedar
  policies, packaged WASM modules, ADRs, seed entities, or Agent Answers
  organism bundle bytes - publish a new pinned Genesis ref and hot-load it into
  the target tenant.
- If the change is worker code, Mission Control UI code, a Temper server
  primitive, storage schema, WASM host capability, or deployment configuration,
  ship it through the normal repository/runtime path and deploy only after the
  local feedback loop is exhausted.
- Local Codex/TemperPaw brain workers are clients of the running Temper server;
  they do not need to run inside Railway. They generate commits, publish pinned
  refs, install those refs into control, variant, or production tenants, and
  write evidence back through Directed Evolution entities.

Genesis may be used directly as the publish/install source of truth, or a
Temper-native bundle may be installed as an individual piece when the caller
already has the pinned ref and target tenant. Both paths converge on the same
running Temper server app registry and must preserve existing tenant state.

## Consequences

- The live control plane can gain or update Temper-native apps without
  rebuilding or redeploying the Genesis backend.
- Directed Evolution iteration should spend most of its time in the hot-load
  loop, not the Railway deploy loop.
- Mission Control can read real OData collections from the same public Genesis
  backend it already targets.
- App dependency closure matters: `directed-evolution` installs its
  `intent-discovery` and `evolution` dependencies from the Genesis registry
  closure, not from the backend image.
- App authors must avoid entity-name collisions when multiple bundles share a
  tenant. The Agent Answers evaluator uses `TrialMetricDefinition` rather than
  `MetricDefinition` to keep Directed Evolution's metric vocabulary intact.

## Verification

- `App.Install` for `nerdsane/directed-evolution@...` returns a
  `postAction.kind` of `genesis_app_install` and lists
  `directed-evolution`, `intent-discovery`, and `evolution` in
  `materializedApps`.
- `GET /tdata/Directions`, `/tdata/Episodes`, `/tdata/Variants`,
  `/tdata/Promotions`, and `/tdata/LineageEdges` no longer return
  `EntitySetNotFound` for the target tenant.
- `GET /tdata/Questions`, `/tdata/Answers`, `/tdata/TrialSuites`,
  `/tdata/TrialMetricDefinitions`, and `/tdata/ValidatorRuns` are available.
- The deployed Mission Control route loads without fixture mode and renders
  live collections from the Genesis backend.
