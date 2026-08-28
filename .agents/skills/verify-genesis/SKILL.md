---
name: verify-genesis
description: Verify Genesis (the Temper-native git/registry server) end to end - boot it, drive the real read surface, prove the state machines moved. Use before declaring any Genesis change done.
---

# Verify Genesis

Genesis is the source of truth for app installs; a broken read path breaks
every deploy downstream. Verification drives the REAL surfaces, never proxies.

## Launch

Per README: build the WASM modules, then serve Temper with Genesis
bootstrapped (the README's serve command). Wait for the HTTP port.

## Drive (the features that matter)

1. **Smart-HTTP read**: `git ls-remote` a seeded repo through the wire
   protocol. A ref list is the pass.
2. **Bundle read**: GET `/api/genesis/apps/{owner}/{name}/versions/{hash}/bundle`
   for a seeded app returns 200 with files.
3. **Publish flow**: push a commit to a seeded repo, dispatch
   `PublishNewVersion`, then read back `Apps('{id}')` over OData and confirm
   `LatestVersionHash` MOVED to the pushed hash - the state machine, not the
   HTTP 200, is the proof.
4. **Registry install read**: the pinned-ref bundle URL for the new hash
   serves the new content.

## Prove

Read entities back over OData after every action. Capture command + output
per feature. Against production, use read-only probes (1, 2, 4) only - never
publish test data to the prod registry outside a dedicated throwaway app.

## Feature map

features/ next to this skill; keep it in sync when surfaces change
(maintain-verification-skill).
