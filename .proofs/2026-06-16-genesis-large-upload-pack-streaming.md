# Genesis large upload-pack streaming proof — 2026-06-16

## Summary

Fixed and proved the Genesis large-object clone/fetch path. The original live
failure was:

```text
fetch Blobs(d3a2bc16ea2f2d62b992e4cf00569c1b02d908ce): HTTP response too large for buffer
```

`git_receive_pack` and `scm_ingest_pack` could ingest the large WASM/thin-pack
case, but `git_upload_pack` still used bounded `http_call` for the fallback
OData object row and field-overflow dereference. `git_upload_pack` now streams
the fallback read and requests only `CanonicalBytes`.

## Commits

- Genesis branch: `codex/genesis-upload-pack-stream-large-objects-20260616`
- Genesis commit: `2da0f55 fix(genesis): stream upload-pack large object reads`
- Temper submodule branch: `codex/upload-pack-stream-large-objects-20260616`
- Temper commit: `a0b976d8 test(server): persist large composite sub-write fields`

## Deployment

- Railway project: `temper`
- Service: `genesis`
- URL: `https://genesis-production-164d.up.railway.app`
- Deployment: `729f0520-989e-4aa7-b618-0324b5872a3c`
- Status: `SUCCESS`
- Message: `Fix Genesis upload-pack large object streaming 2da0f55`
- Image digest: `sha256:b900ca81a7b7dac7b995578282b6a8e0574aa4eb05347f88c8b1f72294956e8b`

## Local verification

Red test:

```text
cargo test -p git_upload_pack -- --nocapture
```

Failed as expected after adding tests:

```text
cannot find function `existing_object_lookup_url` in this scope
cannot find function `canonical_body_from_field_value` in this scope
```

Green tests:

```text
cargo test -p git_upload_pack -- --nocapture
```

Result: `9 passed`.

```text
cargo test -p scm_ingest_pack -- --nocapture
```

Result: `8 passed`.

```text
cargo test -p temper-server --features sim \
  composite_ingest_pack_large_blob_sub_write_persists_overflow_fields \
  -- --nocapture
```

Result: `1 passed`.

Builds:

```text
cargo build -p git_upload_pack --target wasm32-wasip1 --release
cargo build -p git_refs_advertise -p git_upload_pack -p git_receive_pack \
  -p scm_ingest_pack -p app_registry --target wasm32-wasip1 --release
```

Both passed. `wasm/git_upload_pack/git_upload_pack.wasm` was rebuilt from the
release artifact.

Note: `cargo build --workspace` on the native macOS target fails for the WASM
integration crates with unresolved host imports such as `host_get_context` and
`host_http_call`. That is the existing workspace shape for cdylib WASM guests;
the intended `wasm32-wasip1` build passed.

## Live A/B clone proof

Repo from the earlier failing proof:

```text
https://genesis-production-164d.up.railway.app/proof-large-thin-20260616175438/wasm-delta.git
```

Before this fix, receive-pack advanced the ref but clone failed on upload-pack
while reading blob `d3a2bc16ea2f2d62b992e4cf00569c1b02d908ce`.

After deploying `729f0520-989e-4aa7-b618-0324b5872a3c`:

```text
HEAD=66d354a5d2925c3fd400f58d6aca00cbacd7a937
BLOB=d3a2bc16ea2f2d62b992e4cf00569c1b02d908ce
SIZE=7340032
FSCK=ok
CLONE_LOG=Cloning into '/tmp/genesis-upload-pack-clone-proof.02FSbk/clone'...
```

OData state:

```text
Ref TargetCommitSha=66d354a5d2925c3fd400f58d6aca00cbacd7a937
Ref PreviousCommitSha=6c5c3e45898daf5c0d7d05ea268c17eb8523af2f
Blob Size=7340032
Blob Status=Durable
```

HTTP logs for the clone show cache misses followed by streamed OData reads and
a successful upload-pack response:

```text
GET /_internal/blobs/git-objects/.../d3a2bc16ea2f2d62b992e4cf00569c1b02d908ce.raw 404
GET /tdata/Blobs 200 533ms
POST /proof-large-thin-20260616175438/wasm-delta.git/git-upload-pack 200 1603ms
```

## Fresh pre-created repo proof

The documented Genesis v0 flow requires explicit repository pre-create and
provision callback before git push. Auto-provision-on-push is deferred in
`docs/rfc/0002-push-and-clone.md`.

Run:

```text
RUN_ID=upload-stream-precreated-20260616181927
REMOTE=https://genesis-production-164d.up.railway.app/proof-upload-stream-precreated-20260616181927/wasm-delta.git
REPO_ID=rp-proof-upload-stream-precreated-20260616181927-wasm-delta
```

Steps:

1. `POST /tdata/Repositories`
2. `POST /tdata/Repositories('...')/Temper.Git.MarkProvisioned`
3. Normal `git push origin main` of a 7,340,032-byte `wasm/monty_repl.wasm`
4. Manual forced thin-pack receive-pack update
5. `git clone`
6. `cmp` source vs clone
7. `git fsck --strict`

Result:

```text
BASE_COMMIT=2bf6a21c8173ed39bb5dbe9d0eae0e5874b1cc17
BASE_BLOB=8e1d11be2b538788721e163d301e35ad447dd731
NEW_COMMIT=35aea6d58ee58f2fdf8804903f55ffa3d8fdfd6a
NEW_BLOB=0e7b5b32730feb02437eecc43a7c3d9f054f78c8
THIN_PACK_BYTES=609
RECEIVE_HTTP=200
RECEIVE_RESPONSE=unpack ok; ok refs/heads/main
REF_TARGET=35aea6d58ee58f2fdf8804903f55ffa3d8fdfd6a
CLONE_HEAD=35aea6d58ee58f2fdf8804903f55ffa3d8fdfd6a
CLONE_BLOB=0e7b5b32730feb02437eecc43a7c3d9f054f78c8
SIZE=7340032
CMP=ok
FSCK=ok
```

OData state:

```text
Repository Status=Active
Repository LibsqlDbName=rp-proof-upload-stream-precreated-20260616181927-wasm-delta.db
Ref Status=Active
Ref TargetCommitSha=35aea6d58ee58f2fdf8804903f55ffa3d8fdfd6a
Ref PreviousCommitSha=2bf6a21c8173ed39bb5dbe9d0eae0e5874b1cc17
Blob Status=Durable
Blob Size=7340032
Blob CanonicalBytesLength=9786728
```

HTTP logs:

```text
POST /tdata/Repositories 201
POST /tdata/Repositories('rp-proof-upload-stream-precreated-20260616181927-wasm-delta')/Temper.Git.MarkProvisioned 200
POST /proof-upload-stream-precreated-20260616181927/wasm-delta.git/git-receive-pack 200
POST /proof-upload-stream-precreated-20260616181927/wasm-delta.git/git-receive-pack 200
GET /_internal/blobs/git-objects/.../0e7b5b32730feb02437eecc43a7c3d9f054f78c8.raw 404
GET /tdata/Blobs 200 378ms
POST /proof-upload-stream-precreated-20260616181927/wasm-delta.git/git-upload-pack 200 1262ms
```

## Follow-up noted

An attempted push to a never-precreated repo created a `Repository` entity at
the initial `Provisioning` state, then failed receive-pack with:

```text
Action 'IngestPack' not valid from state 'Provisioning'
```

That matches the current documented v0 boundary: explicit pre-create is in tree;
auto-provision-on-push is deferred. It is not the large upload-pack streaming
bug fixed here.
