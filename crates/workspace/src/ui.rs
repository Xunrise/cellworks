//! The launcher view: the header, the centred card (search row, location chip
//! row and the project list) and the preview pane of STORY-082's keyboard-
//! first palette, which replaces STORY-078's three-column layout.
//!
//! The only module of this crate that depends on egui, and the only one that
//! holds a word the engineer reads. Everything that stays within the
//! launcher — the search text, the picked location, the selection, arrow-key
//! movement — is applied directly to the [`Launcher`] passed in; only opening
//! a project or changing what Navigation tracks reaches beyond this view, as
//! a [`LauncherAction`]. Tab (scope cycling) and Esc (clear search) are routed
//! in `src/app.rs`'s key block, next to Enter and the arrows (STORY-082
//! Decision 5); this module only draws and reports clicks.

use egui::StrokeKind;

use crate::Sources;
use crate::launcher::{HostState, Launcher, ProjectKind, match_range};
use crate::project::{Project, ProjectId};
use crate::universe::{Notice, NoticeKind, ScanState, Universe};

/// Shown from the first frame until the first result arrives, so a slow walk
/// never flashes an empty launcher on the way.
const SCANNING: &str = "Scanning for projects…";

/// Shown when the scan ended without reporting, rather than leaving
/// "Scanning" on the screen for ever.
const FAILED: &str = "Could not finish scanning for projects.";

/// The card's footer hints (STORY-082 requirement 9).
const HINTS: &str = "↑ ↓ move   Enter open   Tab scope   Esc clear search";

/// The centred card's max width (STORY-082 requirement 1): wide enough for
/// the mockup's layout, narrow enough to stay one card on a wider window.
const CARD_WIDTH: f32 = 920.0;

/// The preview pane's width, to the right of the list (STORY-082
/// requirement 7).
const PREVIEW_WIDTH: f32 = 300.0;

/// The `egui::Id` the search field is given, so [`search_focused`] can check
/// the same id `search_row` builds its `TextEdit` with.
const SEARCH_ID: &str = "cellworks-launcher-search";

/// Whether the launcher's search field currently has keyboard focus, so the
/// caller can suppress the map's letter shortcuts while a project name is
/// typed into it. The field is focused from the first frame (requirement 2),
/// so this is the launcher's ordinary state, not a corner of it — `src/app.rs`
/// reads the keys before this module draws, so only the id tells it apart.
pub fn search_focused(ui: &egui::Ui) -> bool {
    ui.memory(|memory| memory.has_focus(egui::Id::new(SEARCH_ID)))
}

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

/// Draws the launcher: the header above the card, the centred card (search
/// row, location chip row and the project list), the preview pane to its
/// right, the footer and the notices below the card. Mutates `launcher`
/// directly for anything that stays within it (search, pick, selection by
/// click or arrow key); returns `Some` only when the engineer asked to open a
/// project.
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
    // The preview pane keeps its width while nothing is selected: if it
    // appeared only on selection, the click that selects a project would
    // narrow the list, a group heading could wrap, and the row would move
    // from under the second click of a double-click.
    let selected = launcher.selected_project().cloned();
    let opened = egui::Panel::right("launcher-inspector")
        .exact_size(PREVIEW_WIDTH)
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
            // `vertical_centered` would centre on a negative offset, pushing
            // the card half off-screen to the left, if the window is
            // narrower than `CARD_WIDTH` once the preview pane has taken its
            // share — so the cap is never wider than what is actually here.
            let width = CARD_WIDTH.min(ui.available_width());
            let card_action = ui
                .vertical_centered(|ui| {
                    ui.set_max_width(width);
                    card(ui, universe, launcher)
                })
                .inner;
            // Below the card, not inside the scrollable list, so a notice is
            // never scrolled out of sight (requirement 11).
            for notice in universe.notices() {
                ui.label(notice_text(notice));
            }
            card_action
        })
        .inner;
    if action.is_none() {
        action = central;
    }

    action
}

/// "Cellworks", the project and host counts, and the disabled "Add project
/// ⌘N" button (STORY-082 requirement 1).
fn header(ui: &mut egui::Ui, launcher: &Launcher) {
    ui.horizontal(|ui| {
        ui.heading("Cellworks");
        let chips = launcher.location_chips();
        let projects = chips.first().map_or(0, |chip| chip.count);
        // Every chip but "All" and "This Mac" is a remote host.
        let hosts = chips.len().saturating_sub(2);
        ui.label(format!("{projects} projects · {hosts} hosts"));
        add_project_button(ui);
    });
}

