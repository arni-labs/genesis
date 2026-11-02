// Genesis's git and REST modules read repository state over local OData.
//
// The kernel runs a guest's internal calls as the guest itself (principal
// Agent::"<module>"), never as the caller it is serving, so the caller-facing
// read permits in repository.cedar, ref.cedar and objects.cedar cannot match
// those calls. The modules decide who may see a repository themselves, with
// genesis_git_auth::may_read_repository, before they serve anything; this
// permit only lets them load the rows that decision and the response need.
//
// Without it, these reads pass only where some other installed app's policy
// happens to permit every read on the tenant. A Genesis with only its own
// policies answers every clone with a timeout, and the private-read check
// fails closed for everyone.
//
// The action-triggered modules (pack parser, merge engine, PR numbering) read
// /tdata as the caller that caused the transition, with no module attribute
// (temper state/dispatch/wasm.rs, LocalTDataWasmHost), so no Genesis permit can
// name them yet; their reads still rely on the tenant's other policies.
//
// Scoped to Genesis's own guests (any other agent reading private repository
// rows directly would bypass the visibility rule) and to repository entities.

permit(
  principal is Agent,
  action in [Action::"read", Action::"list"],
  resource
) when {
  [
    Agent::"git_refs_advertise",
    Agent::"git_upload_pack",
    Agent::"git_receive_pack",
    Agent::"github_rest_repos",
    Agent::"github_rest_refs",
    Agent::"github_rest_pulls"
  ].contains(principal) &&
  (
    resource is Repository ||
    resource is Ref ||
    resource is Commit ||
    resource is Tree ||
    resource is TreeEntry ||
    resource is Blob ||
    resource is Tag ||
    resource is PullRequest ||
    resource is Review ||
    resource is ReviewComment
  )
};
