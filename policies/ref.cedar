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
