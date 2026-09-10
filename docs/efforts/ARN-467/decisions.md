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

## Permit `read_blob_object` / `write_blob_object` on the git-object namespace

**Decision.** Add a `BlobObject` permit to `policies/objects.cedar` scoped to
keys matching `git-objects/*`, keyed on the object namespace rather than on the
calling principal.

**Came up because.** With `http_call` and `access_secret` fixed, a clone got past
ref advertisement and then failed with `object-cache GET <sha> returned HTTP
403`. The kernel logged it exactly:

```
internal blob object access denied
  action: read_blob_object   principal_id: anonymous
  key: git-objects/rp-temperpaw-paw-compute/62e183d1….b64
  reason: no matching permit policy
```

`/_internal/blobs` has its own gate (`require_blob_object_authorization`,
temper-server/src/blobs.rs:33) and Genesis had no `BlobObject` policy at all.

**Options.**
- Permit by principal — e.g. only the repository's authorized reader.
- Permit by object namespace (`git-objects/*`), any principal that reaches the
  endpoint.
- Widen `is_public_kernel_request` to exempt `/_internal/blobs`.

**Chose the namespace over the principal because** this gate structurally cannot
see a git identity. The kernel edge does not understand a GitToken, so an
inbound git request is `anonymous` to it — confirmed empirically: an
authenticated clone with `gt-paw-agent` produced the identical
`principal_id: anonymous` deny. temper-git authenticates the caller *inside* the
guest via `git_auth`, and every repository read it then performs goes through
repository.cedar under the resolved principal. By the time an object is fetched
the caller has already been authorized for that repository, so a principal
condition here could only ever be a tautology or a lockout.

**Rejected exempting the path from the auth middleware because** that would make
the object cache reachable unauthenticated from the internet. Keeping the
middleware means the endpoint is reachable only with a real tenant credential or
a capability the kernel itself minted for an in-process guest call to loopback.

**Residual, recorded deliberately:** an operator-credential holder can read any
repository's objects through this path given the SHA. That is not a new grant —
an operator can already read the object rows directly — but it is why the permit
stays scoped to `git-objects/` and must not widen to other overflow-blob
namespaces.

**Note on scope discovery.** I twice claimed the gate class was closed and was
twice wrong, because I enumerated by grepping one call form. The kernel's
non-entity Cedar gates are `http_call`/HttpEndpoint, `access_secret`/Secret,
`read_blob_object`+`write_blob_object`/BlobObject, `submit_specs`/SpecRegistry,
`manage_policies`/PolicySet, `execute_repl`/Repl and
`manage_decisions`/AuthorizationDenied. The blob actions are invisible to an
action-name grep because they are passed as a variable; only the resource type
is greppable. Enumerate by resource type as well as action name.

**Where.** `policies/objects.cedar`.

## Derive the guests' OData base from the kernel's loopback origin, not the Host header

**Decision.** Add `temper_api_base(ctx, headers)` to `git_upload_pack`,
`git_receive_pack` and `git_refs_advertise`: prefer the kernel's own loopback
origin, derived by stripping `/_internal/blobs` off the `blob_endpoint` secret,
and keep the Host header only as a fallback.

**Came up because.** With all four policy fixes live, ref advertisement and the
blob cache worked but a clone still died:

```
fatal: remote error: Commits(4441bf07…) status 401
```

The split is in `LocalTDataWasmHost`: `http_call` tries `local_http_call` and
serves `/tdata` in-process, while `http_stream_begin_outbound` delegates
straight through with no interception. `fetch_refs_for_repo` uses `http_call`
(intercepted → 200); the object-row lookup uses `streaming_get` →
`streaming_call` (not intercepted). That request therefore left the process to
whatever base the guest had — the Host-derived *public* domain — and
`is_internal_url` matches only the kernel's configured internal origin, so no
capability was minted, and the request re-entered through the public edge with
no bearer and hit the global auth middleware: 401.

The blob cache is the control that proves it: it uses the *same* `streaming_get`
path and returns 200, because its URL comes from the `blob_endpoint` secret,
which is loopback and therefore *is* the internal origin.

**Options.**
- Switch the object-row lookup from `streaming_get` to `ctx.http_call` so
  `LocalTDataWasmHost` intercepts it.
- Teach `LocalTDataWasmHost` to intercept the streaming path too (kernel).
- Give the guests the kernel's loopback origin so every call — streaming or
  not — is classified internal and gets a capability.

