//! Host-side unit tests for the pure pieces of the merge engine:
//! trigger parsing, tree merge cleanliness, commit construction.
//! (Merge-base walk tests live inline in `merge_base.rs`; parity
//! tests against real `git` live in `parity_tests.rs`.)

use std::collections::BTreeMap;

use genesis_git_object::{Mode, ParsedTreeEntry, TreeEntry, tree_canonical_bytes, tree_hash};
use serde_json::json;
use temper_wasm_sdk::prelude::*;

use crate::commits::{
    CommitInputs, build_merge_commit, build_squash_commit, default_merge_message,
    default_squash_message, normalize_message, timestamp_from_identity_line,
};
use crate::conflict_error;
use crate::request::{MergeRequest, Strategy, Trigger};
use crate::tree_merge::{EMPTY_TREE_SHA, TreeMergeConflict, merge_trees};

// --- Trigger / request parsing ---------------------------------------

fn ctx(entity_type: &str, entity_id: &str, action: &str, params: serde_json::Value) -> Context {
    Context {
        config: BTreeMap::new(),
        trigger_params: params,
        entity_state: json!({}),
        tenant: "t".to_string(),
        entity_type: entity_type.to_string(),
        entity_id: entity_id.to_string(),
        trigger_action: action.to_string(),
        wasm_module: "scm_merge_pr".to_string(),
        http_request: None,
    }
}

#[test]
fn pull_request_merge_trigger_shape() {
    let request = MergeRequest::from_context(&ctx(
        "PullRequest",
        "pr-1",
        "Merge",
        json!({ "Strategy": "squash", "Message": "m", "ClientRequestId": "c-1" }),
    ))
    .unwrap();
    assert_eq!(
        request.trigger,
        Trigger::PullRequest {
            pr_id: "pr-1".to_string()
        }
    );
    assert_eq!(request.strategy, Strategy::Squash);
    assert_eq!(request.message.as_deref(), Some("m"));
    assert_eq!(request.client_request_id, "c-1");
}

#[test]
fn repository_merge_trigger_shape() {
    let request = MergeRequest::from_context(&ctx(
        "Repository",
        "rp-acme-demo",
        "MergePullRequest",
        json!({ "PullRequestId": "pr-9", "Strategy": "merge" }),
    ))
    .unwrap();
    assert_eq!(
        request.trigger,
        Trigger::Repository {
            repository_id: "rp-acme-demo".to_string(),
            pr_id: Some("pr-9".to_string()),
            pr_number: None,
        }
    );
}

#[test]
fn repository_trigger_accepts_pr_number() {
    let request = MergeRequest::from_context(&ctx(
        "Repository",
        "rp-1",
        "MergePullRequest",
        json!({ "PullRequestNumber": 7 }),
    ))
    .unwrap();
    assert_eq!(
        request.trigger,
        Trigger::Repository {
            repository_id: "rp-1".to_string(),
            pr_id: None,
            pr_number: Some(7),
        }
    );
    // Strategy omitted → GitHub's default.
    assert_eq!(request.strategy, Strategy::Merge);
}

#[test]
fn unsupported_trigger_is_refused() {
    let err = MergeRequest::from_context(&ctx("Webhook", "w-1", "Deliver", json!({}))).unwrap_err();
    assert!(err.contains("unsupported trigger"), "{err}");
}

#[test]
fn strategy_vocabulary_covers_github_merge_methods() {
    assert_eq!(Strategy::parse("merge").unwrap(), Strategy::Merge);
    assert_eq!(Strategy::parse("SQUASH").unwrap(), Strategy::Squash);
    for ff in ["ff", "fast-forward", "fast_forward", "fastforward"] {
        assert_eq!(Strategy::parse(ff).unwrap(), Strategy::FastForward);
    }
    let rebase = Strategy::parse("rebase").unwrap_err();
    assert!(rebase.contains("not supported"), "{rebase}");
    assert!(Strategy::parse("yolo").is_err());
}

