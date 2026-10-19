//! An opened project: the state its read is in, and what the read could not
//! do.
//!
//! The same shape as the Workspace's `Universe`: Reading is a state of its own
//! so a slow read never flashes an empty project, and a notice accumulates
//! beside whichever state is current and never replaces the map. A project
//! read again keeps its map on screen until the new one arrives.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use cellworks_code_analysis::CodeAnalysisEvent;
use cellworks_domain_knowledge::DomainKnowledgeEvent;
use cellworks_map::{BoundedContexts, Flight, Lanes, Lens, Network, ProjectMap, relens};
use cellworks_workspace::Project;
use cellworks_workspace::{Arrangement, WorkspaceEvent};

/// How far the read has got.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectState {
    /// From the moment the project is opened until its map arrives.
    Reading,
    /// The project's map, with at least one district; boxed, it being far
    /// larger than the other states.
    Mapped(Box<ProjectMap>),
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
    /// The map as read, by the folders lens and not turned: what every lens
    /// is made from.
    folders: Option<ProjectMap>,
    /// The lens the map is seen through; a map read again keeps it.
    lens: Lens,
    /// The towers flying to their places after the lens changed.
    flight: Option<Flight>,
    /// The lanes shown: the lens's own when it is switched to.
    lanes: Lanes,
    /// The read of the project's domain knowledge, and the bounded contexts
    /// it found; the cells lens needs them.
    knowledge: Option<Receiver<DomainKnowledgeEvent>>,
    contexts: Option<BoundedContexts>,
    /// The read of the project's custom arrangement, and the arrangement.
    arranged: Option<Receiver<WorkspaceEvent>>,
    arrangement: Arrangement,
    /// When the lens last changed, in seconds.
    switched_at: Option<f64>,
    /// The map's lanes, worked out once for the map as it is now.
    network: Option<Network>,
    /// The map of a new lens and its lanes, worked out off the frame loop
    /// while the towers lift off, and the turns it was worked out for.
    relensing: Option<(Receiver<(ProjectMap, Network)>, u8)>,
    /// The lanes and building features of the map as it is now, worked out
    /// off the frame loop, and the turns they were worked out for.
    preparing: Option<(Receiver<Network>, u8)>,
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
            folders: None,
            lens: Lens::Folders,
            flight: None,
            lanes: Lanes::default(),
            knowledge: None,
            contexts: None,
            arranged: None,
            arrangement: Arrangement::default(),
            switched_at: None,
            network: None,
            relensing: None,
            preparing: None,
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
                self.flight = None;
                if map.is_empty() {
                    self.folders = None;
                    self.state = ProjectState::Empty;
                } else {
                    let mut map = *map;
                    map.contexts = self.contexts.clone();
                    map.arrangement = self.arrangement.clone();
                    if !self.lens.offered(&map) {
                        self.lens = Lens::Folders;
                    }
                    self.state = ProjectState::Mapped(Box::new(self.seen(&map)));
                    self.network = None;
                    self.relensing = None;
                    self.preparing = None;
                    self.folders = Some(map);
                }
            }
            CodeAnalysisEvent::ProjectFolderSkipped { path, reason, .. } => {
                self.note(OpenedNotice::FolderSkipped { path, reason });
            }
            CodeAnalysisEvent::GitExcludesUnavailable { reason, .. } => {
                self.note(OpenedNotice::GitExcludesUnavailable { reason });
            }
        }
    }

    /// Whether the map of a new lens is still being worked out.
    pub fn is_relensing(&self) -> bool {
        self.relensing.is_some()
    }

    /// The lens the map is seen through.
    pub fn lens(&self) -> Lens {
        self.lens
    }

    /// When the lens last changed, in seconds; `None` before the first
    /// switch.
    pub fn switched_at(&self) -> Option<f64> {
        self.switched_at
    }

    /// Reads the project's domain knowledge over `knowledge`.
    pub fn know(&mut self, knowledge: Receiver<DomainKnowledgeEvent>) {
        self.knowledge = Some(knowledge);
    }

    /// Reads the project's custom arrangement over `arranged`.
    pub fn arrange(&mut self, arranged: Receiver<WorkspaceEvent>) {
        self.arranged = Some(arranged);
    }

    /// The custom arrangement, once there is a map.
    pub fn arrangement(&self) -> Option<Arrangement> {
        self.folders.as_ref().map(|_| self.arrangement.clone())
    }

    /// Moves the district or block at `path` by `by`, as seen on the map: in
    /// the custom lens it stays where it was put. Nothing in another lens.
    pub fn move_by(&mut self, path: &Path, by: egui::Vec2) {
        let Some(folders) = &mut self.folders else {
            return;
        };
        if self.lens != Lens::Custom {
            return;
        }
        // The arrangement is kept unturned: turn the move back.
        let mut by = by;
        for _ in 0..self.turns {
            by = egui::vec2(by.y, -by.x);
        }
        let relative = path
            .strip_prefix(&folders.root)
            .unwrap_or(path)
            .to_path_buf();
        self.arrangement.moved(relative, (by.x, by.y));
        folders.arrangement = self.arrangement.clone();
        let folders = folders.clone();
        self.state = ProjectState::Mapped(Box::new(self.seen(&folders)));
        self.network = None;
        self.relensing = None;
        self.preparing = None;
    }

    /// Whether the map can be seen through `lens`.
    pub fn offers(&self, lens: Lens) -> bool {
        self.folders
            .as_ref()
            .is_some_and(|folders| lens.offered(folders))
    }

    /// The lens after the current one that the map can be seen through.
    pub fn next_lens(&self) -> Lens {
        let mut lens = self.lens.next();
        while !self.offers(lens) && lens != Lens::Folders {
            lens = lens.next();
        }
        lens
    }

    /// The lanes shown.
    pub fn lanes(&self) -> Lanes {
        self.lanes
    }

    /// Shows or hides the import lanes.
    pub fn toggle_import_lanes(&mut self) {
        self.lanes.imports = !self.lanes.imports;
    }

    /// Shows or hides the call lanes.
    pub fn toggle_call_lanes(&mut self) {
        self.lanes.calls = !self.lanes.calls;
    }

    /// Sees the map through `lens`, its towers flying from where they stand
    /// at time `now` (seconds) to their new places. Nothing while there is
    /// no map. The new map is worked out off the frame loop; until it
    /// arrives the towers lift off where they stand.
    pub fn set_lens(&mut self, lens: Lens, now: f64) {
        let (Some(folders), ProjectState::Mapped(_)) = (&self.folders, &self.state) else {
            return;
        };
        if lens == self.lens {
            return;
        }
        let before = self.shown(now).map(|map| Flight::from(&map, now));
        self.switched_at = Some(now);
        self.lens = lens;
        self.lanes = lens.lanes();
        let (folders, turns) = (folders.clone(), self.turns);
        let (sender, relensed) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut map = relens(&folders, lens);
            map.turn(turns);
            let network = Network::of(&map);
            // The engineer may have switched again or closed the project.
            let _ = sender.send((map, network));
        });
        self.relensing = Some((relensed, turns));
        self.flight = before;
    }

    /// The map as it is drawn at time `now`: mid-flight while the towers fly
    /// to the places of a new lens. `None` while there is no map.
    ///
    /// Borrowed as it is, and copied only while towers fly.
    pub fn shown(&self, now: f64) -> Option<Cow<'_, ProjectMap>> {
        let ProjectState::Mapped(map) = &self.state else {
            return None;
        };
        match &self.flight {
            Some(flight) if !flight.landed(now) => {
                let mut map = (**map).clone();
                flight.apply(&mut map, now);
                Some(Cow::Owned(map))
            }
            _ => Some(Cow::Borrowed(map)),
        }
    }

    /// Works the map's lanes and what its buildings show out once per map,
    /// whether lanes are shown or not, off the frame loop: for the map the
    /// towers fly to, so they are ready when they land.
    pub fn prepare_network(&mut self) {
        if self.network.is_some() || self.preparing.is_some() {
            return;
        }
        if let ProjectState::Mapped(map) = &self.state {
            let map = (**map).clone();
            let (sender, prepared) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                // The map may have changed or the project closed meanwhile.
                let _ = sender.send(Network::of(&map));
            });
            self.preparing = Some((prepared, self.turns));
        }
    }

    /// Whether the lanes and building features of the map are still being
    /// worked out.
    pub fn is_preparing(&self) -> bool {
        self.preparing.is_some()
    }

    /// The map's lanes, if worked out: none while towers fly, whose lanes
    /// would run where they land.
    pub fn network(&self) -> Option<&Network> {
        self.network.as_ref().filter(|_| self.flight.is_none())
    }

    /// Whether towers are still flying at time `now`; ends the flight once
    /// all have landed.
    pub fn flying(&mut self, now: f64) -> bool {
        let Some(flight) = &self.flight else {
            return false;
        };
        let landed = flight.landed(now);
        if landed {
            self.flight = None;
        }
        !landed
    }

    /// `folders` seen through the current lens and turned as far as the map.
    fn seen(&self, folders: &ProjectMap) -> ProjectMap {
        let mut map = relens(folders, self.lens);
        map.turn(self.turns);
        map
    }

    /// Turns the map `quarters` quarters clockwise, and returns the centre it
    /// turned about. `None` while there is no map to turn.
    pub fn turn(&mut self, quarters: u8) -> Option<egui::Pos2> {
        let ProjectState::Mapped(map) = &mut self.state else {
            return None;
        };
        self.turns = (self.turns + quarters) % 4;
        self.flight = None;
        let centre = map.ground().center();
        map.turn(quarters);
        self.network = None;
        self.preparing = None;
        Some(centre)
    }

    /// Takes everything the read has sent so far. Never waits. A read that
    /// goes away before its map arrives has failed; after it, it is just over.
    /// A read again that goes away without a map leaves the old one on screen.
    /// Starts working out the lanes and building features of a map without.
    pub fn drain(&mut self) {
        if let Some((map, network)) = self
            .relensing
            .as_ref()
            .and_then(|(relensed, _)| relensed.try_recv().ok())
            && let Some((_, turns)) = self.relensing.take()
            && let ProjectState::Mapped(_) = self.state
        {
            let mut map = map;
            // Turned while it was worked out: turn it as far.
            let missed = (self.turns + 4 - turns) % 4;
            if missed > 0 {
                map.turn(missed);
                self.network = None;
            } else {
                self.network = Some(network);
            }
            self.preparing = None;
            self.state = ProjectState::Mapped(Box::new(map));
        }
        if let Some((prepared, turns)) = &self.preparing {
            match prepared.try_recv() {
                Ok(network) => {
                    // Turned while it was worked out: work it out again.
                    if *turns == self.turns {
                        self.network = Some(network);
                    }
                    self.preparing = None;
                }
                Err(TryRecvError::Disconnected) => self.preparing = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(WorkspaceEvent::ArrangementRead { arrangement, .. }) = self
            .arranged
            .as_ref()
            .and_then(|arranged| arranged.try_recv().ok())
        {
            self.arranged = None;
            self.arrangement = arrangement;
            if let Some(folders) = &mut self.folders {
                folders.arrangement = self.arrangement.clone();
                if self.lens == Lens::Custom {
                    let folders = folders.clone();
                    self.state = ProjectState::Mapped(Box::new(self.seen(&folders)));
                    self.network = None;
                    self.relensing = None;
                    self.preparing = None;
                }
            }
        }
        let known = self
            .knowledge
            .as_ref()
            .and_then(|knowledge| knowledge.try_recv().ok());
        if let Some(DomainKnowledgeEvent::ContextsRead { contexts, .. }) = known {
            self.knowledge = None;
            self.contexts = contexts;
            if let Some(folders) = &mut self.folders {
                folders.contexts = self.contexts.clone();
            }
        }
        loop {
            match self.events.try_recv() {
                Ok(event) => self.apply(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.rereading = false;
                    if self.state == ProjectState::Reading {
                        self.state = ProjectState::Failed;
                    }
                    break;
                }
            }
        }
        self.prepare_network();
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
            map: Box::new(map(files)),
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
    fn a_move_on_a_turned_map_is_kept_as_on_the_map_unturned() {
        let (mut project, _sender) = opened();
        project.apply(mapped(&["catalog/a.rs", "orders/b.rs"]));
        project.set_lens(Lens::Custom, 0.0);
        project.turn(1);

        // A quarter turn clockwise shows ground y as -x: a move to screen
        // east is a move along ground -y, unturned.
        project.move_by(Path::new("/p/shop/catalog/a.rs"), egui::vec2(10.0, 0.0));

        let moves = project.arrangement().expect("a map").moves;
        assert_eq!(moves.get(Path::new("catalog/a.rs")), Some(&(0.0, -10.0)));
    }

    #[test]
    fn draining_a_silent_read_returns_rather_than_waiting() {
        let (mut project, _sender) = opened();

        project.drain();

        assert_eq!(project.state(), &ProjectState::Reading);
    }
}
