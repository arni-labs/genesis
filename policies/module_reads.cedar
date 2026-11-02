// Genesis's git and REST modules read repository state over local OData.
//
// Two kinds of module make those reads, and the kernel identifies them
// differently:
//
// - The HttpEndpoint guests (wire and REST handlers) run their internal calls
//   as the guest itself (principal Agent::"<module>"), never as the caller they
//   are serving, so the caller-facing read permits in repository.cedar,
//   ref.cedar and objects.cedar cannot match those calls. The guests decide who
//   may see a repository themselves, with genesis_git_auth::may_read_repository,
//   before they serve anything.
// - The action-triggered modules (pack parser, merge engine, PR numbering) run
//   as the caller that caused the transition, and the kernel adds
//   context.module, the name of the module making the call (temper
//   state/dispatch/wasm.rs, LocalTDataWasmHost). The principal is whoever was
//   already allowed to make that move, so the permit names the module, not the
//   principal. context.module is a kernel-only key; no request or record field
//   can set it.
//
// This permit only lets those modules load the rows their work and their
// response need. Without it, these reads pass only where some other installed
// app's policy happens to permit every read on the tenant; a Genesis with only
// its own policies answers every clone with a timeout, and the private-read
// check fails closed for everyone.
//
// Scoped to Genesis's own modules (any other agent reading private repository
// rows directly would bypass the visibility rule) and to repository entities.

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource
) when {
  (
    (
      principal is Agent &&
      [
        Agent::"git_refs_advertise",
        Agent::"git_upload_pack",
        Agent::"git_receive_pack",
        Agent::"github_rest_repos",
        Agent::"github_rest_refs",
        Agent::"github_rest_pulls"
      ].contains(principal)
    ) ||
    (
      context has module &&
      ["scm_ingest_pack", "scm_merge_pr", "scm_assign_pr_number"].contains(context.module)
    )
  ) &&
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

// The registry composites' module (app_registry: RegisterNewApp, Fork,
// PublishNewVersion, Install) reads the repository, its default ref, the
// published commit and a fork's parent App as the caller, under the same
// kernel-set context.module.
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource
) when {
  context has module && context.module == "app_registry" &&
  (resource is Repository || resource is Ref || resource is Commit || resource is App)
};