#[test]
fn timestamp_override_is_validated() {
    let request = MergeRequest::from_context(&ctx(
        "PullRequest",
        "pr-1",
        "Merge",
        json!({ "CommitTimestamp": 1234567890u64, "CommitTimezone": "+0200" }),
    ))
    .unwrap();
    assert_eq!(
        request.timestamp_override,
        Some(("1234567890".to_string(), "+0200".to_string()))
    );
    let bad = MergeRequest::from_context(&ctx(
        "PullRequest",
        "pr-1",
        "Merge",
        json!({ "CommitTimestamp": "12x" }),
    ));
    assert!(bad.is_err());
}

#[test]
fn conflict_error_carries_the_rest_shim_marker() {
    let conflict = TreeMergeConflict {
        paths: vec!["a.txt".to_string(), "dir/b.txt".to_string()],
    };
    let err = conflict_error("refs/heads/feat", "refs/heads/main", &conflict);
    assert!(err.starts_with("merge-conflict:"), "{err}");
    assert!(err.contains("a.txt") && err.contains("dir/b.txt"), "{err}");
    assert!(err.contains("rebasing locally"), "{err}");
}

// --- Tree merge -------------------------------------------------------

/// In-memory tree store: builds real tree objects so SHAs are honest.
#[derive(Default)]
struct TreeStore {
    trees: BTreeMap<String, Vec<ParsedTreeEntry>>,
}

impl TreeStore {
    fn put(&mut self, entries: &[(&str, &str, &str)]) -> String {
        let typed: Vec<TreeEntry> = entries
            .iter()
            .map(|(mode, name, sha)| TreeEntry {
                mode: Mode::from_git_str(mode).expect("test mode"),
                name: name.as_bytes().to_vec(),
                object_sha: (*sha).to_string(),
            })
            .collect();
        let sha = tree_hash(typed.clone());
        let parsed = typed
            .iter()
            .map(|e| ParsedTreeEntry {
                mode: e.mode.as_git_str().to_string(),
                name: String::from_utf8(e.name.clone()).expect("utf8 name"),
                sha: e.object_sha.clone(),
                is_tree: e.mode == Mode::Tree,
            })
            .collect();
        self.trees.insert(sha.clone(), parsed);
        sha
    }

    fn fetch(&self) -> impl FnMut(&str) -> Result<Vec<ParsedTreeEntry>, String> + '_ {
        move |sha: &str| {
            self.trees
                .get(sha)
                .cloned()
                .ok_or_else(|| format!("tree {sha} not in store"))
        }
    }
}

const BLOB_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BLOB_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const BLOB_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const BLOB_D: &str = "dddddddddddddddddddddddddddddddddddddddd";

#[test]
fn disjoint_adds_merge_cleanly_into_new_tree() {
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "keep.txt", BLOB_A)]);
    let ours = store.put(&[
        ("100644", "keep.txt", BLOB_A),
        ("100644", "ours.txt", BLOB_B),
    ]);
    let theirs = store.put(&[
        ("100644", "keep.txt", BLOB_A),
        ("100644", "theirs.txt", BLOB_C),
    ]);
    let expected = store.put(&[
        ("100644", "keep.txt", BLOB_A),
        ("100644", "ours.txt", BLOB_B),
        ("100644", "theirs.txt", BLOB_C),
    ]);

    let outcome = merge_trees(&base, &ours, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap();
    assert_eq!(outcome.root_sha, expected);
    assert_eq!(outcome.new_trees.len(), 1);
    assert_eq!(outcome.new_trees[0].sha, expected);
    let canonical = tree_canonical_bytes(vec![
        TreeEntry {
            mode: Mode::RegularFile,
            name: b"keep.txt".to_vec(),
            object_sha: BLOB_A.to_string(),
        },
        TreeEntry {
            mode: Mode::RegularFile,
            name: b"ours.txt".to_vec(),
            object_sha: BLOB_B.to_string(),
        },
        TreeEntry {
            mode: Mode::RegularFile,
            name: b"theirs.txt".to_vec(),
            object_sha: BLOB_C.to_string(),
        },
    ]);
    assert_eq!(outcome.new_trees[0].canonical, canonical);
}

