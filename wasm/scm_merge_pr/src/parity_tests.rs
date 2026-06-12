//! Byte-exactness tests against real `git` (host-side only).
//!
//! For each strategy: build a tiny repository with the git CLI under
//! pinned author/committer identities and dates, perform the merge
//! with git, then compute the same merge with this engine — feeding
//! it git's own objects via `cat-file` — and require the resulting
//! commit and tree SHAs (and canonical commit bytes) to be identical.
//!
//! If `git` is not on PATH the tests skip rather than fail, matching
//! `wire/tests/git_parity.rs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use genesis_git_object::{ParsedTreeEntry, parse_commit, parse_tree};

use crate::commits::{CommitInputs, build_merge_commit, build_squash_commit};
use crate::merge_base::{MergeBase, find_merge_base};
use crate::tree_merge::merge_trees;

const IDENTITY: &str = "Tester <t@example.invalid>";
const MERGE_DATE: &str = "1234567890 +0000";

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

struct GitRepo {
    dir: PathBuf,
}

impl GitRepo {
    fn new(label: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("scm-merge-parity-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create tmp repo dir");
        let repo = GitRepo { dir };
        repo.run(&["init", "--quiet"]);
        repo.run(&["symbolic-ref", "HEAD", "refs/heads/main"]);
        for (key, value) in [
            ("user.name", "Tester"),
            ("user.email", "t@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            repo.run(&["config", key, value]);
        }
        repo
    }

    fn run(&self, args: &[&str]) -> String {
        self.run_dated(args, MERGE_DATE)
    }

    /// Run git with pinned author/committer dates so every object is
    /// reproducible.
    fn run_dated(&self, args: &[&str], date: &str) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Tester")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "Tester")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn run_expect_failure(&self, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .output()
            .expect("run git");
        assert!(!out.status.success(), "git {args:?} unexpectedly succeeded");
    }

    fn write(&self, name: &str, content: &str) {
        let path = self.dir.join(name);
        if let Some(parent) = Path::parent(&path) {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn commit(&self, message: &str, date: &str) -> String {
        self.run_dated(&["add", "--all"], date);
        self.run_dated(&["commit", "--quiet", "-m", message], date);
        self.rev_parse("HEAD")
    }

    fn rev_parse(&self, rev: &str) -> String {
        self.run(&["rev-parse", rev])
    }

    fn cat_file(&self, kind: &str, sha: &str) -> Vec<u8> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.dir)
            .args(["cat-file", kind, sha])
            .output()
            .expect("git cat-file");
        assert!(out.status.success(), "cat-file {kind} {sha} failed");
        out.stdout
    }

    fn commit_parents(&self, sha: &str) -> Vec<String> {
        parse_commit(&self.cat_file("commit", sha))
            .expect("parse commit")
            .parents
    }

    fn commit_tree(&self, sha: &str) -> String {
        parse_commit(&self.cat_file("commit", sha))
            .expect("parse commit")
            .tree
    }

    fn tree_entries(&self, sha: &str) -> Vec<ParsedTreeEntry> {
        parse_tree(&self.cat_file("tree", sha)).expect("parse tree")
    }

    /// The engine's tree-fetch closure, backed by git's own objects.
    fn fetch_tree(&self) -> impl FnMut(&str) -> Result<Vec<ParsedTreeEntry>, String> + '_ {
        move |sha: &str| Ok(self.tree_entries(sha))
    }

    fn parents_of(&self) -> impl FnMut(&str) -> Result<Vec<String>, String> + '_ {
        let mut cache: BTreeMap<String, Vec<String>> = BTreeMap::new();
        move |sha: &str| {
            if let Some(parents) = cache.get(sha) {
                return Ok(parents.clone());
            }
            let parents = self.commit_parents(sha);
            cache.insert(sha.to_string(), parents.clone());
            Ok(parents)
        }
    }
}

impl Drop for GitRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn merge_inputs(message: &str) -> CommitInputs {
    CommitInputs {
        identity: IDENTITY.to_string(),
        timestamp: "1234567890".to_string(),
        timezone: "+0000".to_string(),
        message: format!("{message}\n"),
    }
}

