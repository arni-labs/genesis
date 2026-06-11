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
