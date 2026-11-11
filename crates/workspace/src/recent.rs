//! Keeping which projects were recently opened (ADR-0006): a `recent` file
//! beside the settings file, one project id per line, most recent first, at
//! most five distinct ids — Cellworks' own file, never the hand-edited
//! settings file, in the shape `ArrangementStore`/`ArrangementKeeper` already
//! give an arrangement.
//!
//! STORY-078, requirement 6. Reading is tolerant: a line that does not parse
//! as a [`crate::ProjectId`] is left out rather than failing the whole file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::ProjectId;

/// Where recently opened projects are kept: a single file, one id per line,
/// most recent first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentStore {
    file: PathBuf,
}

impl RecentStore {
    /// The store backed by `file`, which need not exist yet.
    pub fn at(file: PathBuf) -> RecentStore {
        RecentStore { file }
    }

    /// The store beside the settings file `settings_file`.
    pub fn beside(settings_file: &Path) -> RecentStore {
        let file = settings_file
            .parent()
            .unwrap_or(Path::new("."))
            .join("recent");
        RecentStore::at(file)
    }

    /// The ids kept, most recent first: empty when none were kept or the
    /// file cannot be read. A line that does not parse as a `ProjectId` is
    /// left out rather than failing the whole read, and at most five are
    /// kept, matching what `save` ever writes.
    pub fn load(&self) -> Vec<ProjectId> {
        let Ok(text) = fs::read_to_string(&self.file) else {
            return Vec::new();
        };
        text.lines().filter_map(ProjectId::parse).take(5).collect()
    }

    /// Keeps `ids` as the recently opened projects, most recent first. A
    /// remote id's path is the settings file's own text and could hold a
    /// newline; such an id is left out rather than written across two lines,
    /// which `load` would then read back as two different ids.
    pub fn save(&self, ids: &[ProjectId]) -> io::Result<()> {
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent)?;
        }
        let text: String = ids
            .iter()
            .filter(|id| !id.to_string().contains('\n'))
            .take(5)
            .map(|id| format!("{id}\n"))
            .collect();
        // Written whole beside it, then put in its place: a write cut short
        // never leaves half a recent list.
        let written = self.file.with_extension("new");
        fs::write(&written, text)?;
        fs::rename(written, &self.file)
    }

    /// A thread that keeps whatever list of ids is handed to it, one after
    /// the other in the order handed: the last one kept is the last one
    /// handed.
    pub fn start_keeping(&self) -> RecentKeeper {
        let (sender, receiver) = mpsc::channel::<Vec<ProjectId>>();
        let store = self.clone();
        std::thread::spawn(move || {
            for ids in receiver {
                // A failed write only loses the update.
                let _ = store.save(&ids);
            }
        });
        RecentKeeper { sender }
    }
}

/// Where the recent list goes to be kept, off the frame loop and in order.
#[derive(Clone, Debug)]
pub struct RecentKeeper {
    sender: mpsc::Sender<Vec<ProjectId>>,
}

impl RecentKeeper {
    /// Keeps `ids` as the recently opened projects.
    pub fn keep(&self, ids: Vec<ProjectId>) {
        let _ = self.sender.send(ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectId;

    /// STORY-078, requirement 6: the order written is the order read back —
    /// most recent first — which is what lets the recent row show them in
    /// that order without sorting them itself.
    #[test]
    fn recent_projects_read_back_most_recent_first() {
        let folder = tempfile::tempdir().expect("a temporary directory");
        let store = RecentStore::at(folder.path().join("recent"));
        let cellworks =
            ProjectId::parse("/Users/engineer/Projects/cellworks").expect("a local id parses");
        let api = ProjectId::parse("az.wsl:~/work/api").expect("a remote id parses");

        store
            .save(&[cellworks.clone(), api.clone()])
            .expect("the recent file is written");

        assert_eq!(store.load(), vec![cellworks, api]);
    }

    /// STORY-078, requirement 6: a line that does not parse as a project id
    /// is left out, rather than the whole file being treated as unreadable.
    #[test]
    fn a_recent_line_that_does_not_parse_is_left_out() {
        let folder = tempfile::tempdir().expect("a temporary directory");
        let path = folder.path().join("recent");
        std::fs::write(&path, "/Users/engineer/Projects/cellworks\nrelative/path\n")
            .expect("write the recent file");
        let store = RecentStore::at(path);

        assert_eq!(
            store.load(),
            vec![
                ProjectId::parse("/Users/engineer/Projects/cellworks").expect("a local id parses")
            ]
        );
    }
}
