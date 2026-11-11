//! Per-line provenance: the commit that last wrote each line of a file,
//! joined to the pull request and the stories its message names.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One commit a project's lines may point to.
#[derive(Clone, Debug, PartialEq)]
pub struct Commit {
    pub sha: String,
    /// The commit time, seconds since the Unix epoch.
    pub time: i64,
    /// The first line of its message.
    pub summary: String,
    /// The last `(#N)` of the subject, as a squash merge onto GitHub writes
    /// it; `None` without one or for a malformed one.
    pub pull_request: Option<u32>,
    /// Every `STORY-NNN` the whole message names, any case, in order of
    /// first mention, de-duplicated, upper case with at least three digits.
    pub stories: Vec<String>,
}

/// Per-line provenance for a project: which commit last wrote each line of
/// each file. Empty by default, and for a folder that is not a git
/// repository.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineHistory {
    /// Oldest first, so a replay can step through them in order.
    pub commits: Vec<Commit>,
    /// Keyed by the same path form the map keys a floor's file by (see
    /// `ProjectMap::of_code`); each entry holds one slot per line of the
    /// file, a `Some` indexing `commits`, a line not yet committed `None`.
    pub lines: BTreeMap<PathBuf, Vec<Option<usize>>>,
}

impl LineHistory {
    /// The commit that last wrote line `number` of `file`, `number` from 1 as
    /// [`crate::Floor::number`] counts it; `None` for line 0, past the end,
    /// an unknown file or an uncommitted line.
    pub fn of_line(&self, file: &Path, number: usize) -> Option<&Commit> {
        let index = number.checked_sub(1)?;
        let commit = (*self.lines.get(file)?.get(index)?)?;
        self.commits.get(commit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(sha: &str) -> Commit {
        Commit {
            sha: sha.to_owned(),
            time: 0,
            summary: "a change".to_owned(),
            pull_request: None,
            stories: Vec::new(),
        }
    }

    #[test]
    fn a_floor_finds_the_commit_that_last_wrote_it() {
        let first = commit("a");
        let second = commit("b");
        let mut lines = BTreeMap::new();
        lines.insert(PathBuf::from("/p/a.rs"), vec![Some(0), Some(1), None]);
        let history = LineHistory {
            commits: vec![first.clone(), second.clone()],
            lines,
        };

        assert_eq!(history.of_line(Path::new("/p/a.rs"), 1), Some(&first));
        assert_eq!(history.of_line(Path::new("/p/a.rs"), 2), Some(&second));
    }

    #[test]
    fn line_zero_past_the_end_and_unknown_files_have_no_commit() {
        let mut lines = BTreeMap::new();
        lines.insert(PathBuf::from("/p/a.rs"), vec![Some(0), None]);
        let history = LineHistory {
            commits: vec![commit("a")],
            lines,
        };

        assert_eq!(history.of_line(Path::new("/p/a.rs"), 0), None);
        assert_eq!(
            history.of_line(Path::new("/p/a.rs"), 2),
            None,
            "past the end"
        );
        assert_eq!(
            history.of_line(Path::new("/p/a.rs"), 3),
            None,
            "past the end"
        );
        assert_eq!(
            history.of_line(Path::new("/p/unknown.rs"), 1),
            None,
            "unknown file"
        );
        assert_eq!(
            history.of_line(Path::new("/p/a.rs"), 2),
            None,
            "an uncommitted line"
        );
    }
}
