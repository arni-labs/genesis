//! Merge / squash commit construction (ADR-0024).
//!
//! Determinism contract: this module never reads a wall clock. The
//! commit timestamp comes from trigger params when provided
//! (`CommitTimestamp` seconds + optional `CommitTimezone`), otherwise
//! it is taken from the PR head commit's committer line. Replaying
//! the same trigger against the same entity state therefore always
//! produces byte-identical commit objects — which Temper's
//! deterministic simulation and the byte-exact compat contract both
//! require.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_object::{Commit, commit_canonical_bytes, commit_hash};

/// Deterministic server identity used when the trigger carries no
/// committer identity (the current specs pass none). The `.invalid`
/// TLD is reserved (RFC 2606) — it can never be a deliverable address.
pub const SERVER_IDENTITY: &str = "Genesis <merge@genesis.invalid>";

/// A constructed commit object ready for persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltCommit {
    pub sha: String,
    /// Full canonical bytes including the `commit <len>\0` header.
    pub canonical: Vec<u8>,
    pub tree_sha: String,
    pub parent_shas: Vec<String>,
    pub author: String,
    pub committer: String,
    pub message: String,
}

/// Inputs that determine the commit bytes; everything is explicit so
/// the same inputs always hash identically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInputs {
    /// `Name <email>` — no timestamp suffix.
    pub identity: String,
    /// Unix seconds, as decimal string.
    pub timestamp: String,
    /// e.g. `+0000`.
    pub timezone: String,
    pub message: String,
}

impl CommitInputs {
    fn identity_line(&self) -> String {
        format!("{} {} {}", self.identity, self.timestamp, self.timezone)
    }
}

/// Build the two-parent merge commit: first parent is the mainline
/// (target-branch tip), second is the PR head.
pub fn build_merge_commit(
    tree_sha: &str,
    base_tip: &str,
    head_tip: &str,
    inputs: &CommitInputs,
) -> BuiltCommit {
    assert_eq!(tree_sha.len(), 40, "merge tree sha must be 40 hex chars");
    assert_ne!(base_tip, head_tip, "merge parents must differ");
    build(
        tree_sha,
        alloc::vec![base_tip.to_string(), head_tip.to_string()],
        inputs,
    )
}

/// Build the squash commit: one parent (the target-branch tip).
pub fn build_squash_commit(tree_sha: &str, base_tip: &str, inputs: &CommitInputs) -> BuiltCommit {
    assert_eq!(tree_sha.len(), 40, "squash tree sha must be 40 hex chars");
    assert_eq!(base_tip.len(), 40, "squash parent sha must be 40 hex chars");
    build(tree_sha, alloc::vec![base_tip.to_string()], inputs)
}

fn build(tree_sha: &str, parents: Vec<String>, inputs: &CommitInputs) -> BuiltCommit {
    let line = inputs.identity_line();
    let commit = Commit {
        tree: tree_sha.to_string(),
        parents: parents.clone(),
        author: line.clone(),
        committer: line,
        pgp_signature: None,
        message: inputs.message.clone(),
    };
    let sha = commit_hash(&commit);
    let canonical = commit_canonical_bytes(&commit);
    debug_assert_eq!(sha.len(), 40, "commit sha must be 40 hex chars");
    debug_assert!(!canonical.is_empty(), "commit canonical bytes empty");
    BuiltCommit {
        sha,
        canonical,
        tree_sha: tree_sha.to_string(),
        parent_shas: parents,
        author: commit.author,
        committer: commit.committer,
        message: inputs.message.clone(),
    }
}

/// Extract `(unix_seconds, timezone)` from a git identity line such
/// as `Name <email> 1234567890 +0000` — the last two whitespace-
/// separated tokens.
pub fn timestamp_from_identity_line(line: &str) -> Result<(String, String), String> {
    let mut tokens = line.split_whitespace().rev();
    let timezone = tokens
        .next()
        .ok_or_else(|| format!("identity line '{line}' has no timezone token"))?;
    let timestamp = tokens
        .next()
        .ok_or_else(|| format!("identity line '{line}' has no timestamp token"))?;
    if !timestamp.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "identity line '{line}' timestamp '{timestamp}' is not numeric"
        ));
    }
    let tz_ok = timezone.len() == 5
        && (timezone.starts_with('+') || timezone.starts_with('-'))
        && timezone[1..].chars().all(|c| c.is_ascii_digit());
    if !tz_ok {
        return Err(format!(
            "identity line '{line}' timezone '{timezone}' is not ±HHMM"
        ));
    }
    Ok((timestamp.to_string(), timezone.to_string()))
}

/// Default merge-commit message, GitHub-shaped:
/// `Merge pull request #N from <branch>` + blank line + title.
pub fn default_merge_message(number: Option<u64>, source_ref: &str, title: &str) -> String {
    let branch = source_ref.strip_prefix("refs/heads/").unwrap_or(source_ref);
    let subject = match number {
        Some(n) => format!("Merge pull request #{n} from {branch}"),
        None => format!("Merge pull request from {branch}"),
    };
    if title.is_empty() {
        format!("{subject}\n")
    } else {
        format!("{subject}\n\n{title}\n")
    }
}

/// Default squash-commit message, GitHub-shaped: `<title> (#N)`.
pub fn default_squash_message(number: Option<u64>, source_ref: &str, title: &str) -> String {
    let branch = source_ref.strip_prefix("refs/heads/").unwrap_or(source_ref);
    match (title.is_empty(), number) {
        (false, Some(n)) => format!("{title} (#{n})\n"),
        (false, None) => format!("{title}\n"),
        (true, Some(n)) => format!("Squash merge pull request #{n} from {branch}\n"),
        (true, None) => format!("Squash merge pull request from {branch}\n"),
    }
}

/// Normalize a caller-supplied message: git commits conventionally
/// end with a newline; `git commit -m` adds one, so we match it.
pub fn normalize_message(message: &str) -> String {
    if message.ends_with('\n') {
        message.to_string()
    } else {
        format!("{message}\n")
    }
}
