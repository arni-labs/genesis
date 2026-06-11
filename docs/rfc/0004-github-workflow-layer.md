# RFC-0004: The GitHub workflow layer — pull requests, reviews, branches, merge, REST v3

- Status: Draft
- Date: 2026-06-11
- Authors: Rita, with implementation by Claude (vision-completion effort)
- Related:
  - [RFC-0001](0001-architecture.md) (v1 architecture; Phase 1.3 and Phase 2 items this RFC implements)
  - [RFC-0002](0002-push-and-clone.md) (push and clone; Slice C auto-provision is closed here)
  - [ADR-0024](../adr/0024-server-side-merge-strategies.md) (merge strategies and conflict semantics — new in this PR)
  - [ADR-0025](../adr/0025-push-auth-and-force-classification.md) (push-path authentication and force-push classification — new in this PR)
  - [VISION.md](../../VISION.md) ("Pull requests, reviews, and comments in the same model")

## TL;DR

Genesis today is a governed git remote plus an app registry: clone, push,
compare-and-swap refs, content-addressed app versions — all real, all
live-proven. But the workflow layer that makes it *GitHub*-compatible —
opening a pull request, reviewing it, merging it, operating branches
through an API — exists only as entity specs and Cedar policies with no
callers. The specs reference WASM modules that were never written; no
`/api/v3/*` endpoint exists; the push path performs no authentication and
never classifies force-pushes, so the protection policies are decorative.

This RFC specifies v1 of that layer: five slices that together make a real
`git` + `gh` workflow run end-to-end against Genesis —

```
git clone → git checkout -b → git push (token-authed, force-classified)
  → gh pr create → gh pr review --approve (second principal)
  → gh pr merge (FF / squash / merge) → re-clone → git fsck clean
```

Everything stays inside the existing architecture: entities own state,
WASM integrations own protocol, Cedar gates every mutation, and the byte
emitted on the wire is verified against real `git`.

## Motivation

VISION.md's central bet is that pull requests, reviews, and comments
belong in the *same substrate* as the commits they point at — same event
log, same policy engine, same audit surface. The entity model for that bet
is already merged (`specs/pull_request.ioa.toml`, `review.ioa.toml`,
`review_comment.ioa.toml`); the state machines are well-formed
(Draft→Open→UnderReview→Approved→Merged/Closed) and the Cedar policies
encode the interesting rules (the author of a PR cannot approve it; Merge
is reachable only from Approved). None of it can be exercised: the only PR
action with a live caller is `UpdateHead`, fired as a sub-write on push.

Meanwhile a Dark Factory's primary writers — agents — interact with
version control through `gh` and the GitHub REST API. Until Genesis speaks
`/api/v3`, no agent toolchain can target it without bespoke adapters,
which defeats the byte-exact-compatibility premise: *if a third-party tool
works against github.com, it works against Genesis.*

Two enforcement holes also undermine the governed-remote story and must
close before new endpoints ship:

1. `git_receive_pack` performs no authentication. Push is gated only by
   endpoint registration, and the live smokes register it
   `RequiresAuth:false`. `GitToken.MarkUsed` has never fired.
2. The push path never classifies a non-fast-forward update. Every ref
   update maps to plain `Ref.Update`; `Ref.ForceUpdate` and its Cedar
   `force`-scope gate are unreachable. Force-push protection exists on
   paper only.

## Design

### Slice 1 — push authentication and force-push classification (ADR-0025)

`git_receive_pack` adopts the same GitToken resolution already used by
`git_refs_advertise` and `git_upload_pack` (SHA-256 token hash lookup over
`/tdata/GitTokens`, scopes forwarded as `X-Temper-Principal-Scopes`).
Anonymous push is rejected with the standard git HTTP 401 challenge.
Successful resolution fires `GitToken.MarkUsed`.

`scm_ingest_pack` classifies each ref update by walking the commit DAG
from the new tip toward the advertised old tip (bounded walk, explicit
limit). If the old tip is not an ancestor of the new tip, the sub-write
dispatches `Ref.ForceUpdate` instead of `Ref.Update`; Cedar then requires
the `force` scope. Deleting or force-updating the default branch keeps its
existing deny policy.

### Slice 2 — repository and branch REST (`/api/v3`)

Two new WASM integrations, `github_rest_repos` and `github_rest_refs`,
served through the kernel's HttpEndpoint action bridge (the machinery that
already powers `git_receive_pack`):

