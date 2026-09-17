# Decisions

## Resolve commits by their stored identity

Decision: Make app publication resolve the repository-scoped commit key used by Git ingestion, with a repository-checked legacy lookup.
Came up because: Both Katagami pushes succeeded, but publication reported missing commits; direct reads of their scoped keys returned the expected durable commits.
Options: Duplicate commit records under bare hashes; bypass registry publication; repair the lookup.
Chose the lookup repair because it preserves the existing data and publication validation boundary.
Where: wasm/app_registry/src/lib.rs and the shared Git object identity helper.
