# ADR-0029: Receive-pack Legacy Delta Base Cache Fallback

## Status

Accepted

## Context

`git_receive_pack` accepts thin packs: Git clients may delta new objects against
objects the server already advertised. `scm_ingest_pack` resolves those external
delta bases from Genesis object rows by reading `CanonicalBytes`.

Production has legacy object rows that predate ADR-0027's "object content in
object store" shape. Some rows are durable and have a matching raw object cache
entry at `git-objects/{repository_id}/{sha}.b64`, but the OData projection does
not expose `CanonicalBytes`. A live publish of the updated `temperpaw/paw-agent`
app failed on exactly that shape:

```text
pack delta base not found: 26db9810424dbc4a8be15c9870fa5f8189a48a29:
Blobs(26db9810424dbc4a8be15c9870fa5f8189a48a29): no CanonicalBytes
```

Rejecting the push is overly strict because the delta base body is available in
the raw object cache and is exactly what the pack delta applier needs.

## Decision

When `scm_ingest_pack` finds an existing object row without `CanonicalBytes`, it
falls back to the raw object cache key:

```text
git-objects/{repository_id}/{sha}.b64
```

The fallback decodes the stored base64 object body and uses it as the external
delta base. Rows with `CanonicalBytes` still use the canonical field path first.
If both canonical bytes and raw cache are unavailable, receive-pack fails closed
with an explicit error that names the missing fallback.

## Consequences

- Legacy Genesis repositories remain push-compatible without rewriting existing
  object rows before agents can publish app updates.
- Current and future rows still write canonical bytes and raw object cache
  entries; this is a compatibility fallback, not a new primary storage shape.
- Delta-base resolution remains deterministic and bounded by the server's
  existing pack parser path.

## Verification

- `cargo test -p scm_ingest_pack` covers canonical-first behavior, raw-cache
  fallback behavior, and the raw cache key format shared with receive-pack
  object writes.
- Live verification is recorded in the associated proof once the updated module
  is deployed and the `temperpaw/paw-agent` publish succeeds.