| Endpoint | Entity action |
|---|---|
| `POST /api/v3/user/repos` | `Repository` create + provision (closes RFC-0002 Slice C) |
| `GET /api/v3/repos/{owner}/{repo}` | Repository read projection |
| `GET /api/v3/repos/{owner}/{repo}/branches` | Ref list projection (`refs/heads/*`) |
| `GET/POST/DELETE /api/v3/repos/{owner}/{repo}/git/refs[/...]` | `Ref.Create` / `Ref.Update` / `Ref.Delete` |

Response shapes follow github.com structurally — field names, types,
required-field presence — and every shipped endpoint carries a shape test
comparing a recorded github.com fixture with the Genesis response (hard
rule 7).

### Slice 3 — pull request REST and lifecycle

`github_rest_pulls` maps REST verbs onto the existing state machines:

| Endpoint | Entity action |
|---|---|
| `POST /repos/{o}/{r}/pulls` | `PullRequest.Create` + `Open` |
| `GET /repos/{o}/{r}/pulls[/{n}]` | projection |
| `PATCH /repos/{o}/{r}/pulls/{n}` | `Close` / title-body updates |
| `POST /repos/{o}/{r}/pulls/{n}/reviews` | `Review.Create` + `Approve` / `RequestChanges` |
| `PUT /repos/{o}/{r}/pulls/{n}/merge` | `PullRequest.Merge` (Slice 4) |

`scm_assign_pr_number` — declared by the spec since v1 but never written —
assigns `max(Number)+1` scoped to the repository. With these callers in
place, the dormant Cedar rules become live enforcement: author
self-approval is denied, merge is reachable only from `Approved`.

### Slice 4 — server-side merge (`scm_merge_pr`, ADR-0024)

The one genuinely hard piece. A Composite producer (same machinery as
`Repository.IngestPack`) that:

1. computes the merge base by a bounded, paged walk of the Commit DAG;
2. executes the requested strategy:
   - **fast-forward** — Ref CAS advance, no new objects;
   - **squash** — one new single-parent commit;
   - **merge** — one new two-parent commit, accepted only when the
     three-way tree resolution is *clean at tree level* (no path modified
     on both sides since the base);
3. on divergent content, returns **409** with a body that tells the
   caller exactly what to do (rebase locally, push, retry);
4. emits Commit/Tree rows through the proven sub-write shapes, advances
   the ref by CAS, transitions `PullRequest.Merge`, and writes the new
   objects into the ADR-0011 raw-object cache so the next clone is warm.

Commit and tree bytes come from `crates/git_object` —
`commit_canonical_bytes` already produces hash-identical two-parent merge
commits in tests. Every merge the engine performs is verified in tests by
performing the identical merge with real `git` and comparing SHAs.

Full blob-level three-way content merge is explicitly out of v1 (see
ADR-0024 for the decision record); GitHub parity for conflicting merges is
a recorded follow-up, and `docs/PARITY.md` says so plainly.

### Slice 5 — CI round-trip gate

The live-smoke pattern moves into CI: boot a Genesis, mint two scoped
tokens, and run the full clone → branch → push → `gh pr create` → approve
(second token) → `gh pr merge` (all three strategies) → re-clone →
`git fsck` flow, plus the denial cases (anonymous push, author
self-approve, non-FF without `force` scope, conflicting merge → 409) and
the REST shape suite. PRs that break the gate do not merge.

## What v1 deliberately does not include

Recorded as deferrals (PARITY.md rows, not silent omissions): webhook
delivery, GraphQL, full 3-way content merge, a PR web UI (the workflow is
`gh`-first; VISION keeps source browsing out of scope), multi_ack
negotiation, and pack delta *emission* — the last conditional on the
Genesis-vs-GitHub benchmark in this same effort: if clone latency misses
the at-least-match bar and wire size is the dominant lever, delta emission
comes into scope.

## Compatibility and risks

- Auth lands first so every endpoint added afterward is born gated; the
  live smokes are updated in the same PR (push stops being
  `RequiresAuth:false`).
- The merge engine is the riskiest code: mitigations are the
  clean-tree-only restriction, SHA-equality tests against real `git`,
  `git fsck` in CI, and conflict-409 rather than guessing.
- Existing pushes keep working: a fast-forward push behaves exactly as
  today; only non-FF updates change behavior (they now require `force`).