#[test]
fn one_sided_change_reuses_existing_tree() {
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "a.txt", BLOB_A)]);
    let theirs = store.put(&[("100644", "a.txt", BLOB_B)]);
    let outcome = merge_trees(&base, &base, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap();
    assert_eq!(outcome.root_sha, theirs);
    assert!(outcome.new_trees.is_empty(), "no synthesis needed");
}

#[test]
fn identical_change_on_both_sides_is_clean() {
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "a.txt", BLOB_A)]);
    let both = store.put(&[("100644", "a.txt", BLOB_B)]);
    let outcome = merge_trees(&base, &both, &both, &mut store.fetch())
        .unwrap()
        .unwrap();
    assert_eq!(outcome.root_sha, both);
    assert!(outcome.new_trees.is_empty());
}

#[test]
fn overlapping_file_modification_conflicts() {
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "a.txt", BLOB_A)]);
    let ours = store.put(&[("100644", "a.txt", BLOB_B)]);
    let theirs = store.put(&[("100644", "a.txt", BLOB_C)]);
    let conflict = merge_trees(&base, &ours, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap_err();
    assert_eq!(conflict.paths, vec!["a.txt".to_string()]);
}

#[test]
fn delete_vs_modify_conflicts() {
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "a.txt", BLOB_A), ("100644", "z.txt", BLOB_D)]);
    let ours = store.put(&[("100644", "z.txt", BLOB_D)]); // deleted a.txt
    let theirs = store.put(&[("100644", "a.txt", BLOB_B), ("100644", "z.txt", BLOB_D)]);
    let conflict = merge_trees(&base, &ours, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap_err();
    assert_eq!(conflict.paths, vec!["a.txt".to_string()]);
}

#[test]
fn nested_disjoint_changes_synthesize_subtree_and_root() {
    let mut store = TreeStore::default();
    let base_sub = store.put(&[("100644", "x.txt", BLOB_A)]);
    let ours_sub = store.put(&[("100644", "x.txt", BLOB_A), ("100644", "y.txt", BLOB_B)]);
    let theirs_sub = store.put(&[("100644", "x.txt", BLOB_A), ("100644", "z.txt", BLOB_C)]);
    let base = store.put(&[("40000", "dir", &base_sub)]);
    let ours = store.put(&[("40000", "dir", &ours_sub)]);
    let theirs = store.put(&[("40000", "dir", &theirs_sub)]);
    let merged_sub = store.put(&[
        ("100644", "x.txt", BLOB_A),
        ("100644", "y.txt", BLOB_B),
        ("100644", "z.txt", BLOB_C),
    ]);
    let expected_root = store.put(&[("40000", "dir", &merged_sub)]);

    let outcome = merge_trees(&base, &ours, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap();
    assert_eq!(outcome.root_sha, expected_root);
    let new_shas: Vec<&str> = outcome.new_trees.iter().map(|t| t.sha.as_str()).collect();
    assert!(new_shas.contains(&merged_sub.as_str()));
    assert!(new_shas.contains(&expected_root.as_str()));
}

#[test]
fn nested_conflict_reports_full_path() {
    let mut store = TreeStore::default();
    let base_sub = store.put(&[("100644", "f.txt", BLOB_A)]);
    let ours_sub = store.put(&[("100644", "f.txt", BLOB_B)]);
    let theirs_sub = store.put(&[("100644", "f.txt", BLOB_C)]);
    let base = store.put(&[("40000", "dir", &base_sub)]);
    let ours = store.put(&[("40000", "dir", &ours_sub)]);
    let theirs = store.put(&[("40000", "dir", &theirs_sub)]);
    let conflict = merge_trees(&base, &ours, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap_err();
    assert_eq!(conflict.paths, vec!["dir/f.txt".to_string()]);
}

#[test]
fn deletion_on_one_side_is_preserved() {
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "a.txt", BLOB_A), ("100644", "b.txt", BLOB_B)]);
    let ours = store.put(&[("100644", "b.txt", BLOB_B)]); // deleted a.txt
    let theirs = store.put(&[
        ("100644", "a.txt", BLOB_A),
        ("100644", "b.txt", BLOB_B),
        ("100644", "c.txt", BLOB_C),
    ]);
    let expected = store.put(&[("100644", "b.txt", BLOB_B), ("100644", "c.txt", BLOB_C)]);
    let outcome = merge_trees(&base, &ours, &theirs, &mut store.fetch())
        .unwrap()
        .unwrap();
    assert_eq!(outcome.root_sha, expected);
}

