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

## Consequences

- The live control plane can gain or update Temper-native apps without
  rebuilding or redeploying the Genesis backend.
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
