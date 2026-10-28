//! The window the binary owns: the universe of the Workspace context or the
//! project opened from it, and the options the window opens with.
//!
//! `draw` is kept apart from [`CellworksApp`] so a test can run a frame
//! through an [`egui::Ui`] without eframe, winit or a window, and
//! `native_options` is kept pure so the title and the renderer can be asserted
//! the same way.
//!
//! This is the first story that needs a background thread, so it is where
//! STORY-001's deferred conventions land: the scan runs on a `std::thread`,
//! its results arrive as `WorkspaceEvent` values over a `std::sync::mpsc`
//! channel, and the UI thread drains them once per frame with `try_recv` and
//! never waits. A project opened from the universe is read the same way, with
//! `CodeAnalysisEvent` values.

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cellworks_agent_sessions::ui::{DrawnSession, SessionsView};
use cellworks_agent_sessions::{Session, SessionSources};
use cellworks_code_analysis::{CodeAnalysisEvent, GitCli, GitListing, start_details, start_load};
use cellworks_map::View;
use cellworks_map::ui::{Drawn, DrawnBridge, DrawnBuilding, DrawnLane, DrawnTower, Outline};
use cellworks_map::{Lens, ProjectMap, Why, default_prefix_rule, prefix_rule};
use cellworks_navigation::{Moved, Navigation};
use cellworks_workspace::ui::LauncherAction;
use cellworks_workspace::{
    ArrangementKeeper, ArrangementStore, HostProbe, Launcher, Project, ProjectDetails, ProjectId,
    RecentKeeper, RecentStore, ScanState, Sources, SshProbe, Universe, WorkspaceEvent,
    start_probes, start_scan,
};

use crate::opened::{OpenedNotice, OpenedProject, ProjectState};

/// Shown from the moment a project is opened until its map arrives.
const READING: &str = "Reading the project…";

/// Shown when a project was read and has nothing to draw.
const EMPTY: &str = "This project has no files to show.";

/// Shown when the read ended without a map.
const FAILED: &str = "Could not read this project.";

/// Beside the breadcrumb: whether zooming moves the layer.
const FOLLOWS_ZOOM: &str = "Layer follows the zoom (Z to lock)";
const LOCKED: &str = "Layer locked (Z to follow the zoom)";

/// Screen points one press of an arrow moves the view by.
const PAN: f32 = 120.0;

/// Points of scrolling that zoom in by a factor of e.
const SCROLL_PER_E: f32 = 200.0;

/// The fixed width of the why panel, the right column beside the city
/// (STORY-019).
const WHY_WIDTH: f32 = 420.0;

/// The Cellworks application: the universe it shows, the scan that fills it
/// in, the project opened from it, if any, and where the view is.
pub struct CellworksApp {
    universe: Universe,
    events: Option<Receiver<WorkspaceEvent>>,
    pub(crate) opened: Option<OpenedProject>,
    navigation: Navigation,
    git: Arc<dyn GitListing + Send + Sync>,
    /// Whether git-ignored files and hidden folders are on the map. Off at
    /// every start, kept for every project opened in this session.
    show_ignored: bool,
    /// Whether the tree is open beside the map.
    tree_open: bool,
    /// The towers the last frame drew.
    towers: Vec<DrawnTower>,
    /// The buildings the last frame drew.
    buildings: Vec<DrawnBuilding>,
    /// The skybridges the last frame drew.
    bridges: Vec<DrawnBridge>,
    /// The project lenses and the Claude session that shapes them.
    pub(crate) lens_panel: crate::lens_panel::LensPanel,
    /// The cables the last frame drew, from base to base.
    cables: Vec<(std::path::PathBuf, std::path::PathBuf)>,
    /// The skybridge the pointer was over in the last frame.
    bridge_hovered: Option<cellworks_map::Bridge>,
    /// Where the last frame saw the map from, and the space it drew it in;
    /// the why panel takes its own column out of the window (STORY-019).
    view: Option<View>,
    panel: Option<egui::Rect>,
    /// Whether this frame's view is underground: turned so, or Shift held.
    underground: bool,
    /// The districts the last frame drew, with their outlines.
    districts: Vec<(std::path::PathBuf, Outline)>,
    /// The folder outlines the last frame drew, and whether each was named.
    folders: Vec<(std::path::PathBuf, bool)>,
    /// The blocks, folders and districts the last frame drew by their
    /// outlines.
    outlines: Vec<std::path::PathBuf>,
    /// The lanes the last frame drew, and the one under the pointer.
    lanes: Vec<DrawnLane>,
    lane_hovered: Option<cellworks_map::LaneId>,
    /// Where custom arrangements are kept, and the thread keeping them;
    /// none, none are kept.
    arrangements: Option<(ArrangementStore, ArrangementKeeper)>,
    /// Why the selected tower was built: the path it was worked out for and
    /// the explanation, `None` where the selection is no tower. Worked out
    /// on the UI thread only when the selected path changes (STORY-019).
    why: Option<(std::path::PathBuf, Option<Why>)>,
    /// The launcher's own view model: search, pick, selection, host states and
    /// project details, read off the frame loop.
    launcher: Launcher,
    /// How a remote host's reachability is decided (STORY-078).
    probe: Arc<dyn HostProbe + Send + Sync>,
    /// PROTOTYPE (stakeholder demo): the guided tour, in demo mode only.
    pub(crate) demo: Option<crate::demo::Tour>,
    /// Where the host probes report, once started.
    probed: Option<Receiver<WorkspaceEvent>>,
    /// Whether a host probe is still running.
    probing: bool,
    /// The reads of a selected project's branch and file count still
    /// running, by project; one per project, started when the engineer
    /// selects it.
    project_details: Vec<(std::path::PathBuf, Receiver<CodeAnalysisEvent>)>,
    /// Whether the host probes may start on their own once the scan
    /// reports, and a selected project's details be read. False from
    /// [`CellworksApp::over`], so a test that never asks for this story's git
    /// or ssh behaviour can never trigger either by accident;
    /// [`CellworksApp::start`] and the `with_probe`/`with_git` builders turn
    /// it on, matching the design's "`over()` starts neither probes nor
    /// details".
    auto_detail: bool,
    /// Where recently opened projects are kept, and the thread keeping them;
    /// none, none are kept.
    recent: Option<(RecentStore, RecentKeeper)>,
    /// Every running Claude Code session on the machine, and the sidebar and
    /// overlay's own view model (STORY-030).
    sessions: SessionsView,
    /// The sessions the last frame drew over the city.
    drawn_sessions: Vec<DrawnSession>,
    /// The open project's own git common dir, cached by its root so it is
    /// read from disk once per project, not every frame.
    common_dir_cache: Option<(std::path::PathBuf, Option<std::path::PathBuf>)>,
    /// PROTOTYPE: the iterate loop running from this window, if any.
    pub(crate) iterate: Option<crate::iterate::Iterate>,
    /// PROTOTYPE: whether the iterate session's pane was opened once.
    iterate_pane: bool,
    /// PROTOTYPE: what stages 4 and 5 keep between frames.
    iterate_view: crate::iterate_view::ViewState,
    /// PROTOTYPE: the open case, which takes over the window.
    pub(crate) case: Option<cellworks_map::ui::case::Case>,
    /// PROTOTYPE: the ticket menu (T), from which a case opens.
    tickets_open: bool,
    /// PROTOTYPE: Jira, read-only, with the issues fetched this session.
    jira: cellworks_map::ui::jira::Jira,
    /// PROTOTYPE: the key typed into the ticket picker.
    typed_key: String,
}

/// What the keyboard and the pointer asked for in one frame.
struct Keys {
    enter: bool,
    esc: bool,
    ignored: bool,
    lock: bool,
    tree: bool,
    /// Arrows down less arrows up, and right less left.
    rows: isize,
    columns: isize,
    /// Quarters to turn the map clockwise: Q one, Shift+Q three.
    turn: u8,
    fit: bool,
    /// Tab: the next tower on the block.
    cycle: bool,
    /// U turns the view underground or back; Shift shows it while held.
    underground: bool,
    held_underground: bool,
    /// M shows or hides the cables from the masts.
    cables: bool,
    /// Inside a tower: j less k, a floor down or up.
    lines: isize,
    /// L: the next lens.
    lens: bool,
    /// G and A: show or hide the import lanes and the call lanes.
    import_lanes: bool,
    call_lanes: bool,
    zoom: f32,
    pointer: Option<egui::Pos2>,
    /// `[` / `]`: step to the previous / next session in this city.
    session_prev: bool,
    session_next: bool,
}

impl CellworksApp {
    /// Starts the scan of this machine and hands back the app that shows it.
    ///
    /// With no `$HOME` there is nowhere to scan: no thread starts, and the
    /// universe is reported empty rather than left scanning for ever.
    pub fn start() -> CellworksApp {
        match crate::demo::sources().or_else(Sources::for_this_user) {
            Some(sources) => CellworksApp {
                arrangements: Some(kept(ArrangementStore::beside(&sources.settings_file))),
                launcher: Launcher::new(
                    sources
                        .projects_folder
                        .parent()
                        .map(std::path::Path::to_path_buf),
                ),
                probe: crate::demo::probe().unwrap_or_else(|| Arc::new(SshProbe)),
                probed: None,
                probing: false,
                project_details: Vec::new(),
                auto_detail: true,
                recent: Some(kept_recent(RecentStore::beside(&sources.settings_file))),
                sessions: match crate::demo::session_sources()
                    .or_else(SessionSources::for_this_user)
                {
                    Some(session_sources) => SessionsView::watching(session_sources),
                    None => SessionsView::none(),
                },
                drawn_sessions: Vec::new(),
                common_dir_cache: None,
                iterate: None,
                demo: crate::demo::tour(),
                iterate_pane: false,
                iterate_view: Default::default(),
                case: None,
                tickets_open: false,
                jira: cellworks_map::ui::jira::Jira::new(),
                typed_key: String::new(),
                universe: Universe::scanning(&sources),
                events: Some(start_scan(sources)),
                opened: None,
                navigation: Navigation::default(),
                git: Arc::new(GitCli),
                show_ignored: false,
                tree_open: false,
                towers: Vec::new(),
                buildings: Vec::new(),
                bridges: Vec::new(),
                lens_panel: Default::default(),
                cables: Vec::new(),
                bridge_hovered: None,
                view: None,
                panel: None,
                underground: false,
                districts: Vec::new(),
                folders: Vec::new(),
                outlines: Vec::new(),
                lanes: Vec::new(),
                lane_hovered: None,
                why: None,
            },
            None => CellworksApp {
                arrangements: None,
                launcher: Launcher::new(None),
                probe: Arc::new(SshProbe),
                probed: None,
                probing: false,
                project_details: Vec::new(),
                auto_detail: true,
                recent: None,
                sessions: match crate::demo::session_sources()
                    .or_else(SessionSources::for_this_user)
                {
                    Some(session_sources) => SessionsView::watching(session_sources),
                    None => SessionsView::none(),
                },
                drawn_sessions: Vec::new(),
                common_dir_cache: None,
                iterate: None,
                demo: crate::demo::tour(),
                iterate_pane: false,
                iterate_view: Default::default(),
                case: None,
                tickets_open: false,
                jira: cellworks_map::ui::jira::Jira::new(),
                typed_key: String::new(),
                universe: Universe::without_home(),
                events: None,
                opened: None,
                navigation: Navigation::default(),
                git: Arc::new(GitCli),
                show_ignored: false,
                tree_open: false,
                towers: Vec::new(),
                buildings: Vec::new(),
                bridges: Vec::new(),
                lens_panel: Default::default(),
                cables: Vec::new(),
                bridge_hovered: None,
                view: None,
                panel: None,
                underground: false,
                districts: Vec::new(),
                folders: Vec::new(),
                outlines: Vec::new(),
                lanes: Vec::new(),
                lane_hovered: None,
                why: None,
            },
        }
    }

