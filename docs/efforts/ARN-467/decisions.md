# ARN-467 — decisions (genesis)

## Permit the deployment's own public domain in the WASM `http_call` gate

**Decision.** Add `genesis-production-164d.up.railway.app` to the self-host
domain list in all five `policies/wasm.cedar` permits, rather than changing the
guests to call loopback.

**Came up because.** After bumping Genesis to the latest temper kernel, every
git operation returned 504 with `guest integration timed out before submitting
response head`. Datadog gave the cause:

```
WASM host authorization denied outbound HTTP call
  custom.module: git_refs_advertise
  custom.http_method: GET
  custom.domain: genesis-production-164d.up.railway.app
  custom.reason: no matching permit policy
```

The guests reconstruct their callback base from the inbound `Host` header
(`temper_api_from_headers`, wasm/git_upload_pack/src/lib.rs:81), so behind
Railway they call the public domain. All five permits hardcoded
`["127.0.0.1", "localhost"]`, so nothing matched — the four pre-existing
permits were as broken as the six git/REST modules with no permit at all.

**Options.**
- *(rejected default)* Add the six git/REST modules to the policy but keep the
  loopback-only domain list — what the previous commit (2d6dbf8) did. Still
  denied; the module list was never the whole problem.
- Force the guests to `http://127.0.0.1:3000` and delete the Host-header
  derivation. Removes a header-controlled URL and needs no policy change.
- Add the deployment's own domain to the existing domain lists.
- One tenant-wide `forbid` carrying the domain list once, with the per-module
  permits dropping their domain clause.

**Chose the domain list over forcing loopback because** the guests' non-`/tdata`
call (`PUT /_internal/blobs`, from upload-pack/receive-pack) cannot be served by
the in-process router and must reach a real listening port. The port comes from
Railway's `$PORT` at runtime (`railway.toml` start command), and a WASM guest
has no way to read it — pinning `:3000` in the guest would be a latent trap the
day `PORT` changes. The kernel already classifies `RAILWAY_PUBLIC_DOMAIN` as
self (`env_local_tdata_hosts`, temper-server/src/state/mod.rs:285), so `/tdata`
calls to the public domain are still served in-process; only the blob PUT takes
the real edge, which is what this deployment already did on the previous kernel.
Given up: the Host header still selects the callback base, so the Cedar domain
list is the only thing keeping a `Host: attacker.com` request from making a
guest call out with its `X-Tenant-Id` headers. That gate now works as intended,
but the smell stays.

**Chose repetition over one `forbid` because** OS-app Cedar files are
concatenated into a single per-tenant policy text (`merge_bundle_policies`,
temper-platform/src/os_apps/mod.rs:82). A blanket `forbid` on
`action == Action::"http_call"` would apply to every other app installed on the
same kernel and kill their legitimate outbound calls (e.g. temper-agent
reaching a model provider). Cedar has no constants, so the list is repeated five
times with a header comment binding them together. The durable fix — a
kernel-supplied `context.is_self_host` derived from `local_tdata_hosts`, so the
list has exactly one home — is recorded as follow-up, not done here.

**Where.** `policies/wasm.cedar`; follows 2d6dbf8 (which added the six missing
modules but kept the wrong domain list).
