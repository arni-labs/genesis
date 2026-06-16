# Genesis main sync proof — 2026-06-16

## Summary

Merged the Genesis upload-pack streaming fix line into Genesis `main`, merged the
required Temper support into Temper `main`, updated the Genesis submodule to the
merged Temper commit, deployed Genesis production from the synced worktree, and
ran live post-deploy smoke checks.

## Git state

- Genesis branch: `codex/main-sync-genesis-upload-pack-20260616`
- Genesis `main`: `dacf8ed chore(temper): bump merged upload-pack support`
- Temper PR: `nerdsane/temper#311`
- Temper `main`: `f200ba62 Merge pull request #311 from nerdsane/codex/upload-pack-stream-large-objects-20260616`

## Local verification before merge

```text
cargo test -p git_upload_pack -- --nocapture
result: 4 passed

cargo test -p scm_ingest_pack -- --nocapture
result: 13 passed

cargo build -p git_upload_pack -p scm_ingest_pack --target wasm32-wasip1 --release
result: passed

cd temper
cargo test -p temper-server --features sim composite_ingest_pack_large_blob_sub_write_persists_overflow_fields -- --nocapture
result: 1 passed
```

## Temper PR verification

`nerdsane/temper#311` merged after required GitHub checks passed on head
`733a67e2`:

```text
Verification Contract (verification.v1): success
Compile & Lint: success
Integrity & DST Patterns: success
Tests: success
DST/Platform Tests (core): success
DST/Platform Tests (platform-boot): success
DST/Platform Tests (platform-consistency): success
DST/Platform Tests (platform-random): success
Spec Verification (L0-L3): success
Instrumentation Hygiene (ADR-0052): success
Bench Build: skipped
```

## Railway deployment

- Project: `temper`
- Service: `genesis`
- Environment: `production`
- URL: `https://genesis-production-164d.up.railway.app`
- Deployment: `4bbc1574-01ac-4914-83fc-d55a3ca55d28`
- Status: `SUCCESS`
- Message: `Sync Genesis main upload-pack streaming dacf8ed`

## Live post-deploy upload-pack proof

Cloned the known large proof repo from the newly deployed service:

```text
REMOTE=https://genesis-production-164d.up.railway.app/proof-upload-stream-precreated-20260616181927/wasm-delta.git
HEAD=35aea6d58ee58f2fdf8804903f55ffa3d8fdfd6a
BLOB=0e7b5b32730feb02437eecc43a7c3d9f054f78c8
SIZE=7340032
FSCK=ok
```

## Live post-deploy receive-pack proof

Ran the live ingest-pack concurrency smoke against production:

```text
RUN_ID=postdeploy-20260616193959
repository=rp-race-postdeploy-20260616193959-ingestpack-postdeploy-20260616193959
winner=right 226863d69c05d82ff7cfa0ea48d7dcca4793dcfa
loser status: left=1 right=0
winner objects verified=3
loser unique objects absent=3
ref=rf-rp-race-postdeploy-20260616193959-ingestpack-postdeploy-20260616193959-refs-heads-main -> 226863d69c05d82ff7cfa0ea48d7dcca4793dcfa
result=PASS
```
