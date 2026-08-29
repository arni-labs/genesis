# Review instructions

## Passes
Run four passes and tag each finding with its pass:
- Bugs: logic errors, broken edge cases, state machines that can strand or lose an acknowledged write
- Security: auth bypasses on the wire protocols (smart-HTTP, OData), token leakage in logs or errors, unvalidated external input at the git receive path
- Contract fidelity: byte-exactness against real git behavior for the wire surface; GitHub REST subset responses matching the documented shape; registry state machines (App, Repository, Ref) moving only through their declared transitions
- Determinism and durability: unordered iteration where order is observable, unbounded reads of pack data, partial-write recovery on the object store

## What Important means here
Reserve Important for findings that would corrupt or lose repository data, serve wrong bytes on the wire, bypass authorization, or let registry state diverge from git reality. Style and naming are nits.

## Cap the nits
At most five nits per review; summarize the rest as a count.

## Do not report
Generated artifacts and anything CI already enforces.
