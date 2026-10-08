//! The window the binary owns: one empty frame, and the options it opens with.
//!
//! `draw` is kept apart from [`CellworksApp`] so a test can run a frame through
//! an [`egui::Ui`] without eframe, winit or a window, and `native_options` is
//! kept pure so the title and the renderer can be asserted the same way.

/// The Cellworks application. It holds no state yet.
pub struct CellworksApp;

impl eframe::App for CellworksApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        draw(ui);
    }
}

/// Paints one frame: an empty central panel, the seam the map canvas replaces.
pub fn draw(ui: &mut egui::Ui) {
    egui::CentralPanel::default().show(ui, |_ui| {});
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
        Box::new(|_cc| Ok(Box::new(CellworksApp))),
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
