// GitToken Cedar policies.
//
// Token creation and revocation are the sharpest surface in the app —
// a rogue Create grants a long-lived bearer credential. Rules:
//
//   * Create / Revoke by a principal for their OWN tokens requires
//     admin:tokens (a principal minting their own tokens is fine if
//     they already have the scope; it's how rotation works).
//   * Create / Revoke by a principal on someone ELSE's tokens
//     requires admin:tokens AND the actor is a system/admin agent
//     (support path, audited).
//   * MarkUsed is fired by the auth integration on every successful
//     auth — system-only.
//
// Read: the token's own principal, or admin:tokens.

// --- Module-internal token resolution: the wire/REST WASM handlers
// look tokens up under their own admin-kind module identity (no
// customer scopes). Without this, every authenticated caller silently
// degraded to anonymous and MarkUsed never fired.
permit(
  principal is Admin,
  action in [Action::"read", Action::"list"],
  resource is GitToken
);

// The same module-internal token resolution, under the identity the kernel
// actually supplies. The comment above describes handlers looking tokens up
// "under their own admin-kind module identity" — that was asserted with
// `X-Temper-Principal-Kind` headers, which the kernel now strips on purpose and
// replaces with the module's own principal (Agent, id = module name). Without a
// permit for it the lookup returns nothing, every caller degrades to anonymous,
// and `git push` answers 401 with a perfectly valid token.
//
// Scoped to the six wire/REST handlers that authenticate callers. A module may
// read GitTokens only to resolve the credential presented to it; nothing here
// grants Create, Revoke, or any customer scope.
permit(
  principal is Agent,
  action in [Action::"read", Action::"list"],
  resource is GitToken
) when {
  [
    Agent::"git_refs_advertise",
    Agent::"git_upload_pack",
    Agent::"git_receive_pack",
    Agent::"github_rest_repos",
    Agent::"github_rest_refs",
    Agent::"github_rest_pulls"
  ].contains(principal)
};

// MarkUsed fires on every successful auth check and is likewise performed by
// the module, not by the caller it just authenticated.
permit(
  principal is Agent,
  action == Action::"MarkUsed",
  resource is GitToken
) when {
  [
    Agent::"git_refs_advertise",
    Agent::"git_upload_pack",
    Agent::"git_receive_pack",
    Agent::"github_rest_repos",
    Agent::"github_rest_refs",
    Agent::"github_rest_pulls"
  ].contains(principal)
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is GitToken
) when {
  resource.PrincipalId == principal.id ||
  principal.scopes.contains("admin:tokens")
};

// --- Create: own-principal tokens + admin:tokens scope.

// --- Row creation (POST /tdata/GitTokens): operator-only. Resource
// attributes are not populated at row-create time, so per-principal
// ownership conditions live on the spec actions below.
permit(
  principal,
  action == Action::"create",
  resource is GitToken
) when {
  principal.scopes.contains("admin:tokens") &&
  principal.agent_type == "admin"
};

permit(
  principal,
  action == Action::"Create",
  resource is GitToken
) when {
  resource.PrincipalId == principal.id &&
  principal.scopes.contains("admin:tokens")
};

// --- Create on behalf of another principal: admin-only, audited.

permit(
  principal,
  action == Action::"Create",
  resource is GitToken
) when {
  resource.PrincipalId != principal.id &&
  principal.scopes.contains("admin:tokens") &&
  principal.agent_type == "admin"
};

// --- Revoke: own principal, or admin:tokens.

permit(
  principal,
  action == Action::"Revoke",
  resource is GitToken
) when {
  resource.PrincipalId == principal.id ||
  principal.scopes.contains("admin:tokens")
};

// --- MarkUsed: system principal only (auth integration).

// The auth resolver fires MarkUsed under the module's admin-kind
// identity (no agent_type header on its internal calls).
permit(
  principal is Admin,
  action == Action::"MarkUsed",
  resource is GitToken
);

permit(
  principal,
  action == Action::"MarkUsed",
  resource is GitToken
) when {
  principal.agent_type == "system"
};

// --- Defense-in-depth: revoked tokens are inert. The state machine
//     also enforces this; Cedar catches any bypass attempt.

forbid(
  principal,
  action,
  resource is GitToken
) when {
  resource.Status == "Revoked" &&
  action != Action::"read" &&
  action != Action::"list"
};