/// Three-way merge where each side touched different paths; the
/// engine's merged tree, merge commit SHA, and canonical bytes must
/// equal git's.
#[test]
fn merge_commit_matches_git_on_disjoint_paths() {
    if !git_available() {
        eprintln!("git not available; skipping");
        return;
    }
    let repo = GitRepo::new("merge");
    repo.write("a.txt", "alpha\n");
    repo.write("dir/b.txt", "bravo\n");
    let fork = repo.commit("initial", "1234567800 +0000");
    repo.run(&["checkout", "--quiet", "-b", "feat"]);
    repo.write("dir/c.txt", "charlie\n");
    let head = repo.commit("feat: add c", "1234567830 +0000");
    repo.run(&["checkout", "--quiet", "main"]);
    repo.write("a.txt", "alpha two\n");
    let base_tip = repo.commit("main: tweak a", "1234567860 +0000");

    repo.run_dated(
        &["merge", "--no-ff", "-m", "Merge feat", "feat"],
        MERGE_DATE,
    );
    let git_merge_sha = repo.rev_parse("HEAD");

    // Engine: merge base, merged tree, then the commit object.
    let base = find_merge_base(&base_tip, &head, repo.parents_of()).unwrap();
    assert_eq!(base, MergeBase::Found(fork.clone()));
    let outcome = merge_trees(
        &repo.commit_tree(&fork),
        &repo.commit_tree(&base_tip),
        &repo.commit_tree(&head),
        &mut repo.fetch_tree(),
    )
    .unwrap()
    .expect("disjoint paths must merge cleanly");
    assert_eq!(outcome.root_sha, repo.commit_tree(&git_merge_sha));

    let built = build_merge_commit(
        &outcome.root_sha,
        &base_tip,
        &head,
        &merge_inputs("Merge feat"),
    );
    assert_eq!(
        built.sha, git_merge_sha,
        "merge commit SHA diverged from git"
    );
    let git_body = repo.cat_file("commit", &git_merge_sha);
    let nul = built.canonical.iter().position(|&b| b == 0).unwrap();
    assert_eq!(
        &built.canonical[nul + 1..],
        git_body.as_slice(),
        "canonical commit bytes diverged from git"
    );
}

/// Squash with an unchanged base: the squash tree is exactly the PR
/// head's tree (ADR-0024's stated shape) and the commit SHA matches
/// git's `merge --squash` + `commit`.
#[test]
fn squash_commit_matches_git_with_unchanged_base() {
    if !git_available() {
        eprintln!("git not available; skipping");
        return;
    }
    let repo = GitRepo::new("squash");
    repo.write("a.txt", "alpha\n");
    let fork = repo.commit("initial", "1234567800 +0000");
    repo.run(&["checkout", "--quiet", "-b", "feat"]);
    repo.write("b.txt", "bravo\n");
    repo.commit("feat: add b", "1234567830 +0000");
    repo.write("c.txt", "charlie\n");
    let head = repo.commit("feat: add c", "1234567860 +0000");
    repo.run(&["checkout", "--quiet", "main"]);

    repo.run_dated(&["merge", "--squash", "feat"], MERGE_DATE);
    repo.run_dated(&["commit", "--quiet", "-m", "Squash feat"], MERGE_DATE);
    let git_squash_sha = repo.rev_parse("HEAD");

    let outcome = merge_trees(
        &repo.commit_tree(&fork),
        &repo.commit_tree(&fork),
        &repo.commit_tree(&head),
        &mut repo.fetch_tree(),
    )
    .unwrap()
    .expect("clean squash");
    assert_eq!(
        outcome.root_sha,
        repo.commit_tree(&head),
        "squash tree = head tree"
    );
    assert!(outcome.new_trees.is_empty());

    let built = build_squash_commit(&outcome.root_sha, &fork, &merge_inputs("Squash feat"));
    assert_eq!(
        built.sha, git_squash_sha,
        "squash commit SHA diverged from git"
    );
}