    /// The app over a channel the caller owns, so a test can drive the frame
    /// loop without a scan thread.
    pub fn over(universe: Universe, events: Receiver<WorkspaceEvent>) -> CellworksApp {
        CellworksApp {
            lens_panel: Default::default(),
            arrangements: None,
            launcher: Launcher::new(None),
            demo: None,
            probe: Arc::new(SshProbe),
            probed: None,
            probing: false,
            project_details: Vec::new(),
            auto_detail: false,
            recent: None,
            sessions: SessionsView::none(),
            drawn_sessions: Vec::new(),
            common_dir_cache: None,
            iterate: None,
            iterate_pane: false,
            iterate_view: Default::default(),
            case: None,
            tickets_open: false,
            jira: cellworks_map::ui::jira::Jira::new(),
            typed_key: String::new(),
            universe,
            events: Some(events),
            opened: None,
            navigation: Navigation::default(),
            git: Arc::new(GitCli),
            show_ignored: false,
            tree_open: false,
            towers: Vec::new(),
            buildings: Vec::new(),
            bridges: Vec::new(),
            cables: Vec::new(),
            bridge_hovered: None,
            view: None,
            panel: None,
            underground: false,
            districts: Vec::new(),
            folders: Vec::new(),
            outlines: Vec::new(),
            lanes: Vec::new(),
            lane_hovered: None,
            why: None,
        }
    }

    /// The same app asking `git` about repositories instead of the real git,
    /// so a test never runs it. Also turns on the host probes and the
    /// selected project's details (`auto_detail`) — a test that calls this without also
    /// calling `with_probe`, and whose fixture names a remote host, still
    /// reaches the real `ssh`; call `with_probe` too whenever the fixture has
    /// one.
    pub fn with_git(self, git: Arc<dyn GitListing + Send + Sync>) -> CellworksApp {
        CellworksApp {
            git,
            auto_detail: true,
            ..self
        }
    }

    /// The same app, keeping custom arrangements in `store`.
    pub fn with_arrangements(self, store: ArrangementStore) -> CellworksApp {
        CellworksApp {
            arrangements: Some(kept(store)),
            ..self
        }
    }

    /// The same app, deciding a remote host's reachability with `probe`
    /// instead of the real `ssh`, so a test never runs it. Also turns on
    /// `auto_detail`, as `with_git` does.
    pub fn with_probe(self, probe: Arc<dyn HostProbe + Send + Sync>) -> CellworksApp {
        CellworksApp {
            probe,
            auto_detail: true,
            ..self
        }
    }

    /// The same app, with its launcher showing `home` as `~`.
    pub fn with_home(self, home: std::path::PathBuf) -> CellworksApp {
        CellworksApp {
            launcher: Launcher::new(Some(home)),
            ..self
        }
    }

    /// The same app, keeping recently opened projects in `store`.
    pub fn with_recent(self, store: RecentStore) -> CellworksApp {
        CellworksApp {
            recent: Some(kept_recent(store)),
            ..self
        }
    }

    /// The same app, showing `sessions` instead of watching the machine's
    /// real ones. No background watch is ever started — a test seam.
    pub fn with_sessions(self, sessions: Vec<Session>) -> CellworksApp {
        CellworksApp {
            sessions: SessionsView::with_sessions(sessions),
            ..self
        }
    }

    /// The same app, showing `sessions` and pointing a session pane at
    /// `claude` and `transcripts` - a test seam; nothing is watched.
    pub fn with_pane_sources(
        self,
        sessions: Vec<Session>,
        claude: std::path::PathBuf,
        transcripts: std::path::PathBuf,
    ) -> CellworksApp {
        CellworksApp {
            sessions: SessionsView::with_sessions_and_sources(sessions, claude, transcripts),
            ..self
        }
    }

    /// Whether a session pane is open.
    pub fn pane_open(&self) -> bool {
        self.sessions.pane_open()
    }

    /// The open pane's text, for tests; `None` when no pane is open.
    pub fn pane_text(&self) -> Option<String> {
        self.sessions.pane_text()
    }

    /// Reads the branch and file count of every local project the launcher
    /// lists, once each, so each row can show its branch. Git runs only in
    /// projects listed under the current pick and search.
    // ponytail: one thread per listed project; read only the rows in view if
    // a discovery ever lists hundreds of repositories.
    fn request_details(&mut self) {
        if !self.auto_detail {
            return;
        }
        let listed: Vec<std::path::PathBuf> = self
            .launcher
            .groups()
            .into_iter()
            .flat_map(|(_, projects)| projects)
            .filter(|project| self.launcher.details_of(&project.id).is_none())
            .filter_map(|project| project.id.local_path())
            .filter(|path| !self.project_details.iter().any(|(read, _)| read == path))
            .map(std::path::Path::to_path_buf)
            .collect();
        for path in listed {
            let receiver = start_details(vec![path.clone()], Arc::clone(&self.git));
            self.project_details.push((path, receiver));
        }
    }

