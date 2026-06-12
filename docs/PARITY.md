# GitHub Parity

What works against Genesis exactly as it would against github.com, what
partially works, and what is deliberately not built. Every **works** row names
the test or smoke that proves it; nothing here is asserted from memory. Rows
marked *(this PR)* land with the vision-completion effort (genesis#25).

## Git wire protocol

| Capability | Status | Proof |
|---|---|---|
| `git clone` / `fetch` (smart HTTP, pack v2, sideband) | Works | `wire/tests/git_parity.rs` (advertisement byte-parity vs `git-http-backend`), `scripts/live-genesis-clone-performance-smoke.sh` |
| Object hashing (blob/tree/commit/tag byte-exact) | Works | `crates/git_object/tests/git_parity.rs` (hash match vs `git hash-object` / `cat-file`) |
| `git push` (pack ingest incl. ofs/ref delta + thin packs) | Works | `wire/tests/git_pack_parity.rs`, `scripts/live-genesis-install-e2e-smoke.sh` |
| Push authentication (GitToken Basic/Bearer, 401 challenge) | Works *(this PR)* | `crates/git_auth` tests; receive-pack + refs-advertise challenge; CI round-trip gate |
| Force-push classification (`force` scope enforced) | Works *(this PR)* | `scm_ingest_pack` ancestry-walk tests; CI denial case |
| Compare-and-swap ref updates | Works | kernel composite applier; exercised by every push round-trip |
| Pack **delta emission** on clone | Not built (measured 1.32× wire size on the benchmark corpus) | Cold clone missed the bar at ~3.5×, but pack-emission speed — not wire size — dominates; measured analysis and ordered follow-up in `docs/PERFORMANCE.md` |
| multi_ack / shallow negotiation | Not built | NAK-only v0 (`git_upload_pack`); recorded follow-up |

## GitHub REST v3 (`/api/v3`)

| Capability | Status | Proof |
|---|---|---|
| Create repo (`POST /user/repos`, auto-provision) | Works *(this PR)* | shape tests vs github.com fixtures; CI round-trip gate |
| Get repo, list branches, get/create/update/delete refs | Works *(this PR)* | shape tests per endpoint |
| Pull requests: create / list / get / close | Works *(this PR)* | shape tests + state-machine tests |
| Reviews: approve / request changes (author self-approve denied by Cedar) | Works *(this PR)* | state-machine tests; CI second-token approval flow |
| Merge (`PUT /pulls/{n}/merge`): merge, squash, fast-forward | Works *(this PR)* | `scm_merge_pr` byte-parity tests vs real `git merge` (32 tests); CI matrix over strategies |
| Merge with content conflicts | **Deliberate divergence**: returns 409 telling the caller to resolve locally; GitHub would 3-way-merge non-overlapping hunks in the same file | ADR-0024; conflict-409 test |
| Rebase merge method | Not built (409s with instructive error) | ADR-0024 records the follow-up |
| Issues, comments on PRs, statuses/checks, webhooks delivery | Not built | `specs/webhook.ioa.toml` exists; delivery is RFC-0001 Phase 2 |
| GraphQL v4 | Not built | RFC-0001 Phase 3 |

## Authentication and authorization

| Capability | Status | Proof |
|---|---|---|
| Token auth (Basic username, `Bearer`, gh-style `token`) | Works *(this PR)* | `crates/git_auth` tests |
| Scoped tokens (Cedar evaluates scopes per action) | Works *(this PR)* | `policies/*.cedar` + bridge-principal kernel tests (temper ADR-0138) |
| Token usage trail (`MarkUsed`) | Works *(this PR)* | fired on authenticated push |
| Fine-grained per-repo permissions / collaborator roles | Not built | scope strings only; owner model per RFC-0003 |

## Not GitHub-shaped (Genesis-specific surfaces)

The app registry (`App`/`Lineage`/`Closure`, pinned `owner/app@hash` installs,
commons mode) and the Directed Evolution control plane have no GitHub
equivalent and are documented in RFC-0003 and RFC-0005 respectively.