/// Squash where the base advanced on a disjoint path: git's squash
/// commits the three-way merged tree — so does the engine.
#[test]
fn squash_commit_matches_git_with_advanced_base() {
    if !git_available() {
        eprintln!("git not available; skipping");
        return;
    }
    let repo = GitRepo::new("squash-adv");
    repo.write("a.txt", "alpha\n");
    let fork = repo.commit("initial", "1234567800 +0000");
    repo.run(&["checkout", "--quiet", "-b", "feat"]);
    repo.write("b.txt", "bravo\n");
    let head = repo.commit("feat: add b", "1234567830 +0000");
    repo.run(&["checkout", "--quiet", "main"]);
    repo.write("a.txt", "alpha two\n");
    let base_tip = repo.commit("main: tweak a", "1234567860 +0000");

    repo.run_dated(&["merge", "--squash", "feat"], MERGE_DATE);
    repo.run_dated(&["commit", "--quiet", "-m", "Squash feat"], MERGE_DATE);
    let git_squash_sha = repo.rev_parse("HEAD");

    let outcome = merge_trees(
        &repo.commit_tree(&fork),
        &repo.commit_tree(&base_tip),
        &repo.commit_tree(&head),
        &mut repo.fetch_tree(),
    )
    .unwrap()
    .expect("disjoint squash must be clean");
    assert_eq!(outcome.root_sha, repo.commit_tree(&git_squash_sha));

    let built = build_squash_commit(&outcome.root_sha, &base_tip, &merge_inputs("Squash feat"));
    assert_eq!(
        built.sha, git_squash_sha,
        "squash commit SHA diverged from git"
    );
}

/// Both sides touch the same file: the engine refuses (conflict) and
/// so does git.
#[test]
fn conflict_detection_matches_git() {
    if !git_available() {
        eprintln!("git not available; skipping");
        return;
    }
    let repo = GitRepo::new("conflict");
    repo.write("a.txt", "alpha\n");
    let fork = repo.commit("initial", "1234567800 +0000");
    repo.run(&["checkout", "--quiet", "-b", "feat"]);
    repo.write("a.txt", "feature version\n");
    let head = repo.commit("feat: rewrite a", "1234567830 +0000");
    repo.run(&["checkout", "--quiet", "main"]);
    repo.write("a.txt", "main version\n");
    let base_tip = repo.commit("main: rewrite a", "1234567860 +0000");

    let conflict = merge_trees(
        &repo.commit_tree(&fork),
        &repo.commit_tree(&base_tip),
        &repo.commit_tree(&head),
        &mut repo.fetch_tree(),
    )
    .unwrap()
    .expect_err("overlapping edits must conflict");
    assert_eq!(conflict.paths, vec!["a.txt".to_string()]);

    repo.run_expect_failure(&["merge", "--no-ff", "-m", "boom", "feat"]);
}

/// Head strictly ahead of base: the merge base equals the base tip,
/// which is exactly the engine's fast-forward condition; git agrees
/// the ff lands on the head SHA.
#[test]
fn fast_forward_detection_matches_git() {
    if !git_available() {
        eprintln!("git not available; skipping");
        return;
    }
    let repo = GitRepo::new("ff");
    repo.write("a.txt", "alpha\n");
    let base_tip = repo.commit("initial", "1234567800 +0000");
    repo.run(&["checkout", "--quiet", "-b", "feat"]);
    repo.write("b.txt", "bravo\n");
    let head = repo.commit("feat: add b", "1234567830 +0000");
    repo.run(&["checkout", "--quiet", "main"]);

    let base = find_merge_base(&base_tip, &head, repo.parents_of()).unwrap();
    assert_eq!(
        base,
        MergeBase::Found(base_tip.clone()),
        "ff condition holds"
    );

    repo.run(&["merge", "--ff-only", "feat"]);
    assert_eq!(repo.rev_parse("HEAD"), head, "git ff lands on head");

    // Negative: once main advances, ff must be refused.
    repo.run(&["reset", "--quiet", "--hard", &base_tip]);
    repo.write("c.txt", "charlie\n");
    let advanced = repo.commit("main: add c", "1234567860 +0000");
    let base = find_merge_base(&advanced, &head, repo.parents_of()).unwrap();
    assert_ne!(
        base,
        MergeBase::Found(advanced.clone()),
        "ff no longer possible"
    );
}
