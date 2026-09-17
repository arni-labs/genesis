# Scoped Git commit publication

Katagami's authorized release cannot publish commits pushed successfully to Genesis. Ingestion stores commit entities under a repository-scoped key; App.PublishNewVersion queries Id using the bare Git hash and returns a false absence.

Resolve the same durable key used by ingestion, retain a repository-checked fallback for legacy bare-hash entities, and fail closed on missing commits, mismatched repositories, malformed responses, and HTTP errors. Preserve the public full-hash app reference and all publication actions and authorization checks. No kernel or policy change is required.

Verify with unit tests and a real local registry using a pushed commit. The corrected module must publish that hash and reject an absent hash. Deploy the reviewed module through the existing app release route, then retry the Katagami publication and verify the returned version.
