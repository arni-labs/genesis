# Decisions & Tradeoffs

## Resolve delta bases by their stored identity

Decision: Use repository-scoped point reads with a checked legacy fallback for thin-pack base objects.
Came up because: A 131072-byte Git blob uploaded successfully, but the next standard delta pack reported its SHA missing; its stored Id is repository-prefixed while the parser filters Id by bare SHA.
Options: Widen raw-blob permissions; duplicate objects under legacy IDs; repair the existing lookup and stage bounded standard packs.
Chose the lookup repair because it preserves existing objects, caller identity and the governed publication action. Bounded delta steps cost more calls but preserve the prepared package bytes and avoid the older runtime's independently reproduced context overwrite.
Where: wasm/scm_ingest_pack/src/lib.rs and its module-level verification.
