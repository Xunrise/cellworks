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

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cellworks_code_analysis::{CodeAnalysisEvent, GitCli, GitListing, start_load};
use cellworks_map::View;
use cellworks_map::ui::{Drawn, DrawnBridge, DrawnBuilding, DrawnLane, DrawnTower, Outline};
use cellworks_map::{Lens, ProjectMap, default_prefix_rule, prefix_rule};
use cellworks_navigation::{Moved, Navigation};
use cellworks_workspace::ui::UniverseClick;
use cellworks_workspace::{
    ArrangementKeeper, ArrangementStore, Project, ScanState, Sources, Universe, WorkspaceEvent,
    start_scan,
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

/// The Cellworks application: the universe it shows, the scan that fills it
/// in, the project opened from it, if any, and where the view is.
pub struct CellworksApp {
    universe: Universe,
    events: Option<Receiver<WorkspaceEvent>>,
    opened: Option<OpenedProject>,
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
    /// The cables the last frame drew, from base to base.
    cables: Vec<(std::path::PathBuf, std::path::PathBuf)>,
    /// The skybridge the pointer was over in the last frame.
    bridge_hovered: Option<cellworks_map::Bridge>,
    /// Where the last frame saw the map from.
    view: Option<View>,
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
}

impl CellworksApp {
    /// Starts the scan of this machine and hands back the app that shows it.
    ///
    /// With no `$HOME` there is nowhere to scan: no thread starts, and the
    /// universe is reported empty rather than left scanning for ever.
    pub fn start() -> CellworksApp {
        match Sources::for_this_user() {
            Some(sources) => CellworksApp {
                arrangements: Some(kept(ArrangementStore::beside(&sources.settings_file))),
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
                cables: Vec::new(),
                bridge_hovered: None,
                view: None,
                underground: false,
                districts: Vec::new(),
                folders: Vec::new(),
                outlines: Vec::new(),
                lanes: Vec::new(),
                lane_hovered: None,
            },
            None => CellworksApp {
                arrangements: None,
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
                cables: Vec::new(),
                bridge_hovered: None,
                view: None,
                underground: false,
                districts: Vec::new(),
                folders: Vec::new(),
                outlines: Vec::new(),
                lanes: Vec::new(),
                lane_hovered: None,
            },
        }
    }

    /// The app over a channel the caller owns, so a test can drive the frame
    /// loop without a scan thread.
    pub fn over(universe: Universe, events: Receiver<WorkspaceEvent>) -> CellworksApp {
        CellworksApp {
            arrangements: None,
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
            underground: false,
            districts: Vec::new(),
            folders: Vec::new(),
            outlines: Vec::new(),
            lanes: Vec::new(),
            lane_hovered: None,
        }
    }

    /// The same app asking `git` about repositories instead of the real git,
    /// so a test never runs it.
    pub fn with_git(self, git: Arc<dyn GitListing + Send + Sync>) -> CellworksApp {
        CellworksApp { git, ..self }
    }

    /// The same app, keeping custom arrangements in `store`.
    pub fn with_arrangements(self, store: ArrangementStore) -> CellworksApp {
        CellworksApp {
            arrangements: Some(kept(store)),
            ..self
        }
    }

    /// Takes whatever the scan and the read have sent so far. Never waits.
    pub fn apply_pending(&mut self) {
        if let Some(events) = &self.events {
            self.universe.drain(events);
        }
        if let Some(opened) = &mut self.opened {
            opened.drain();
            match opened.state() {
                ProjectState::Mapped(map) => self.navigation.remapped(map),
                // Nothing left to frame or select: the project, whole.
                ProjectState::Empty => self
                    .navigation
                    .opened(opened.project().id.path().to_owned()),
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

    /// The layer the view is on, what is framed and what is selected.
    pub fn navigation(&self) -> &Navigation {
        &self.navigation
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

    /// The universe's project whose folder is `path`.
    fn project_at(&self, path: &std::path::Path) -> Option<Project> {
        match self.universe.scan() {
            ScanState::Reported(projects) => projects
                .iter()
                .find(|project| project.id.path() == path)
                .cloned(),
            _ => None,
        }
    }

    /// Opens `project` and starts reading it.
    pub fn open(&mut self, project: Project) {
        let path = project.id.path().to_owned();
        let events = self.read(&project);
        let knowledge =
            cellworks_domain_knowledge::start_read(path.clone(), project.contexts.clone());
        // Read off the frame loop, like the code.
        let arranged = self
            .arrangements
            .as_ref()
            .map(|(store, _)| store.start_read(project.id.clone()));
        let mut opened = OpenedProject::reading(project, events);
        opened.know(knowledge);
        if let Some(arranged) = arranged {
            opened.arrange(arranged);
        }
        note_unknown_prefix(&mut opened);
        self.opened = Some(opened);
        self.navigation.opened(path);
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

    /// Starts reading `project` with the district prefix rule its settings
    /// name, or the default when the name is unknown.
    fn read(&self, project: &Project) -> Receiver<CodeAnalysisEvent> {
        let rule = project.district_prefix.as_deref().and_then(prefix_rule);
        start_load(
            project.id.path().to_owned(),
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
        draw(ui, self);

        // egui draws on demand, and a scan or read finishing is not an input
        // event. Asking for the next frame while one is still running is what
        // gets its result onto the screen without anyone touching the mouse.
        // Asked after `draw`, because the frame that opens a project is the
        // one `draw` opens it in.
        let reading = self.opened.as_ref().is_some_and(OpenedProject::is_reading);
        if self.universe.scan() == &ScanState::Scanning || reading {
            ui.ctx().request_repaint();
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
    if app.opened.is_none() {
        app.towers.clear();
        app.buildings.clear();
        app.bridges.clear();
        app.cables.clear();
    }
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
    } = ui.input(|input| {
        let plain = input.modifiers.is_none();
        let pressed = |key| input.key_pressed(key);
        let towards = |less, more| isize::from(pressed(more)) - isize::from(pressed(less));
        Keys {
            enter: pressed(egui::Key::Enter),
            esc: pressed(egui::Key::Escape),
            ignored: plain && pressed(egui::Key::I),
            lock: plain && pressed(egui::Key::Z),
            tree: plain && pressed(egui::Key::E),
            rows: towards(egui::Key::ArrowUp, egui::Key::ArrowDown),
            columns: towards(egui::Key::ArrowLeft, egui::Key::ArrowRight),
            turn: match (pressed(egui::Key::Q), input.modifiers.shift_only()) {
                (false, _) => 0,
                (true, false) => 1,
                (true, true) => 3,
            },
            fit: plain && pressed(egui::Key::F),
            cycle: plain && pressed(egui::Key::Tab),
            underground: plain && pressed(egui::Key::U),
            lens: plain && pressed(egui::Key::L),
            import_lanes: plain && pressed(egui::Key::G),
            call_lanes: plain && pressed(egui::Key::A),
            held_underground: input.modifiers.shift_only(),
            cables: plain && pressed(egui::Key::M),
            lines: if plain {
                towards(egui::Key::K, egui::Key::J)
            } else {
                0
            },
            zoom: input.zoom_delta() * (input.smooth_scroll_delta.y / SCROLL_PER_E).exp(),
            pointer: input.pointer.hover_pos(),
        }
    });
    if turn != 0
        && let Some(opened) = &mut app.opened
        && let Some(centre) = opened.turn(turn)
    {
        app.navigation.turned(centre, turn);
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
    app.underground = app.navigation.is_underground() || held_underground;
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
    if esc && app.navigation.esc(mapped(&app.opened)) == Some(Moved::Closed) {
        app.opened = None;
    }
    if enter
        && let Some(Moved::Opened(path)) = app.navigation.enter(mapped(&app.opened))
        && let Some(project) = app.project_at(&path)
    {
        app.open(project);
    }

    // What the lens bar and the lane toggles asked for: applied once the
    // frame no longer borrows the opened project.
    let mut bar: (Option<Lens>, (bool, bool)) = (None, (false, false));
    match &app.opened {
        None => {
            match cellworks_workspace::ui::universe(ui, &app.universe, app.navigation.selection()) {
                Some(UniverseClick::Selected(project)) => {
                    app.navigation.select(project.id.path().to_owned());
                }
                Some(UniverseClick::Opened(project)) => app.open(project),
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
            let (drawn, minimap, floor) = egui::CentralPanel::default()
                .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                .show(ui, |ui| {
                    let drawn = project(ui, opened, &app.navigation, app.underground);
                    let (Some(shown), ProjectState::Mapped(map)) = (&drawn, opened.state()) else {
                        return (drawn, None, None);
                    };
                    let minimap = cellworks_map::ui::minimap(ui, map, &shown.view, shown.panel);
                    if let Some(hints) = app.navigation.hints(map) {
                        cellworks_navigation::ui::hints(ui, shown.panel, &hints);
                    }
                    cellworks_map::ui::caption(ui, shown.panel, opened.lens());
                    if let Some(age) = opened.switched_at().map(|at| ui.input(|i| i.time) - at) {
                        cellworks_map::ui::banner(ui, shown.panel, opened.lens(), age);
                    }
                    cellworks_map::ui::legend(ui, shown.panel, opened.lanes());
                    let floor = app.navigation.inside().and_then(|(item, floor)| {
                        cellworks_map::ui::inside(ui, map, item, floor, shown.panel)
                    });
                    (drawn, minimap, floor)
                })
                .inner;
            let Some(drawn) = drawn else {
                app.towers.clear();
                app.buildings.clear();
                app.bridges.clear();
                app.cables.clear();
                app.view = None;
                return;
            };
            app.towers = drawn.towers;
            app.buildings = drawn.buildings;
            app.bridges = drawn.bridges;
            app.cables = drawn.cables;
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
            if let Some(click) = drawn.lane_clicked {
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
            if let Some(tower) = drawn.entered
                && let Some(map) = mapped(&app.opened)
            {
                app.navigation.step_inside(map, tower);
            }
            if zoom != 1.0
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
            },
        )),
    }
}

/// `store`, with a thread keeping what is handed to it.
fn kept(store: ArrangementStore) -> (ArrangementStore, ArrangementKeeper) {
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
        viewport: egui::ViewportBuilder::default()
            .with_title("Cellworks")
            .with_app_id("cellworks"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    }
}

/// Opens the window and runs until it closes.
///
/// Returns `Err` when there is no wgpu adapter and no display — a headless
/// machine, a broken driver, a remote shell. The caller reports it; nothing
/// here panics, and no test calls this.
pub fn run() -> eframe::Result {
    eframe::run_native(
        "Cellworks",
        native_options(),
        Box::new(|cc| {
            // The city is drawn at night, whatever the system's appearance.
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(CellworksApp::start()))
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
}