    /// Takes whatever the scan, the host probes and the project details have
    /// sent so far. Never waits.
    pub fn apply_pending(&mut self) {
        self.sessions.drain();
        if let Some(it) = &mut self.iterate {
            it.drain();
            // The session walks in: open its pane the first time it shows.
            if !self.iterate_pane
                && let Some(session) = self
                    .sessions
                    .sessions()
                    .iter()
                    .find(|s| {
                        s.name == it.session_name
                            && (crate::demo::is_demo() || s.attach_id.is_some())
                    })
                    .cloned()
            {
                self.sessions.open_pane(&session);
                self.iterate_pane = true;
            }
            for bytes in it.due() {
                self.sessions.send_to_pane(&bytes);
            }
        }
        if let Some(events) = &self.events {
            for event in self.universe.drain(events) {
                self.launcher.apply(event);
            }
        }
        let starting = match self.universe.scan() {
            ScanState::Reported(projects) if self.auto_detail && self.probed.is_none() => {
                let hosts: Vec<String> = projects
                    .iter()
                    .filter_map(|project| project.id.host())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                Some(hosts)
            }
            _ => None,
        };
        if let Some(hosts) = starting {
            self.probed = Some(start_probes(hosts, Arc::clone(&self.probe)));
            self.probing = true;
        }
        if let Some(probed) = &self.probed {
            loop {
                match probed.try_recv() {
                    Ok(event) => self.launcher.apply(event),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.probing = false;
                        break;
                    }
                }
            }
        }
        if self.opened.is_none() {
            self.request_details();
        }
        let launcher = &mut self.launcher;
        self.project_details.retain(|(_, details)| {
            loop {
                match details.try_recv() {
                    Ok(CodeAnalysisEvent::DetailsRead {
                        project,
                        branch,
                        files,
                        committed,
                        changed,
                    }) => launcher.detailed(
                        project,
                        ProjectDetails {
                            branch,
                            files,
                            committed,
                            changed,
                        },
                    ),
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => return true,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return false,
                }
            }
        });
        if let Some(opened) = &mut self.opened {
            opened.drain();
            // A new map, or the domain knowledge: what a tower is explained
            // from has moved, so the explanation is worked out again.
            if opened.take_learned() {
                self.why = None;
            }
            match opened.state() {
                ProjectState::Mapped(map) => self.navigation.remapped(map),
                // Nothing left to frame or select: the project, whole.
                ProjectState::Empty => self.navigation.opened(
                    opened
                        .project()
                        .id
                        .local_path()
                        .expect("apply_pending is only reached by a local project")
                        .to_owned(),
                ),
                ProjectState::Reading | ProjectState::Failed => {}
            }
        }
    }

    /// The project that is open, if any.
    pub fn opened(&self) -> Option<&OpenedProject> {
        self.opened.as_ref()
    }

    /// Whether the tree is open beside the map.
    pub fn tree_open(&self) -> bool {
        self.tree_open
    }

    /// The towers the last frame drew, back to front.
    pub fn towers(&self) -> &[DrawnTower] {
        &self.towers
    }

    /// The buildings the last frame drew, by their bases, back to front.
    pub fn buildings(&self) -> &[DrawnBuilding] {
        &self.buildings
    }

    /// The skybridges the last frame drew.
    pub fn bridges(&self) -> &[DrawnBridge] {
        &self.bridges
    }

    /// The cables the last frame drew: from the base of the building whose
    /// mast it leaves to the base of the building it calls.
    pub fn cables(&self) -> &[(std::path::PathBuf, std::path::PathBuf)] {
        &self.cables
    }

    /// The skybridge the pointer was over in the last frame, if any.
    pub fn bridge_hovered(&self) -> Option<&cellworks_map::Bridge> {
        self.bridge_hovered.as_ref()
    }

    /// The lanes the last frame drew.
    pub fn lanes(&self) -> &[DrawnLane] {
        &self.lanes
    }

    /// The lane under the pointer in the last frame.
    pub fn hovered_lane(&self) -> Option<&cellworks_map::LaneId> {
        self.lane_hovered.as_ref()
    }

    /// Every running session on the machine, as the sidebar lists them.
    pub fn sessions(&self) -> &[Session] {
        self.sessions.sessions()
    }

    /// The sessions the last frame drew over the city.
    pub fn sessions_drawn(&self) -> &[DrawnSession] {
        &self.drawn_sessions
    }

    /// The districts the last frame drew, with the outline of each.
    pub fn districts(&self) -> &[(std::path::PathBuf, Outline)] {
        &self.districts
    }

    /// The folders the last frame drew by their outlines, and whether the
    /// name of each was written on it.
    pub fn folders(&self) -> &[(std::path::PathBuf, bool)] {
        &self.folders
    }

    /// The blocks, folders and districts the last frame drew by their
    /// outlines, each with its outline on the ground.
    ///
    /// The outlines are the network's, worked out once per map; they are
    /// copied out only when asked, not every frame.
    pub fn outlines(&self) -> Vec<(std::path::PathBuf, Vec<egui::Pos2>)> {
        let Some(outlines) = self
            .opened
            .as_ref()
            .and_then(|opened| opened.network())
            .map(|network| &network.buildings.outlines)
        else {
            return Vec::new();
        };
        self.outlines
            .iter()
            .filter_map(|path| Some((path.clone(), outlines.get(path)?.clone())))
            .collect()
    }

    /// Where the last frame saw the open project's map from.
    pub fn view(&self) -> Option<View> {
        self.view
    }

    /// The space the last frame drew the map in: the window less the
    /// breadcrumb, the tree and the why panel (STORY-019).
    pub fn panel(&self) -> Option<egui::Rect> {
        self.panel
    }

    /// The layer the view is on, what is framed and what is selected.
    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    /// Why the selected tower was built, once the selection is one.
    pub fn why(&self) -> Option<&Why> {
        self.why.as_ref().and_then(|(_, why)| why.as_ref())
    }

    /// Works out why the selected tower was built, unless it was already
    /// worked out for this path: the join walks every floor of the tower, so
    /// it runs when the selection moves and not every frame.
    fn explain_selection(&mut self) {
        let Some(selected) = self
            .navigation
            .selection()
            .map(std::path::Path::to_path_buf)
        else {
            self.why = None;
            return;
        };
        if self.why.as_ref().is_some_and(|(at, _)| *at == selected) {
            return;
        }
        let why = mapped(&self.opened).and_then(|map| cellworks_map::why_of(map, &selected));
        self.why = Some((selected, why));
    }

    /// Moves straight to the element of the open project at `path`, showing
    /// the folder holding it in its folder view.
    pub fn jump(&mut self, path: std::path::PathBuf) {
        if let Some(map) = mapped(&self.opened) {
            self.navigation.jump(map, path);
        }
    }

    /// The opened project's map, once it has arrived.
    fn map(&self) -> Option<&ProjectMap> {
        mapped(&self.opened)
    }

    /// The open project's own git common dir, read once per project (git's
    /// files are cheap to read but there is no reason to do it every frame)
    /// and cached by its root.
    fn city_common_dir(&mut self) -> Option<std::path::PathBuf> {
        let root = self.map()?.root.clone();
        let stale = match &self.common_dir_cache {
            Some((cached_root, _)) => *cached_root != root,
            None => true,
        };
        if stale {
            self.common_dir_cache =
                Some((root.clone(), cellworks_agent_sessions::common_dir(&root)));
        }
        self.common_dir_cache
            .as_ref()
            .and_then(|(_, common)| common.clone())
    }

    /// The universe's project whose folder is `path`.
    fn project_at(&self, path: &std::path::Path) -> Option<Project> {
        match self.universe.scan() {
            ScanState::Reported(projects) => projects
                .iter()
                .find(|project| project.id.local_path() == Some(path))
                .cloned(),
            _ => None,
        }
    }

    /// PROTOTYPE: the folder holding every project Cellworks knows, where an
    /// investigation starts.
    fn workspace(&self) -> Option<std::path::PathBuf> {
        let ScanState::Reported(projects) = self.universe.scan() else {
            return None;
        };
        let mut paths = projects
            .iter()
            .filter_map(|project| project.id.local_path());
        let mut common = paths.next()?.parent()?.to_path_buf();
        for path in paths {
            while !path.starts_with(&common) {
                common = common.parent()?.to_path_buf();
            }
        }
        Some(common)
    }

    /// PROTOTYPE: opens the case for the ticket `key`.
    pub fn open_case(&mut self, key: &str, now: f64) {
        let mut case = cellworks_map::ui::case::Case::new(key, now);
        case.workspace = self.workspace();
        self.case = Some(case);
        self.tickets_open = false;
    }

    /// Opens `project` and starts reading it.
    /// Prototype: the lens panel, for a headless render.
    pub fn lens_panel_mut(&mut self) -> &mut crate::lens_panel::LensPanel {
        &mut self.lens_panel
    }

    pub fn open(&mut self, project: Project) {
        let path = project
            .id
            .local_path()
            .expect("open is only called with a local project")
            .to_owned();
        let events = self.read(&project);
        let knowledge =
            cellworks_domain_knowledge::start_read(path.clone(), project.contexts.clone());
        // Read off the frame loop, like the code.
        let arranged = self
            .arrangements
            .as_ref()
            .map(|(store, _)| store.start_read(project.id.clone()));
        self.launcher.opened(project.id.clone());
        if let Some((_, keeper)) = &self.recent {
            let ids = self
                .launcher
                .recent_row()
                .into_iter()
                .map(|project| project.id.clone())
                .collect();
            keeper.keep(ids);
        }
        let mut opened = OpenedProject::reading(project, events);
        opened.know(knowledge);
        if let Some(arranged) = arranged {
            opened.arrange(arranged);
        }
        note_unknown_prefix(&mut opened);
        self.opened = Some(opened);
        self.navigation.opened(path);
        // Another project's towers: nothing explained carries over.
        self.why = None;
    }

    /// Switches ignored files on or off and reads the open project again,
    /// leaving the view where it is.
    fn toggle_ignored(&mut self) {
        let Some(project) = self.opened.as_ref().map(|opened| opened.project().clone()) else {
            return;
        };
        self.show_ignored = !self.show_ignored;
        let events = self.read(&project);
        if let Some(opened) = &mut self.opened {
            opened.reread(events);
            note_unknown_prefix(opened);
        }
    }

    /// PROTOTYPE (stakeholder demo): reads the open project again, leaving
    /// the view where it is.
    pub(crate) fn rescan(&mut self) {
        let Some(project) = self.opened.as_ref().map(|opened| opened.project().clone()) else {
            return;
        };
        let events = self.read(&project);
        if let Some(opened) = &mut self.opened {
            opened.reread(events);
        }
    }

    /// Starts reading `project` with the district prefix rule its settings
    /// name, or the default when the name is unknown.
    fn read(&self, project: &Project) -> Receiver<CodeAnalysisEvent> {
        let rule = project.district_prefix.as_deref().and_then(prefix_rule);
        start_load(
            project
                .id
                .local_path()
                .expect("read is only called with a local project")
                .to_owned(),
            project.name.clone(),
            rule.unwrap_or_else(default_prefix_rule),
            Arc::clone(&self.git),
            self.show_ignored,
        )
    }

    /// The universe as it currently stands.
    pub fn universe(&self) -> &Universe {
        &self.universe
    }
}

impl eframe::App for CellworksApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.apply_pending();
        // Prototype: open a project and a lens straight away.
        if self.opened.is_none()
            && !self.lens_panel.autostarted
            && let Ok(path) = std::env::var("CELLWORKS_OPEN")
            && let Ok(path) = std::path::Path::new(&path).canonicalize()
        {
            self.lens_panel.autostarted = true;
            self.lens_panel.open = true;
            self.lens_panel.preset(std::env::var("CELLWORKS_LENS").ok());
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.open(Project {
                id: cellworks_workspace::ProjectId::Local(path),
                name,
                district_prefix: None,
                contexts: Default::default(),
            });
        }
        draw(ui, self);

        // egui draws on demand, and a scan or read finishing is not an input
        // event. Asking for the next frame while one is still running is what
        // gets its result onto the screen without anyone touching the mouse.
        // Asked after `draw`, because the frame that opens a project is the
        // one `draw` opens it in.
        let reading = self.opened.as_ref().is_some_and(OpenedProject::is_reading);
        if self.universe.scan() == &ScanState::Scanning
            || reading
            || self.probing
            || !self.project_details.is_empty()
        {
            ui.ctx().request_repaint();
        }
        if self.iterate.is_some() || self.demo.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

