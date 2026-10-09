//! An opened project: the state its read is in, and what the read could not
//! do.
//!
//! The same shape as the Workspace's `Universe`: Reading is a state of its own
//! so a slow read never flashes an empty project, and a notice accumulates
//! beside whichever state is current and never replaces the map.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use cellworks_code_analysis::CodeAnalysisEvent;
use cellworks_map::ProjectMap;

/// How far the read has got.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectState {
    /// From the moment the project is opened until its map arrives.
    Reading,
    /// The project's map, with at least one district.
    Mapped(ProjectMap),
    /// The project was read and has no included file.
    Empty,
    /// The read ended without a map.
    Failed,
}

/// Something about the opened project the engineer should know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenedNotice {
    /// A folder of the project could not be read.
    FolderSkipped { path: PathBuf, reason: String },
    /// git could not say which files it ignores.
    GitExcludesUnavailable { reason: String },
    /// The settings name a district prefix rule that does not exist; the
    /// default was used instead.
    UnknownDistrictPrefix { name: String },
}

/// The project on screen, and the read that fills it in.
pub struct OpenedProject {
    state: ProjectState,
    notices: Vec<OpenedNotice>,
    events: Receiver<CodeAnalysisEvent>,
}

impl OpenedProject {
    /// A project whose read is under way and reports over `events`.
    pub fn reading(events: Receiver<CodeAnalysisEvent>) -> OpenedProject {
        OpenedProject {
            state: ProjectState::Reading,
            notices: Vec::new(),
            events,
        }
    }

    /// How far the read has got.
    pub fn state(&self) -> &ProjectState {
        &self.state
    }

    /// Everything the engineer should know about it, in the order it was said.
    pub fn notices(&self) -> &[OpenedNotice] {
        &self.notices
    }

    /// Adds a notice that did not come from the read.
    pub fn note(&mut self, notice: OpenedNotice) {
        self.notices.push(notice);
    }

    /// Folds one event into the state.
    pub fn apply(&mut self, event: CodeAnalysisEvent) {
        match event {
            CodeAnalysisEvent::ProjectMapped { map, .. } if map.is_empty() => {
                self.state = ProjectState::Empty;
            }
            CodeAnalysisEvent::ProjectMapped { map, .. } => {
                self.state = ProjectState::Mapped(map);
            }
            CodeAnalysisEvent::ProjectFolderSkipped { path, reason, .. } => {
                self.note(OpenedNotice::FolderSkipped { path, reason });
            }
            CodeAnalysisEvent::GitExcludesUnavailable { reason, .. } => {
                self.note(OpenedNotice::GitExcludesUnavailable { reason });
            }
        }
    }

    /// Takes everything the read has sent so far. Never waits. A read that
    /// goes away before its map arrives has failed; after it, it is just over.
    pub fn drain(&mut self) {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.apply(event),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    if self.state == ProjectState::Reading {
                        self.state = ProjectState::Failed;
                    }
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::mpsc;

    use cellworks_map::{Directories, default_prefix_rule};

    use super::*;

    fn map(files: &[&str]) -> ProjectMap {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of("shop", Path::new("/p/shop"), &files, &strategy)
    }

    fn opened() -> (OpenedProject, mpsc::Sender<CodeAnalysisEvent>) {
        let (sender, receiver) = mpsc::channel();
        (OpenedProject::reading(receiver), sender)
    }

    fn mapped(files: &[&str]) -> CodeAnalysisEvent {
        CodeAnalysisEvent::ProjectMapped {
            project: PathBuf::from("/p/shop"),
            map: map(files),
        }
    }

    fn skipped() -> CodeAnalysisEvent {
        CodeAnalysisEvent::ProjectFolderSkipped {
            project: PathBuf::from("/p/shop"),
            path: PathBuf::from("/p/shop/closed"),
            reason: "denied".to_owned(),
        }
    }

    #[test]
    fn a_map_with_districts_is_mapped_and_one_without_is_empty() {
        let (mut project, _sender) = opened();
        project.apply(mapped(&["src/a.rs"]));
        assert!(matches!(project.state(), ProjectState::Mapped(_)));

        let (mut project, _sender) = opened();
        project.apply(mapped(&[]));
        assert_eq!(project.state(), &ProjectState::Empty);
    }

    #[test]
    fn a_read_that_goes_away_before_its_map_has_failed() {
        let (mut project, sender) = opened();
        drop(sender);

        project.drain();

        assert_eq!(project.state(), &ProjectState::Failed);
    }

    #[test]
    fn a_read_that_goes_away_after_its_map_has_not_failed() {
        let (mut project, sender) = opened();
        sender.send(mapped(&["src/a.rs"])).expect("listening");
        drop(sender);

        project.drain();

        assert!(matches!(project.state(), ProjectState::Mapped(_)));
    }

    #[test]
    fn a_notice_neither_ends_reading_nor_replaces_the_map() {
        let (mut project, _sender) = opened();
        project.apply(skipped());
        assert_eq!(project.state(), &ProjectState::Reading);

        project.apply(mapped(&["src/a.rs"]));
        project.apply(skipped());
        assert!(matches!(project.state(), ProjectState::Mapped(_)));
        assert_eq!(project.notices().len(), 2);
    }

    #[test]
    fn draining_a_silent_read_returns_rather_than_waiting() {
        let (mut project, _sender) = opened();

        project.drain();

        assert_eq!(project.state(), &ProjectState::Reading);
    }
}
