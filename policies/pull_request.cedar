// PullRequest Cedar policies — draft, open, review, merge, close.

// --- Read: same visibility rules as the owning repo.

// Scope-based read: the visibility/owner conditions below reference
// repository context the kernel does not yet inject, so that permit
// errors and never applies (the left operand of the `||` errors before
// admin:repos is reached). A token with repo:read suffices; the
// IngestPack composite reads open PRs to update their heads under this
// permit. Per-repo confidentiality awaits kernel context injection
// (recorded residual; mirrored in ref.cedar and objects.cedar).
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is PullRequest
) when {
  principal.scopes.contains("repo:read") ||
  principal.scopes.contains("repo:write") ||
  principal.scopes.contains("admin:repos")
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is PullRequest
) when {
  context.repositoryVisibility == "public" ||
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

// --- Create/Open: pr:write + principal is OpenedBy, or admin:repos.

// --- Row creation (POST /tdata/PullRequests): pr:write. Authorship
// conditions live on the spec actions; row attrs are empty at create.
permit(
  principal,
  action == Action::"create",
  resource is PullRequest
) when {
  principal.scopes.contains("pr:write") ||
  principal.scopes.contains("admin:repos")
};

permit(
  principal,
  action in [Action::"Create", Action::"Open"],
  resource is PullRequest
) when {
  principal.scopes.contains("admin:repos") ||
  (principal.scopes.contains("pr:write") &&
   resource.OpenedBy == principal.id)
};

// OpenedBy is empty while Create/Open are evaluated (resource attrs are
// not populated at action time), so the author-binding clause above
// errors and never applies. pr:write suffices to open a PR; the REST
// module stamps OpenedBy from the caller. Author-only conditions are
// enforced where the attr exists (e.g. Approve). Recorded residual
// pending kernel action-param-as-resource-attr injection.
permit(
  principal,
  action in [Action::"Create", Action::"Open"],
  resource is PullRequest
) when {
  principal.scopes.contains("pr:write")
};

// --- Receive-pack updates PR heads only from the Repository.IngestPack
//     composite. The git wire adapter never dispatches this action.

permit(
  principal,
  action == Action::"UpdateHead",
  resource is PullRequest
) when {
  principal has action_context &&
  principal.action_context == "composite:Repository.IngestPack"
};

// --- Author actions: only OpenedBy may request review on their own PR.

// The author-binding clauses below reference resource.OpenedBy, which
// is empty while the action is evaluated (resource attrs are not
// populated at action time), so each errors and never applies —
// denying even legitimate callers. Authorization is therefore
// scope-based here, and the author semantics (only the author opens
// review; the author cannot approve their own PR) are enforced by the
// github_rest_pulls module, which reads OpenedBy from the PR row before
// dispatching. Recorded residual: move these back into Cedar once the
// kernel injects action params / resource attrs at evaluation time.

permit(
  principal,
  action in [Action::"RequestReview", Action::"Close"],
  resource is PullRequest
) when {
  principal.scopes.contains("pr:write") ||
  principal.scopes.contains("admin:repos")
};

// The github_rest_pulls module bridges the Open -> UnderReview hop as
// its trusted admin identity before recording a reviewer's verdict
// (the reviewer's own scopes gate Approve/RequestChanges below).
permit(
  principal is Admin,
  action == Action::"RequestReview",
  resource is PullRequest
);

permit(
  principal,
  action in [
    Action::"Approve",
    Action::"RequestChanges",
    Action::"DismissReview"
  ],
  resource is PullRequest
) when {
  principal.scopes.contains("pr:write")
};

// --- Number backfill (scm_assign_pr_number): the module assigns the
// sequential PR number with an OData field PATCH (Cedar action
// "update") under its admin:repos system identity, since the spec has
// no number-carrying action. Recorded residual: replace with a
// declared action once one exists.
permit(
  principal,
  action in [Action::"update", Action::"AssignNumber"],
  resource is PullRequest
) when {
  principal.scopes.contains("admin:repos")
};

// --- Merge: pr:merge scope. The state machine gates state (Approved).

permit(
  principal,
  action == Action::"Merge",
  resource is PullRequest
) when {
  principal.scopes.contains("pr:merge")
};