/// Paints one frame: the universe, or the opened project under its
/// breadcrumb. A click selects a project or an element of the map, a
/// double-click opens a project; Enter moves one layer in and Esc one layer
/// out; I switches ignored files on and off while a project is open. Pinching
/// or scrolling zooms the map around the pointer, and Z, or the button beside
/// the breadcrumb, locks the layer against the zoom or lets it follow again.
/// E opens and closes the tree, where the up and down arrows move the
/// selection one row. In the city, Q turns it a quarter (Shift+Q back), F fits
/// it into view, the arrows or a drag pan, Tab moves to the next tower on the
/// block, and Enter or a double-click on a tower steps inside it, where j and
/// k or the arrows move between floors.
pub fn draw(ui: &mut egui::Ui, app: &mut CellworksApp) {
    // PROTOTYPE: an open case lies over the app. From stage 2 on it wants the
    // map of the repository it is shown on, and the camera gliding onto the
    // case's buildings.
    if let Some(base) = app
        .case
        .as_ref()
        .and_then(cellworks_map::ui::case::Case::wants)
        && base.is_dir()
    {
        let base = base.canonicalize().unwrap_or(base);
        if app
            .opened
            .as_ref()
            .is_none_or(|opened| opened.project().id.local_path() != Some(base.as_path()))
        {
            let project = app.project_at(&base).unwrap_or_else(|| Project::at(base));
            app.open(project);
        }
    }
    let case_now = ui.input(|input| input.time);
    if let Some(case) = &mut app.case
        && let Some(map) = mapped(&app.opened)
        && let Some(area) = case.camera(
            map,
            app.view.map(|view| view.visible),
            ui.ctx().content_rect(),
            case_now,
        )
    {
        app.navigation.look_at(area);
        ui.ctx().request_repaint();
    }
    // PROTOTYPE: the case's PROTOTYPE act hands over its worktree, branch
    // and prompt; the iterate loop starts there and the case moves to stage 4.
    if let Some(case) = &mut app.case {
        if let Some(p) = case.prototype.take() {
            match crate::iterate::Iterate::start_at(
                &p.root,
                &p.worktree,
                &p.branch,
                &p.base,
                &p.name,
                &p.prompt,
            ) {
                Ok(it) => {
                    app.iterate = Some(it);
                    app.iterate_pane = false;
                    app.iterate_view = Default::default();
                }
                Err(e) => eprintln!("prototype failed: {e}"),
            }
            case.iterating = app.iterate.is_some();
            case.go(4, case_now);
        }
        case.iterating = app.iterate.is_some();
        if let (Some(line), Some(it)) = (case.typed.take(), app.iterate.as_mut()) {
            it.type_line(line);
        }
    }
    let spots = app
        .case
        .as_ref()
        .zip(mapped(&app.opened))
        .and_then(|(case, map)| case.spots(map));
    let casing = app.case.is_some();
    if app.opened.is_none() {
        app.towers.clear();
        app.buildings.clear();
        app.bridges.clear();
        app.cables.clear();
        app.sessions.close_pane();
    }
    // A focused session terminal owns the keyboard: the map's keys do nothing.
    let terminal = app.sessions.has_keyboard(ui)
        || ui.memory(|m| m.has_focus(egui::Id::new("iterate-note")))
        || (app.case.is_none() && app.iterate.as_ref().is_some_and(|it| it.draft.is_some()));
    // So does the launcher, whole: in CMD the search field takes the letters,
    // and in NAV the launcher reads them itself (j, k, g, G, ":", "/"), so
    // none of them may also toggle underground, the cables or the navigation
    // lock, which act whether a project is open or not (STORY-083
    // requirement 8). The keys are read here, before `ui::launcher` draws the
    // field, so the field never gets to swallow them itself.
    let launcher_shown = app.opened.is_none();
    let sessions_search = app.sessions.search_focused(ui);
    let in_field = launcher_shown && cellworks_workspace::ui::search_focused(ui);
    let was_in_field = launcher_shown && cellworks_workspace::ui::search_was_focused(ui);
    let typing = sessions_search
        || ui.ctx().egui_wants_keyboard_input()
        || launcher_shown || terminal;
    let Keys {
        enter,
        esc,
        ignored,
        lock,
        tree,
        rows,
        columns,
        turn,
        fit,
        cycle,
        lines,
        underground,
        held_underground,
        cables,
        lens,
        import_lanes,
        call_lanes,
        zoom,
        pointer,
        session_prev,
        session_next,
    } = ui.input(|input| {
        let closes_pane =
            !terminal && app.sessions.pane_open() && input.key_pressed(egui::Key::Escape);
        let plain = input.modifiers.is_none();
        let pressed = |key| !terminal && input.key_pressed(key);
        let towards = |less, more| isize::from(pressed(more)) - isize::from(pressed(less));
        Keys {
            enter: pressed(egui::Key::Enter),
            esc: pressed(egui::Key::Escape) && !closes_pane,
            ignored: !typing && plain && pressed(egui::Key::I),
            lock: !typing && plain && pressed(egui::Key::Z),
            tree: !typing && plain && pressed(egui::Key::E),
            rows: towards(egui::Key::ArrowUp, egui::Key::ArrowDown),
            columns: towards(egui::Key::ArrowLeft, egui::Key::ArrowRight),
            turn: if typing {
                0
            } else {
                match (pressed(egui::Key::Q), input.modifiers.shift_only()) {
                    (false, _) => 0,
                    (true, false) => 1,
                    (true, true) => 3,
                }
            },
            fit: !typing && plain && pressed(egui::Key::F),
            cycle: plain && pressed(egui::Key::Tab),
            underground: !typing && plain && pressed(egui::Key::U),
            lens: !typing && plain && pressed(egui::Key::L),
            import_lanes: !typing && plain && pressed(egui::Key::G),
            call_lanes: !typing && plain && pressed(egui::Key::A),
            held_underground: input.modifiers.shift_only(),
            cables: !typing && plain && pressed(egui::Key::M),
            lines: if typing || !plain {
                0
            } else {
                towards(egui::Key::K, egui::Key::J)
            },
            zoom: input.zoom_delta() * (input.smooth_scroll_delta.y / SCROLL_PER_E).exp(),
            pointer: input.pointer.hover_pos(),
            session_prev: !typing && plain && pressed(egui::Key::OpenBracket),
            session_next: !typing && plain && pressed(egui::Key::CloseBracket),
        }
    });

    // PROTOTYPE: T opens the ticket menu or closes the open case.
    // The case keeps Enter, Esc and the digits.
    if !typing && ui.input(|input| input.modifiers.is_none() && input.key_pressed(egui::Key::T)) {
        if casing {
            app.case = None;
        } else {
            app.tickets_open = !app.tickets_open;
        }
    }
    let (enter, esc, tree, turn) = if casing {
        (false, false, false, 0)
    } else {
        (enter, esc, tree, turn)
    };
    let over_case = pointer
        .is_some_and(|pointer| app.case.as_ref().is_some_and(|case| case.covers(pointer)))
        || app.case.as_ref().is_some_and(|case| {
            case.stage() == 5
                || (matches!(case.stage(), 4 | 6) && app.iterate_view.m(ui.input(|i| i.time)) < 0.5)
        });
    let mut case_key: Option<(String, f64)> = None;
    // Esc with a pane open but its terminal unfocused ends the attach and
    // does nothing else that frame.
    if !terminal && app.sessions.pane_open() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.sessions.close_pane();
    }
    let common_dir = app.city_common_dir();
    if turn != 0
        && let Some(opened) = &mut app.opened
        && let Some(centre) = opened.turn(turn)
    {
        app.navigation.turned(centre, turn);
    }
    let mut launcher_rows = if launcher_shown { rows } else { 0 };
    let mut jumped = false;
    let mut page = 0;
    // A NAV frame: egui's own focus moves (Tab, the arrows) must not leave
    // the focus on the field or a row once the frame is drawn.
    let mut nav_frame = false;
    if launcher_shown && !terminal && !sessions_search {
        // ⌘K flips between the modes; in NAV ":" and "/" enter CMD and are
        // taken off the frame's events, so the field never types them.
        let cmd_k = ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::K));
        // Ctrl+D / Ctrl+U: a page down / up, in NAV and CMD alike (STORY-083
        // requirement 14), taken off the frame so the field never types them.
        let (page_down, page_up) = ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::CTRL, egui::Key::D),
                input.consume_key(egui::Modifiers::CTRL, egui::Key::U),
            )
        });
        page = isize::from(page_down) - isize::from(page_up);
        if in_field {
            if cmd_k {
                app.launcher.request_unfocus();
            }
        } else {
            let (enter_cmd, down, up, first, last) = ui.input_mut(|input| {
                let is_cmd_text =
                    |event: &egui::Event| matches!(event, egui::Event::Text(t) if t == ":" || t == "/");
                let mut enter_cmd = input.events.iter().any(is_cmd_text);
                input.events.retain(|event| !is_cmd_text(event));
                let slash = input.modifiers.is_none() && input.key_pressed(egui::Key::Slash);
                let colon = input.key_pressed(egui::Key::Colon)
                    || (input.modifiers.shift_only() && input.key_pressed(egui::Key::Semicolon));
                enter_cmd |= slash || colon;
                let plain = input.modifiers.is_none();
                (
                    cmd_k || enter_cmd,
                    plain && input.key_pressed(egui::Key::J),
                    plain && input.key_pressed(egui::Key::K),
                    plain && input.key_pressed(egui::Key::G),
                    input.modifiers.shift_only() && input.key_pressed(egui::Key::G),
                )
            });
            if enter_cmd {
                app.launcher.request_focus();
            } else {
                nav_frame = true;
            }
            launcher_rows += isize::from(down) - isize::from(up);
            if first || last {
                app.launcher.select_end(last);
                jumped = true;
            }
        }
    }
    if page != 0 {
        let rows = cellworks_workspace::ui::list_page_rows(ui) as isize;
        app.launcher.move_selection_clamped(page * rows);
    }
    if launcher_rows != 0 || jumped || page != 0 {
        app.launcher.move_selection(launcher_rows);
        app.request_details();
        match app.launcher.selected().and_then(ProjectId::local_path) {
            Some(path) => app.navigation.select(path.to_owned()),
            None => app.navigation.deselect(),
        }
    }
    // Shift+Tab cycles the sort (STORY-083 requirement 15); it is taken
    // first so the plain Tab below never sees it.
    let launcher_sort = launcher_shown
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab));
    if launcher_sort {
        app.launcher.cycle_sort();
    }
    let launcher_tab = launcher_shown
        && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
    if launcher_tab {
        app.launcher.cycle_pick();
    }
    let now = ui.input(|input| input.time);
    if lens && let Some(opened) = &mut app.opened {
        let next = opened.next_lens();
        opened.set_lens(next, now);
    }
    if let Some(opened) = &mut app.opened {
        if import_lanes {
            opened.toggle_import_lanes();
        }
        if call_lanes {
            opened.toggle_call_lanes();
        }
    }
    if let Some(opened) = &mut app.opened {
        if opened.flying(now) {
            ui.ctx().request_repaint();
        }
        opened.prepare_network();
    }
    if fit && app.opened.is_some() {
        app.navigation.fit();
    }
    if underground {
        app.navigation.toggle_underground();
    }
    if cables {
        app.navigation.toggle_cables();
    }
    app.underground = app.navigation.is_underground() || (held_underground && !launcher_shown);
    if cycle && let Some(map) = mapped(&app.opened) {
        app.navigation.next_in_block(map);
    }
    let inside = app.navigation.inside().is_some();
    // Facing a facade, the arrows walk between the towers of its block.
    let walking = app.navigation.is_walking() && !inside;
    if walking && let Some(map) = mapped(&app.opened) {
        use cellworks_map::city::Toward;
        let toward = match (columns, rows) {
            (1, _) => Some(Toward::Right),
            (-1, _) => Some(Toward::Left),
            (_, -1) => Some(Toward::Behind),
            (_, 1) => Some(Toward::Front),
            _ => None,
        };
        if let Some(toward) = toward {
            app.navigation.walk(map, toward);
        }
    }
    if inside && let Some(map) = mapped(&app.opened) {
        app.navigation.move_floor(map, rows + lines);
    }
    if !inside
        && !walking
        && !app.tree_open
        && (rows, columns) != (0, 0)
        && let Some(view) = app.view
    {
        let by = view.ground_delta(egui::vec2(columns as f32, rows as f32) * PAN);
        app.navigation.pan(view.visible, by);
    }
    if ignored {
        app.toggle_ignored();
    }
    if tree && app.opened.is_some() {
        app.tree_open = !app.tree_open;
    }
    if app.tree_open
        && !inside
        && !walking
        && rows != 0
        && let Some(map) = mapped(&app.opened)
    {
        app.navigation.move_in_tree(map, rows);
    }
    if lock {
        app.navigation.toggle_lock();
    }
    if esc && app.opened.is_none() {
        // egui drops the field's focus on an unmodified Escape itself, which
        // is the whole of leaving CMD: the text stays. Only an Esc pressed in
        // NAV clears it.
        if !was_in_field {
            app.launcher.clear_search();
        }
    } else if esc && app.sessions.deselect() {
        // handled: a session selection is cleared before anything else.
    } else if esc && app.navigation.esc(mapped(&app.opened)) == Some(Moved::Closed) {
        app.opened = None;
        app.launcher.request_unfocus();
    }
    if enter
        && (app.opened.is_some()
            || app
                .launcher
                .selected()
                .is_some_and(|id| app.launcher.can_open(id)))
        && let Some(Moved::Opened(path)) = app.navigation.enter(mapped(&app.opened))
        && let Some(project) = app.project_at(&path)
    {
        app.open(project);
    }
    // A single-line `TextEdit` ends its input by surrendering the focus on
    // Enter, including on the frames where the launcher opens nothing — a
    // remote project (requirement 8), or no selection at all. CMD stays CMD:
    // the field is drawn after this block, so asking now puts the focus back
    // the same frame.
    if enter && in_field && app.opened.is_none() {
        app.launcher.request_focus();
    }

    iterate_keys(ui, app, terminal || typing);

    // Why the selected tower was built: worked out before the frame borrows
    // the opened project, and only when the selection has moved.
    app.explain_selection();

    // What the lens bar and the lane toggles asked for: applied once the
    // frame no longer borrows the opened project.
    if let Some(opened) = &mut app.opened {
        app.lens_panel.drain();
        app.lens_panel.watch(opened, now);
        if app.lens_panel.open {
            let panel = &mut app.lens_panel;
            egui::Panel::right("lenses")
                .exact_size(360.0)
                .show(ui, |ui| panel.ui(ui, opened, now));
        }
    }
    let mut bar: (Option<Lens>, (bool, bool)) = (None, (false, false));
    match &app.opened {
        None if app.tickets_open => {
            // PROTOTYPE: the ticket picker stands in for the launcher.
            egui::Panel::top("picker tabs").show(ui, |ui| {
                if ui.button("PROJECTS (T)").clicked() {
                    app.tickets_open = false;
                }
            });
            let picked = egui::CentralPanel::default()
                .show(ui, |ui| {
                    cellworks_map::ui::case::picker(ui, &mut app.jira, &mut app.typed_key)
                })
                .inner;
            case_key = picked.map(|key| (key, ui.input(|input| input.time)));
        }
        None => {
            let action = cellworks_workspace::ui::launcher(ui, &app.universe, &mut app.launcher);
            if nav_frame && let Some(id) = ui.memory(|memory| memory.focused()) {
                let clicked_field = cellworks_workspace::ui::search_focused(ui)
                    && ui.input(|input| input.pointer.any_pressed());
                if !clicked_field {
                    ui.memory_mut(|memory| memory.surrender_focus(id));
                }
            }
            match action {
                Some(LauncherAction::Selected(project)) => {
                    app.launcher.select(Some(project.id.clone()));
                    app.request_details();
                    match project.id.local_path() {
                        Some(path) => app.navigation.select(path.to_owned()),
                        None => app.navigation.deselect(),
                    }
                }
                Some(LauncherAction::Opened(project)) => app.open(project),
                None => {}
            }
        }
        Some(opened) => {
            let crumbs = app.navigation.breadcrumb(&opened.project().name, app.map());
            let mode = if app.navigation.is_locked() {
                LOCKED
            } else {
                FOLLOWS_ZOOM
            };
            let offered: Vec<Lens> = Lens::ALL
                .into_iter()
                .filter(|lens| opened.offers(*lens))
                .collect();
            let (switched, picked, toggled) = egui::Panel::top("breadcrumb")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(crumbs);
                        ui.separator();
                        let picked = cellworks_map::ui::lens_bar(ui, opened.lens(), &offered);
                        ui.separator();
                        let toggled = cellworks_map::ui::lane_toggles(ui, opened.lanes());
                        ui.separator();
                        ui.toggle_value(&mut app.lens_panel.open, "lens panel");
                        ui.separator();
                        (ui.button(mode).clicked(), picked, toggled)
                    })
                    .inner
                })
                .inner;
            if switched {
                app.navigation.toggle_lock();
            }
            bar = (picked, toggled);
            if app.tree_open
                && let Some(map) = app.map()
            {
                let row = egui::Panel::left("tree")
                    .show(ui, |ui| {
                        cellworks_navigation::ui::tree(ui, map, &app.navigation)
                    })
                    .inner;
                if let Some(path) = row {
                    app.navigation.select(path);
                }
            }
            // The why panel, in its own right column between the tree and
            // the city; nothing or a district selected shows no panel.
            let why = app.why().cloned();
            if let Some(why) = &why {
                // Inside the tower, the floor the engineer stands on is
                // explained too.
                let floor = app.navigation.inside().and_then(|(item, floor)| {
                    let map = mapped(&app.opened)?;
                    cellworks_map::floor_commit(map, map.item(item)?, floor)
                });
                egui::Panel::right("why")
                    .exact_size(WHY_WIDTH)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .show(ui, |ui| cellworks_map::ui::why_panel(ui, why, floor));
                    });
            }
            let tint = why.as_ref().map(|why| why.floors.as_slice());
            let mut sidebar_centre: Option<egui::Pos2> = None;
            if app.sessions.has_sources()
                && !casing
                && let Some(map) = mapped(&app.opened)
            {
                if let Some((_, centre)) = egui::Panel::left("sessions")
                    .exact_size(300.0)
                    .show(ui, |ui| {
                        app.sessions.sidebar(ui, map, common_dir.as_deref())
                    })
                    .inner
                {
                    sidebar_centre = centre;
                }
            }
            app.sessions.set_hosted(
                app.case
                    .as_ref()
                    .is_some_and(|c| matches!(c.stage(), 4 | 6))
                    && app.iterate.is_some(),
            );
            app.sessions.pane(ui);
            let sessions_view = &mut app.sessions;
            let (drawn, minimap, floor, (sessions_drawn, session_clicked, pane_opened)) =
                egui::CentralPanel::default()
                    .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                    .show(ui, |ui| {
                        let drawn = project(ui, opened, &app.navigation, app.underground, tint);
                        let (Some(shown), ProjectState::Mapped(map)) = (&drawn, opened.state())
                        else {
                            return (drawn, None, None, (Vec::new(), None, false));
                        };
                        // PROTOTYPE: a case brings its own chrome; the map's stays away.
                        let minimap = if casing {
                            None
                        } else {
                            let minimap =
                                cellworks_map::ui::minimap(ui, map, &shown.view, shown.panel);
                            if let Some(hints) = app.navigation.hints(map) {
                                cellworks_navigation::ui::hints(ui, shown.panel, &hints);
                            }
                            cellworks_map::ui::caption(ui, shown.panel, opened.lens());
                            if let Some(age) =
                                opened.switched_at().map(|at| ui.input(|i| i.time) - at)
                            {
                                cellworks_map::ui::banner(ui, shown.panel, opened.lens(), age);
                            }
                            cellworks_map::ui::legend(ui, shown.panel, opened.lanes());
                            minimap
                        };
                        let sessions_result =
                            sessions_view.paint(ui, map, common_dir.as_deref(), &shown.view);
                        if let Some(tour) = app.demo.as_mut() {
                            crate::demo::paint_world(ui, tour, map, &shown.view);
                        }
                        let floor = app.navigation.inside().and_then(|(item, floor)| {
                            cellworks_map::ui::inside(ui, map, item, floor, shown.panel)
                        });
                        (drawn, minimap, floor, sessions_result)
                    })
                    .inner;
            let Some(drawn) = drawn else {
                app.towers.clear();
                app.buildings.clear();
                app.bridges.clear();
                app.cables.clear();
                app.drawn_sessions = Vec::new();
                app.view = None;
                app.panel = None;
                // The tickets stay at hand while the project is read.
                let picked = if app.tickets_open {
                    tickets_window(ui, app)
                } else {
                    None
                };
                show_case(ui, app);
                if let Some((key, now)) = picked {
                    app.open_case(&key, now);
                }
                return;
            };
            app.towers = drawn.towers;
            // PROTOTYPE: the case's buildings outlined on the live map (the
            // source branch also receded the rest; skipped here).
            if let Some(spots) = &spots {
                let pulse = 0.5 + 0.5 * (case_now as f32 * 4.0).sin();
                for tower in &app.towers {
                    if let Some(spot) = spots.iter().find(|spot| tower.path.starts_with(&spot.path))
                    {
                        let width = if spot.pulse { 1.5 + 1.5 * pulse } else { 1.5 };
                        ui.painter().rect_stroke(
                            tower.outline,
                            2.0,
                            egui::Stroke::new(
                                width,
                                egui::Color32::from_rgb(255, 190, 60)
                                    .gamma_multiply(spot.light.clamp(0.2, 1.0)),
                            ),
                            egui::StrokeKind::Outside,
                        );
                    }
                }
            }
            app.buildings = drawn.buildings;
            app.bridges = drawn.bridges;
            app.cables = drawn.cables;
            app.drawn_sessions = sessions_drawn;
            app.bridge_hovered = drawn.bridge_hovered;
            app.districts = drawn.districts;
            app.folders = drawn.folders;
            app.outlines = drawn.outlines;
            app.lanes = drawn.lanes;
            app.lane_hovered = drawn.lane_hovered;
            if let Some(floor) = floor {
                app.navigation.pick_floor(floor);
            }
            let visible = drawn.view.visible;
            app.view = Some(drawn.view);
            app.panel = Some(drawn.panel);
            // Close enough, the camera turns to face a facade; the pivot
            // glides to it over the next frames.
            if let Some(map) = mapped(&app.opened) {
                let turn = cellworks_map::turn_at(drawn.view.scale());
                let middle = drawn.view.to_ground(drawn.panel.center());
                let nearest = map
                    .item_nearest(middle, app.underground)
                    .map(|item| item.path.clone());
                let dt = ui.input(|input| input.stable_dt).min(0.1);
                let pointed = drawn.hovered.as_deref();
                // Half the view's height on the ground: a facade taller than
                // twice this scrolls.
                let half = drawn.panel.height() / 2.0 / drawn.view.scale() * 0.9;
                let nearest = nearest.as_deref();
                if app
                    .navigation
                    .face(map, turn, pointed, nearest, visible, half, dt)
                {
                    ui.ctx().request_repaint();
                }
            }
            if drawn.dragged != egui::Vec2::ZERO {
                app.navigation.pan(visible, -drawn.dragged);
            }
            if let (Some((path, by)), Some(opened)) = (&drawn.moved, &mut app.opened) {
                opened.move_by(path, *by);
            }
            if drawn.move_ended
                && let (Some((_, keeper)), Some(opened)) = (&app.arrangements, &app.opened)
                && let Some(arrangement) = opened.arrangement()
            {
                // Kept off the frame loop, in order.
                keeper.keep(opened.project().id.clone(), arrangement);
            }
            if let Some(district) = minimap
                && let Some(found) = app.map().and_then(|map| map.find(&district))
            {
                let centre = found.area.center();
                app.navigation.centre_on(visible, centre);
            }
            if (session_prev || session_next)
                && let Some(map) = mapped(&app.opened)
                && let Some(centre) = app.sessions.step(map, common_dir.as_deref(), session_next)
            {
                app.navigation.centre_on(visible, centre);
            }
            if let Some(centre) = sidebar_centre {
                app.navigation.centre_on(visible, centre);
            }
            if session_clicked.is_some() {
                // The session's own selection already happened inside
                // `SessionsView::paint`; this branch only exists to pre-empt
                // the tower/lane/ground click below.
            } else if let Some(click) = drawn.lane_clicked {
                match (click.travel, mapped(&app.opened)) {
                    // Travelling along a lane: the view frames its far end.
                    (Some(far), Some(map)) => app.navigation.jump(map, far),
                    _ => app.navigation.pick_lane(click.lane),
                }
            } else if let Some(path) = drawn.clicked {
                app.navigation.select(path);
            } else if drawn.ground_clicked {
                app.navigation.deselect();
                app.navigation.drop_lane();
            }
            if !pane_opened
                && let Some(tower) = drawn.entered
                && let Some(map) = mapped(&app.opened)
            {
                app.navigation.step_inside(map, tower);
            }
            if zoom != 1.0
                && !over_case
                && let Some(map) = mapped(&app.opened)
            {
                // Turned towards a facade, zooming anchors on it.
                let pivot = app.navigation.pivot(map).map(|pivot| pivot.ground);
                let around = match (pivot, pointer) {
                    (Some(pivot), _) => pivot,
                    (None, Some(pointer)) => drawn.view.to_ground(pointer),
                    (None, None) => visible.center(),
                };
                app.navigation
                    .zoomed(map, zoomed(visible, around, zoom), around);
                ui.ctx().request_repaint();
            }
        }
    }
    if app.opened.is_some() && app.tickets_open {
        case_key = tickets_window(ui, app);
    }
    iterate_screen(ui, app);
    show_case(ui, app);
    if let Some((key, now)) = case_key {
        app.open_case(&key, now);
    }
    iterate_overlay(ui, app);
    crate::demo::overlay(ui, app);
    let (picked, (import_lanes, call_lanes)) = bar;
    if let Some(opened) = &mut app.opened {
        if let Some(lens) = picked {
            opened.set_lens(lens, ui.input(|input| input.time));
        }
        if import_lanes {
            opened.toggle_import_lanes();
        }
        if call_lanes {
            opened.toggle_call_lanes();
        }
    }
}

