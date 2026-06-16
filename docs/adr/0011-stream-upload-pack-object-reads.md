# ADR-0011: Stream upload-pack object reads

## Status

Accepted — 2026-06-16.

## Context

Genesis stores Git objects as Temper entities. `git_upload_pack` walks those
entities and emits a pack during `git clone` and `git fetch`.

Large object fields may be represented as field-overflow blob refs. The first
upload-pack fallback path read the full OData row with bounded `http_call` and
then dereferenced field-overflow refs with another bounded `http_call`. That
works for small objects but fails for large WASM artifacts. A live proof with a
large `monty_repl.wasm` blob showed receive-pack succeed and upload-pack fail
with:

```text
fetch Blobs(...): HTTP response too large for buffer
```

## Decision

`git_upload_pack` keeps its raw object-cache fast path, then falls back to a
streamed OData lookup when the cache misses.

The fallback lookup requests only the canonical serialized object bytes:

```text
$select=CanonicalBytes&$top=1
```

If `CanonicalBytes` is stored as a field-overflow ref, upload-pack dereferences
that blob through the streamed HTTP host API as well. The integration still
streams the final smart-HTTP response body to the client.

## Consequences

- Clones and fetches can serve large blobs that are stored in Temper entity
  fields or field-overflow blobs without tripping the bounded host response
  buffer.
- Upload-pack no longer fetches the `Content` field when it only needs
  `CanonicalBytes`.
- Git object state remains entity-first. Genesis does not introduce bare repos,
  host-side git helpers, or a second storage tier.
- Upload-pack may still materialize one decoded object body while deflating it
  into the emitted pack. That is the current `PackEmitter` contract and is
  bounded independently from the host response-buffer issue fixed here.
