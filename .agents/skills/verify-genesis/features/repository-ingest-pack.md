# repository-ingest-pack

Repository.IngestPack accepts a real Git base push followed by a thin-pack
update whose delta references the existing repository-scoped base.

Drive: run `scripts/live-ingestpack-high-volume-smoke.sh` against a locally
booted Genesis instance with the changed WASM. The script must mint a scoped
token, create a unique repository, push the base and related update through
smart HTTP, point-read both blob identities, read the moved ref, clone the
repository, and recursively compare the clone with the source worktree.

Pass: the base and expanded target exist at repository-scoped identities, the
ref and clone resolve to the second commit, and the cloned files match exactly.
