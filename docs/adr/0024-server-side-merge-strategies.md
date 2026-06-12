# ADR-0024: Server-Side Merge Strategies and Conflict Semantics

## Status

Accepted

## Context

`PullRequest.Merge` and `Repository.MergePullRequest` have declared the
`scm_merge_pr` WASM integration since the specs were written, but the module
was never implemented — there is no way to merge a pull request in Genesis.
Implementing it requires the server to author new git objects: a merge or
squash commit, possibly new trees, and a compare-and-swap ref advance. GitHub
performs full blob-level three-way content merges with conflict detection;
that engine is large and is the riskiest possible code in a system whose
product is byte-exact git compatibility.

The byte-level primitives already exist and are parity-tested:
`crates/git_object` emits hash-identical commit/tree bytes (including a
two-parent merge commit), and `scm_ingest_pack` proves the composite
sub-write shapes for writing objects and advancing refs.

## Decision

v1 of `scm_merge_pr` supports three strategies with a deliberately restricted
merge engine:

- **Fast-forward**: when the base branch tip is an ancestor of the PR head,
  advance the ref by CAS. No new objects.
- **Squash**: author one new single-parent commit whose tree is the PR head's
  tree, parented on the base tip.
- **Merge**: author one new two-parent commit, accepted only when the
  three-way resolution is *clean at tree level* — no path was modified on
  both sides since the merge base. The resulting tree takes each side's
  changes path-by-path.

When both sides modified the same path (divergent content), the merge
endpoint returns **HTTP 409** with a body naming the conflicting paths and
the remedy: rebase or merge locally, push, and retry. Genesis never guesses
content resolution in v1.

The merge base is computed by a bounded, paged walk of the Commit DAG read
through OData. Every object the engine authors is also written to the
ADR-0011 raw-object cache, and every engine-produced merge is verified in
tests by performing the identical merge with real `git` and comparing SHAs.

## Consequences

- `gh pr merge --merge/--squash` and fast-forward merges work end-to-end;
  the CI round-trip gate exercises all three strategies plus the 409 path.
- Agents whose branches conflict resolve conflicts locally, where their
  tooling already works — Genesis stays out of content-resolution guessing.
- Full blob-level three-way merge is a recorded follow-up, listed in
  `docs/PARITY.md` as a known divergence from github.com behavior (GitHub
  would merge cleanly when changes touch different hunks of the same file;
  Genesis v1 returns 409 for that case).
- The engine's failure mode is refusal (409), never a wrong merge commit.
