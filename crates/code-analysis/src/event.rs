//! What the CodeAnalysis context tells the rest of the application.

use std::collections::HashSet;
use std::path::PathBuf;

use cellworks_map::ProjectMap;

/// Something reading a project has finished doing.
///
/// Sent from the read thread over a `std::sync::mpsc` channel and drained by
/// the UI thread once per frame.
#[derive(Clone, Debug, PartialEq)]
pub enum CodeAnalysisEvent {
    /// The project was read: this is its map. Sent once, last, when the read
    /// got as far as building one.
    /// Boxed, the map being far larger than the other events.
    ProjectMapped {
        project: PathBuf,
        map: Box<ProjectMap>,
    },
    /// A folder of the project could not be read. The read skipped it and
    /// carried on; when it is the project folder itself, nothing is mapped.
    ProjectFolderSkipped {
        project: PathBuf,
        path: PathBuf,
        reason: String,
    },
    /// git could not say which files it ignores, so the project was read with
    /// the hidden-folder rule alone and ignored files may be on the map.
    GitExcludesUnavailable { project: PathBuf, reason: String },
    /// A project's branch, file count, last commit time and folder
    /// modification time were read, for the launcher's inspector and its
    /// sorts. `None` for a field this story leaves unknown: a plain folder
    /// has no branch, no file count and no commit time — never zero. Times
    /// are seconds since the Unix epoch.
    DetailsRead {
        project: PathBuf,
        branch: Option<String>,
        files: Option<usize>,
        committed: Option<i64>,
        changed: Option<i64>,
    },
    /// The open project's uncommitted changes were read: absolute paths under
    /// the project root with a modified or untracked working-tree file.
    ChangesRead {
        project: PathBuf,
        changed: HashSet<PathBuf>,
    },
}
