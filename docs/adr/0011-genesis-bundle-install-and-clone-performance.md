# ADR 0011: Genesis Bundle Install And Clone Performance

## Status

Accepted

## Context

Genesis is the source of truth for Temper apps, but install performance exposed
two different paths that had been conflated:

- `git clone` is the authoring and inspection transport for agents and humans.
- app install is a pinned runtime materialization operation.

Using `git clone` as the normal install primitive made a large app such as
`temperpaw/paw-agent` pay Git negotiation, upload-pack object walking, pack
emission, checkout, full OS-app reconcile, WASM persistence, and TemperFS
bootstrap in one synchronous path. Warm restart then amplified the problem when
legacy installed-app rows lacked Genesis provenance and repeatedly attempted a
first-time reconcile.

Skipping unchanged apps is necessary, but it is not a sufficient performance
fix: new app installs must also be fast and observable.

## Decision

Genesis install uses a pinned bundle fetch as the primary install transport:

```text
GET /api/genesis/apps/{owner}/{name}/versions/{hash}/bundle
```

The response contains the resolved app/dependency closure and deterministic
package files for each app. The installer materializes that bundle into the
local app cache and runs the digest-aware reconcile path. Git fallback is
disabled by default and can only be enabled with
`TEMPER_GENESIS_INSTALL_GIT_FALLBACK=1` for admin/debug recovery.

Git smart-HTTP remains required and must be fast. Clone/fetch now has a
read-only live performance smoke that measures `ls-remote`, info/refs first
byte, tiny clone, and large warm clone. Upload-pack emits phase timing for
request parsing, reachable-object walking, and pack emission.

`PublishNewVersion` may only advance an app to a hash that exists as a Git
`Commit` in that app's backing repository. App/version hashes are therefore
commit hashes, not independent bundle digests. This keeps Genesis refs clonable
and keeps `owner/app@hash` install refs tied to the same object graph that Git
serves.

Genesis also maintains a raw Git object body cache at
`git-objects/{repository_id}/{sha}.b64` in the Temper blob store. Pack ingest
populates the cache when objects are written. Upload-pack reads from it before
falling back to entity/field lookup, and fills it on fallback for older objects.
This keeps new repos clone-ready and lets existing repos become faster without
destructive migration or database rewrites.

The upload-pack prefetch path pages selected object fields so a large app cannot
overflow the WASM host-call response buffer with one giant OData payload. If a
repo is historically inconsistent and advertises a ref whose target is not a
stored commit, upload-pack returns a Git protocol `ERR` pkt-line instead of
leaving the client waiting on an open response.

Temper app loading installs only manifest-declared `[[wasm_modules]]`. Extra
bundled artifacts under `wasm/` are ignored with a warning; generated `target/`
content is excluded from bundle export.

Genesis installs use the existing digest-aware app reconcile path. An app that
is already installed and whose bundle digest/runtime state matches is adopted
by recording Genesis provenance instead of replaying specs, WASM, content, or
seed bootstrap.

## Consequences

- Fresh installs use the same `App.Install` semantic but avoid full Git clone.
- Warm restarts recover installed app metadata and skip unchanged pinned refs.
- Large app install phases are attributable in logs instead of appearing as a
  single 120-300 second timeout.
- New pushed Git objects are cached for clone; older objects are cached
  opportunistically the first time upload-pack has to read them.
- Invalid future app versions are rejected at `PublishNewVersion`; historical
  invalid rows must be repaired through a governed Genesis update, not by
  deleting or rewriting the database out of band.
- App packages must declare every installed WASM module explicitly.
- Git clone remains a product surface and is tested separately from install.

## Verification

- `cargo check -p temper-platform`
- `cargo check -p git_upload_pack`
- `scripts/live-genesis-clone-performance-smoke.sh`
- Railway live clone smoke on a valid production app:
  `temperpaw/paw-patrol` cold clone ~3s, warm clone <1s.
- focused tests for manifest-declared WASM loading and Genesis bundle install
- live TemperPaw warm restart proving unchanged Genesis refs skip after
  adoption/reconcile
