//! What the CodeAnalysis context tells the rest of the application.

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
    ProjectMapped { project: PathBuf, map: ProjectMap },
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
}
