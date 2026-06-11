//! scm_merge_pr — server-side merge engine (ADR-0024).
//!
//! Triggered by `PullRequest.Merge` (entity = the PR) and
//! `Repository.MergePullRequest` (entity = the repository). Like
//! `scm_ingest_pack`, this module is a pure integration-result
//! producer: it reads state over OData, computes the merge, and
//! returns a `sub_writes` envelope for the kernel to validate and
//! apply atomically. It never dispatches actions itself.
//!
//! Strategies (ADR-0024):
//! - `ff` — base tip must be an ancestor of the PR head; the ref
//!   advances by CAS, no new objects.
//! - `squash` — one new single-parent commit on the base tip.
//! - `merge` — one new two-parent commit, accepted only when the
//!   three-way resolution is clean at tree level.
//!
//! Conflicts are refused with an error prefixed `merge-conflict:` so
//! the REST layer can map it to HTTP 409. The engine's failure mode
//! is refusal — never a wrong merge commit.

#![forbid(unsafe_code)]

extern crate alloc;

mod commits;
mod merge_base;
mod odata;
mod request;
mod sub_writes;
mod tree_merge;

#[cfg(test)]
mod parity_tests;
#[cfg(test)]
mod unit_tests;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

use commits::{BuiltCommit, CommitInputs};
use merge_base::MergeBase;
use odata::{PullRequestRow, RefRow};
use request::{MergeRequest, Strategy, Trigger};
use tree_merge::NewTree;

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        run_merge(&ctx)
    }
}

/// What the strategy computation produced: the new target-ref tip and
/// any objects that must be written before the ref can point at them.
struct MergeComputation {
    new_tip: String,
    commit: Option<BuiltCommit>,
    new_trees: Vec<NewTree>,
    /// Message recorded on the PullRequest.Merge sub-write.
    message: String,
}

fn run_merge(ctx: &Context) -> Result<Value, String> {
    let request = MergeRequest::from_context(ctx)?;
    let api_base = odata::temper_api_base(ctx);
    let blob_endpoint = odata::blob_endpoint(ctx, &api_base);
    let pr = resolve_pull_request(ctx, &api_base, &request)?;
    let repo = pr.repository_id.clone();

    // On the Repository trigger the PR transition runs as a sub-write,
    // so surface a clear refusal here instead of a preflight error.
    if matches!(request.trigger, Trigger::Repository { .. }) && pr.status != "Approved" {
        return Err(format!(
            "pull request {} is in state '{}'; only Approved pull requests can merge",
            pr.entity_id, pr.status
        ));
    }

    // Refs are the source of truth for both tips; the target tip is
    // also the CAS guard — if it moves before the envelope applies,
    // the kernel rejects the whole merge.
    let source = odata::fetch_ref(ctx, &api_base, &repo, &pr.source_ref)?;
    let target = odata::fetch_ref(ctx, &api_base, &repo, &pr.target_ref)?;
    let head = source.target_commit_sha.clone();
    let base_tip = target.target_commit_sha.clone();
    if head == base_tip {
        return Err(format!(
            "nothing to merge: '{}' and '{}' both point at {head}",
            pr.source_ref, pr.target_ref
        ));
    }

    let base_sha = compute_merge_base(ctx, &api_base, &repo, &base_tip, &head, &pr)?;
    let computed = match request.strategy {
        Strategy::FastForward => fast_forward(&base_sha, &base_tip, &head, &pr, &request)?,
        Strategy::Merge | Strategy::Squash => build_new_commit(
            ctx, &api_base, &blob_endpoint, &repo, &request, &pr, &base_sha, &base_tip, &head,
        )?,
    };
    let sub_writes =
        assemble_sub_writes(ctx, &api_base, &blob_endpoint, &repo, &request, &pr, &target, &base_tip, &computed)?;

    let _ = ctx.log_structured(
        "info",
        "scm_merge_pr_result",
        &json!({
            "repository_id": repo,
            "pull_request_id": pr.entity_id,
            "strategy": request.strategy.as_str(),
            "merge_commit_sha": computed.new_tip,
            "previous_target_sha": base_tip,
            "new_tree_count": computed.new_trees.len(),
            "sub_write_count": sub_writes.len(),
        }),
    );
    Ok(json!({
        "strategy": request.strategy.as_str(),
        "merge_commit_sha": computed.new_tip,
        "previous_target_sha": base_tip,
        "target_ref": pr.target_ref,
        "pull_request_id": pr.entity_id,
        "sub_writes": sub_writes,
    }))
}

