//! Tree-level three-way merge (ADR-0024).
//!
//! Genesis v1 accepts a merge only when the three-way resolution is
//! *clean at tree level*: within every directory, an entry is taken
//! unmodified from one side, or both sides agree on it. The moment
//! both sides changed the same path to different contents the merge
//! is refused — Genesis never guesses content resolution.
//!
//! Cleanliness, precisely: for each name in a directory, with `base`,
//! `ours` (target-branch tip) and `theirs` (PR head) versions,
//! - `ours == theirs`              → take it (identical entries);
//! - `theirs == base`              → take ours (only ours changed);
//! - `ours == base`                → take theirs (only theirs changed);
//! - both differ and both sides
//!   still have a subtree there    → recurse (per-directory disjointness);
//! - anything else                 → conflict at that path.
//!
//! Merging may synthesize new Tree objects for every directory whose
//! merged entry set matches neither input; those are returned for the
//! caller to persist.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_object::{Mode, ParsedTreeEntry, TreeEntry, tree_canonical_bytes};

/// Well-known SHA-1 of the empty tree — present in every git install,
/// usable without a backing row.
pub const EMPTY_TREE_SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// Maximum tree entries examined across the whole merge. Exceeding it
/// refuses the merge rather than doing unbounded work.
pub const TREE_MERGE_MAX_ENTRIES: usize = 65_536;

/// Maximum directory nesting depth walked during the merge.
pub const TREE_MERGE_MAX_DEPTH: usize = 64;

/// Cap on conflicting paths listed in the error message; the count is
/// always reported in full.
pub const CONFLICT_PATH_LIST_MAX: usize = 32;

/// A tree object the merge synthesized; `canonical` includes the
/// `tree <len>\0` header (the exact SHA-1 input).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTree {
    pub sha: String,
    pub canonical: Vec<u8>,
}

/// Result of a clean merge: the merged root tree plus every new tree
/// object that must be written (root-first is not guaranteed; callers
/// write all of them in one atomic envelope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeMergeOutcome {
    pub root_sha: String,
    pub new_trees: Vec<NewTree>,
}

/// Paths modified on both sides — the refusal case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeMergeConflict {
    pub paths: Vec<String>,
}

impl TreeMergeConflict {
    /// Human-readable, bounded path list for the action error string.
    pub fn describe(&self) -> String {
        let shown: Vec<&str> = self
            .paths
            .iter()
            .take(CONFLICT_PATH_LIST_MAX)
            .map(String::as_str)
            .collect();
        let suffix = if self.paths.len() > shown.len() {
            format!(" (and {} more)", self.paths.len() - shown.len())
        } else {
            String::new()
        };
        format!("{}{}", shown.join(", "), suffix)
    }
}

struct MergeWalk<'a> {
    fetch: &'a mut dyn FnMut(&str) -> Result<Vec<ParsedTreeEntry>, String>,
    entries_budget: usize,
    new_trees: Vec<NewTree>,
    conflicts: Vec<String>,
}

/// Three-way merge of `ours` (base-branch tip tree) and `theirs`
/// (PR head tree) against `base` (merge-base tree).
///
/// `fetch` resolves a tree SHA to its parsed entries (one OData read
/// per tree in production). Outer `Err` = infrastructure failure;
/// inner `Err` = clean refusal with the conflicting paths.
pub fn merge_trees(
    base_sha: &str,
    ours_sha: &str,
    theirs_sha: &str,
    fetch: &mut dyn FnMut(&str) -> Result<Vec<ParsedTreeEntry>, String>,
) -> Result<Result<TreeMergeOutcome, TreeMergeConflict>, String> {
    assert_eq!(base_sha.len(), 40, "base tree sha must be 40 hex chars");
    assert_eq!(ours_sha.len(), 40, "ours tree sha must be 40 hex chars");
    assert_eq!(theirs_sha.len(), 40, "theirs tree sha must be 40 hex chars");

    let mut walk = MergeWalk {
        fetch,
        entries_budget: TREE_MERGE_MAX_ENTRIES,
        new_trees: Vec::new(),
        conflicts: Vec::new(),
    };
    let root_sha = walk.merge_dir(base_sha, ours_sha, theirs_sha, "", 0)?;
    if !walk.conflicts.is_empty() {
        return Ok(Err(TreeMergeConflict {
            paths: walk.conflicts,
        }));
    }
    let root_sha = root_sha.unwrap_or_else(|| EMPTY_TREE_SHA.to_string());
    Ok(Ok(TreeMergeOutcome {
        root_sha,
        new_trees: walk.new_trees,
    }))
}

