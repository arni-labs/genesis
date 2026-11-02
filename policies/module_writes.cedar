// The writes Genesis's own modules make, each under the module's name.
//
// The kernel runs an HttpEndpoint guest's internal calls as the guest itself
// (principal Agent::"<module>"), never as the caller it is serving, and an
// action-triggered module as the caller that caused the transition, with
// context.module naming it. A caller's GitToken never reaches Cedar on either
// path. So each REST module decides the write on the token and the Repository
// row first (crates/git_auth/src/write.rs, the same rules as the caller-facing
// permits in repository.cedar, ref.cedar, pull_request.cedar and review.cedar),
// and Cedar lets that module make exactly the writes its routes perform.
//
// The REST create route's permits are in repository.cedar (#58). The git push
// path needs none here: git_receive_pack authorizes the push and bridges
// Repository.IngestPack, whose sub-writes carry the composite action_context.

// github_rest_pulls: open, review, close and merge pull requests.
permit(
  principal == Agent::"github_rest_pulls",
  action in [
    Action::"create",
    Action::"Create",
    Action::"Open",
    Action::"RequestReview",
    Action::"Approve",
    Action::"RequestChanges",
    Action::"Close"
  ],
  resource is PullRequest
);

permit(
  principal == Agent::"github_rest_pulls",
  action in [Action::"create", Action::"Create"],
  resource is Review
);

permit(
  principal == Agent::"github_rest_pulls",
  action == Action::"MergePullRequest",
  resource is Repository
);

// Repository.MergePullRequest's declared PullRequest.Merge sub-write. The
// parent action was decided above; the sub-write carries its action_context.
permit(
  principal,
  action == Action::"Merge",
  resource is PullRequest
) when {
  principal has action_context &&
  principal.action_context == "composite:Repository.MergePullRequest"
};

// scm_assign_pr_number stamps the per-repository number when a pull request
// opens (an OData field PATCH, Cedar action "update"). It runs as whoever
// opened it, so the permit names the module.
permit(
  principal,
  action in [Action::"update", Action::"AssignNumber"],
  resource is PullRequest
) when {
  context has module && context.module == "scm_assign_pr_number"
};

// github_rest_refs: create, advance, force and delete refs through REST.
permit(
  principal == Agent::"github_rest_refs",
  action in [
    Action::"create",
    Action::"Create",
    Action::"Update",
    Action::"ForceUpdate",
    Action::"Delete"
  ],
  resource is Ref
);
