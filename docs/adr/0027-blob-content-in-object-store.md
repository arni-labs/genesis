# ADR-0027: File Bytes Live in the Object Store, Metadata in Entity Rows

## Status

Accepted

## Context

Genesis stores git file content in a hybrid today: blob/tree/commit canonical
bytes at or under 128 KiB (`FIELD_INLINE_MAX_BYTES = 131_072`) sit inline
(base64) on the entity row in the event store, while larger values are staged
to the Temper kernel BlobStore (S3/R2 or local filesystem, per-tenant via
vault `blob_endpoint`) with the row holding a content-addressed reference
(`scm_ingest_pack::maybe_stage_field_value`). Separately, ADR-0011 added a
raw git object cache (`git-objects/{repository_id}/{sha}.b64`) in the same
BlobStore, which serves the clone path cache-first.

The hybrid means most repository bytes — source files are typically small —
live inside event-store rows. Consequences: the authoritative event log
carries bulk content it never needs for replay semantics, row reads pay for
content even when only metadata is wanted, and the spec documentation
(`specs/blob.ioa.toml`: "Content lives inline on the entity row. No spill
tier") no longer describes reality.

## Decision

All git object **content** lives in the object store; entity rows carry
metadata plus references:

- Blob/Tree/Commit/Tag rows persist hash, sizes, parsed metadata (parents,
  author, message, tree entries), and a content-addressed blob reference.
  `CanonicalBytes`/`Content` are written through the existing overflow-ref
  mechanism regardless of size — the inline threshold for git object content
  drops from 128 KiB to zero. (The 128 KiB threshold remains for non-object
  fields such as staged `PackBytes`.)
- The read path keeps its existing ordering: ADR-0011 raw-object cache first,
  then the blob reference, then — for rows written before this change — the
  legacy inline value. Reads opportunistically backfill the object store the
  first time they resolve a legacy inline row (the same non-destructive
  migration pattern ADR-0011 used). Nothing rewrites history; no migration
  job runs against the database.
- The kernel side already exists (`temper-server` BlobStore with S3/R2 and
  LocalFs backends, `get_blob_with_legacy_fallback`); no kernel changes are
  required.
- `specs/blob.ioa.toml` documentation is corrected to describe the reference
  model.

## Consequences

- The event log stays lean: replay and audit queries stop paying for file
  bytes; entity reads return metadata-sized rows.
- Clone latency is unchanged in the common case (the raw-object cache already
  served it) and the first read of an old object now also primes the store.
- The "one source of truth" rule is preserved: the entity row remains the
  authority — it owns the hash, and the blob store is addressed by that hash;
  the cache and store are projections, not replicas with independent truth.
- Production requires a configured blob backend (vault `blob_endpoint` →
  R2/S3, or `TEMPER_LOCAL_BLOB_DIR`); the deployment verification step for
  this effort records which backend the Railway tenant actually uses.
- Verified by pushing files both under and over 128 KiB and confirming both
  land in the object store with row references, while clone round-trips stay
  byte-exact (`git fsck` clean).
