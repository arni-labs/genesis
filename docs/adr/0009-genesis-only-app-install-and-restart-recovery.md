# ADR-0009: Genesis-only app install and restart recovery

## Status

Accepted - 2026-05-22. Pairs with
[ADR-0007](0007-content-addressed-dependency-closure.md) and
[RFC-0003](../rfc/0003-genesis-app-registry.md).

## Context

Temper apps had too many effective sources: repo-local app directories,
production catalogs, GitHub/submodule/symlink sources, older skill-shaped
aliases, direct OData calls, CLI helpers, and agent tools. That made it unclear
where an app came from, whether an install was reproducible, and what should
happen on restart.

The agent is the primary user. It needs one clear workflow:

```text
publish/update app -> pinned owner/app@hash -> install that pinned ref
```

At runtime, an installed app must be durable tenant state. A restart or redeploy
with the same database must recover the already-installed app stack instead of
rerunning install effects, seed data, or agent/skill bootstrap.

## Decision

Genesis is the source of truth for normal Temper apps. The canonical install
semantic is spec-owned Genesis `App.Install`; OData, CLI, UI copy surfaces, and
TemperPaw tools are clients of that semantic rather than separate install
models.

Temper persists Genesis provenance for installed apps:

- `source_kind`
- `app_ref`
- `version_hash`
- `closure_id`
- `registry_url`
- `registry_tenant`
- bundle/spec/policy/WASM/content/seed digests
- install status and reconciliation timestamps

On restart, Temper rebuilds local Genesis materialization cache roots from the
durable installed-app records, restores specs/Cedar/WASM/runtime state, and
skips unchanged pinned refs. A changed pinned ref reconciles once. A wiped
database is a fresh instance.

TemperPaw treats local app directories as development/test fixtures. Production
bootstrap may configure pinned Genesis refs with
`TEMPERPAW_GENESIS_BOOTSTRAP_REFS`; fresh databases install those refs once,
while warm restarts recover and skip unchanged refs. Agent-facing tools are:

- `temper.search_apps({...})`
- `temper.install_app({"app_ref":"owner/app@hash", ...})`
- `temper.publish_app({...})`
- `temper.update_app({...})`

Direct git push and direct OData remain low-level transport/admin surfaces.
Agents should use the native tools.

## Consequences

Positive:

- App provenance is explicit and queryable in the target Temper instance.
- Warm restart is bounded by metadata/cache recovery and digest checks.
- Agents have one install vocabulary: pinned Genesis refs.
- Genesis can be hosted independently while target Temper instances keep their
  own installed-app state in Postgres/Turso.

Trade-offs:

- Install still materializes app bytes into a local cache before using the
  existing Temper app installer. This preserves current app loading behavior
  while avoiding local catalogs as the source of truth.
- Genesis does not yet build WASM during publish. Packaged
  `wasm/<module>/<module>.wasm` artifacts remain part of the app bundle until a
  build-on-publish pipeline exists.
- Dependency closure determinism is strongest when app manifests use pinned
  dependency refs. Unpinned dependencies are resolved at materialization time
  for compatibility and should be tightened by the Genesis resolver.

## Verification

Required verification for this decision:

- Temper unit/integration tests for Genesis install parsing, cache recovery,
  removed local install routes, and Postgres/Turso installed-app schema
  migration.
- Genesis UI/tests proving the registry exposes file browsing and copyable
  OData/CLI/TemperPaw install commands for the same pinned ref.
- TemperPaw checks proving the native tool catalog exposes Genesis
  search/install/publish/update and no longer advertises local app/skill install
  side doors.
- Local E2E: publish/register a tiny app, install via OData, CLI, and
  TemperPaw/native path, use an installed entity/action, restart with the same
  DB, and verify recovery without reinstall.
- Live E2E: repeat the smoke against the Railway Genesis backend and verify the
  Vercel Genesis UI points at the same registry.

