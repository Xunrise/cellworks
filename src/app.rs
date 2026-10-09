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

use cellworks_code_analysis::{GitCli, GitListing, start_load};
use cellworks_map::{ProjectMap, default_prefix_rule, prefix_rule};
use cellworks_navigation::{Moved, Navigation};
use cellworks_workspace::ui::UniverseClick;
use cellworks_workspace::{Project, ScanState, Sources, Universe, WorkspaceEvent, start_scan};

use crate::opened::{OpenedNotice, OpenedProject, ProjectState};

/// Shown from the moment a project is opened until its map arrives.
const READING: &str = "Reading the project…";

/// Shown when a project was read and has nothing to draw.
const EMPTY: &str = "This project has no files to show.";

/// Shown when the read ended without a map.
const FAILED: &str = "Could not read this project.";

/// The Cellworks application: the universe it shows, the scan that fills it
/// in, the project opened from it, if any, and where the view is.
pub struct CellworksApp {
    universe: Universe,
    events: Option<Receiver<WorkspaceEvent>>,
    opened: Option<OpenedProject>,
    navigation: Navigation,
    git: Arc<dyn GitListing + Send + Sync>,
}

impl CellworksApp {
    /// Starts the scan of this machine and hands back the app that shows it.
    ///
    /// With no `$HOME` there is nowhere to scan: no thread starts, and the
    /// universe is reported empty rather than left scanning for ever.
    pub fn start() -> CellworksApp {
        match Sources::for_this_user() {
            Some(sources) => CellworksApp {
                universe: Universe::scanning(&sources),
                events: Some(start_scan(sources)),
                opened: None,
                navigation: Navigation::default(),
                git: Arc::new(GitCli),
            },
            None => CellworksApp {
                universe: Universe::without_home(),
                events: None,
                opened: None,
                navigation: Navigation::default(),
                git: Arc::new(GitCli),
            },
        }
    }

    /// The app over a channel the caller owns, so a test can drive the frame
    /// loop without a scan thread.
    pub fn over(universe: Universe, events: Receiver<WorkspaceEvent>) -> CellworksApp {
        CellworksApp {
            universe,
            events: Some(events),
            opened: None,
            navigation: Navigation::default(),
            git: Arc::new(GitCli),
        }
    }

    /// The same app asking `git` about repositories instead of the real git,
    /// so a test never runs it.
    pub fn with_git(self, git: Arc<dyn GitListing + Send + Sync>) -> CellworksApp {
        CellworksApp { git, ..self }
    }

    /// Takes whatever the scan and the read have sent so far. Never waits.
    pub fn apply_pending(&mut self) {
        if let Some(events) = &self.events {
            self.universe.drain(events);
        }
        if let Some(opened) = &mut self.opened {
            opened.drain();
        }
    }

    /// The project that is open, if any.
    pub fn opened(&self) -> Option<&OpenedProject> {
        self.opened.as_ref()
    }

    /// The layer the view is on, what is framed and what is selected.
    pub fn navigation(&self) -> &Navigation {
        &self.navigation
    }

    /// The opened project's map, once it has arrived.
    fn map(&self) -> Option<&ProjectMap> {
        match self.opened.as_ref()?.state() {
            ProjectState::Mapped(map) => Some(map),
            _ => None,
        }
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

    /// Opens `project` and starts reading it with the district prefix rule its
    /// settings name; an unknown name reads with the default and says so.
    pub fn open(&mut self, project: Project) {
        let named = project.district_prefix.clone();
        let rule = named.as_deref().and_then(prefix_rule);
        let events = start_load(
            project.id.path().to_owned(),
            project.name.clone(),
            rule.unwrap_or_else(default_prefix_rule),
            Arc::clone(&self.git),
        );
        let mut opened = OpenedProject::reading(events);
        if let (Some(name), None) = (named, rule) {
            opened.note(OpenedNotice::UnknownDistrictPrefix { name });
        }
        self.opened = Some(opened);
        self.navigation.opened(project.id.path().to_owned());
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
        let reading = self
            .opened
            .as_ref()
            .is_some_and(|opened| opened.state() == &ProjectState::Reading);
        if self.universe.scan() == &ScanState::Scanning || reading {
            ui.ctx().request_repaint();
        }
    }
}

/// Paints one frame: the universe, or the opened project. A click selects a
/// project or an element of the map, a double-click opens a project; Enter
/// moves one layer in and Esc one layer out.
pub fn draw(ui: &mut egui::Ui, app: &mut CellworksApp) {
    let (enter, esc) = ui.input(|input| {
        (
            input.key_pressed(egui::Key::Enter),
            input.key_pressed(egui::Key::Escape),
        )
    });
    if esc {
        let map = app.map().cloned();
        if app.navigation.esc(map.as_ref()) == Some(Moved::Closed) {
            app.opened = None;
        }
    }
    if enter {
        let map = app.map().cloned();
        if let Some(Moved::Opened(path)) = app.navigation.enter(map.as_ref())
            && let Some(project) = app.project_at(&path)
        {
            app.open(project);
        }
    }

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
            let clicked = egui::CentralPanel::default()
                .show(ui, |ui| project(ui, opened, &app.navigation))
                .inner;
            if let Some(path) = clicked {
                app.navigation.select(path);
            }
        }
    }
}

/// Draws the opened project, and returns the element clicked, if any.
fn project(
    ui: &mut egui::Ui,
    opened: &OpenedProject,
    navigation: &Navigation,
) -> Option<std::path::PathBuf> {
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
        ProjectState::Mapped(map) => {
            cellworks_map::ui::project(ui, map, navigation.framing(map), navigation.selection())
        }
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
        Box::new(|_cc| Ok(Box::new(CellworksApp::start()))),
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
    fn native_options_selects_the_wgpu_renderer() {
        let options = native_options();

        assert!(matches!(options.renderer, eframe::Renderer::Wgpu));
    }
}
