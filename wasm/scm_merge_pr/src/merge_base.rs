//! Bounded merge-base computation over the Commit DAG.
//!
//! The walk reads parents through a caller-supplied closure (one
//! OData `Commits('{entity_id}')` read per commit in production,
//! a synthetic map in tests) so the algorithm itself stays pure and
//! deterministic.
//!
//! Strategy: simultaneous breadth-first expansion from both tips,
//! coloring each visited commit by the side(s) that reached it. The
//! first commit reached from both sides is returned. BFS-by-parent-
//! edges yields the nearest common ancestor by edge count — for the
//! histories ADR-0024 targets this is the merge base; in pathological
//! criss-cross histories it is *a* common ancestor, which only makes
//! the tree-level cleanliness check more conservative (more paths
//! look changed on both sides), never less. Refusal over wrong merge.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Upper bound on commits visited across both sides of the walk.
/// Past this budget the merge base is treated as undecidable and the
/// merge is refused — bounded work per ADR-0024 / TigerStyle.
pub const MERGE_BASE_WALK_MAX_COMMITS: usize = 4096;

/// Which side(s) of the walk reached a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Ours,
    Theirs,
}

/// Outcome of the bounded walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeBase {
    /// A common ancestor was found.
    Found(String),
    /// The histories share no common ancestor within budget.
    None,
    /// The walk budget was exhausted before an answer was reached.
    BudgetExhausted,
}

/// Find the merge base of `ours` and `theirs`.
///
/// `parents_of` returns the parent SHAs of a commit; unknown commits
/// contribute no parents (the walk cannot pass through them). Errors
/// from the closure abort the walk.
pub fn find_merge_base(
    ours: &str,
    theirs: &str,
    mut parents_of: impl FnMut(&str) -> Result<Vec<String>, String>,
) -> Result<MergeBase, String> {
    assert!(!ours.is_empty(), "merge-base ours tip must be non-empty");
    assert!(
        !theirs.is_empty(),
        "merge-base theirs tip must be non-empty"
    );
    if ours == theirs {
        return Ok(MergeBase::Found(ours.to_string()));
    }

    let mut colors: BTreeMap<String, Color> = BTreeMap::new();
    let mut queue: VecDeque<(String, Color)> = VecDeque::new();
    colors.insert(ours.to_string(), Color::Ours);
    colors.insert(theirs.to_string(), Color::Theirs);
    queue.push_back((ours.to_string(), Color::Ours));
    queue.push_back((theirs.to_string(), Color::Theirs));

    let mut visited = 0usize;
    while let Some((sha, color)) = queue.pop_front() {
        visited += 1;
        if visited > MERGE_BASE_WALK_MAX_COMMITS {
            return Ok(MergeBase::BudgetExhausted);
        }
        for parent in parents_of(&sha)? {
            match colors.get(&parent) {
                Some(existing) if *existing != color => {
                    // Reached from both sides — this is the base.
                    return Ok(MergeBase::Found(parent));
                }
                Some(_) => {} // already queued from this side
                None => {
                    colors.insert(parent.clone(), color);
                    queue.push_back((parent, color));
                }
            }
        }
    }
    Ok(MergeBase::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(
        edges: &[(&str, &[&str])],
    ) -> impl FnMut(&str) -> Result<Vec<String>, String> + use<> {
        let map: BTreeMap<String, Vec<String>> = edges
            .iter()
            .map(|(sha, parents)| {
                (
                    sha.to_string(),
                    parents.iter().map(|p| p.to_string()).collect(),
                )
            })
            .collect();
        move |sha: &str| Ok(map.get(sha).cloned().unwrap_or_default())
    }

    #[test]
    fn same_tip_is_its_own_base() {
        let base = find_merge_base("c1", "c1", |_| Ok(Vec::new())).unwrap();
        assert_eq!(base, MergeBase::Found("c1".to_string()));
    }

    #[test]
    fn linear_ancestor_is_base() {
        // main: c1 <- c2 <- c3 ; feature tip c3, base tip c1.
        let parents = graph(&[("c3", &["c2"]), ("c2", &["c1"]), ("c1", &[])]);
        let base = find_merge_base("c1", "c3", parents).unwrap();
        assert_eq!(base, MergeBase::Found("c1".to_string()));
    }

    #[test]
    fn diverged_branches_share_fork_point() {
        //      a1 <- a2   (ours)
        // c0 <
        //      b1         (theirs)
        let parents = graph(&[
            ("a2", &["a1"]),
            ("a1", &["c0"]),
            ("b1", &["c0"]),
            ("c0", &[]),
        ]);
        let base = find_merge_base("a2", "b1", parents).unwrap();
        assert_eq!(base, MergeBase::Found("c0".to_string()));
    }

    #[test]
    fn merge_commit_parents_are_walked() {
        // ours tip is a merge commit; theirs hangs off one parent.
        let parents = graph(&[
            ("m1", &["a1", "b1"]),
            ("a1", &["c0"]),
            ("b1", &["c0"]),
            ("b2", &["b1"]),
            ("c0", &[]),
        ]);
        let base = find_merge_base("m1", "b2", parents).unwrap();
        assert_eq!(base, MergeBase::Found("b1".to_string()));
    }

    #[test]
    fn unrelated_histories_have_no_base() {
        let parents = graph(&[("a1", &["a0"]), ("a0", &[]), ("b1", &["b0"]), ("b0", &[])]);
        let base = find_merge_base("a1", "b1", parents).unwrap();
        assert_eq!(base, MergeBase::None);
    }

    #[test]
    fn budget_exhaustion_is_reported_not_guessed() {
        // Endless synthetic chains that never intersect.
        let mut calls = 0usize;
        let base = find_merge_base("a", "b", |sha| {
            calls += 1;
            Ok(alloc::vec![alloc::format!("{sha}x")])
        })
        .unwrap();
        assert_eq!(base, MergeBase::BudgetExhausted);
        assert!(calls <= MERGE_BASE_WALK_MAX_COMMITS + 2);
    }

    #[test]
    fn closure_errors_abort_the_walk() {
        let err = find_merge_base("a", "b", |_| Err("backend down".to_string())).unwrap_err();
        assert!(err.contains("backend down"));
    }
}
