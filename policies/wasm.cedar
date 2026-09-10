// WASM host-function policies.
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
  ["127.0.0.1", "localhost"].contains(context.domain)
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
  ["127.0.0.1", "localhost"].contains(context.domain)
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
  ["127.0.0.1", "localhost"].contains(context.domain)
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
  ["127.0.0.1", "localhost"].contains(context.domain)
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
  ["127.0.0.1", "localhost"].contains(context.domain)
};
