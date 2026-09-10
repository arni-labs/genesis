// WASM host-function policies.
//
// SELF-HOST DOMAIN LIST. Every permit below ends with the same list of hosts
// that are *this deployment itself*:
//
//     127.0.0.1, localhost, genesis-production-164d.up.railway.app
//
// A git/REST guest reconstructs its callback base from the inbound Host
// header (see `temper_api_from_headers` in wasm/git_upload_pack/src/lib.rs),
// so behind Railway it calls the public domain rather than loopback. The
// kernel already classifies that domain as self — `env_local_tdata_hosts`
// folds RAILWAY_PUBLIC_DOMAIN into the local-TData host set, so a `/tdata`
// call to it is served in-process — but the Cedar gate runs *outside* that
// router (the stack is AuthorizedWasmHost(LocalTDataWasmHost(...))), so the
// gate denies before the in-process shortcut is ever reached. Omitting the
// public domain here kills every guest with "no matching permit policy".
//
// Cedar has no constants, so the list is repeated per permit. Keep the five
// copies identical. ARN-467 tracks moving this to a kernel-supplied
// `context.is_self_host` so the list has exactly one home.
//
// Repository.IngestPack's parser integration may read local TData so it can
// resolve existing delta bases and find open PRs whose SourceRef matches an
// incoming ref update. It still returns data only; the kernel applies writes
// through the Repository.IngestPack Composite contract.

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "scm_ingest_pack" &&
  context.method == "GET" &&
  [
    "127.0.0.1", "localhost",
    "genesis-production-164d.up.railway.app"
  ].contains(context.domain)
};

// App.Fork's registry integration may read the parent App row. It returns
// declared sub-write data only; the kernel applies App.Fork's Composite
// contract.

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "app_registry" &&
  context.method == "GET" &&
  [
    "127.0.0.1", "localhost",
    "genesis-production-164d.up.railway.app"
  ].contains(context.domain)
};

// PullRequest.Open's number-assignment integration scans the repo's
// PullRequest rows (GET) and backfills the computed Number onto the
// opened row (PATCH). Spec-triggered modules pass through the Cedar
// WASM gate, so both verbs need an explicit local permit
// (scm_assign_pr_number, RFC-0004 Slice 3).

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "scm_assign_pr_number" &&
  ["GET", "POST"].contains(context.method) &&
  [
    "127.0.0.1", "localhost",
    "genesis-production-164d.up.railway.app"
  ].contains(context.domain)
};

// PullRequest.Merge / Repository.MergePullRequest's merge engine
// (scm_merge_pr, ADR-0024) walks the Commit DAG and reads Refs/Trees/
// Blobs over local OData GETs. It returns Composite sub-write data
// only; the kernel applies the writes. (Its raw-object cache PUTs go
// to the local _internal/blobs endpoint, which the kernel's WASM gate
// already exempts.)

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "scm_merge_pr" &&
  context.method == "GET" &&
  [
    "127.0.0.1", "localhost",
    "genesis-production-164d.up.railway.app"
  ].contains(context.domain)
};

// Git smart-HTTP and the GitHub-compatible REST surface authenticate every
// inbound caller through `git_auth`, which SHA-256s the presented token and
// reads the matching GitToken row over local OData (GET), then dispatches the
// resolved entity action through the kernel's action bridge (POST). Without an
// explicit permit the WASM host denies that lookup, `git_auth` can never
// resolve a principal, and the guest never submits a response head — the caller
// sees "guest integration timed out" and git is dead.
//
// These modules were exempt in practice until the kernel began enforcing the
// WASM http_call gate for every spec-triggered module; the four permits above
// were written when only those four reached the gate. Raw-object PUTs from
// upload-pack/receive-pack go to the local `_internal/blobs` endpoint, which
// the kernel's WASM gate already exempts, so they need nothing here.
permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  [
    "git_refs_advertise",
    "git_upload_pack",
    "git_receive_pack",
    "github_rest_repos",
    "github_rest_refs",
    "github_rest_pulls"
  ].contains(context.module) &&
  ["GET", "POST"].contains(context.method) &&
  [
    "127.0.0.1", "localhost",
    "genesis-production-164d.up.railway.app"
  ].contains(context.domain)
};

// The raw-object cache lives at the kernel's internal blob endpoint. The
// kernel seeds that address itself as the platform secret `blob_endpoint`
// (`http://127.0.0.1:{port}/_internal/blobs`, temper-cli/src/serve/mod.rs) and
// gates a guest's read of it through Cedar as `access_secret` on `Secret`.
//
// Without this permit the secret is filtered out of the host's bootstrap set
// (`get_authorized_wasm_host_bootstrap_secrets`), and three things follow at
// once: the kernel builds no local-blob interceptor, `bind_local_blob_endpoint`
// installs no gate exemption for that endpoint, and the guest falls back to
// `{Host-header base}/_internal/blobs`. The object fetch then leaves the
// process, re-enters through the public edge with no bearer, and dies on the
// auth middleware — a clone fails with
// `object-cache GET <sha> returned HTTP 401`.
//
// Scoped to the one key and the four modules that read it; this is an address,
// not a credential, but the gate is per-key so it stays per-key.
permit(
  principal is Agent,
  action == Action::"access_secret",
  resource is Secret
) when {
  resource.id == "blob_endpoint" &&
  [
    "git_upload_pack",
    "git_receive_pack",
    "scm_ingest_pack",
    "scm_merge_pr"
  ].contains(context.module)
};
