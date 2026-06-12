# Genesis vs GitHub — measured performance

Production Genesis on Railway against github.com, same repository on both
hosts, measured 2026-06-12 with `scripts/bench-genesis-vs-github.sh`.
Two independent passes; numbers below give both (pass 1 / pass 2).

## Method

- **Hosts:** `https://genesis-production-164d.up.railway.app` (genesis
  `bb680bbd`, kernel `aca5e392`, Railway us-east + Postgres) and
  `https://github.com`. Client: macOS, residential network, same machine
  and hour for both hosts.
- **Corpus:** synthetic repo, 30 commits, ~50 source/doc files plus a
  220 KiB binary refreshed 4 times across history; identical tip
  `3ddb2020` on both hosts (`bench-author/genesis-bench-corpus` on
  Genesis, `rita-aga/genesis-bench-corpus` on GitHub).
- **Measure:** wall-clock per operation, p50 over 5 runs per pass. Both
  remotes authenticated the same way (token in the URL for git; same
  REST calls with each host's token). The Genesis PR-merge measurement
  times only `PUT .../merge`; the reviewer approval Genesis requires
  between open and merge is untimed plumbing, so the timed operation is
  identical on both hosts.
- Cold clone uses a fresh directory each run; wire size is the received
  packfile on disk (the fetched pack is stored verbatim; the locally
  built `.idx` is excluded).

## Results (p50, pass 1 / pass 2)

| Operation | Genesis | GitHub | Genesis ÷ GitHub |
|---|---|---|---|
| `git ls-remote` | 329 / 330 ms | 485 / 403 ms | **0.74× (faster)** |
| Cold clone | 2249 / 2252 ms | 670 / 625 ms | **3.5× (slower)** |
| Warm fetch (no-op) | 343 / 305 ms | 427 / 384 ms | **0.80× (faster)** |
| Push (1 empty commit) | 921 / 943 ms | 934 / 851 ms | **1.04× (parity)** |
| REST PR open | 545 / 600 ms | 1049 / 1042 ms | **0.55× (faster)** |
| REST PR merge | 735 / 763 ms | 1875 / 1918 ms | **0.40× (faster)** |
| Pack wire size | 1.72 MiB | 1.30 MiB | 1.32× |
| Clone throughput | 0.76 MiB/s | 1.94 / 2.09 MiB/s | 0.38× |

Derivations: ratios divide the mean of the two Genesis p50s by the mean
of the two GitHub p50s. Throughput = pack bytes ÷ cold-clone p50.

## Reading

Against the plan's bar — *within ~1× of GitHub per operation class* —
Genesis **meets or beats the bar on five of six classes**, and the
GitHub-workflow operations this effort added are the strongest: PR open
is ~2× faster and PR merge ~2.5× faster than github.com, with ref
advertisement, no-op fetch, and push at or better than parity.

**Cold clone misses the bar at ~3.5×.** Two factors separate cleanly:

- *Wire size* is only 1.32× GitHub's (Genesis emits whole objects;
  GitHub delta-compresses). If size were the whole story the gap would
  be ~1.3×, not 3.5×.
- The remaining ~2.7× is **pack-emission speed**: assembling the pack
  from entity rows and the raw-object cache at 0.76 MiB/s. This is
  server-side time, not network.

So delta emission alone (the contingency named in the plan) would
recover at most ~25% of the gap on this corpus. The recorded follow-up
is therefore ordered: (1) profile the emission path — row reads,
object-cache hits, pack assembly — which the ADR-0011 cache layers were
built to serve; (2) pack delta emission for the wire-size factor.
Tracked as a Track A residual in PR #25's handover.

## Caveats

- Small-repo regime (1.7 MiB pack, 233 objects). Large-repo streaming
  behavior is exercised by the install smokes (paw-patrol cold ~3 s)
  but is not part of this measurement.
- One client vantage point. Railway us-east vs GitHub's CDN-fronted
  edge; absolute numbers shift with geography, ratios less so.
- GitHub PR-merge latency includes their mergeability machinery; the
  comparison is end-to-end API latency for the same client call, which
  is what a `gh`-driven workflow experiences.

## Reproducing

```
GENESIS_REMOTE=https://<token>:x@<genesis-host>/<owner>/<repo>.git \
GITHUB_REMOTE=https://<user>:<token>@github.com/<owner>/<repo>.git \
GENESIS_TOKEN=<push-capable token> GITHUB_TOKEN=<repo-scope token> \
GENESIS_API=https://<genesis-host>/api/v3 GENESIS_REST_REPO=<owner>/<repo> \
GENESIS_TOKEN_A=<author token> GENESIS_TOKEN_B=<reviewer token> \
GITHUB_API=https://api.github.com GITHUB_REST_REPO=<owner>/<repo> \
RUNS=5 scripts/bench-genesis-vs-github.sh
```
