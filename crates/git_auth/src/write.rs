//! Who may change a repository: push, write refs, and act on pull requests.
//!
//! The rules are the write permits in `policies/repository.cedar`,
//! `policies/ref.cedar`, `policies/pull_request.cedar` and
//! `policies/review.cedar`, applied to the GitToken a request presented.
//! Cedar cannot apply them on the git and REST surfaces: the kernel runs an
//! HttpEndpoint guest's internal calls as the guest itself, never as the caller
//! (temper `state/dispatch/wasm.rs`), and an action an HttpEndpoint route
//! bridges is dispatched with no Cedar precondition, because the route's module
//! is the enforcement point for its own protocol (temper
//! `state/dispatch/actions.rs`, `dispatch_outside_action`). So every write
//! module decides here, on the resolved token and the Repository row, before it
//! changes anything, and then acts under its own name.

use crate::Principal;

/// A change a module is about to make to one repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryWrite {
    /// Push objects and fast-forward, create or delete refs, or write a ref
    /// through REST.
    Push,
    /// Move a ref to a commit that does not descend from its current one.
    ForcePush,
    /// Delete the repository's default branch.
    DeleteDefaultBranch,
    /// Open, close or request review on a pull request.
    PullRequest,
    /// Approve or request changes on a pull request.
    Review,
    /// Merge a pull request into its target branch.
    Merge,
}

/// Whether `principal` may make `write` to the repository whose row has these
/// fields.
///
/// An archived repository refuses every write (repository.cedar's forbid). A
/// row with no owner belongs to no one, so only `admin:repos` writes to it.
pub fn may_write_repository(
    principal: &Principal,
    repository_id: &str,
    fields: &serde_json::Value,
    write: RepositoryWrite,
) -> bool {
    if principal.is_anonymous() || principal.kind != "customer" {
        return false;
    }
    if fields.get("Status").and_then(|v| v.as_str()) == Some("Archived") {
        return false;
    }
    let admin = principal.has_scope("admin:repos");
    let owner = fields.get("OwnerAccountId").and_then(|v| v.as_str());
    let owns = owner.is_some_and(|owner| !owner.is_empty() && owner == principal.id);
    let writes = (principal.has_scope("repo:write") || admin) && (owns || admin);
    let reads = crate::may_read_repository(principal, repository_id, fields);
    match write {
        RepositoryWrite::Push => writes,
        RepositoryWrite::ForcePush => writes && principal.has_scope("force"),
        RepositoryWrite::DeleteDefaultBranch => writes && admin,
        RepositoryWrite::PullRequest => reads && (principal.has_scope("pr:write") || admin),
        RepositoryWrite::Review => reads && principal.has_scope("pr:write"),
        RepositoryWrite::Merge => principal.has_scope("pr:merge") && (owns || admin),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    const RP: &str = "rp-octo-hello";

    fn token(id: &str, scopes: &[&str]) -> Principal {
        Principal {
            kind: "customer".to_string(),
            id: id.to_string(),
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
            repositories: alloc::vec::Vec::new(),
            token_id: Some("tk-1".to_string()),
            tenant: "default".to_string(),
        }
    }

    fn repo(owner: &str, visibility: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "OwnerAccountId": owner,
            "Visibility": visibility,
            "Status": status,
        })
    }

    #[test]
    fn only_the_owner_or_admin_repos_with_repo_write_pushes() {
        let r = repo("octo", "public", "Active");
        assert!(may_write_repository(
            &token("octo", &["repo:write"]),
            RP,
            &r,
            RepositoryWrite::Push
        ));
        assert!(may_write_repository(
            &token("ops", &["admin:repos"]),
            RP,
            &r,
            RepositoryWrite::Push
        ));
        assert!(!may_write_repository(
            &token("octo", &["repo:read"]),
            RP,
            &r,
            RepositoryWrite::Push
        ));
        assert!(!may_write_repository(
            &token("other", &["repo:read", "repo:write"]),
            RP,
            &r,
            RepositoryWrite::Push
        ));
    }

    #[test]
    fn a_force_push_also_needs_the_force_scope() {
        let r = repo("octo", "private", "Active");
        assert!(!may_write_repository(
            &token("octo", &["repo:write"]),
            RP,
            &r,
            RepositoryWrite::ForcePush
        ));
        assert!(may_write_repository(
            &token("octo", &["repo:write", "force"]),
            RP,
            &r,
            RepositoryWrite::ForcePush
        ));
        assert!(!may_write_repository(
            &token("other", &["repo:write", "force"]),
            RP,
            &r,
            RepositoryWrite::ForcePush
        ));
    }

    #[test]
    fn an_archived_or_ownerless_repository_refuses_writes() {
        let archived = repo("octo", "public", "Archived");
        assert!(!may_write_repository(
            &token("ops", &["admin:repos"]),
            RP,
            &archived,
            RepositoryWrite::Push
        ));
        let ownerless = serde_json::json!({ "Visibility": "public" });
        assert!(!may_write_repository(
            &token("", &["repo:write"]),
            RP,
            &ownerless,
            RepositoryWrite::Push
        ));
        assert!(may_write_repository(
            &token("ops", &["admin:repos"]),
            RP,
            &ownerless,
            RepositoryWrite::Push
        ));
    }

    #[test]
    fn pull_requests_need_pr_write_and_a_readable_repository() {
        let public = repo("octo", "public", "Active");
        let private = repo("octo", "private", "Active");
        let contributor = token("other", &["repo:read", "pr:write"]);
        assert!(may_write_repository(
            &contributor,
            RP,
            &public,
            RepositoryWrite::PullRequest
        ));
        assert!(may_write_repository(
            &contributor,
            RP,
            &public,
            RepositoryWrite::Review
        ));
        assert!(!may_write_repository(
            &contributor,
            RP,
            &private,
            RepositoryWrite::PullRequest
        ));
        assert!(!may_write_repository(
            &token("other", &["repo:read"]),
            RP,
            &public,
            RepositoryWrite::Review
        ));
    }

    #[test]
    fn merging_needs_pr_merge_and_ownership_or_admin_repos() {
        let r = repo("octo", "public", "Active");
        assert!(may_write_repository(
            &token("octo", &["pr:merge"]),
            RP,
            &r,
            RepositoryWrite::Merge
        ));
        assert!(!may_write_repository(
            &token("other", &["pr:merge"]),
            RP,
            &r,
            RepositoryWrite::Merge
        ));
        assert!(!may_write_repository(
            &token("octo", &["pr:write"]),
            RP,
            &r,
            RepositoryWrite::Merge
        ));
        assert!(!may_write_repository(
            &Principal::anonymous(&crate::AuthEnv {
                temper_api: "http://127.0.0.1:3000",
                tenant: "default",
                system_principal: "t",
            }),
            RP,
            &r,
            RepositoryWrite::Merge
        ));
    }
}
