// Git object Cedar policies — Blob, Tree, TreeEntry, Commit, Tag.
//
// Git objects are append-only and content-addressed by SHA-1: their
// identity IS their bytes. Cedar here is only a read gate, piggy-backed
// on the owning repository's visibility. Writes are not addressed
// here — object rows are produced by spec-triggered integrations such
// as scm_write_file, scm_merge_pr, and Repository.IngestPack's
// scm_ingest_pack result path, which the kernel applies as declared
// composite sub-writes.
//
// Reads: any principal that can read Repository with the same
// RepositoryId can read the object.

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Blob
) when {
  resource.RepositoryId == context.repositoryId &&
  context.repositoryVisibility == "public"
} unless {
  principal.agent_type == "anonymous" &&
  context.repositoryVisibility != "public"
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Blob
) when {
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Tree
) when {
  context.repositoryVisibility == "public" ||
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is TreeEntry
) when {
  context.repositoryVisibility == "public" ||
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Commit
) when {
  context.repositoryVisibility == "public" ||
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Tag
) when {
  context.repositoryVisibility == "public" ||
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

// Authenticated read access by scope: the visibility conditions above
// reference repository context the kernel does not yet inject, so they
// error and never apply. repo:read tokens may read git object rows;
// anonymous visibility is gated by the wire/REST modules against the
// Repository row. Per-repo confidentiality for token holders awaits
// kernel context injection (recorded residual).
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Blob
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Tree
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is TreeEntry
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Commit
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Tag
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write")
};

// The git wire protocol modules read object rows under the trusted
// system/admin identity to stream packs.

permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is Blob
);

permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is Tree
);

permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is TreeEntry
);

permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is Commit
);

permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is Tag
);

// --- Writes: only system principals (integrations) may create object
//     rows. Human / agent writes flow through Repository actions
//     (WriteFile, BatchWriteFiles, IngestPack); integrations return
//     data and the kernel applies declared composite sub-writes.

permit(
  principal,
  action in [Action::"Create"],
  resource is Blob
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Blob
) when {
  principal has action_context &&
  (principal.action_context == "composite:Repository.IngestPack" ||
   principal.action_context == "composite:Repository.MergePullRequest")
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Tree
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Tree
) when {
  principal has action_context &&
  (principal.action_context == "composite:Repository.IngestPack" ||
   principal.action_context == "composite:Repository.MergePullRequest")
};

permit(
  principal,
  action in [Action::"Create"],
  resource is TreeEntry
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"Create"],
  resource is TreeEntry
) when {
  principal has action_context &&
  (principal.action_context == "composite:Repository.IngestPack" ||
   principal.action_context == "composite:Repository.MergePullRequest")
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Commit
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Commit
) when {
  principal has action_context &&
  (principal.action_context == "composite:Repository.IngestPack" ||
   principal.action_context == "composite:Repository.MergePullRequest")
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Tag
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"Create"],
  resource is Tag
) when {
  principal has action_context &&
  (principal.action_context == "composite:Repository.IngestPack" ||
   principal.action_context == "composite:Repository.MergePullRequest")
};

// --- BlobObject: the raw-object cache behind the git wire.
//
// `BlobObject` is not a Genesis entity — it is the kernel's internal blob
// store, gated at `/_internal/blobs` by `read_blob_object` /
// `write_blob_object` (temper-server/src/blobs.rs). temper-git keeps every git
// object there under `git-objects/{repository_id}/{sha}.b64`, and
// upload-pack/receive-pack read and write that cache on the hot path. With no
// permit a clone dies with `object-cache GET <sha> returned HTTP 403`.
//
// This gate cannot be the repository-level control, because it never sees a
// git identity. The kernel edge does not understand a GitToken, so an inbound
// git request is `anonymous` to it; temper-git authenticates the caller itself
// inside the guest (`git_auth`) and every repository read it performs goes
// through repository.cedar under the resolved principal. By the time an object
// is fetched the caller has already been authorized for that repository — so
// the check that belongs here is the namespace, not the principal.
//
// Reaching the endpoint at all requires passing the kernel's global auth
// middleware, which `/_internal/blobs` is not exempt from: either a real tenant
// credential (operator-level, already privileged) or a capability the kernel
// itself minted for an in-process guest call to loopback. It is not reachable
// unauthenticated from the internet.
//
// Residual, recorded deliberately: an operator-credential holder can read any
// repository's objects through this path if they know the SHA. That is not a
// new grant — an operator can already read the object rows directly — but it
// does mean this permit must stay scoped to the `git-objects/` namespace so it
// never widens to other tenants' overflow blobs.

permit(
  principal,
  action in [Action::"read_blob_object", Action::"write_blob_object"],
  resource is BlobObject
) when {
  resource.id like "git-objects/*" ||
  resource.id like "field-overflow/*"
};

// `field-overflow/` is the kernel's own storage for entity field values too
// large to keep inline (ADR-0045). Genesis puts git object CanonicalBytes there,
// so the install path reads it on every tree it materializes — scoping this
// permit to `git-objects/` alone left the bundle endpoint failing with
// "Genesis field overflow blob ... not found" on data that was present and
// intact. The reader reports any non-200 as "not found", so an authorization
// denial was indistinguishable from missing data, which is worth fixing
// separately.
//
// Wider than the git-object namespace, deliberately and with the same
// reasoning: reaching `/_internal/blobs` at all requires either a real tenant
// credential or a capability the kernel minted for an in-process guest call, so
// this is not reachable unauthenticated. It is not scoped per repository
// because these keys are content-addressed by sha256 and carry no owner.