fn resolve_pull_request(
    ctx: &Context,
    api_base: &str,
    request: &MergeRequest,
) -> Result<PullRequestRow, String> {
    let pr = match &request.trigger {
        Trigger::PullRequest { pr_id } => odata::fetch_pull_request(ctx, api_base, pr_id)?,
        Trigger::Repository {
            repository_id,
            pr_id: Some(pr_id),
            ..
        } => {
            let pr = odata::fetch_pull_request(ctx, api_base, pr_id)?;
            if pr.repository_id != *repository_id {
                return Err(format!(
                    "pull request {pr_id} belongs to repository {}, not {repository_id}",
                    pr.repository_id
                ));
            }
            pr
        }
        Trigger::Repository {
            repository_id,
            pr_id: None,
            pr_number: Some(number),
        } => odata::fetch_pull_request_by_number(ctx, api_base, repository_id, *number)?,
        Trigger::Repository { .. } => {
            return Err(
                "Repository.MergePullRequest requires PullRequestId (or PullRequestNumber)"
                    .to_string(),
            );
        }
    };
    assert!(!pr.entity_id.is_empty(), "resolved PR must carry an id");
    assert!(!pr.repository_id.is_empty(), "resolved PR must carry a repo");
    Ok(pr)
}

fn compute_merge_base(
    ctx: &Context,
    api_base: &str,
    repo: &str,
    base_tip: &str,
    head: &str,
    pr: &PullRequestRow,
) -> Result<String, String> {
    let walked = merge_base::find_merge_base(base_tip, head, |sha| {
        odata::fetch_commit_parents(ctx, api_base, repo, sha)
    })?;
    match walked {
        MergeBase::Found(sha) => Ok(sha),
        MergeBase::None => Err(format!(
            "cannot merge '{}' into '{}': the branches share no common ancestor",
            pr.source_ref, pr.target_ref
        )),
        MergeBase::BudgetExhausted => Err(format!(
            "cannot merge '{}' into '{}': merge-base walk exceeded {} commits; \
             merge locally, push, and retry",
            pr.source_ref,
            pr.target_ref,
            merge_base::MERGE_BASE_WALK_MAX_COMMITS
        )),
    }
}

fn fast_forward(
    base_sha: &str,
    base_tip: &str,
    head: &str,
    pr: &PullRequestRow,
    request: &MergeRequest,
) -> Result<MergeComputation, String> {
    if base_sha != base_tip {
        return Err(format!(
            "fast-forward is not possible: '{}' has commits not contained in '{}'; \
             rebase or merge locally, push, and retry",
            pr.target_ref, pr.source_ref
        ));
    }
    debug_assert_ne!(base_tip, head, "ff with identical tips is filtered earlier");
    Ok(MergeComputation {
        new_tip: head.to_string(),
        commit: None,
        new_trees: Vec::new(),
        message: request.message.clone().unwrap_or_default(),
    })
}

/// Compute the merged tree and author the merge/squash commit.
///
/// Squash note: ADR-0024 describes the squash tree as "the PR head's
/// tree". We compute the three-way merged tree instead, which is
/// byte-identical to the head's tree whenever the target branch did
/// not change since the merge base (the case the ADR describes), and
/// — unlike taking the head tree unconditionally — never silently
/// reverts target-branch changes when it did. Conflicts refuse.
#[allow(clippy::too_many_arguments)]
fn build_new_commit(
    ctx: &Context,
    api_base: &str,
    blob_endpoint: &str,
    repo: &str,
    request: &MergeRequest,
    pr: &PullRequestRow,
    base_sha: &str,
    base_tip: &str,
    head: &str,
) -> Result<MergeComputation, String> {
    if base_sha == head {
        return Err(format!(
            "nothing to merge: '{}' is already contained in '{}'",
            pr.source_ref, pr.target_ref
        ));
    }
    let fetch_commit = |sha: &str| -> Result<odata::CommitRow, String> {
        odata::fetch_commit_row(ctx, api_base, repo, sha)?
            .ok_or_else(|| format!("commit {sha} has no row on repository {repo}"))
    };
    let base_commit = fetch_commit(base_sha)?;
    let ours_commit = fetch_commit(base_tip)?;
    let theirs_commit = fetch_commit(head)?;

    let mut fetch_tree =
        |sha: &str| odata::fetch_tree_entries(ctx, api_base, blob_endpoint, repo, sha);
    let outcome = tree_merge::merge_trees(
        &base_commit.tree_sha,
        &ours_commit.tree_sha,
        &theirs_commit.tree_sha,
        &mut fetch_tree,
    )?
    .map_err(|conflict| conflict_error(&pr.source_ref, &pr.target_ref, &conflict))?;

    let inputs = commit_inputs(request, pr, &theirs_commit.committer)?;
    let built = match request.strategy {
        Strategy::Merge => commits::build_merge_commit(&outcome.root_sha, base_tip, head, &inputs),
        Strategy::Squash => commits::build_squash_commit(&outcome.root_sha, base_tip, &inputs),
        Strategy::FastForward => {
            return Err("internal: fast-forward authors no commit".to_string());
        }
    };
    Ok(MergeComputation {
        new_tip: built.sha.clone(),
        message: built.message.clone(),
        commit: Some(built),
        new_trees: outcome.new_trees,
    })
}