/// PROTOTYPE: the iterate loop's keys. Cmd+P prototypes the selected or open
/// project; with a loop running, P pins the selection, C captures a region,
/// `,` and `.` step through the rounds.
fn iterate_keys(ui: &mut egui::Ui, app: &mut CellworksApp, blocked: bool) {
    let start = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P));
    if start {
        prototype_this(app);
    }
    iterate_drive(ui, app);
    iterate_rest(ui, app, blocked);
}

/// PROTOTYPE: a drive file (`CELLWORKS_DRIVE`) the window polls, so the
/// prototype can be operated without synthetic keystrokes. One command per
/// line: open <path>, select <path>, proto, pin <anchor>, note <text>,
/// image <png>, send, step <n>, revert <k>, shot <png>.
fn iterate_drive(ui: &mut egui::Ui, app: &mut CellworksApp) {
    let Ok(file) = std::env::var("CELLWORKS_DRIVE") else {
        return;
    };
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(200));
    // Screenshots requested earlier.
    let shots: Vec<(String, std::sync::Arc<egui::ColorImage>)> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Screenshot {
                    user_data, image, ..
                } => user_data
                    .data
                    .as_ref()
                    .and_then(|d| d.downcast_ref::<String>())
                    .map(|path| (path.clone(), image.clone())),
                _ => None,
            })
            .collect()
    });
    for (path, image) in shots {
        write_png(&path, &image);
    }
    let Ok(text) = std::fs::read_to_string(&file) else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let _ = std::fs::write(&file, "");
    for line in text.lines() {
        drive_line(ui, app, line);
    }
}

