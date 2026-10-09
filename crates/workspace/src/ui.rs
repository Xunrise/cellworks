//! The universe view: one labelled entry per project, the state the scan is
//! in, and whatever it could not do.
//!
//! The only module of this crate that depends on egui, and the only one that
//! holds a word the engineer reads. There is no layout, no district, no zoom
//! and no selection here — those belong to the Map and Navigation contexts.

use crate::Sources;
use crate::universe::{Notice, NoticeKind, ScanState, Universe};

/// Shown from the first frame until the first result arrives, so a slow walk
/// never flashes an empty universe on the way.
const SCANNING: &str = "Scanning for projects…";

/// Shown when the scan ended without reporting, rather than leaving
/// "Scanning" on the screen for ever.
const FAILED: &str = "Could not finish scanning for projects.";

/// Draws the universe.
pub fn universe(ui: &mut egui::Ui, universe: &Universe) {
    egui::CentralPanel::default().show(ui, |ui| {
        // Every project must be reachable however many there are, so the
        // list scrolls once it is taller than the window.
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| entries(ui, universe));
    });
}

fn entries(ui: &mut egui::Ui, universe: &Universe) {
    match universe.scan() {
        ScanState::Scanning => {
            ui.label(SCANNING);
        }
        ScanState::Failed => {
            ui.label(FAILED);
        }
        ScanState::Reported(projects) if projects.is_empty() => {
            ui.label(empty_state(universe.sources()));
        }
        ScanState::Reported(projects) => {
            for project in projects {
                ui.label(project.name.as_str());
            }
        }
    }

    for notice in universe.notices() {
        ui.label(notice_text(notice));
    }
}

/// What an empty universe tells the engineer.
///
/// The settings file is named because, until EPIC-008 brings the interface
/// for adding a folder, hand-editing it is the only way to add one. Product
/// owner, 2026-10-08.
fn empty_state(sources: Option<&Sources>) -> String {
    match sources {
        Some(sources) => format!(
            "No projects yet. Put a git repository in {}, or add a folder's path to {}.",
            sources.projects_folder.display(),
            sources.settings_file.display(),
        ),
        // There is no projects folder and no settings file to name: the home
        // directory this machine would take them from is unset.
        None => "No projects yet. Cellworks cannot find your home directory, \
                 so it has no projects folder to scan."
            .to_owned(),
    }
}

/// A failure, written beside the folder or the file it belongs to.
fn notice_text(notice: &Notice) -> String {
    let path = notice.path.display();
    let reason = &notice.reason;

    match notice.kind {
        NoticeKind::ProjectsFolderUnreadable => format!("Could not read {path}: {reason}"),
        NoticeKind::AddedFolderSkipped => format!("Skipped the added folder {path}: {reason}"),
        NoticeKind::SettingsUnreadable => {
            format!("Could not read the settings file {path}: {reason}")
        }
    }
}
