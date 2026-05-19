// Commons-mode Owner verification guardrails.
//
// Owner rows are public namespace claims. A caller may create a pending claim
// only for their own authenticated account; provider/operator verification is
// required before the namespace can write apps, repositories, refs, or blobs.

forbid(
  principal,
  action == Action::"Create",
  resource is Owner
) when {
  !(principal has accountId &&
    resource.AccountId == principal.accountId) &&
  !(principal has scopes &&
    principal.scopes.contains("admin:owners")) &&
  !(principal is Admin) &&
  !(principal is System)
};

forbid(
  principal,
  action in [
    Action::"MarkVerified",
    Action::"SetQuota",
    Action::"Suspend",
    Action::"Unsuspend"
  ],
  resource is Owner
) when {
  !(principal has scopes &&
    principal.scopes.contains("admin:owners")) &&
  !(principal is Admin) &&
  !(principal is System)
};
