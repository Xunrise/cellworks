//! The launcher view: the header, the locations column, the main area and the
//! inspector of STORY-078's requirement 1.
//!
//! The only module of this crate that depends on egui, and the only one that
//! holds a word the engineer reads. Everything that stays within the
//! launcher — the search text, the picked location, the selection, arrow-key
//! movement — is applied directly to the [`Launcher`] passed in; only opening
//! a project or changing what Navigation tracks reaches beyond this view, as
//! a [`LauncherAction`].

use egui::StrokeKind;

use crate::Sources;
use crate::launcher::{HostState, Launcher, Location, Pick, ProjectKind};
use crate::project::{Project, ProjectId};
use crate::universe::{Notice, NoticeKind, ScanState, Universe};

/// Shown from the first frame until the first result arrives, so a slow walk
/// never flashes an empty launcher on the way.
const SCANNING: &str = "Scanning for projects…";

/// Shown when the scan ended without reporting, rather than leaving
/// "Scanning" on the screen for ever.
const FAILED: &str = "Could not finish scanning for projects.";

/// The footer's hint of requirement 1.
const HINTS: &str = "↑↓ move   Enter open   ⌘K search   ⌘N add";

/// A click the launcher reports upward: everything else (search text, the
/// picked location, the selection, arrow-key movement) is applied directly
/// to the `Launcher` passed in, because only opening a project or changing
/// what Navigation tracks reaches beyond this view.
#[derive(Clone, Debug, PartialEq)]
pub enum LauncherAction {
    /// The engineer selected this project, by a click or the arrow keys.
    Selected(Project),
    /// The engineer opened this project: double-clicked its name, or pressed
    /// Enter or "Open" while it was selected. Always a local project — the
    /// view never emits this for a remote one.
    Opened(Project),
}

/// Draws the launcher: the header, the locations column, the main area and
/// the inspector of requirement 1. Mutates `launcher` directly for anything
/// that stays within it (search, pick, selection by click or arrow key);
/// returns `Some` only when the engineer asked to open a project.
pub fn launcher(
    ui: &mut egui::Ui,
    universe: &Universe,
    launcher: &mut Launcher,
) -> Option<LauncherAction> {
    let mut action = None;

    egui::Panel::top("launcher-header").show(ui, |ui| header(ui, launcher));
    egui::Panel::bottom("launcher-footer").show(ui, |ui| {
        ui.label(HINTS);
    });
    egui::Panel::left("launcher-locations")
        .exact_size(248.0)
        .resizable(false)
        .show(ui, |ui| locations(ui, launcher));
    // The inspector keeps its width while nothing is selected: if it appeared
    // only on selection, the click that selects a project would narrow the
    // main area, a group heading could wrap, and the row would move from
    // under the second click of a double-click.
    let selected = launcher.selected_project().cloned();
    let opened = egui::Panel::right("launcher-inspector")
        .exact_size(300.0)
        .resizable(false)
        .show(ui, |ui| {
            selected
                .as_ref()
                .and_then(|project| inspector(ui, launcher, project))
        })
        .inner;
    if opened.is_some() {
        action = opened;
    }
    let central = egui::CentralPanel::default()
        .show(ui, |ui| {
            // Every project must be reachable however many there are, so the
            // list scrolls once it is taller than the window.
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| main_area(ui, universe, launcher))
                .inner
        })
        .inner;
    if action.is_none() {
        action = central;
    }

    action
}

/// The title, the search box (⌘K focuses it) and the disabled "Add
/// project…" button.
fn header(ui: &mut egui::Ui, launcher: &mut Launcher) {
    let search_id = ui.id().with("launcher-search");
    ui.horizontal(|ui| {
        ui.heading("Cellworks");
        let mut text = launcher.search_text().to_owned();
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(search_id)
                .hint_text("projects, folders or host:/path, ⌘K"),
        );
        if response.changed() {
            launcher.search(text);
        }
        add_project_button(ui);
    });
    // ⌘N does nothing yet (requirement 10).
    if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K)) {
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
}

/// "All projects", "Recent" (inert — no requirement wires it to a pick),
/// "This Mac", and the "HOSTS" heading with one row per host.
fn locations(ui: &mut egui::Ui, launcher: &mut Launcher) {
    if ui
        .label("All projects")
        .interact(egui::Sense::click())
        .clicked()
    {
        launcher.pick(Pick::All);
    }
    ui.label("Recent");
    if ui
        .label("This Mac")
        .interact(egui::Sense::click())
        .clicked()
    {
        launcher.pick(Pick::Location(Location::ThisMac));
    }
    ui.heading("HOSTS");
    let hosts: Vec<(String, HostState)> = launcher
        .hosts()
        .into_iter()
        .map(|(name, state)| (name.to_owned(), state))
        .collect();
    for (name, state) in hosts {
        ui.horizontal(|ui| {
            if ui
                .label(name.as_str())
                .interact(egui::Sense::click())
                .clicked()
            {
                launcher.pick(Pick::Location(Location::Host(name.clone())));
            }
            ui.label(state.to_string());
        });
    }
    ui.add_enabled(false, egui::Button::new("+ Connect host…"))
        .on_disabled_hover_text("Coming soon");
}

