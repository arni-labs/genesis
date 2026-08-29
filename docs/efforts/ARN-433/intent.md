# ARN-433: genesis needs its review lens

## Problem
The panel injects each repo's root REVIEW.md into every reviewer prompt;
genesis is the only gated repo without one, so its reviews run on global
guidance alone.

## Proposed outcome
A genesis-specific REVIEW.md: wire-protocol byte-exactness, registry
state-machine fidelity, git-data durability as first-class review passes.