/// PROTOTYPE: runs one drive command; shared by the drive file and the demo tour.
pub fn drive_line(ui: &mut egui::Ui, app: &mut CellworksApp, line: &str) {
    if crate::demo::is_demo() && crate::demo::command(ui, app, line) {
        return;
    }
    let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
    match cmd {
        "open" => app.open(Project::at(std::path::PathBuf::from(arg))),
        "select" => {
            if let Some(project) = app.project_at(std::path::Path::new(arg)) {
                app.launcher.select(Some(project.id.clone()));
            }
        }
        "proto" => prototype_this(app),
        "case-go" => {
            let now = ui.input(|i| i.time);
            if let Some(case) = &mut app.case {
                case.go(arg.parse().unwrap_or(1), now);
            }
        }
        "case-prototype" => {
            let now = ui.input(|i| i.time);
            if let Some(case) = &mut app.case {
                case.prototype_act(now);
            }
        }
        "file" => {
            if let Some(it) = &app.iterate {
                let (label, lines) = it.file_view(arg);
                let now = ui.input(|i| i.time);
                app.iterate_view.file = Some(crate::iterate_view::OpenFile {
                    rel: arg.to_owned(),
                    label,
                    lines,
                    opened: now,
                });
            }
        }
        "dock" => app.sessions.toggle_dock(),
        "fold" => app.sessions.toggle_collapsed(),
        "swap" => {
            let now = ui.input(|i| i.time);
            app.iterate_view.swap(now);
        }
        "type" => app.iterate_view.note = arg.to_owned(),
        "attach" => {
            let (path, name) = arg.split_once(' ').unwrap_or((arg, ""));
            match crate::iterate::Iterate::attach(std::path::Path::new(path), name) {
                Ok(it) => {
                    let worktree = it.worktree.clone();
                    app.iterate = Some(it);
                    app.iterate_pane = false;
                    app.open(Project::at(worktree));
                }
                Err(e) => eprintln!("attach failed: {e}"),
            }
        }
        "jump" => {
            if let Some(root) = app.map().map(|m| m.root.clone()) {
                app.jump(root.join(arg));
            }
        }
        "shot" => ui
            .ctx()
            .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                arg.to_owned(),
            ))),
        _ => {
            let Some(it) = &mut app.iterate else {
                return;
            };
            let draft = || crate::iterate::Draft {
                anchor: None,
                image: None,
                note: String::new(),
            };
            match cmd {
                "pin" => it.pin(arg.to_owned()),
                "note" => it.draft.get_or_insert_with(draft).note = arg.to_owned(),
                "image" => {
                    it.draft.get_or_insert_with(draft).image = Some(arg.into());
                }
                "send" => {
                    let typed = std::mem::take(&mut app.iterate_view.note);
                    if !typed.trim().is_empty() {
                        it.draft.get_or_insert_with(draft).note = typed;
                    }
                    let session = app
                        .sessions
                        .sessions()
                        .iter()
                        .find(|s| {
                            s.name == it.session_name
                                && (crate::demo::is_demo() || s.attach_id.is_some())
                        })
                        .cloned();
                    let delay = match &session {
                        Some(s) if app.sessions.pane_session() == Some(s.id.as_str()) => {
                            std::time::Duration::ZERO
                        }
                        Some(s) => {
                            app.sessions.open_pane(s);
                            std::time::Duration::from_secs(2)
                        }
                        None => std::time::Duration::ZERO,
                    };
                    it.send(delay);
                }
                "respawn" => it.respawn(),
                "close" => it.close_round(),
                "step" => it.step(arg.parse().unwrap_or(0)),
                "revert" => it.revert(arg.parse().unwrap_or(0)),
                _ => eprintln!("drive: unknown {line}"),
            }
        }
    }
}

/// PROTOTYPE: writes `image` as a PNG via a BMP and `sips`.
fn write_png(path: &str, image: &egui::ColorImage) {
    let [w, h] = image.size;
    let mut bmp = Vec::with_capacity(54 + w * h * 4);
    let size = (54 + w * h * 4) as u32;
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&size.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&54u32.to_le_bytes());
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(w as i32).to_le_bytes());
    bmp.extend_from_slice(&(-(h as i32)).to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes());
    bmp.extend_from_slice(&32u16.to_le_bytes());
    bmp.extend_from_slice(&[0u8; 24]);
    for px in &image.pixels {
        bmp.extend_from_slice(&[px.b(), px.g(), px.r(), 255]);
    }
    let tmp = format!("{path}.bmp");
    if std::fs::write(&tmp, bmp).is_ok() {
        let _ = std::process::Command::new("sips")
            .args(["-s", "format", "png", &tmp, "--out", path])
            .output();
        let _ = std::fs::remove_file(&tmp);
    }
}

/// PROTOTYPE: Cmd+P on the selected or open project.
fn prototype_this(app: &mut CellworksApp) {
    {
        let root = match &app.opened {
            Some(opened) => opened.project().id.local_path().map(|p| p.to_owned()),
            None => app
                .launcher
                .selected_project()
                .and_then(|p| p.id.local_path())
                .map(|p| p.to_owned()),
        };
        if let Some(root) = root {
            match crate::iterate::Iterate::start(&root) {
                Ok(it) => {
                    let worktree = it.worktree.clone();
                    app.iterate = Some(it);
                    app.iterate_pane = false;
                    app.open(Project::at(worktree));
                }
                Err(e) => eprintln!("prototype-this failed: {e}"),
            }
        }
    }
}

fn iterate_rest(ui: &mut egui::Ui, app: &mut CellworksApp, blocked: bool) {
    if blocked || app.opened.is_none() {
        return;
    }
    let (close, pin, capture, back, forward, swap) = ui.input(|i| {
        let plain = i.modifiers.is_none();
        (
            plain && i.key_pressed(egui::Key::R),
            plain && i.key_pressed(egui::Key::P),
            plain && i.key_pressed(egui::Key::C),
            plain && i.key_pressed(egui::Key::Comma),
            plain && i.key_pressed(egui::Key::Period),
            plain && i.key_pressed(egui::Key::V),
        )
    });
    let now = ui.input(|i| i.time);
    let on_plan = app
        .case
        .as_ref()
        .is_some_and(|c| matches!(c.stage(), 4 | 6))
        && !app.iterate_view.city;
    if swap
        && app
            .case
            .as_ref()
            .is_some_and(|c| matches!(c.stage(), 4 | 6))
    {
        app.iterate_view.swap(now);
    }
    let anchor = {
        let root = app.map().map(|m| m.root.clone());
        let rel = |p: &std::path::Path| {
            root.as_ref()
                .and_then(|r| p.strip_prefix(r).ok())
                .unwrap_or(p)
                .display()
                .to_string()
        };
        match app.navigation.inside() {
            Some((item, floor)) => Some(format!("{}:{}", rel(item), floor + 1)),
            None => app.navigation.selection().map(rel),
        }
    };
    let Some(it) = &mut app.iterate else {
        return;
    };
    if pin {
        // On the plan P arms the next click; on the city it pins the selection.
        match (on_plan, anchor) {
            (true, _) | (false, None) => app.iterate_view.pin_armed = !app.iterate_view.pin_armed,
            (false, Some(anchor)) => it.pin(anchor),
        }
    }
    if capture {
        it.capture();
    }
    if back {
        it.step(-1);
    }
    if forward {
        it.step(1);
    }
    if close {
        it.close_round();
    }
}