/// `project`'s kind, branch and file count (each left out when unknown), and
/// the "Open (Enter)" and "Sync" buttons.
fn inspector(ui: &mut egui::Ui, launcher: &Launcher, project: &Project) -> Option<LauncherAction> {
    let mut action = None;

    ui.label(match launcher.kind(&project.id) {
        ProjectKind::Repository => "git repository",
        ProjectKind::Folder => "folder",
        ProjectKind::Remote => "remote",
    });
    if let Some(details) = launcher.details_of(&project.id) {
        if let Some(branch) = &details.branch {
            ui.label(branch.as_str());
        }
        if let Some(files) = details.files {
            ui.label(format!("{files} file{}", if files == 1 { "" } else { "s" }));
        }
    }

    let can_open = project.id.local_path().is_some();
    let open = ui.add_enabled(can_open, egui::Button::new("Open (Enter)"));
    let open = if can_open {
        open
    } else {
        open.on_disabled_hover_text("Connect the host to open it")
    };
    if open.clicked() {
        action = Some(LauncherAction::Opened(project.clone()));
    }

    ui.add_enabled(false, egui::Button::new("Sync"))
        .on_disabled_hover_text("Coming soon");

    action
}

/// The recent row, then the scanning/failed/empty-state/grouped list
/// (requirement 12, kept), then the notices.
fn main_area(
    ui: &mut egui::Ui,
    universe: &Universe,
    launcher: &mut Launcher,
) -> Option<LauncherAction> {
    let mut action = None;

    let recent: Vec<Project> = launcher.recent_row().into_iter().cloned().collect();
    if !recent.is_empty() {
        ui.heading("Recent");
        ui.horizontal(|ui| {
            for project in &recent {
                ui.label(format!("recent: {}", project.name));
            }
        });
    }

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
        ScanState::Reported(_) => {
            let groups: Vec<(String, Vec<Project>)> = launcher
                .groups()
                .into_iter()
                .map(|(label, projects)| (label, projects.into_iter().cloned().collect()))
                .collect();
            if groups.is_empty() && !launcher.search_text().is_empty() {
                ui.label(format!(
                    "No project matches \"{}\".",
                    launcher.search_text()
                ));
                add_project_button(ui);
            } else {
                let selected = launcher.selected().cloned();
                for (label, projects) in &groups {
                    ui.heading(label);
                    for project in projects {
                        if let Some(click) = project_row(ui, launcher, project, selected.as_ref()) {
                            match click {
                                RowClick::Selected => {
                                    launcher.select(Some(project.id.clone()));
                                    action = Some(LauncherAction::Selected(project.clone()));
                                }
                                RowClick::Opened => {
                                    action = Some(LauncherAction::Opened(project.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for notice in universe.notices() {
        ui.label(notice_text(notice));
    }

    action
}

/// What a click on one project's row meant.
enum RowClick {
    Selected,
    Opened,
}

/// One project's row: its name (outlined when selected), its path, its
/// branch when known, and its state.
fn project_row(
    ui: &mut egui::Ui,
    launcher: &Launcher,
    project: &Project,
    selected: Option<&ProjectId>,
) -> Option<RowClick> {
    let mut click = None;
    ui.horizontal(|ui| {
        // The sense goes on the response, not the widget, so the entry stays
        // a label in the accessibility tree.
        let entry = ui
            .label(project.name.as_str())
            .interact(egui::Sense::click());
        if selected == Some(&project.id) {
            ui.painter().rect_stroke(
                entry.rect.expand(2.0),
                2.0,
                ui.visuals().selection.stroke,
                StrokeKind::Outside,
            );
            ui.ctx()
                .accesskit_node_builder(entry.id, |node| node.set_selected(true));
        }
        // A remote project's name does not open on double-click
        // (requirement 11); a double-click there still selects it.
        if entry.double_clicked() && project.id.local_path().is_some() {
            click = Some(RowClick::Opened);
        } else if entry.clicked() {
            click = Some(RowClick::Selected);
        }
        ui.label(launcher.path_of(project));
        if let Some(details) = launcher.details_of(&project.id)
            && let Some(branch) = &details.branch
        {
            ui.label(branch.as_str());
        }
        ui.label(launcher.state_of(&project.id));
    });
    click
}

/// The disabled "Add project…" button of requirement 10.
fn add_project_button(ui: &mut egui::Ui) {
    ui.add_enabled(false, egui::Button::new("Add project…"))
        .on_disabled_hover_text("Coming soon");
}

/// What an empty launcher tells the engineer.
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
        NoticeKind::HostSkipped => format!("Skipped the host {path}: {reason}"),
    }
}
