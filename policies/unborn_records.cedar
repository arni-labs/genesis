// A record exists once its Create has run. Before that, an id names nothing.
//
// The kernel starts a record in its initial state the first time any action
// names its id, so an action sent to an id that was never created would act on
// an empty record: one with no owner, no repository and no principal, which
// the owner and scope permits elsewhere cannot reason about. These forbids
// refuse every action on such a record except the ones that bring it into
// being (OData create and the spec's Create) and reads, which find nothing.
// Each type is keyed on a field its Create always sets.
//
// App, AppInstallation, HttpEndpoint, Owner and RateLimit are left out: the
// kernel itself keeps records under those type names (its app registry,
// Genesis installs, route and host-call checks, account verification and rate
// buckets) and acts on them with its own lifecycle, without these fields.

forbid(principal, action, resource is Repository) unless {
  resource has OwnerAccountId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Ref) unless {
  resource has RepositoryId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Commit) unless {
  resource has repository_id ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Tree) unless {
  resource has repository_id ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is TreeEntry) unless {
  resource has repository_id ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Blob) unless {
  resource has repository_id ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Tag) unless {
  resource has repository_id ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is PullRequest) unless {
  resource has RepositoryId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Review) unless {
  resource has PullRequestId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is ReviewComment) unless {
  resource has ReviewId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Webhook) unless {
  resource has RepositoryId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is Lineage) unless {
  resource has ChildRepositoryId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

forbid(principal, action, resource is GitToken) unless {
  resource has PrincipalId ||
  action in [Action::"create", Action::"Create", Action::"read", Action::"list"]
};

// Closure.ResolveClosure computes and stores a new closure on a fresh id.
forbid(principal, action, resource is Closure) unless {
  resource has Root ||
  action in [Action::"create", Action::"Create", Action::"ResolveClosure", Action::"read", Action::"list"]
};