/// The card's content: the search row, the location chip row and the
/// scrollable list (STORY-082 requirements 2 to 6, 10 and 11).
fn card(ui: &mut egui::Ui, universe: &Universe, launcher: &mut Launcher) -> Option<LauncherAction> {
    search_row(ui, launcher);
    chip_row(ui, launcher);
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| list(ui, universe, launcher))
        .inner
}

/// The search field, focused on the first frame and again on ⌘K; while
/// searching, the match count and the "Esc clear" hint (STORY-082
/// requirement 2).
fn search_row(ui: &mut egui::Ui, launcher: &mut Launcher) {
    let search_id = egui::Id::new(SEARCH_ID);
    ui.horizontal(|ui| {
        let mut text = launcher.search_text().to_owned();
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                // Tab scopes the list (requirement 4), so the field keeps the
                // focus on it: egui decides the focus direction from the raw
                // input before the key block runs, and `consume_key` there
                // cannot take that decision back. `lock_focus` only tells
                // egui's focus pass to leave this widget alone — a
                // single-line field inserts no tab character, and the key
                // block has consumed the key before the field reads its
                // events anyway.
                .lock_focus(true)
                .id(search_id)
                .hint_text("projects, folders or host:/path, ⌘K"),
        );
        if response.changed() {
            launcher.search(text);
        }
        if !launcher.search_text().is_empty() {
            ui.label(format!("{} matches", launcher.match_count()));
            ui.label("Esc clear");
        }
    });
    if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K)) {
        launcher.request_focus();
    }
    if launcher.take_focus_request() {
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
}

/// The location chip row: "All", "This Mac", then one chip per remote host
/// (STORY-082 requirement 3), replacing STORY-078's left locations column.
/// Clicking a chip picks it; Tab cycles it (`src/app.rs`'s key block, not
/// here).
fn chip_row(ui: &mut egui::Ui, launcher: &mut Launcher) {
    let picked = launcher.picked().clone();
    ui.horizontal(|ui| {
        for chip in launcher.location_chips() {
            let is_picked = chip.pick == picked;
            let group = ui.horizontal(|ui| {
                // The state's glyph and word together (requirement 3's "a
                // glyph paired with a word, never colour alone"): the glyph
                // is painted, the word is the only text, which is what
                // ASSUMED (Q-1) requires a label to carry.
                if let Some(state) = chip.state {
                    state_glyph(ui, state);
                }
                let name = ui.label(chip.label.as_str()).interact(egui::Sense::click());
                if let Some(state) = chip.state {
                    ui.label(state.word());
                }
                ui.label(chip.count.to_string());
                name
            });
            if is_picked {
                ui.painter().rect_stroke(
                    group.response.rect.expand(2.0),
                    2.0,
                    ui.visuals().selection.stroke,
                    StrokeKind::Outside,
                );
            }
            if group.inner.clicked() {
                launcher.pick(chip.pick.clone());
            }
        }
    });
}

/// A host chip's state glyph, painted beside the word rather than written
/// into it (ASSUMED (Q-1)): a filled dot for a connected host, a ring around
/// a dot for one still being checked, an empty ring for an offline one. The
/// shape differs as well as the colour, and the word beside it names the
/// state, so the glyph never carries the meaning alone.
fn state_glyph(ui: &mut egui::Ui, state: HostState) {
    let size = ui.text_style_height(&egui::TextStyle::Body) * 0.5;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let centre = rect.center();
    let radius = size / 2.0;
    let color = ui.visuals().text_color();
    let stroke = egui::Stroke::new(1.0, color);
    let painter = ui.painter();
    match state {
        HostState::Connected => {
            painter.circle_filled(centre, radius, color);
        }
        HostState::Checking => {
            painter.circle_stroke(centre, radius, stroke);
            painter.circle_filled(centre, radius * 0.4, color);
        }
        HostState::Offline => {
            painter.circle_stroke(centre, radius, stroke);
        }
    }
}

