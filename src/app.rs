//! The window the binary owns: the universe of the Workspace context, and the
//! options the window opens with.
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
//! never waits.

use std::sync::mpsc::Receiver;

use cellworks_workspace::{ScanState, Sources, Universe, WorkspaceEvent, start_scan};

/// The Cellworks application: the universe it shows, and the scan that fills
/// it in.
pub struct CellworksApp {
    universe: Universe,
    events: Option<Receiver<WorkspaceEvent>>,
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
            },
            None => CellworksApp {
                universe: Universe::without_home(),
                events: None,
            },
        }
    }

    /// The app over a channel the caller owns, so a test can drive the frame
    /// loop without a scan thread.
    pub fn over(universe: Universe, events: Receiver<WorkspaceEvent>) -> CellworksApp {
        CellworksApp {
            universe,
            events: Some(events),
        }
    }

    /// Takes whatever the scan has sent so far. Never waits.
    pub fn apply_pending(&mut self) {
        if let Some(events) = &self.events {
            self.universe.drain(events);
        }
    }

    /// The universe as it currently stands.
    pub fn universe(&self) -> &Universe {
        &self.universe
    }
}

impl eframe::App for CellworksApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.apply_pending();

        // egui draws on demand, and a scan finishing is not an input event.
        // Asking for the next frame while the scan is still running is what
        // gets its result onto the screen without anyone touching the mouse.
        if self.universe.scan() == &ScanState::Scanning {
            ui.ctx().request_repaint();
        }

        draw(ui, &self.universe);
    }
}

/// Paints one frame: the universe of the Workspace context.
pub fn draw(ui: &mut egui::Ui, universe: &Universe) {
    cellworks_workspace::ui::universe(ui, universe);
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