**Chose the loopback base because** it fixes the whole class in one place rather
than one call site. Switching that single lookup to `http_call` would have left
every other streaming call in these guests with the same latent bug, and the
`prefetch` path next to it fails identically. Teaching the streaming path to
intercept is the deeper fix but it is a kernel change with real streaming
semantics to get right, and it is not needed once the guests address the kernel
by the origin it actually recognises.

**Why `blob_endpoint` and not a hardcoded port:** the kernel seeds that secret
as `http://127.0.0.1:{port}/_internal/blobs` from its real listen port
(temper-cli/src/serve/mod.rs), so the derived base tracks the port automatically.
This is what the previous decision's `PORT` caveat was worried about, and it
removes the concern rather than living with it. If an operator points
`BLOB_ENDPOINT` at external object storage, the suffix does not match and the
Host header fallback still applies.

**Also fixes, deliberately:** the guests no longer take their callback origin
from an attacker-controllable request header. That smell was recorded as a
residual in the first decision; it is closed here rather than left open.

**Where.** `wasm/git_upload_pack/src/lib.rs`, `wasm/git_receive_pack/src/lib.rs`,
`wasm/git_refs_advertise/src/lib.rs`, plus their rebuilt `.wasm` artifacts.

## Instrument the Genesis object lookup instead of guessing at the 404

**Decision.** Add a temporary `tracing::warn!` to both silent return paths in
`load_genesis_object_by_key` (temper submodule, branch
`claude/arn467-genesis-bundle-diagnostic`) and deploy Genesis on it, rather than
attempting a fix against a hypothesis.

**Came up because.** With git working end to end, the bundle endpoint still
returned 404 instantly for every well-formed request, and it is the code
`install-from-genesis` runs, so installing an app through Genesis is blocked on
it. Everything observable from outside checks out: the row reads back 200 over
OData by the exact composite key the kernel builds
(`Commits('rp-paw-agent-dsf-factory-77150dbf…')`), with `fields.Id`,
`fields.RepositoryId` and `fields.TreeSha` all matching what the comparison
uses; tenant is the same value on both sides (`export_genesis_registry_bundle`
does `let tenant = TenantId::new(registry_tenant)`); `validate_git_object_id` is
pass-through; and the `read_app_bundle` Cedar gate is passed, since a denial
there returns 403 rather than this 404.

I also tested and *disproved* my own leading hypothesis: warming the entity with
an OData read immediately before the bundle call changed nothing, so
`ensure_entity_loaded` failing is not the explanation on its own.

**Options.**
- Guess at the most likely cause and ship a fix.
- Add the diagnostic, deploy, read one line, then fix precisely.
- Leave it and pursue a clone-based install instead.

**Chose instrumenting because** the function has exactly two ways to return
`Ok(None)` and *neither logs anything*, which is why the cause is invisible from
outside — that absence is itself the defect that made this expensive. A fix
chosen without knowing which branch fires would be a guess against production
authorization-adjacent code.

