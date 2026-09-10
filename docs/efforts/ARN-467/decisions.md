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
the in-process router and must reach a real listening port. The port is
whatever Railway supplies: the start command is `temper serve --port
${PORT:-3000}` (`railway.toml`). The genesis service has no `PORT` variable set
today, so it does listen on 3000 and pinning that in the guest would work right
now — but a WASM guest cannot read the environment, so the day Railway injects
`PORT` (as it already does on the sibling temper-server service) the guests
would lose their callback with nothing tying the breakage to the port. That is a
latent trap, not a fix. The kernel already classifies `RAILWAY_PUBLIC_DOMAIN` as
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

## Permit the `blob_endpoint` bootstrap secret for the four modules that read it

**Decision.** Add an `access_secret` permit for `resource.id == "blob_endpoint"`
scoped to `git_upload_pack`, `git_receive_pack`, `scm_ingest_pack` and
`scm_merge_pr`, rather than pointing the kernel's internal API base at the
public domain.

**Came up because.** With the `http_call` gate fixed, ref advertisement and the
REST surface returned 200 but a real clone still failed:

```
fatal: remote error: object-cache GET 62e183d19e11… returned HTTP 401
```

`/tdata` calls are served in-process by `LocalTDataWasmHost`, which is why refs
worked. `/_internal/blobs` is not under `/tdata`, so it is not; it is supposed to
be handled by a separate binary interceptor. That interceptor is only built when
the host's bootstrap secret set contains `blob_endpoint`
(`local_blob_binary_interceptor`, wasm.rs:97), and the same value is what
`bind_local_blob_endpoint` uses to install the gate exemption. The kernel does
seed the secret at startup (`http://127.0.0.1:{port}/_internal/blobs`,
temper-cli/src/serve/mod.rs) — but `get_authorized_wasm_host_bootstrap_secrets`
filters every key through `gate.authorize_secret_access`, which asks Cedar for
`access_secret` on `Secret`. `wasm.cedar` had no secret permits at all, so the
key was dropped, no interceptor was built, no exemption was installed, and the
guest fell back to `{Host-header base}/_internal/blobs` — out through the public
edge, no bearer, 401.

**Options.**
- Set `TEMPER_API_URL` to the public origin, so `is_internal_url` classifies the
  guest's public-domain callback as internal and the host mints an internal
  capability bearer for it.
- Set a `BLOB_ENDPOINT` env var to the public origin's blob path.
- Permit `access_secret` for `blob_endpoint` so the kernel's own seeded loopback
  value reaches the guest.

**Chose the secret permit because** it is the configuration the kernel already
expects: it makes the blob fetch an in-process call that opens no socket at all,
which is both faster and strictly safer than the alternatives. It also disposes
of the port question raised in the previous decision — the seeded value contains
`{port}`, but since the interceptor matches the URL and serves it internally,
the port is only a string both sides agree on and is never dialed.

**Rejected `TEMPER_API_URL` = public origin because** `is_internal_url` matches
on exact scheme+host+port against a single configured base, so pointing it at
the public origin would stop classifying loopback as internal, and would send
freshly minted capability bearers out across the public edge on every guest
callback. `host_trait.rs:691` is explicit that this classification is
server-owned precisely so a request-bound capability is never redirected;
widening it to the public origin to work around a missing policy line inverts
that intent. Rejected `BLOB_ENDPOINT` for the same reason plus it would leave the
interceptor unable to match, keeping the network hop.

**Where.** `policies/wasm.cedar`.