/// The refusal string for divergent content. The `merge-conflict:`
/// prefix is the contract with the REST layer, which maps it to
/// HTTP 409 (ADR-0024).
fn conflict_error(
    source_ref: &str,
    target_ref: &str,
    conflict: &tree_merge::TreeMergeConflict,
) -> String {
    debug_assert!(!conflict.paths.is_empty(), "conflict must name paths");
    format!(
        "merge-conflict: cannot merge '{source_ref}' into '{target_ref}': \
         both branches modified: {}; resolve by merging or rebasing locally, \
         pushing, and retrying",
        conflict.describe()
    )
}

fn commit_inputs(
    request: &MergeRequest,
    pr: &PullRequestRow,
    head_committer_line: &str,
) -> Result<CommitInputs, String> {
    let identity = request
        .committer_identity
        .clone()
        .unwrap_or_else(|| commits::SERVER_IDENTITY.to_string());
    // Determinism: timestamp from trigger params when provided, else
    // the head commit's committer timestamp. Never the wall clock.
    let (timestamp, timezone) = match &request.timestamp_override {
        Some((ts, tz)) => (ts.clone(), tz.clone()),
        None => commits::timestamp_from_identity_line(head_committer_line)?,
    };
    let message = match (&request.message, request.strategy) {
        (Some(message), _) => commits::normalize_message(message),
        (None, Strategy::Squash) => {
            commits::default_squash_message(pr.number, &pr.source_ref, &pr.title)
        }
        (None, _) => commits::default_merge_message(pr.number, &pr.source_ref, &pr.title),
    };
    Ok(CommitInputs {
        identity,
        timestamp,
        timezone,
        message,
    })
}

#[allow(clippy::too_many_arguments)]
fn assemble_sub_writes(
    ctx: &Context,
    api_base: &str,
    blob_endpoint: &str,
    repo: &str,
    request: &MergeRequest,
    pr: &PullRequestRow,
    target: &RefRow,
    base_tip: &str,
    computed: &MergeComputation,
) -> Result<Vec<Value>, String> {
    assert_ne!(computed.new_tip, *base_tip, "merge must advance the ref");
    let mut out = Vec::new();
    // Objects first, ref advance after — same ordering as a push.
    // Skip objects that already exist (e.g. an identical retried
    // merge): Create of an existing row would fail the envelope.
    for tree in &computed.new_trees {
        if !odata::object_exists(ctx, api_base, "Trees", repo, &tree.sha)? {
            out.push(sub_writes::tree_create_sub_write(
                ctx,
                blob_endpoint,
                repo,
                tree,
            )?);
        }
    }
    if let Some(built) = &computed.commit
        && !odata::object_exists(ctx, api_base, "Commits", repo, &built.sha)?
    {
        out.push(sub_writes::commit_create_sub_write(
            ctx,
            blob_endpoint,
            repo,
            built,
        )?);
    }
    out.push(sub_writes::ref_update_sub_write(
        &target.entity_id,
        base_tip,
        &computed.new_tip,
    ));
    if matches!(request.trigger, Trigger::Repository { .. }) {
        out.push(sub_writes::pull_request_merge_sub_write(
            &pr.entity_id,
            request.strategy.as_str(),
            &computed.message,
            &request.client_request_id,
            &computed.new_tip,
        ));
    }
    Ok(out)
}
