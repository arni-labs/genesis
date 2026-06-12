# ADR-0025: Push-Path Authentication and Force-Push Classification

## Status

Accepted

## Context

Two enforcement holes make the governed-remote story partly decorative on
the main write path:

1. `git_receive_pack` performs no authentication. The refs-advertise and
   upload-pack handlers resolve `GitToken`s (SHA-256 hash lookup, scopes
   forwarded as `X-Temper-Principal-Scopes`), but push relies on endpoint
   registration alone, and the live smokes register the push endpoint with
   `RequiresAuth:false`. `GitToken.MarkUsed` has never been fired by any
   code path.
2. The push path never classifies non-fast-forward updates. `scm_ingest_pack`
   maps every non-create/non-delete ref command to plain `Ref.Update`; the
   compare-and-swap check passes whenever the client sends the correct old
   SHA, so a history-rewriting push with a correct old tip is applied. The
   Cedar `force`-scope gate on `Ref.ForceUpdate` exists but is unreachable,
   and protected-branch force-push rejection is therefore not enforced.

VISION.md sells scoped, programmatic credentials and policy-as-code at every
transition; both must be true on push, the highest-stakes write.

## Decision

- `git_receive_pack` adopts the shared GitToken resolution used by the read
  handlers. Anonymous push receives the standard git smart-HTTP 401
  challenge. Successful resolution fires `GitToken.MarkUsed` (best-effort —
  usage telemetry must not block a push).
- `scm_ingest_pack` classifies every ref update by walking the commit DAG
  from the new tip toward the old tip with an explicit bounded depth. If the
  old tip is not reachable, the sub-write dispatches `Ref.ForceUpdate`;
  Cedar then requires the `force` scope. Fast-forward pushes are unchanged.
- The upload-pack anonymous fallback to the system principal is removed;
  anonymous reads resolve to an anonymous principal whose access Cedar
  decides explicitly per deployment mode (operator vs commons).
- Live smokes and CI register the push endpoint with auth required and mint
  scoped tokens for the round-trip flows.

## Consequences

- Every REST endpoint added by RFC-0004 is born behind the same token model
  rather than retrofitted later.
- Force-pushes now fail without the `force` scope — including for existing
  automation. Tokens used by legitimate history-rewriting jobs must be
  re-minted with the scope; this is an intended behavior change, called out
  in the PR.
- A bounded ancestry walk adds reads to non-FF pushes only; fast-forward
  pushes (the overwhelmingly common case) pay one ancestor check that
  terminates on the first-parent chain quickly.
- The audit question "which agent pushed what, with which credential" now
  has a complete answer: principal, scopes, and `MarkUsed` trail.
