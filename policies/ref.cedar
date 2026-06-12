// Ref Cedar policies — branch/tag pointers.
//
// CAS updates (expected old SHA + new SHA) are the normal fast-forward
// path and require repo:write. ForceUpdate (non-FF) additionally
// requires `force` scope; deletion of a protected branch requires
// admin:repos. The matching invariants live in ref.ioa.toml; this
// file is the scope gate.

// --- Read: same visibility rules as the owning repo.

// Repository.IngestPack owns git receive-pack ref mutations through the
// spec-declared Composite sub-write contract. The kernel authorizes the parent
// Repository.IngestPack action once and applies declared ref_updates with CAS;
// the git wire WASM adapter never dispatches Ref actions directly.

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Ref
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
  resource is Ref
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write")
};
// The git wire protocol modules (refs-advertise, upload-pack) read
// refs under the trusted system/admin identity to build the
// advertisement and pack; the repository-level access was already
// resolved. Per-repo confidentiality for anonymous callers awaits
// kernel visibility-context injection (recorded residual).
permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is Ref
);

// --- Create + fast-forward Update: repo:write.

permit(
  principal,
  action in [Action::"Create", Action::"Update"],
  resource is Ref
) when {
  (principal.scopes.contains("repo:write") ||
   principal.scopes.contains("admin:repos")) &&
  (context.repositoryOwnerAccountId == principal.accountId ||
   principal.scopes.contains("admin:repos"))
};

// --- App registry composites create and advance backing refs through the
//     spec-declared sub-write contract. The kernel stamps action_context while
//     applying the composite result; WASM only returns data.

permit(
  principal,
  action == Action::"Create",
  resource is Ref
) when {
  principal has action_context &&
  principal.action_context == "composite:App.Fork"
};

permit(
  principal,
  action == Action::"Update",
  resource is Ref
) when {
  principal has action_context &&
  principal.action_context == "composite:App.PublishNewVersion"
};

// --- Repository.IngestPack (git push): the parent action is Cedar-gated
//     on the push path (repo:write); its declared ref sub-writes carry the
//     composite action_context. CAS Create, fast-forward Update, and Delete
//     are authorized as one governed intent. Non-FF ForceUpdate is gated
//     separately on the `force` scope below — the ingest WASM only emits a
//     ForceUpdate sub-write after an ancestry walk proves the update is
//     non-fast-forward, so this is where force-push protection is enforced.
permit(
  principal,
  action in [Action::"Create", Action::"Update", Action::"Delete"],
  resource is Ref
) when {
  principal has action_context &&
  principal.action_context == "composite:Repository.IngestPack"
};

// Non-fast-forward push (ForceUpdate sub-write) additionally requires the
// `force` scope — the carried token scopes gate history rewrites.
permit(
  principal,
  action == Action::"ForceUpdate",
  resource is Ref
) when {
  principal has action_context &&
  principal.action_context == "composite:Repository.IngestPack" &&
  principal.scopes.contains("force")
};

// The server-side merge engine (scm_merge_pr, ADR-0024) advances the
// target ref as a declared sub-write of Repository.MergePullRequest;
// the parent action is Cedar-gated on pr:merge.
permit(
  principal,
  action in [Action::"Create", Action::"Update"],
  resource is Ref
) when {
  principal has action_context &&
  principal.action_context == "composite:Repository.MergePullRequest"
};

// --- ForceUpdate: repo:write AND `force`.

permit(
  principal,
  action == Action::"ForceUpdate",
  resource is Ref
) when {
  principal.scopes.contains("repo:write") &&
  principal.scopes.contains("force") &&
  (context.repositoryOwnerAccountId == principal.accountId ||
   principal.scopes.contains("admin:repos"))
};

// --- Delete: repo:write + not the default branch.

permit(
  principal,
  action == Action::"Delete",
  resource is Ref
) when {
  (principal.scopes.contains("repo:write") ||
   principal.scopes.contains("admin:repos")) &&
  resource.Name != context.repositoryDefaultBranch
};

// --- Never allow deleting the default branch without admin:repos.

forbid(
  principal,
  action == Action::"Delete",
  resource is Ref
) when {
  resource.Name == context.repositoryDefaultBranch &&
  !principal.scopes.contains("admin:repos")
};
