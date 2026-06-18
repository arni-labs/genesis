# ADR 0028: Upload-pack fuel budget for agent apps

## Status

Accepted

## Context

Genesis smart HTTP handlers run as WASM integrations behind `HttpEndpoint`
limits. The existing shared 20,000,000,000 fuel budget worked for small clone
smokes, but the production `temperpaw/paw-agent` app now has enough objects and
bundle content that `git_upload_pack` can finish reachable-object walking and
then exhaust fuel while emitting pack bytes.

The failure is visible to Git clients as an incomplete clone/fetch that stalls
part-way through receiving objects. Railway logs show `git_upload_pack` failing
with `fuel exhausted` after walking 494 reachable objects for
`temperpaw/paw-agent`.

## Decision

Seed the production upload-pack endpoint with a 100,000,000,000 fuel budget:

```text
HttpEndpoints('he-upload-pack').MaxFuel = 100000000000
```

Keep `git_receive_pack` at the existing 20,000,000,000 budget. Push-side work is
bounded by auth, pack ingest, and action-bridge semantics, while upload-pack is
the read-heavy clone/fetch path used by humans, agents, and app provenance
inspection.

## Consequences

- Large app clone/fetch operations have enough compute budget to emit pack
  responses instead of failing after object walking.
- The runtime resource change is source-backed in the live smoke/bootstrap
  scripts, so future endpoint reconciliation does not silently revert to 20B.
- The higher limit is scoped to upload-pack; receive-pack's write path remains
  on the smaller budget until a separate proof shows it needs more.

## Verification

- Production `HttpEndpoints('he-upload-pack')` readback shows
  `MaxFuel=100000000000`.
- Live `temperpaw/paw-agent` clone/fetch proof is recorded in
  `.proofs/2026-06-18-paw-agent-upload-pack-fuel.md`.
