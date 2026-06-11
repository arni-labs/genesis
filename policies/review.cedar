// Review Cedar policies.
//
// A Review is a reviewer's verdict on a PR. Reviewer identity is
// captured as ReviewerPrincipal on the row; Cedar ensures only that
// principal can Dismiss their own review (authors cannot silence
// reviews against them).

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Review
) when {
  context.repositoryVisibility == "public" ||
  context.repositoryOwnerAccountId == principal.accountId ||
  principal.scopes.contains("admin:repos")
};

// --- Create: any pr:write principal. The review row is the audit
// record; author-cannot-approve is enforced where it bites, on
// PullRequest.Approve (whose resource attrs exist at evaluation time).
// This previously named a nonexistent "Submit" action and referenced
// never-injected context, denying review creation for every customer
// token.

permit(
  principal,
  action == Action::"Create",
  resource is Review
) when {
  principal.scopes.contains("pr:write")
};

// --- Dismiss: the reviewer themselves, or admin:repos.

permit(
  principal,
  action == Action::"Dismiss",
  resource is Review
) when {
  resource.ReviewerPrincipal == principal.id ||
  principal.scopes.contains("admin:repos")
};
