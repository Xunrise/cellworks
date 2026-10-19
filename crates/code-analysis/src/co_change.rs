//! Co-change: which files git history shows changed together.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cellworks_map::CoChange;

/// A commit changing more files than this is a sweep (a rename, a format
/// run), not a sign that its files belong together.
const SWEEP: usize = 200;

/// The pairs of `files`, relative to `root`, that `commits` changed
/// together: each commit adds one over its number of files to each pair it
/// changed. Paths come out canonical, under `root`.
pub(crate) fn co_changes(
    root: &Path,
    files: &[PathBuf],
    commits: &[Vec<PathBuf>],
) -> Vec<CoChange> {
    let included: BTreeSet<&Path> = files.iter().map(PathBuf::as_path).collect();
    let mut pairs: BTreeMap<(&Path, &Path), f32> = BTreeMap::new();
    for commit in commits.iter().filter(|commit| commit.len() <= SWEEP) {
        let weight = 1.0 / commit.len() as f32;
        let mut changed: Vec<&Path> = commit
            .iter()
            .map(PathBuf::as_path)
            .filter(|file| included.contains(file))
            .collect();
        changed.sort();
        changed.dedup();
        for (i, a) in changed.iter().enumerate() {
            for b in &changed[i + 1..] {
                *pairs.entry((a, b)).or_default() += weight;
            }
        }
    }
    pairs
        .into_iter()
        .map(|((a, b), weight)| CoChange {
            files: (root.join(a), root.join(b)),
            weight,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn files_changed_together_pair_up_weighted_by_the_size_of_each_commit() {
        let files = paths(&["a.rs", "b.rs", "c.rs"]);
        let commits = [
            paths(&["a.rs", "b.rs"]),
            paths(&["a.rs", "b.rs", "c.rs", "gone.rs"]),
        ];

        let pairs = co_changes(Path::new("/p"), &files, &commits);

        let weight = |a: &str, b: &str| {
            pairs
                .iter()
                .find(|pair| pair.files == (PathBuf::from(a), PathBuf::from(b)))
                .map(|pair| pair.weight)
        };
        assert_eq!(weight("/p/a.rs", "/p/b.rs"), Some(0.5 + 0.25));
        assert_eq!(weight("/p/b.rs", "/p/c.rs"), Some(0.25));
        assert_eq!(pairs.len(), 3, "gone.rs is not among the files");
    }

    #[test]
    fn a_sweep_across_the_project_pairs_nothing() {
        let files: Vec<PathBuf> = (0..=SWEEP)
            .map(|n| PathBuf::from(format!("f{n}.rs")))
            .collect();

        assert!(co_changes(Path::new("/p"), &files, std::slice::from_ref(&files)).is_empty());
    }
}
