//! The universe as the view sees it: which state is shown, and what the scan
//! has reported so far.
//!
//! Scanning is a state of its own rather than "empty until proven otherwise",
//! so a slow walk over a large projects folder never flashes an empty universe
//! on the way. A reported failure is accumulated beside whichever state is
//! current and never replaces the project list: a failure belongs to the
//! folder it happened in and takes nothing else down.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::Sources;
use crate::event::WorkspaceEvent;
use crate::project::Project;

/// How far the scan has got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanState {
    /// From the first frame until the first result arrives.
    Scanning,
    /// The scan finished. The list is sorted by canonical path, and it is
    /// empty when nothing was found.
    Reported(Vec<Project>),
    /// The scan ended without reporting — its thread went away.
    Failed,
}

/// Which failure a notice is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    ProjectsFolderUnreadable,
    AddedFolderSkipped,
    SettingsUnreadable,
}

/// Something the scan could not do, shown beside the universe rather than
/// swallowed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    /// The folder or file the failure belongs to.
    pub path: PathBuf,
    /// What the filesystem or the parser said.
    pub reason: String,
}

/// The state the universe view draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Universe {
    scan: ScanState,
    notices: Vec<Notice>,
    sources: Option<Sources>,
}

impl Universe {
    /// A universe whose scan of `sources` is under way.
    pub fn scanning(sources: &Sources) -> Universe {
        Universe {
            scan: ScanState::Scanning,
            notices: Vec::new(),
            sources: Some(sources.clone()),
        }
    }

    /// A universe with nowhere to scan, because `$HOME` is unset: already
    /// reported, already empty, and with no projects folder or settings file
    /// it could name.
    pub fn without_home() -> Universe {
        Universe {
            scan: ScanState::Reported(Vec::new()),
            notices: Vec::new(),
            sources: None,
        }
    }

    /// How far the scan has got.
    pub fn scan(&self) -> &ScanState {
        &self.scan
    }

    /// Everything the scan could not do, in the order it said so.
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    /// Where the projects would come from, when there is anywhere.
    pub fn sources(&self) -> Option<&Sources> {
        self.sources.as_ref()
    }

    /// Folds one event into the state.
    pub fn apply(&mut self, event: WorkspaceEvent) {
        match event {
            WorkspaceEvent::ProjectsDiscovered(projects) => {
                self.scan = ScanState::Reported(projects);
            }
            WorkspaceEvent::ProjectsFolderUnreadable { path, reason } => {
                self.note(NoticeKind::ProjectsFolderUnreadable, path, reason);
            }
            WorkspaceEvent::AddedFolderSkipped { path, reason } => {
                self.note(NoticeKind::AddedFolderSkipped, path, reason);
            }
            WorkspaceEvent::SettingsUnreadable { path, reason } => {
                self.note(NoticeKind::SettingsUnreadable, path, reason);
            }
            // An opened project's, not the universe's.
            WorkspaceEvent::ArrangementRead { .. } => {}
        }
    }

    /// Takes everything the scan thread has sent so far, and nothing else.
    ///
    /// `try_recv` only: the UI thread never waits on a filesystem walk. A
    /// sender that has gone away before reporting is the scan thread having
    /// died, and is the one thing that stops the view showing "Scanning" for
    /// ever; after a report, the sender dropping is just the scan being over.
    pub fn drain(&mut self, events: &Receiver<WorkspaceEvent>) {
        loop {
            match events.try_recv() {
                Ok(event) => self.apply(event),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    if self.scan == ScanState::Scanning {
                        self.scan = ScanState::Failed;
                    }
                    return;
                }
            }
        }
    }

    fn note(&mut self, kind: NoticeKind, path: PathBuf, reason: String) {
        self.notices.push(Notice { kind, path, reason });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn sources() -> Sources {
        Sources {
            projects_folder: PathBuf::from("/nowhere/Projects"),
            settings_file: PathBuf::from("/nowhere/settings.toml"),
        }
    }

    fn a_project(name: &str) -> Project {
        Project::at(PathBuf::from("/nowhere/Projects").join(name))
    }

    fn settings_unreadable() -> WorkspaceEvent {
        WorkspaceEvent::SettingsUnreadable {
            path: PathBuf::from("/nowhere/settings.toml"),
            reason: "expected a value".to_owned(),
        }
    }

    #[test]
    fn a_universe_starts_out_scanning() {
        assert_eq!(Universe::scanning(&sources()).scan(), &ScanState::Scanning);
    }

    #[test]
    fn a_reported_scan_leaves_the_projects_it_found() {
        let mut universe = Universe::scanning(&sources());

        universe.apply(WorkspaceEvent::ProjectsDiscovered(vec![a_project("alpha")]));

        assert_eq!(
            universe.scan(),
            &ScanState::Reported(vec![a_project("alpha")])
        );
    }

    #[test]
    fn a_scan_that_found_nothing_is_reported_rather_than_still_scanning() {
        let mut universe = Universe::scanning(&sources());

        universe.apply(WorkspaceEvent::ProjectsDiscovered(Vec::new()));

        assert_eq!(universe.scan(), &ScanState::Reported(Vec::new()));
    }

    #[test]
    fn a_notice_before_the_scan_reports_leaves_it_scanning() {
        let mut universe = Universe::scanning(&sources());

        universe.apply(settings_unreadable());

        assert_eq!(universe.scan(), &ScanState::Scanning);
        assert_eq!(universe.notices().len(), 1);
        assert_eq!(universe.notices()[0].kind, NoticeKind::SettingsUnreadable);
    }

    #[test]
    fn a_notice_after_the_scan_reports_leaves_the_project_list_alone() {
        let mut universe = Universe::scanning(&sources());
        universe.apply(WorkspaceEvent::ProjectsDiscovered(vec![a_project("alpha")]));

        universe.apply(settings_unreadable());

        assert_eq!(
            universe.scan(),
            &ScanState::Reported(vec![a_project("alpha")])
        );
        assert_eq!(universe.notices().len(), 1);
    }

    #[test]
    fn a_scan_thread_that_dies_before_reporting_leaves_the_scanning_state() {
        let (sender, receiver) = mpsc::channel::<WorkspaceEvent>();
        let mut universe = Universe::scanning(&sources());
        drop(sender);

        universe.drain(&receiver);

        assert_eq!(universe.scan(), &ScanState::Failed);
    }

    #[test]
    fn a_scan_thread_that_ends_after_reporting_is_not_a_failure() {
        let (sender, receiver) = mpsc::channel();
        let mut universe = Universe::scanning(&sources());
        sender
            .send(WorkspaceEvent::ProjectsDiscovered(vec![a_project("alpha")]))
            .expect("the receiver is alive");
        drop(sender);

        universe.drain(&receiver);

        assert_eq!(
            universe.scan(),
            &ScanState::Reported(vec![a_project("alpha")])
        );
    }

    #[test]
    fn draining_an_empty_channel_changes_nothing_and_does_not_wait() {
        let (sender, receiver) = mpsc::channel::<WorkspaceEvent>();
        let mut universe = Universe::scanning(&sources());

        universe.drain(&receiver);

        assert_eq!(universe.scan(), &ScanState::Scanning);
        drop(sender);
    }

    #[test]
    fn a_universe_without_a_home_directory_is_reported_and_empty() {
        let universe = Universe::without_home();

        assert_eq!(universe.scan(), &ScanState::Reported(Vec::new()));
        assert_eq!(universe.sources(), None);
    }
}
