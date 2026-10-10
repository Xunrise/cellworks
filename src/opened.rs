//! An opened project: the state its read is in, and what the read could not
//! do.
//!
//! The same shape as the Workspace's `Universe`: Reading is a state of its own
//! so a slow read never flashes an empty project, and a notice accumulates
//! beside whichever state is current and never replaces the map. A project
//! read again keeps its map on screen until the new one arrives.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use cellworks_code_analysis::CodeAnalysisEvent;
use cellworks_map::ProjectMap;
use cellworks_workspace::Project;

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
    project: Project,
    state: ProjectState,
    notices: Vec<OpenedNotice>,
    events: Receiver<CodeAnalysisEvent>,
    rereading: bool,
    /// Quarters the map is turned clockwise; a map read again is turned as
    /// far.
    turns: u8,
}

impl OpenedProject {
    /// `project`, whose read is under way and reports over `events`.
    pub fn reading(project: Project, events: Receiver<CodeAnalysisEvent>) -> OpenedProject {
        OpenedProject {
            project,
            state: ProjectState::Reading,
            notices: Vec::new(),
            events,
            rereading: false,
            turns: 0,
        }
    }

    /// The project that is open.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// How far the read has got.
    pub fn state(&self) -> &ProjectState {
        &self.state
    }

    /// Whether a read, the first or a later one, has yet to send its map.
    pub fn is_reading(&self) -> bool {
        self.rereading || self.state == ProjectState::Reading
    }

    /// Reads the project again over `events`. The state stays as it is until
    /// the new map arrives; the notices belonged to the read before.
    pub fn reread(&mut self, events: Receiver<CodeAnalysisEvent>) {
        self.events = events;
        self.notices.clear();
        self.rereading = true;
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
            CodeAnalysisEvent::ProjectMapped { map, .. } => {
                self.rereading = false;
                self.state = if map.is_empty() {
                    ProjectState::Empty
                } else {
                    let mut map = map;
                    map.turn(self.turns);
                    ProjectState::Mapped(map)
                };
            }
            CodeAnalysisEvent::ProjectFolderSkipped { path, reason, .. } => {
                self.note(OpenedNotice::FolderSkipped { path, reason });
            }
            CodeAnalysisEvent::GitExcludesUnavailable { reason, .. } => {
                self.note(OpenedNotice::GitExcludesUnavailable { reason });
            }
        }
    }

    /// Turns the map `quarters` quarters clockwise, and returns the centre it
    /// turned about. `None` while there is no map to turn.
    pub fn turn(&mut self, quarters: u8) -> Option<egui::Pos2> {
        let ProjectState::Mapped(map) = &mut self.state else {
            return None;
        };
        self.turns = (self.turns + quarters) % 4;
        let centre = map.ground().center();
        map.turn(quarters);
        Some(centre)
    }

    /// Takes everything the read has sent so far. Never waits. A read that
    /// goes away before its map arrives has failed; after it, it is just over.
    /// A read again that goes away without a map leaves the old one on screen.
    pub fn drain(&mut self) {
        loop {
            match self.events.try_recv() {
                Ok(event) => self.apply(event),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.rereading = false;
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
        let project = Project::at(PathBuf::from("/p/shop"));
        (OpenedProject::reading(project, receiver), sender)
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
    fn a_project_read_again_keeps_its_map_until_the_new_one_arrives() {
        let (mut project, sender) = opened();
        sender.send(mapped(&["src/a.rs"])).expect("listening");
        sender.send(skipped()).expect("listening");
        project.drain();
        let before = project.state().clone();

        let (sender, receiver) = mpsc::channel();
        project.reread(receiver);
        project.drain();
        assert!(project.is_reading());
        assert_eq!(project.state(), &before);
        assert!(project.notices().is_empty());

        sender
            .send(mapped(&["src/a.rs", "src/b.rs"]))
            .expect("listening");
        project.drain();
        assert!(!project.is_reading());
        assert_ne!(project.state(), &before);
    }

    #[test]
    fn a_read_again_that_goes_away_leaves_the_old_map() {
        let (mut project, sender) = opened();
        sender.send(mapped(&["src/a.rs"])).expect("listening");
        project.drain();

        let (sender, receiver) = mpsc::channel();
        project.reread(receiver);
        drop(sender);
        project.drain();

        assert!(!project.is_reading());
        assert!(matches!(project.state(), ProjectState::Mapped(_)));
    }

    #[test]
    fn draining_a_silent_read_returns_rather_than_waiting() {
        let (mut project, _sender) = opened();

        project.drain();

        assert_eq!(project.state(), &ProjectState::Reading);
    }
}
