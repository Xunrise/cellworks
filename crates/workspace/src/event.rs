//! What the Workspace context tells the rest of the application.
//!
//! One enum per context, variants in the past tense: the naming guideline, and
//! the shape context-map relationship 6 (Workspace to Map) takes from here on.
//! Adding a variant later is additive; renaming one is not.

use std::path::PathBuf;

use crate::project::Project;
use crate::{Arrangement, ProjectId};

/// Something the Workspace context has finished doing.
///
/// Sent from the scan thread over a `std::sync::mpsc` channel and drained by
/// the UI thread once per frame.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceEvent {
    /// The scan finished: these are the projects of the universe, sorted by
    /// canonical path. Exactly one of these is sent per scan, even when the
    /// list is empty.
    ProjectsDiscovered(Vec<Project>),
    /// A directory at or below the projects folder could not be read. The walk
    /// skipped it and carried on.
    ProjectsFolderUnreadable { path: PathBuf, reason: String },
    /// A folder listed in the settings file did not resolve to a readable
    /// directory. It is not a project, and it is left in the file untouched.
    AddedFolderSkipped { path: PathBuf, reason: String },
    /// The settings file exists but could not be read or parsed. The projects
    /// found under the projects folder are reported all the same.
    SettingsUnreadable { path: PathBuf, reason: String },
    /// A project's custom arrangement was read: empty when none was kept.
    ArrangementRead {
        project: ProjectId,
        arrangement: Arrangement,
    },
    /// A host named in the settings file was skipped: its name could not be
    /// trusted to reach ssh's argument vector safely.
    HostSkipped { name: String, reason: String },
    /// A background reachability probe answered for one remote host.
    HostProbed { host: String, connected: bool },
    /// The most recently opened projects were read from the `recent` file,
    /// most recent first.
    RecentRead(Vec<ProjectId>),
}