/// PROTOTYPE: stages 4 and 5 of a case, below its bar.
fn iterate_screen(ui: &mut egui::Ui, app: &mut CellworksApp) {
    let Some(case) = &app.case else {
        return;
    };
    let stage = case.stage();
    if stage < 4 {
        return;
    }
    let entered = case.entered();
    let factory = case.flavour() == cellworks_map::ui::case::Flavour::Factory;
    let Some(it) = &mut app.iterate else {
        return;
    };
    let now = ui.input(|i| i.time);
    if stage == 6 {
        let stale = app
            .iterate_view
            .review
            .as_ref()
            .is_none_or(|r| now - r.read > 2.0);
        if stale {
            let base = app
                .case
                .as_ref()
                .and_then(|c| c.base_branch())
                .unwrap_or_else(|| "main".into());
            let (files, commits) = it.since(&base);
            app.iterate_view.review = Some(crate::iterate_view::Review {
                base,
                files,
                commits,
                read: now,
            });
        }
    } else {
        app.iterate_view.review = None;
    }
    let bottom = app
        .sessions
        .pane_top()
        .unwrap_or_else(|| ui.ctx().content_rect().max.y);
    let session = app
        .sessions
        .sessions()
        .iter()
        .find(|s| s.name == it.session_name)
        .cloned();
    let runner = session.as_ref().and_then(|s| {
        let last = s.events.iter().rev().find(|e| e.write)?;
        Some((s.name.as_str(), last.path.as_path()))
    });
    let pane_right =
        (matches!(stage, 4 | 6) && app.sessions.pane_open() && app.sessions.dock_right())
            .then(|| app.sessions.collapsed());
    let (acts, slot) = crate::iterate_view::show(
        ui,
        stage,
        entered,
        bottom,
        &mut app.iterate_view,
        mapped(&app.opened),
        it,
        runner,
        factory,
        pane_right,
        now,
    );
    if let Some(slot) = slot {
        app.sessions.pane_at(ui, slot);
    }
    use crate::iterate_view::Act;
    for act in acts {
        match act {
            Act::Send => {
                let note = std::mem::take(&mut app.iterate_view.note);
                if note.trim().is_empty() {
                    continue;
                }
                it.draft
                    .get_or_insert(crate::iterate::Draft {
                        anchor: None,
                        image: None,
                        note: String::new(),
                    })
                    .note = note;
                let delay = match &session {
                    Some(s) if app.sessions.pane_session() == Some(s.id.as_str()) => {
                        std::time::Duration::ZERO
                    }
                    Some(s) if s.attach_id.is_some() => {
                        app.sessions.open_pane(s);
                        std::time::Duration::from_secs(2)
                    }
                    _ => std::time::Duration::ZERO,
                };
                it.send(delay);
                ui.memory_mut(|m| m.request_focus(egui::Id::new("iterate-note")));
            }
            Act::CloseRound => it.close_round(),
            Act::Undo(k) => {
                it.revert(k);
                it.viewing = None;
            }
            Act::View(v) => {
                it.viewing = v;
                if let Some(file) = &mut app.iterate_view.file {
                    (file.label, file.lines) = it.file_view(&file.rel);
                }
            }
            Act::Pin(anchor) => {
                it.pin(anchor);
                app.iterate_view.pin_armed = false;
            }
            Act::ArmPin => app.iterate_view.pin_armed = !app.iterate_view.pin_armed,
            Act::Capture => it.capture(),
            Act::Open => {
                if let Some(url) = it.url() {
                    let _ = std::process::Command::new("open").arg(url).spawn();
                }
            }
            Act::Swap => app.iterate_view.swap(now),
            Act::OpenFile(rel) => {
                let (label, lines) = match &app.iterate_view.review {
                    Some(review) if stage == 6 => it.file_view_since(&review.base, &rel),
                    _ => it.file_view(&rel),
                };
                app.iterate_view.file = Some(crate::iterate_view::OpenFile {
                    rel,
                    label,
                    lines,
                    opened: now,
                });
            }
            Act::CloseFile => app.iterate_view.file = None,
            Act::Launch(nvim) => {
                if let Some(file) = &app.iterate_view.file {
                    let line = it
                        .draft
                        .as_ref()
                        .and_then(|d| d.anchor.as_deref())
                        .and_then(|a| a.strip_prefix(&format!("{}:", file.rel)))
                        .and_then(|n| n.parse().ok())
                        .or_else(|| file.lines.iter().find(|l| l.mark == '+').and_then(|l| l.no))
                        .unwrap_or(1);
                    let shown =
                        cellworks_map::ui::case::launch(nvim, &it.worktree.join(&file.rel), line);
                    it.toast = Some((format!("Launched: {shown}"), std::time::Instant::now()));
                }
            }
            Act::Freeze => {
                if let Some(case) = &mut app.case {
                    case.freeze(now);
                }
            }
            Act::Reviewed => {
                if let Some(case) = &mut app.case {
                    case.review_act(now);
                }
            }
        }
    }
}

/// PROTOTYPE: the loop's strip, the fires and scaffolding on the towers, and
/// the feedback note.
fn iterate_overlay(ui: &mut egui::Ui, app: &mut CellworksApp) {
    // In a case the iterate screen has its own chrome; on its city view only
    // the marks on the towers come from here.
    let in_case = app.case.is_some();
    let city = app.iterate_view.m(ui.input(|i| i.time)) > 0.5;
    if app.opened.is_none() || (in_case && !city) {
        return;
    }
    let Some(it) = &mut app.iterate else {
        return;
    };
    let ctx = ui.ctx().clone();
    let mut jump: Option<std::path::PathBuf> = None;
    let mut revert: Option<usize> = None;
    let mut retry = false;
    if !in_case {
        egui::Area::new(egui::Id::new("iterate-strip"))
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 40.0))
            .show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(360.0);
                    ui.horizontal(|ui| {
                        ui.strong(format!("proto/{}", it.slug));
                        match (it.status.port, it.status.live) {
                            (Some(port), true) => {
                                ui.colored_label(
                                    egui::Color32::LIGHT_GREEN,
                                    format!("● live :{port}"),
                                );
                                if ui.small_button("open").clicked()
                                    && let Some(url) = it.url()
                                {
                                    let _ = std::process::Command::new("open").arg(url).spawn();
                                }
                            }
                            (Some(port), false) => {
                                ui.colored_label(egui::Color32::GRAY, format!("○ down :{port}"));
                            }
                            (None, _) => {
                                ui.colored_label(egui::Color32::GRAY, "○ no launch.json");
                            }
                        }
                    });
                    match &it.spawned {
                        None => {
                            ui.label("starting session…");
                        }
                        Some(Err(e)) => {
                            ui.colored_label(egui::Color32::LIGHT_RED, e);
                            if ui.small_button("retry session").clicked() {
                                retry = true;
                            }
                        }
                        Some(Ok(_)) => {
                            ui.label(format!("session {}", it.session_name));
                        }
                    }
                    ui.separator();
                    let live = it.rounds.len() - 1;
                    let shown = match it.viewing {
                        Some(k) => format!("viewing round {}: {}", k + 1, it.rounds[k].note),
                        None => format!(
                            "round {} open · {} notes · R closes it",
                            live + 1,
                            it.notes_in_round()
                        ),
                    };
                    ui.horizontal(|ui| {
                        ui.label(shown);
                        if let Some(k) = it.viewing
                            && ui
                                .small_button(format!("undo round {} and later", k + 1))
                                .clicked()
                        {
                            revert = Some(k);
                        }
                    });
                    ui.label(format!("{} files touched", it.scaffolding().len()));
                    if !it.status.fires.is_empty() {
                        ui.separator();
                        for (file, line, text) in &it.status.fires {
                            if ui
                                .link(format!("🔥 {file}:{line}"))
                                .on_hover_text(text)
                                .clicked()
                            {
                                jump = Some(it.worktree.join(file));
                            }
                        }
                    }
                    for line in it.log.iter().rev().take(2) {
                        ui.small(line);
                    }
                    ui.small("P pin · C capture · R close round · , . step");
                });
            });
    }
    if let Some(k) = revert {
        it.revert(k);
    }
    if retry {
        it.respawn();
    }

    // Fires and scaffolding on the files' plots, whatever form they are
    // drawn in at this zoom.
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("iterate-marks"),
    ));
    let scaffold = egui::Stroke::new(2.0, egui::Color32::from_rgb(240, 190, 60));
    let plot = |rel: &str| -> Option<[egui::Pos2; 4]> {
        let map = mapped(&app.opened)?;
        let view = app.view.as_ref()?;
        let area = map.find(&map.root.join(rel))?.area.expand(1.0);
        Some([
            view.to_screen(area.left_top(), 0.0),
            view.to_screen(area.right_top(), 0.0),
            view.to_screen(area.right_bottom(), 0.0),
            view.to_screen(area.left_bottom(), 0.0),
        ])
    };
    for rel in it.scaffolding() {
        if let Some([a, b, c, d]) = plot(rel) {
            painter.extend(egui::Shape::dashed_line(
                &[a, b, c, d, a],
                scaffold,
                6.0,
                4.0,
            ));
        }
    }
    let mut burning: Vec<(&str, Vec<u32>)> = Vec::new();
    for (file, line, _) in &it.status.fires {
        match burning.iter_mut().find(|(f, _)| *f == file.as_str()) {
            Some((_, lines)) => lines.push(*line),
            None => burning.push((file.as_str(), vec![*line])),
        }
    }
    for (file, lines) in burning {
        if let Some(corners) = plot(file) {
            let top = corners
                .iter()
                .fold(corners[0], |m, p| if p.y < m.y { *p } else { m });
            let at = top - egui::vec2(0.0, 14.0);
            painter.circle_filled(at, 8.0, egui::Color32::from_rgb(255, 90, 30));
            painter.text(
                at + egui::vec2(12.0, 0.0),
                egui::Align2::LEFT_CENTER,
                lines
                    .iter()
                    .map(|l| format!("L{l}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                egui::FontId::monospace(13.0),
                egui::Color32::from_rgb(255, 140, 80),
            );
        }
    }

    // The note.
    let mut send = false;
    let mut cancel = false;
    if !in_case && let Some(draft) = &mut it.draft {
        egui::Window::new("Feedback")
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                if let Some(anchor) = &draft.anchor {
                    ui.label(format!("at {anchor}"));
                }
                if let Some(image) = &draft.image {
                    ui.label(format!("screenshot {}", image.display()));
                }
                let response = ui.add(
                    egui::TextEdit::singleline(&mut draft.note)
                        .id(egui::Id::new("iterate-note"))
                        .desired_width(420.0)
                        .hint_text("What should change? Enter sends, Esc cancels"),
                );
                response.request_focus();
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !draft.note.trim().is_empty() {
                    send = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
            });
    }
    if cancel {
        it.draft = None;
    }
    if send {
        let session = app
            .sessions
            .sessions()
            .iter()
            .find(|s| {
                s.name == it.session_name && (crate::demo::is_demo() || s.attach_id.is_some())
            })
            .cloned();
        let delay = match &session {
            Some(s) if app.sessions.pane_session() == Some(s.id.as_str()) => {
                std::time::Duration::ZERO
            }
            Some(s) => {
                app.sessions.open_pane(s);
                std::time::Duration::from_secs(2)
            }
            None => std::time::Duration::ZERO,
        };
        it.send(delay);
    }
    if let Some(path) = jump {
        app.jump(path);
    }
}