#[test]
fn everything_deleted_collapses_to_empty_tree() {
    // ours wiped the whole tree, theirs is unchanged from base →
    // the merged root is the (never-persisted) empty tree.
    let mut store = TreeStore::default();
    let base = store.put(&[("100644", "a.txt", BLOB_A)]);
    let outcome = merge_trees(&base, EMPTY_TREE_SHA, &base, &mut store.fetch())
        .unwrap()
        .unwrap();
    assert_eq!(outcome.root_sha, EMPTY_TREE_SHA);
    assert!(outcome.new_trees.is_empty());
}

// --- Commit construction ----------------------------------------------

fn inputs(message: &str) -> CommitInputs {
    CommitInputs {
        identity: "Genesis <merge@genesis.invalid>".to_string(),
        timestamp: "1234567890".to_string(),
        timezone: "+0000".to_string(),
        message: message.to_string(),
    }
}

#[test]
fn merge_commit_parent_order_is_base_then_head() {
    let built = build_merge_commit(
        EMPTY_TREE_SHA,
        "1111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222",
        &inputs("merge\n"),
    );
    assert_eq!(
        built.parent_shas,
        vec![
            "1111111111111111111111111111111111111111".to_string(),
            "2222222222222222222222222222222222222222".to_string(),
        ]
    );
    // Same inputs → same bytes → same sha: determinism.
    let again = build_merge_commit(
        EMPTY_TREE_SHA,
        "1111111111111111111111111111111111111111",
        "2222222222222222222222222222222222222222",
        &inputs("merge\n"),
    );
    assert_eq!(built.sha, again.sha);
    assert_eq!(built.canonical, again.canonical);
}

#[test]
fn squash_commit_has_single_parent() {
    let built = build_squash_commit(
        EMPTY_TREE_SHA,
        "1111111111111111111111111111111111111111",
        &inputs("squash\n"),
    );
    assert_eq!(built.parent_shas.len(), 1);
    assert!(built.canonical.starts_with(b"commit "));
}

#[test]
fn timestamp_extraction_from_committer_line() {
    let (ts, tz) =
        timestamp_from_identity_line("Alice Smith <alice@example.com> 1700000100 -0500").unwrap();
    assert_eq!(ts, "1700000100");
    assert_eq!(tz, "-0500");
    assert!(timestamp_from_identity_line("not a committer line").is_err());
    assert!(timestamp_from_identity_line("A <a@x> 17000 +05x0").is_err());
}

#[test]
fn default_messages_are_github_shaped() {
    assert_eq!(
        default_merge_message(Some(7), "refs/heads/feat", "Add thing"),
        "Merge pull request #7 from feat\n\nAdd thing\n"
    );
    assert_eq!(
        default_merge_message(None, "refs/heads/feat", ""),
        "Merge pull request from feat\n"
    );
    assert_eq!(
        default_squash_message(Some(7), "refs/heads/feat", "Add thing"),
        "Add thing (#7)\n"
    );
    assert_eq!(
        default_squash_message(Some(7), "refs/heads/feat", ""),
        "Squash merge pull request #7 from feat\n"
    );
    assert_eq!(normalize_message("m"), "m\n");
    assert_eq!(normalize_message("m\n"), "m\n");
}