**Rejected the clone-based install** after checking it rather than assuming: the
clone does carry the complete bundle at the pinned commit (app.toml, 16 specs,
5 policies, 291 wasm files at `77150db`), but there is no supported surface to
install it. `submit_specs` takes specs and not policies — that is exactly the
half-install that leaves all 11 `Dsf.Factory` collections answering 403 — and
local OS-app install was deliberately removed (`install_app` now returns "local
OS-app install is removed from the normal agent path; install pinned Genesis
refs through App.Install or /api/genesis/apps/install"). It would also produce
no `owner/app@hash` pinned ref to verify against.

**Where.** temper `70b76d27`; genesis submodule bump `9718282`. Diagnostic only,
no behaviour change; to be reverted or promoted to a permanent log line once the
cause is known.

## Remove the `Id` workaround rather than keep it

**Decision.** Delete the git-sha comparison in `load_genesis_object_by_key`
instead of the earlier change that made it accept either form, and warn at
registration when a CSDL declares a server-derived field name.

**Came up because.** Rita challenged the pattern — every fix revealing another —
as a symptom of local patches rather than real fixes, and she was right about
this one. `Id`, `id`, `Status`, `status` are formally server-derived
(`temper_spec::automaton::is_server_derived_field_name`), and
`canonicalize_entity_field_map` overwrites them with the entity id and state on
every hydrate. So `fields["Id"]` in that function is *always* the entity id and
never the git sha it was being compared against. Accepting both forms papered
over a comparison that never meant anything.

**Chose deleting the comparison because** it is not needed at all: `entity_id`
is derived from `(repository_id, git_sha)` by `genesis_object_entity_id`, so
once the row is loaded at that key the only independent fact left is whether it
belongs to the requested repository. One meaningful check replaces a meaningless
one.

**Chose warning at registration over rejecting** because Genesis and other
existing apps already declare these names; failing registration would take them
down. The defect being fixed is the *silence* — an app may declare `Id`, nothing
objects, and the value is then destroyed on the actor path while OData still
reports the declared property.

**Where.** `crates/temper-platform/src/genesis_install.rs`,
`crates/temper-server/src/registry/mod.rs` (temper `35fd32cd`).

## Require a declared `Size` only where the model has one

**Decision.** Treat a declared length as optional in git object materialization,
rather than requiring `Size` on every kind.

**Came up because.** With the `Id` fix in, the bundle endpoint moved 404 → 500
`Genesis object is missing a non-negative Size`. Only `Blob` declares `Size`;
`Tree`, `Commit` and `Tag` never have. The requirement arrived in temper
`8840b4fd` (2026-07-11) — *after* Genesis pinned its kernel — so the two sides
disagreed about the data model and nothing caught it until the bump.

**Chose correcting the July change over adding `Size` to Genesis's Tree spec**
because a git object's canonical bytes are self-describing and `git_object_body`
already rejects any object whose `{kind} {len}\0` header disagrees with its own
body. `Size` is a redundant second check that only blobs can offer. Adding it to
trees would mean a data migration over every git object row to satisfy a check
that adds nothing.

**Given up:** where no length is declared, the exact-length assertion no longer
runs; the budget is charged from an upper bound on the encoded length instead,
so materialization stays bounded.

**Where.** `crates/temper-platform/src/genesis_install/blob_materialization.rs`
(temper `e18de36b`).

## Serve a public app bundle without a credential

**Decision.** Allow `GET /api/genesis/apps/{owner}/{name}/versions/{hash}/bundle`
through the edge and refuse inside the handler unless the backing repository is
public, rather than adding registry-credential plumbing to the install client.

**Came up because.** The install client sends only `X-Tenant-Id` and no bearer,
while the bundle endpoint required an authenticated context — verified on
production, 401. There is no registry credential anywhere in the kernel for the
client to send.

**Options.** Add a credential to the installing kernel and send it as a bearer;
add a `registry_token` to the install request; serve public bundles anonymously.

**Chose the public-bundle path because** the same content is *already* served to
anonymous callers over `git clone` — requiring a credential for one encoding and
not the other was inconsistent rather than protective. It is also the only option
that ships entirely on Genesis: the others change the kernel the installing side
runs (openpaw), which would mean a coordinated deploy of the very service the
factory work depends on, to unblock the factory work.

**Given up / bounded:** the handler trusts `X-Tenant-Id` as a namespace selector
on this path. That cannot escalate anything, because the only rows reachable are
ones already world-readable over git, and a non-public repository still answers
401. Recorded rather than hidden.

**Where.** `crates/temper-platform/src/tenant_api/apps.rs`,
`crates/temper-server/src/authz/edge.rs` (temper `795934a2`).

## Permit the `field-overflow` blob namespace

**Decision.** Widen the `BlobObject` permit from `git-objects/*` to also cover
`field-overflow/*`.

**Came up because.** With the public-bundle path working, the bundle failed with
`Genesis field overflow blob field-overflow/sha256/4a962177… not found`. The blob
was not missing: fetched directly it returned **403**, not 404. Genesis stores git
object `CanonicalBytes` in the kernel's field-overflow namespace whenever they
exceed the inline ceiling, and my earlier permit — scoped to `git-objects/` —
did not cover it. My own defect, introduced two commits earlier.

**Chose widening over per-repository scoping because** these keys are
content-addressed by sha256 and carry no owner, so there is nothing to scope
them by. Reaching `/_internal/blobs` still requires a real tenant credential or
a kernel-minted capability, so the namespace is not reachable unauthenticated.

**Worth noting separately:** the reader reports any non-200 as "not found", so an
authorization denial was indistinguishable from missing data. That cost real time
— I went looking for lost blobs and considered republishing the app.

**Where.** `policies/objects.cedar`.

## Give the streaming blob read the same legacy fallback as the buffered read

**Decision.** Move the legacy DB blob-store fallback into `stream_blob_object`,
adding `BlobObjectStream::from_bytes` so the legacy store's bytes are returned in
the same shape as the object store's stream.

**Came up because.** After the permit above, the blob returned **200** over HTTP
and the bundle *still* reported it missing. The kernel has two blob reads and
they disagreed:

```rust
get_blob_with_legacy_fallback(...)   // object store, then legacy DB store
stream_blob_object(...)              // object store only
```

Objects written before the object-store migration live in the legacy DB store.
The HTTP blob route finds them; the streaming read the bundle uses did not. The
same blob was simultaneously readable and "not found".

**Chose fixing the kernel over republishing the app because** the data was never
missing — republishing would have rewritten blobs to work around a reader that
cannot see half its own store, leaving every previously-written object still
unreadable through the streaming path.

**Chose the shared shape deliberately:** callers cannot tell which store
answered. Two reads of the same store disagreeing about what exists is the bug;
hiding the difference behind one type is the fix, not an abstraction for its own
sake.

**Where.** `crates/temper-server/src/blob_store/state.rs`,
`crates/temper-server/src/blob_store/streaming.rs` (temper `3cc6461e`).

**Pattern across this effort, worth stating once.** Three separate failures had
one shape — two paths to the same data that do not agree:

1. `Id` means the domain field over OData and the entity id through the actor.
2. `http_call` is served in-process while the streaming host path is delegated out.
3. Blob existence differs between the buffered and streaming reads.

Each surfaced as a misleading error far from its cause ("commit not found",
"401", "blob not found") on data that was present and correct. When a lookup
insists something is missing that you can see with your own eyes, suspect a
second read path before suspecting the data.

## Do not assert a decoded length the model never declared

**Decision.** Route the undeclared-length overflow read through the bounded
stream and decode it directly, instead of the JSON-base64 stream decoder.

**Came up because.** This was my own regression from the earlier `Size` change.
With no declared `Size` I passed the 16 MiB ceiling as the decoder's
`expected_decoded_bytes` — but that argument is an *exact expectation*, not a
cap, so every tree failed with `decoded blob ended at 328 bytes; expected
16777216`. 328 was the correct size; 16777216 was a number I invented.

**Chose decoding directly over deriving the length because** base64 padding makes
the exact decoded length underivable from the encoded length: `serialized_bytes`
maps to three possible decoded sizes, which is precisely why the code demanded a
declared `Size` in the first place. There is genuinely nothing to assert here, so
asserting anything would be a guess dressed as a check. `git_object_body` already
rejects any object whose `{kind} {len}\0` header disagrees with its own body,
which is the real integrity check and is independent of the declared size.

**Second correction in the same commit — a documented intent I had contradicted.**
`stream_blob_object` carried a comment stating that large field-overflow objects
are deliberately *not* read from the legacy database fallback, because that
interface is buffered. My previous commit added exactly that fallback and read
the whole object before checking its size, silently overriding a recorded
decision. Now the fallback asks the store to bound the read
(`get_blob_if_size_at_most`) so it never materializes an object above the
caller's ceiling, and the comment says what the code actually does. The intent —
never buffer a large blob — is preserved; only the "therefore pretend it does not
exist" part is gone.

**Where.** `crates/temper-platform/src/genesis_install/blob_materialization.rs`,
`crates/temper-server/src/blob_store/state.rs` (temper `c3595b47`).

**Note on method.** Two of the defects in this effort were mine, introduced while
fixing something else, and both were caught only because each fix was verified by
its effect rather than assumed. Deploying and re-reading the actual error is what
kept the chain honest.

## A guest's internal calls run as the guest, not as its caller

**Decision.** For the HttpEndpoint path, bind the guest's internal HTTP
capability to the module's own principal rather than to the inbound caller's
security context. Rita chose this over two narrower options.

**Came up because.** `git push` returned 401 with a valid GitToken, and the
failure was symmetric in a way that made it hard to see:

- **Anonymous caller:** resolving the presented token means reading a `GitToken`
  row. Bound to the anonymous caller, that read is denied, the lookup returns
  nothing, and the guest reports anonymous. Authentication cannot run as the
  identity it is about to establish.
- **Authenticated caller — worse:** the same secret turned out to be registered
  *both* as `gt-paw-agent`'s `HashedSecret` and as the id of an Active
  `AgentCredential`. So the edge authenticated the push, the request was not
  anonymous, and the lookup ran as that principal — which has no GitToken read
  permission either. Being authenticated at the edge was strictly worse than
  arriving anonymous.

My first attempt fixed only the anonymous branch and did nothing, because the
caller was never anonymous. That is recorded here because the wrong fix looked
right and shipped.

**Options.**
- *(A, chosen)* Guest always acts as its own module principal.
- *(B)* Permit the AgentCredential's principal to read GitTokens — smaller, but
  papers over the layering and needs repeating for every future caller.
- *(C)* Un-register the AgentCredential so pushes arrive anonymous — trivial, but
  fixes one token and leaves the next person to rediscover it.

**Chose A because** an HttpEndpoint guest is the enforcement point for its own
protocol. Genesis resolves the token itself and applies repository authorization
inside the module; it cannot delegate that to the kernel, because the resolved
git principal has no way to reach the kernel — the headers that carried it are
stripped on purpose (ARN-208/255). Making the guest act as itself matches what it
actually is, and leaves authorization to the tenant's policy, where each module's
reach is narrow by construction.

**Given up, deliberately:** a guest no longer inherits its caller's reach for
internal calls. That is correct for a protocol guest, which was never enforcing
on the caller's behalf, and would be wrong for a guest that expects the kernel to
scope its reads — so it applies to the HttpEndpoint path only, not to
action-triggered integrations.

**Where.** `crates/temper-server/src/state/dispatch/wasm.rs` (temper `12c590de`),
with `policies/git_token.cedar` granting the six wire modules read/list and
MarkUsed.

## Raise the bundle byte budget rather than shrink the app

**Decision.** `MAX_GENESIS_BUNDLE_TOTAL_BYTES` 64 MiB → 256 MiB. Rita chose this
over stripping symbol names from the WASM modules.

**Came up because.** dsf-factory is ~52 MB of legitimate compiled WASM — 59
modules, one per resource operation, averaging 638 KB. Against a 64 MiB total
that leaves ~12 MB for every app in its dependency closure combined. Verified it
is not junk: no `target/`, no debug sections; the size is real code carrying Rust
symbol names.

**Options.** Strip symbol names (30–50% smaller, loses function names in stack
traces); raise the budget; reduce module count (a redesign).

**Chose raising it because** the budget's job is to bound how much one install
may materialize, not to cap an app below a size the platform's own apps already
exceed. Stripping trades debuggability for headroom we can simply grant, and the
module count is a design question that should not be forced by an install limit.

**Kept unchanged:** per-file 16 MiB and file-count 4096. Those are what actually
catch a runaway publish — the paw-fs case tripped the aggregate only incidentally,
and weakening them while relieving the total would have removed the real guard.

**Where.** `crates/temper-platform/src/genesis_install/bundles.rs`.

## Let a protocol handler see the credential it is required to resolve

**Decision.** Add `ForwardsCredential` to HttpEndpoint, off by default, and
default it on for the six handlers that implement credential-carrying protocols.
Rita chose this over having the kernel resolve GitTokens itself.

**Came up because.** `git push` answered 401 with a valid token, and three
successive fixes changed nothing. Instrumenting `resolve_principal` produced no
log at all — which was the answer: it returns anonymous on its *first* line, the
one exit I had not instrumented.

`guest_visible_headers` strips `Authorization` before a guest sees it (ARN-208:
a caller credential must never reach a WASM guest). Genesis authenticates git
callers by reading the GitToken from that header. So `extract_token` found
nothing, every request was anonymous, and the kernel was removing the only thing
the app could authenticate with.

My three earlier attempts were all downstream of this: I kept repairing what
happens *after* the token is found while the token never arrived.

**Options.** *(A)* Kernel resolves the GitToken and passes the identity —
honours the invariant, but teaches the kernel a Genesis concept and is real
design work. *(B, chosen)* Endpoints opt into seeing the header. *(C)* Move git
auth out of the guest entirely — cleanest, largest.

**Chose B, scoped so the invariant survives.** Off by default; the original test
proving credentials never reach a guest is unchanged and still passes. The six
opted-in handlers are keyed on integration module and overridable per endpoint —
the same shape as the pack-size defaults already in that function.

**Why this is not a hole in ARN-208.** The invariant protects against a guest
inheriting a *caller's kernel authority*. The header these endpoints receive is a
GitToken in HTTP Basic: opaque to the kernel, carrying no kernel authority, and
the app's to resolve. Withholding it protects nothing and guarantees every
authenticated git request arrives anonymous. What is forwarded is not a
credential the kernel could act on.

**Given up:** these six endpoints now see an inbound `Authorization` header, so a
compromised git guest could read a token presented to it. That is the same token
it is being asked to authenticate, so the exposure is bounded to the request's
own credential — it gains nothing it was not already handed.

**Covered by test in both directions:** stripped unless opted in, present when
opted in, so a future widening has to defeat an assertion rather than slip past.

**Where.** `crates/temper-server/src/http_endpoint.rs`,
`crates/temper-server/src/router.rs`, `router_test.rs` (temper `958d1efa`).

**Method note.** Every exit from `resolve_principal` returns `anonymous`, so a
broken lookup and a bad token are indistinguishable from outside — three wrong
fixes came from that. The function is now instrumented at all five exits.
