// Temper answers an action on a missing record by creating it in its initial
// state, and Cedar then sees only id and status. Every Genesis Create sets the
// field named in each policy, so a resource without it is a record this call
// would bring into being. Only the creating action may do that.
//
// Exemptions beyond create, Create, read and list, each because its resource
// is never a stored row:
// - App: RegisterNewApp runs on the id of the App it is about to create;
//   read_app_bundle and install_app name App::"owner/name@hash" with only
//   owner, name and versionHash attributes.
// - HttpEndpoint: http_call is the kernel's check on a guest's outbound call,
//   with the guest's entity type as the resource and no row behind it.

forbid (principal, action, resource is Repository) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has OwnerAccountId && resource.OwnerAccountId != "") };
forbid (principal, action, resource is App) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list", Action::"RegisterNewApp", Action::"read_app_bundle", Action::"install_app"] || (resource has OwnerId && resource.OwnerId != "") };
forbid (principal, action, resource is AppInstallation) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has AppId && resource.AppId != "") };
forbid (principal, action, resource is Owner) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has AccountId && resource.AccountId != "") };
forbid (principal, action, resource is RateLimit) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has OwnerId && resource.OwnerId != "") };
forbid (principal, action, resource is Closure) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has Root && resource.Root != "") };
forbid (principal, action, resource is Lineage) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has ChildRepositoryId && resource.ChildRepositoryId != "") };
forbid (principal, action, resource is GitToken) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has HashedSecret && resource.HashedSecret != "") };
forbid (principal, action, resource is Webhook) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has RepositoryId && resource.RepositoryId != "") };
forbid (principal, action, resource is PullRequest) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has RepositoryId && resource.RepositoryId != "") };
forbid (principal, action, resource is Ref) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has RepositoryId && resource.RepositoryId != "") };
forbid (principal, action, resource is Review) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has PullRequestId && resource.PullRequestId != "") };
forbid (principal, action, resource is ReviewComment) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has ReviewId && resource.ReviewId != "") };
forbid (principal, action, resource is HttpEndpoint) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list", Action::"http_call"] || (resource has PathPrefix && resource.PathPrefix != "") };
