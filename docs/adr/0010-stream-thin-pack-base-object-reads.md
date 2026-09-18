# ADR-0010: Stream thin-pack base object reads

## Status

Accepted — 2026-06-16.

## Context

Git clients may push thin packs that contain REF_DELTA entries against objects
the server already has. Genesis resolves those external bases during
`Repository.IngestPack` so the incoming object can be expanded and hashed
before the composite write lands.

The first implementation resolved existing base objects with a normal OData
collection read of the whole object row. That works for small objects, but a
large binary blob can make the OData response exceed the WASM host's bounded
`http_call` response buffer. A real push updating `monty_repl.wasm` hit this
failure while resolving a large existing blob base:

```text
pack delta base not found: ... fetch Blobs(...): HTTP response too large for buffer
```

## Decision

`scm_ingest_pack` resolves external thin-pack bases with a streaming GET rather
than bounded `http_call`.

The lookup uses the same repository-scoped durable entity identity written by
ingestion, `object_entity_id(repository_id, sha)`, and asks OData for only the
identity fields plus `CanonicalBytes`:

```text
GET /tdata/Blobs('<repository-scoped-id>')?$select=Id,RepositoryId,CanonicalBytes
```

Legacy rows keyed by the bare Git SHA remain readable as a fallback. Both
lookup forms must match the requested repository and SHA. Before a base is
given to the pack parser, Genesis verifies the canonical Git kind and length
header and recomputes the SHA from the body. A row with mismatched identity or
content fails ingestion rather than supplying bytes to delta expansion.

If `CanonicalBytes` is a field-overflow ref, the integration dereferences it
through the same streamed blob endpoint used for staged pack bytes.

## Consequences

- Thin-pack pushes that delta against large existing blobs no longer fail on
  the host response buffer.
- Base lookup avoids fetching the blob `Content` field, which halves the
  response size for legacy inline large blobs.
- Repository-scoped keys prevent one repository's object row from satisfying
  another repository's thin-pack base lookup.
- Corrupt or incorrectly keyed canonical bytes are rejected before delta
  expansion.
- Object state remains Temper-native. No filesystem repo cache or host-side git
  helper is introduced.
- The integration may still materialize one decoded base object in WASM memory
  while applying a delta. That matches the current pack parser contract and is
  separate from the host response-buffer failure fixed here.
