// Temper answers an action on a missing record by creating it in its initial
// state, and Cedar then sees only id and status. Every Genesis Create sets the
// field named in each policy, so a resource without it is a record this call
// would bring into being. Only the creating action may do that: every other
// action is refused unless the record has a non-empty value in that field.
//
// App, AppInstallation, HttpEndpoint, Owner and RateLimit are left out: the
// kernel keeps its own records under those type names (its app registry,
// Genesis installs, route and outbound-call checks, account verification and
// rate buckets) and acts on them without these fields.
//
// Cost: each forbid reads a resource attribute for every action, so list reads
// of these types are decided per row (on that one attribute) instead of once
// per page.

forbid (principal, action, resource is Repository) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has OwnerAccountId && resource.OwnerAccountId != "") };
forbid (principal, action, resource is Ref) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has RepositoryId && resource.RepositoryId != "") };
forbid (principal, action, resource is Commit) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has repository_id && resource.repository_id != "") };
forbid (principal, action, resource is Tree) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has repository_id && resource.repository_id != "") };
forbid (principal, action, resource is TreeEntry) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has repository_id && resource.repository_id != "") };
forbid (principal, action, resource is Blob) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has repository_id && resource.repository_id != "") };
forbid (principal, action, resource is Tag) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has repository_id && resource.repository_id != "") };
forbid (principal, action, resource is PullRequest) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has RepositoryId && resource.RepositoryId != "") };
forbid (principal, action, resource is Review) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has PullRequestId && resource.PullRequestId != "") };
forbid (principal, action, resource is ReviewComment) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has ReviewId && resource.ReviewId != "") };
forbid (principal, action, resource is Webhook) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has RepositoryId && resource.RepositoryId != "") };
forbid (principal, action, resource is Lineage) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has ChildRepositoryId && resource.ChildRepositoryId != "") };
forbid (principal, action, resource is GitToken) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has PrincipalId && resource.PrincipalId != "") };
forbid (principal, action, resource is Closure) unless { action in [Action::"create", Action::"Create", Action::"read", Action::"list"] || (resource has Root && resource.Root != "") };