/// PROTOTYPE: the ticket picker over an open project, and the ticket picked
/// in it, with when.
fn tickets_window(ui: &mut egui::Ui, app: &mut CellworksApp) -> Option<(String, f64)> {
    egui::Window::new("Tickets")
        .anchor(egui::Align2::RIGHT_TOP, [-12.0, 40.0])
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            cellworks_map::ui::case::picker(ui, &mut app.jira, &mut app.typed_key)
        })
        .and_then(|shown| shown.inner.flatten())
        .map(|key| (key, ui.input(|input| input.time)))
}

/// PROTOTYPE: lays the open case over whatever the frame drew.
fn show_case(ui: &mut egui::Ui, app: &mut CellworksApp) {
    if let Some(case) = &mut app.case {
        let live = mapped(&app.opened).map(|map| cellworks_map::ui::case::Live {
            map,
            towers: &app.towers,
        });
        if !cellworks_map::ui::case::show(ui, case, &mut app.jira, live) {
            app.case = None;
        }
    }
}

/// The part of the ground in view, `visible`, after zooming by `factor`
/// around the ground point `fixed`, which stays where it is on screen.
fn zoomed(visible: egui::Rect, fixed: egui::Pos2, factor: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        fixed + (visible.min - fixed) / factor,
        fixed + (visible.max - fixed) / factor,
    )
}

/// Draws the opened project, and reports the map as drawn once there is one.
fn project(
    ui: &mut egui::Ui,
    opened: &OpenedProject,
    navigation: &Navigation,
    underground: bool,
    tint: Option<&[usize]>,
) -> Option<Drawn> {
    for notice in opened.notices() {
        ui.label(notice_text(notice));
    }
    match opened.state() {
        ProjectState::Reading => {
            ui.label(READING);
            None
        }
        ProjectState::Empty => {
            ui.label(EMPTY);
            None
        }
        ProjectState::Failed => {
            ui.label(FAILED);
            None
        }
        ProjectState::Mapped(map) => Some(cellworks_map::ui::project(
            ui,
            &opened
                .shown(ui.input(|input| input.time))
                .unwrap_or(std::borrow::Cow::Borrowed(map)),
            cellworks_map::ui::ViewOptions {
                framing: navigation.framing(map),
                selected: navigation.selection(),
                folder_view: navigation.folder_view(map),
                pivot: navigation.pivot(map),
                underground,
                lanes: opened.lanes(),
                picked: navigation.picked_lane(),
                network: opened.network(),
                cables: navigation.shows_cables(),
                tint,
            },
        )),
    }
}

/// `store`, with a thread keeping what is handed to it.
fn kept(store: ArrangementStore) -> (ArrangementStore, ArrangementKeeper) {
    let keeper = store.start_keeping();
    (store, keeper)
}

/// `store`, with a thread keeping what is handed to it.
fn kept_recent(store: RecentStore) -> (RecentStore, RecentKeeper) {
    let keeper = store.start_keeping();
    (store, keeper)
}

/// The map of `opened`, once it has arrived: borrowed from that field alone,
/// so the navigation can change beside it without copying the map.
fn mapped(opened: &Option<OpenedProject>) -> Option<&ProjectMap> {
    match opened.as_ref()?.state() {
        ProjectState::Mapped(map) => Some(map),
        _ => None,
    }
}

/// Says so beside `opened` when its settings name a district prefix rule that
/// does not exist; the read uses the default instead.
fn note_unknown_prefix(opened: &mut OpenedProject) {
    if let Some(name) = &opened.project().district_prefix
        && prefix_rule(name).is_none()
    {
        let name = name.clone();
        opened.note(OpenedNotice::UnknownDistrictPrefix { name });
    }
}

/// A notice, written beside the opened project.
fn notice_text(notice: &OpenedNotice) -> String {
    match notice {
        OpenedNotice::FolderSkipped { path, reason } => {
            format!("Could not read {}: {reason}", path.display())
        }
        OpenedNotice::GitExcludesUnavailable { reason } => format!(
            "git could not say which files it ignores, so ignored files may be on the map: {reason}"
        ),
        OpenedNotice::UnknownDistrictPrefix { name } => format!(
            "The district prefix \"{name}\" is unknown; using \"{}\".",
            default_prefix_rule().name
        ),
    }
}

/// The options the window opens with: its title, its application id — Wayland
/// needs one to associate the window — and the wgpu renderer of ADR-0001.
pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: {
            let viewport = egui::ViewportBuilder::default()
                .with_title("Cellworks")
                .with_app_id("cellworks");
            // PROTOTYPE: the demo opens large enough to show the city beside the tour card.
            if crate::demo::is_demo() {
                viewport.with_inner_size([1600.0, 1000.0])
            } else {
                viewport
            }
        },
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    }
}

/// Whether `output` — the trimmed stdout of the platform's own read of the
/// system's reduced-motion setting — says to switch motion off (STORY-083
/// Decision 7): `"1"` on macOS (`defaults read
/// com.apple.universalaccess reduceMotion`), `"false"` on Linux (`gsettings
/// get org.gnome.desktop.interface enable-animations`). Anything else,
/// including an empty or garbled read, means not reduced: failure means
/// animate.
pub(crate) fn reduces_motion(output: &str, macos: bool) -> bool {
    let output = output.trim();
    if macos {
        output == "1"
    } else {
        output == "false"
    }
}

/// Reads whether the system currently asks for reduced motion, by a
/// read-only child process (ADR-0003): `defaults read
/// com.apple.universalaccess reduceMotion` on macOS, `gsettings get
/// org.gnome.desktop.interface enable-animations` elsewhere. A command that
/// fails to spawn, exits non-zero, or whose stdout is not valid UTF-8, reads
/// as empty — [`reduces_motion`] already treats that, like every other
/// unrecognised read, as "animate".
pub(crate) fn reduced_motion_requested() -> bool {
    let macos = cfg!(target_os = "macos");
    let output = if macos {
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "reduceMotion"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
    } else {
        Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
    };
    let read = output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default();
    reduces_motion(&read, macos)
}

/// Opens the window and runs until it closes.
///
/// Returns `Err` when there is no wgpu adapter and no display — a headless
/// machine, a broken driver, a remote shell. The caller reports it; nothing
/// here panics, and no test calls this.
pub fn run() -> eframe::Result {
    crate::demo::init();
    eframe::run_native(
        "Cellworks",
        native_options(),
        Box::new(|cc| {
            // The city is drawn at night, whatever the system's appearance.
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            cellworks_workspace::ui::install_fonts(&cc.egui_ctx);
            cellworks_map::ui::case::install_fonts(&cc.egui_ctx);
            if reduced_motion_requested() {
                cc.egui_ctx.all_styles_mut(|style| {
                    style.animation_time = 0.0;
                    style.visuals.text_cursor.blink = false;
                });
            }
            let mut app = CellworksApp::start();
            // PROTOTYPE: CELLWORKS_OPEN_CASE=<KEY> starts with that case open
            // (any other value opens SHOP-42).
            if let Ok(key) = std::env::var("CELLWORKS_OPEN_CASE") {
                let key = if key.contains('-') {
                    key
                } else {
                    "SHOP-42".into()
                };
                app.open_case(&key, 0.0);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_options_titles_the_window_cellworks() {
        let options = native_options();

        assert_eq!(options.viewport.title.as_deref(), Some("Cellworks"));
        assert_eq!(options.viewport.app_id.as_deref(), Some("cellworks"));
    }

    #[test]
    fn zooming_shrinks_the_view_around_the_fixed_point() {
        let visible = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 400.0));
        let fixed = egui::pos2(200.0, 150.0);

        let after = zoomed(visible, fixed, 2.0);

        assert_eq!(after.size(), visible.size() / 2.0);
        assert_eq!(after.min, egui::pos2(150.0, 125.0));
    }

    #[test]
    fn native_options_selects_the_wgpu_renderer() {
        let options = native_options();

        assert!(matches!(options.renderer, eframe::Renderer::Wgpu));
    }

    /// STORY-083 Decision 7: `reduces_motion` reads a platform-specific value
    /// for "reduced" — "1" from `defaults read` on macOS, "false" from
    /// `gsettings get` on Linux — and treats anything else, including an
    /// empty or garbled read, as "animate": failure means animate.
    #[test]
    fn reduces_motion_reads_the_platform_specific_value_for_reduced() {
        assert!(
            reduces_motion("1", true),
            "macOS reports reduced motion with \"1\""
        );
        assert!(
            !reduces_motion("0", true),
            "macOS: anything but \"1\" animates"
        );
        assert!(
            reduces_motion("false", false),
            "Linux reports reduced motion with \"false\""
        );
        assert!(
            !reduces_motion("true", false),
            "Linux: anything but \"false\" animates"
        );
        assert!(
            !reduces_motion("", true),
            "an empty read means animate, not reduced"
        );
        assert!(
            !reduces_motion("garbage", false),
            "a garbled read means animate, not reduced"
        );
    }
}