/// `(mode, sha)` identity of an entry — what "same entry" means here.
fn entry_key(entry: &ParsedTreeEntry) -> (String, String) {
    (entry.mode.clone(), entry.sha.clone())
}

fn same(a: Option<&ParsedTreeEntry>, b: Option<&ParsedTreeEntry>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => entry_key(a) == entry_key(b),
        _ => false,
    }
}

impl MergeWalk<'_> {
    /// Merge one directory level. Returns the merged tree's SHA, or
    /// `None` when the merged directory is empty (the parent then
    /// drops the entry — git prunes empty directories).
    fn merge_dir(
        &mut self,
        base_sha: &str,
        ours_sha: &str,
        theirs_sha: &str,
        path_prefix: &str,
        depth: usize,
    ) -> Result<Option<String>, String> {
        if depth > TREE_MERGE_MAX_DEPTH {
            return Err(format!(
                "tree merge exceeded max depth {TREE_MERGE_MAX_DEPTH} at '{path_prefix}'"
            ));
        }
        // Trivial resolutions: avoid fetching when a side is unchanged.
        if ours_sha == theirs_sha {
            return Ok(some_unless_empty(ours_sha));
        }
        if theirs_sha == base_sha {
            return Ok(some_unless_empty(ours_sha));
        }
        if ours_sha == base_sha {
            return Ok(some_unless_empty(theirs_sha));
        }

        let base = self.fetch_entries(base_sha)?;
        let ours = self.fetch_entries(ours_sha)?;
        let theirs = self.fetch_entries(theirs_sha)?;
        let names = collect_names(&base, &ours, &theirs);
        self.consume_entry_budget(names.len(), path_prefix)?;

        let mut merged: Vec<ParsedTreeEntry> = Vec::with_capacity(names.len());
        for name in names {
            let b = find_entry(&base, &name);
            let o = find_entry(&ours, &name);
            let t = find_entry(&theirs, &name);
            let path = join_path(path_prefix, &name);
            if let Some(entry) = self.merge_entry(b, o, t, &path, depth)? {
                merged.push(entry);
            }
        }

        self.finish_dir(merged, &ours, &theirs, ours_sha, theirs_sha)
    }

    /// Resolve one name within a directory per the cleanliness rules.
    fn merge_entry(
        &mut self,
        base: Option<&ParsedTreeEntry>,
        ours: Option<&ParsedTreeEntry>,
        theirs: Option<&ParsedTreeEntry>,
        path: &str,
        depth: usize,
    ) -> Result<Option<ParsedTreeEntry>, String> {
        debug_assert!(
            base.is_some() || ours.is_some() || theirs.is_some(),
            "merge_entry called for an absent name"
        );
        if same(ours, theirs) {
            return Ok(ours.cloned());
        }
        if same(theirs, base) {
            return Ok(ours.cloned());
        }
        if same(ours, base) {
            return Ok(theirs.cloned());
        }
        // Both sides changed this name. If both still hold a subtree,
        // per-directory disjointness may still resolve it — recurse.
        if let (Some(o), Some(t)) = (ours, theirs)
            && o.is_tree
            && t.is_tree
        {
            let base_subtree = match base {
                Some(b) if b.is_tree => b.sha.clone(),
                _ => EMPTY_TREE_SHA.to_string(),
            };
            let merged_sha = self.merge_dir(&base_subtree, &o.sha, &t.sha, path, depth + 1)?;
            return Ok(merged_sha.map(|sha| ParsedTreeEntry {
                mode: Mode::Tree.as_git_str().to_string(),
                name: o.name.clone(),
                sha,
                is_tree: true,
            }));
        }
        self.conflicts.push(path.to_string());
        Ok(None)
    }

    /// Turn a merged entry list into a tree SHA, reusing an input tree
    /// when the result is identical and synthesizing a new Tree object
    /// otherwise.
    fn finish_dir(
        &mut self,
        merged: Vec<ParsedTreeEntry>,
        ours: &[ParsedTreeEntry],
        theirs: &[ParsedTreeEntry],
        ours_sha: &str,
        theirs_sha: &str,
    ) -> Result<Option<String>, String> {
        if merged.is_empty() {
            return Ok(None);
        }
        if entry_lists_equal(&merged, ours) {
            return Ok(Some(ours_sha.to_string()));
        }
        if entry_lists_equal(&merged, theirs) {
            return Ok(Some(theirs_sha.to_string()));
        }
        let mut entries = Vec::with_capacity(merged.len());
        for entry in &merged {
            let mode = Mode::from_git_str(&entry.mode)
                .ok_or_else(|| format!("invalid tree entry mode '{}'", entry.mode))?;
            entries.push(TreeEntry {
                mode,
                name: entry.name.clone().into_bytes(),
                object_sha: entry.sha.clone(),
            });
        }
        let canonical = tree_canonical_bytes(entries);
        let sha = sha1_hex_of(&canonical);
        debug_assert_eq!(sha.len(), 40, "tree sha must be 40 hex chars");
        if !self.new_trees.iter().any(|t| t.sha == sha) {
            self.new_trees.push(NewTree {
                sha: sha.clone(),
                canonical,
            });
        }
        Ok(Some(sha))
    }

    fn fetch_entries(&mut self, sha: &str) -> Result<Vec<ParsedTreeEntry>, String> {
        if sha == EMPTY_TREE_SHA {
            return Ok(Vec::new());
        }
        (self.fetch)(sha)
    }

    fn consume_entry_budget(&mut self, count: usize, path_prefix: &str) -> Result<(), String> {
        if self.entries_budget < count {
            return Err(format!(
                "tree merge exceeded entry budget {TREE_MERGE_MAX_ENTRIES} at '{path_prefix}'"
            ));
        }
        self.entries_budget -= count;
        Ok(())
    }
}

fn some_unless_empty(sha: &str) -> Option<String> {
    if sha == EMPTY_TREE_SHA {
        None
    } else {
        Some(sha.to_string())
    }
}

fn collect_names(
    base: &[ParsedTreeEntry],
    ours: &[ParsedTreeEntry],
    theirs: &[ParsedTreeEntry],
) -> Vec<String> {
    let mut names: BTreeMap<String, ()> = BTreeMap::new();
    for entry in base.iter().chain(ours).chain(theirs) {
        names.insert(entry.name.clone(), ());
    }
    names.into_keys().collect()
}

fn find_entry<'a>(entries: &'a [ParsedTreeEntry], name: &str) -> Option<&'a ParsedTreeEntry> {
    entries.iter().find(|entry| entry.name == name)
}

fn entry_lists_equal(merged: &[ParsedTreeEntry], side: &[ParsedTreeEntry]) -> bool {
    merged.len() == side.len()
        && merged
            .iter()
            .all(|m| find_entry(side, &m.name).map(entry_key) == Some(entry_key(m)))
}

fn sha1_hex_of(bytes: &[u8]) -> String {
    let mut hasher = genesis_git_object::Sha1::new();
    hasher.update(bytes);
    hasher.hex()
}

fn join_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}