/// The recent row, then the scanning/failed/empty-state/grouped list
/// (STORY-078's states, kept per STORY-082 requirement 11).
fn list(ui: &mut egui::Ui, universe: &Universe, launcher: &mut Launcher) -> Option<LauncherAction> {
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
                    // One group per location, named by `Launcher::groups`
                    // (requirement 5): "This Mac" once for every local
                    // project, then a group per host.
                    ui.heading(label.as_str());
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

    action
}

/// What a click on one project's row meant.
enum RowClick {
    Selected,
    Opened,
}

/// One project's row: its name (underlined where the search matched it,
/// outlined when selected), its path and its branch label.
fn project_row(
    ui: &mut egui::Ui,
    launcher: &Launcher,
    project: &Project,
    selected: Option<&ProjectId>,
) -> Option<RowClick> {
    let mut click = None;
    ui.horizontal(|ui| {
        let name = name_job(ui, &project.name, launcher.search_text());
        // The sense goes on the response, not the widget, so the entry stays
        // a label in the accessibility tree.
        let entry = ui.label(name).interact(egui::Sense::click());
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
        // (requirement 8); a double-click there still selects it.
        if entry.double_clicked() && project.id.local_path().is_some() {
            click = Some(RowClick::Opened);
        } else if entry.clicked() {
            click = Some(RowClick::Selected);
        }
        ui.label(launcher.path_of(project));
        // Only once the branch is known: an unread row says nothing, rather
        // than calling every repository a folder before it is selected.
        if let Some(branch) = launcher.branch_label(&project.id) {
            ui.label(branch);
        }
    });
    click
}

/// `name`, with the byte range `query` matches in it underlined (STORY-082
/// requirement 6); plain text when there is no match, so a path-only hit
/// never underlines the wrong fragment.
fn name_job(ui: &egui::Ui, name: &str, query: &str) -> egui::text::LayoutJob {
    let color = ui.visuals().text_color();
    let plain = egui::TextFormat {
        color,
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::default();
    match match_range(name, query) {
        Some(range) => {
            job.append(&name[..range.start], 0.0, plain.clone());
            job.append(
                &name[range.clone()],
                0.0,
                egui::TextFormat {
                    color,
                    underline: egui::Stroke::new(1.0, color),
                    ..Default::default()
                },
            );
            job.append(&name[range.end..], 0.0, plain);
        }
        None => job.append(name, 0.0, plain),
    }
    job
}

/// `project`'s name, kind and location, path, branch, file count (when
/// known), an Open button and a disabled "Sync · coming soon" button
/// (STORY-082 requirement 7).
fn inspector(ui: &mut egui::Ui, launcher: &Launcher, project: &Project) -> Option<LauncherAction> {
    let mut action = None;

    ui.heading(project.name.as_str());
    let kind = match launcher.kind(&project.id) {
        ProjectKind::Repository => "git repository",
        ProjectKind::Folder => "folder",
        ProjectKind::Remote => "remote",
    };
    // The location is where the project lives — this Mac or the host's name
    // — not the host's connection state, which its chip carries.
    ui.label(format!("{kind} · {}", launcher.location_of(&project.id)));
    ui.label(launcher.path_of(project));
    if let Some(branch) = launcher.branch_label(&project.id) {
        ui.label(branch);
    }
    if let Some(details) = launcher.details_of(&project.id)
        && let Some(files) = details.files
    {
        ui.label(format!("{files} file{}", if files == 1 { "" } else { "s" }));
    }

    let can_open = launcher.can_open(&project.id);
    let label = launcher.open_label(&project.id);
    let open = ui.add_enabled(can_open, egui::Button::new(label));
    let open = if can_open {
        open
    } else if project.id.host().is_some() && label == "Open · Enter" {
        // Decision 1: remote opening is not built, even on a connected host,
        // though the label alone reads as if Enter would work.
        open.on_disabled_hover_text("Remote projects open in a later story")
    } else {
        open
    };
    if open.clicked() {
        action = Some(LauncherAction::Opened(project.clone()));
    }

    ui.add_enabled(false, egui::Button::new("Sync · coming soon"));

    action
}

/// The disabled "Add project ⌘N" button (requirement 1); ⌘N does nothing yet.
fn add_project_button(ui: &mut egui::Ui) {
    ui.add_enabled(false, egui::Button::new("Add project ⌘N"))
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
