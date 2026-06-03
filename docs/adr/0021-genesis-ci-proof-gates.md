# 0021 Genesis CI Proof Gates

## Status

Accepted

## Context

Directed Evolution gap-closure review depends on Genesis Mission Control and
Genesis app/version diff surfaces being trustworthy. Genesis PRs previously had
no GitHub Actions checks, so reviewers had to trust local command output for
Rust canonicalization tests, Svelte diagnostics, production web builds, and the
Playwright Directed Evolution regression.

The fresh Agent Answers proof cycle still needs production credentials, but the
pre-production proof surfaces can be guarded in CI.

## Decision

Genesis will run CI on pull requests, pushes to `main`, and manual dispatch.
The workflow checks:

- Rust workspace formatting and tests with the pinned nightly toolchain.
- Genesis web dependency installation from `package-lock.json`.
- Svelte diagnostics.
- Production web build.
- Playwright E2E over Chromium desktop and mobile projects.

The workflow does not yet enable `cargo clippy -- -D warnings` because existing
canonical SHA-1 code triggers a lint unrelated to Directed Evolution closure.
That can become a future tightening once the existing lint debt is handled.

## Consequences

- Genesis PR #17 and future Directed Evolution UI/diff changes can provide
  first-party check evidence instead of only local proof notes.
- Mission Control proof-gate regressions are caught by the same Playwright test
  that renders the Agent Answers Datadog evidence and terminal-success gate.
- The CI contract stays aligned with checks that currently pass locally, avoiding
  a knowingly-red gate while still improving review confidence.
