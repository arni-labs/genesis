# 0020 Idempotent Genesis App Install

## Status

Accepted

## Context

`App.Install` records a deterministic `AppInstallation` row keyed by app,
target tenant, and pinned version hash. That gives Genesis an audit trail for a
specific materialization request, but it also means a second install of the same
pinned ref can target an existing `Installed` row.

Directed Evolution recovery and proof runs need this to be a reconcile request,
not a conflict. A reviewer or worker may intentionally re-run installation to
reload policies, specs, WASM, or cached Genesis closure bytes for the same
target tenant.

## Decision

`AppInstallation.Create` is idempotent for the deterministic install key. It may
run from `Pending`, `Installed`, or `Failed` and moves the row to `Pending` with
the new request metadata. The platform bridge still performs the materialization
after `App.Install` succeeds, then calls `MarkInstalled` or `MarkFailed`.

## Consequences

- Reinstalling the same pinned Genesis app ref can repair/reconcile a tenant.
- Fresh proof runs can safely retry install steps without deleting audit rows.
- `AppInstallation` remains the durable ledger for install attempts; the state
  transition history, not row uniqueness alone, carries repeat attempts.
