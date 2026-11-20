//! The settings window (Cmd+,): a section list on the left and the selected
//! section's form on the right, in the look of the tickets panel.
//!
//! The form edits a draft. Each frame the fields that pass validation are
//! accepted into the committed settings and put to work at once (tickets
//! reconfigured, map defaults, the update check, the editor and folders); the
//! committed settings are written to `settings.toml` a moment after the last
//! change. A field that fails validation shows its reason and is neither taken
//! up nor saved. Fields an environment variable has taken over are shown
//! disabled with the variable's name.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use cellworks_sources::tickets::{
    SignIn, SourceId, SourceState, TicketDesk, TicketsConfig, test_connection,
};
use cellworks_workspace::theme::{
    self, CYAN, DANGER, DIM, INK, LINE, LINE_BRIGHT, OK, PANEL_RAISED, WARN, YELLOW, YELLOW_SOFT,
};
use cellworks_workspace::{
    DefaultView, Env, Keychain, LENSES, MemoryKeychain, Settings, SettingsStore,
};
use egui::{Align, Align2, Color32, Layout, Order, Sense, Ui, WidgetInfo, WidgetType, vec2};

use crate::app::CellworksApp;
use crate::update::{State, Updater};

const WIDTH: f32 = 960.0;
const HEIGHT: f32 = 760.0;
const NAV_WIDTH: f32 = 200.0;
/// How long after the last change the file is written.
const DEBOUNCE: f64 = 0.3;
/// How long the "saved" note stays.
const NOTE: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    General,
    Projects,
    Tickets,
    Map,
    Editor,
    Updates,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::General,
        Section::Projects,
        Section::Tickets,
        Section::Map,
        Section::Editor,
        Section::Updates,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Projects => "Projects",
            Section::Tickets => "Tickets",
            Section::Map => "Map",
            Section::Editor => "Editor & cases",
            Section::Updates => "Updates",
        }
    }
}

/// A connection test's answer: which source, and what it listed or why not.
type Tested = (SourceId, Result<usize, String>);

enum Test {
    Running,
    Done(Result<usize, String>),
}

/// What the form asked for besides editing settings.
#[derive(Default)]
struct Actions {
    sign_in_jira: bool,
    test: Option<SourceId>,
    check_now: bool,
    store_key: Option<String>,
}

/// The settings window's state.
pub struct SettingsWindow {
    pub open: bool,
    pub section: Section,
    store: Option<SettingsStore>,
    committed: Settings,
    draft: Settings,
    env: Env,
    /// Why the settings file could not be read, if it could not.
    problem: Option<String>,
    dirty_since: Option<f64>,
    saved_at: Option<f64>,
    save_error: Option<String>,
    folders_text: String,
    repos_text: String,
    key_input: String,
    key_note: Option<String>,
    /// The Linear key from the keychain, read once.
    linear_key: Option<String>,
    keychain: Arc<dyn Keychain>,
    tests: BTreeMap<SourceId, Test>,
    results: (Sender<Tested>, Receiver<Tested>),
    /// The project folders changed: rescan once they are written.
    rescan_after_save: bool,
    /// Whether a change to the tickets sources lists them again at once.
    pub(crate) list_on_change: bool,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        SettingsWindow::with(
            None,
            Settings::default(),
            None,
            Arc::new(MemoryKeychain::default()),
        )
    }
}

impl SettingsWindow {
    fn with(
        store: Option<SettingsStore>,
        settings: Settings,
        problem: Option<String>,
        keychain: Arc<dyn Keychain>,
    ) -> SettingsWindow {
        SettingsWindow {
            open: false,
            section: Section::General,
            store,
            folders_text: settings.project_folders.join("\n"),
            repos_text: settings.github.repos.join(", "),
            committed: settings.clone(),
            draft: settings,
            env: Env::default(),
            problem,
            dirty_since: None,
            saved_at: None,
            save_error: None,
            key_input: String::new(),
            key_note: None,
            linear_key: None,
            keychain,
            tests: BTreeMap::new(),
            results: channel(),
            rescan_after_save: false,
            list_on_change: false,
        }
    }

    /// The window over the settings file `store`, with the process
    /// environment as overrides and `keychain` for the Linear key.
    pub fn stored(store: SettingsStore, keychain: Arc<dyn Keychain>, env: Env) -> SettingsWindow {
        let loaded = store.load();
        let mut window =
            SettingsWindow::with(Some(store), loaded.settings, loaded.problem, keychain);
        window.env = env;
        if window.committed.linear_enabled && window.env.linear_key().is_none() {
            window.linear_key = window.keychain.get();
        }
        window
    }

    /// The settings in force, before the environment is applied.
    pub fn committed(&self) -> &Settings {
        &self.committed
    }

    fn tickets_config(&self, settings: &Settings) -> TicketsConfig {
        TicketsConfig::from_settings(
            &settings.effective(&self.env),
            self.env.linear_key().or_else(|| self.linear_key.clone()),
        )
    }

    /// Writes the committed settings now.
    fn save(&mut self, now: f64) {
        self.dirty_since = None;
        let Some(store) = &self.store else {
            return;
        };
        match store.save(&self.committed) {
            Ok(()) => {
                self.save_error = None;
                self.saved_at = Some(now);
            }
            Err(error) => self.save_error = Some(error.to_string()),
        }
    }
}

// ---------------------------------------------------------------- app

impl CellworksApp {
    /// Test seam: the app over the settings file at `path`, with an empty
    /// environment and an in-memory keychain. The file is read and applied.
    pub fn with_settings_file(self, path: std::path::PathBuf) -> CellworksApp {
        self.with_settings(path, Env::default(), Arc::new(MemoryKeychain::default()))
    }

    /// The same, with `env` as the environment and `keychain` for the key.
    pub fn with_settings(
        mut self,
        path: std::path::PathBuf,
        env: Env,
        keychain: Arc<dyn Keychain>,
    ) -> CellworksApp {
        self.settings = SettingsWindow::stored(SettingsStore::at(path), keychain, env);
        apply(&mut self, None);
        self
    }

    /// Whether the settings window is showing.
    pub fn settings_open(&self) -> bool {
        self.settings.open
    }

    /// The settings in force.
    pub fn settings(&self) -> &Settings {
        self.settings.committed()
    }

    /// Writes the settings now instead of after the debounce.
    pub fn flush_settings(&mut self) {
        if self.settings.dirty_since.is_some() {
            self.settings.save(0.0);
        }
    }

    /// The ticket sources the facade is running.
    pub fn ticket_sources(&self) -> Vec<SourceId> {
        self.tickets.sources().into_iter().map(|s| s.id).collect()
    }
}

/// Puts the settings read at start-up to work.
pub fn apply_all(app: &mut CellworksApp) {
    apply(app, None);
}

/// Puts `settings.committed` to work: everything that differs from `old`, or
/// everything when `old` is `None` (start-up).
fn apply(app: &mut CellworksApp, old: Option<&Settings>) {
    let new = app.settings.committed.clone();
    let changed = |f: &dyn Fn(&Settings) -> bool| old.is_none_or(|old| f(old) != f(&new));
    cellworks_workspace::apply_runtime(&new.effective(&app.settings.env));
    app.updates.auto_check = new.auto_check;
    app.launcher.set_theme_words(new.theme_words);
    if changed(&|s| s.map.show_ignored) {
        app.show_ignored = new.map.show_ignored;
    }
    if changed(&|s| s.map.underground) && app.navigation.is_underground() != new.map.underground {
        app.navigation.toggle_underground();
    }
    let config = app.settings.tickets_config(&new);
    if old.is_none_or(|old| app.settings.tickets_config(old) != config) {
        app.tickets.reconfigure(config);
        if app.settings.list_on_change {
            app.tickets.all();
        }
    }
    if old.is_some_and(|old| old.project_folders != new.project_folders) {
        app.settings.rescan_after_save = true;
    }
}

/// Keys, read early in the frame so nothing behind the window sees them:
/// Cmd+, toggles the window and Esc closes it.
pub fn keys(ui: &mut Ui, app: &mut CellworksApp) {
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
        app.settings.open = !app.settings.open;
    }
    if app.launcher.take_settings_request() {
        app.settings.open = true;
    }
    if app.settings.open
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        app.settings.open = false;
    }
}

/// Draws the window when open, takes up what was edited and writes it.
pub fn show(ui: &mut Ui, app: &mut CellworksApp) {
    let now = ui.input(|i| i.time);
    if app.settings.open {
        let mut actions = Actions::default();
        {
            let CellworksApp {
                settings,
                tickets,
                updates,
                ..
            } = app;
            window(ui.ctx(), settings, tickets, updates, now, &mut actions);
        }
        act(app, actions);
        let accepted = app.settings.draft.accepted(&app.settings.committed);
        if accepted != app.settings.committed {
            let old = std::mem::replace(&mut app.settings.committed, accepted);
            apply(app, Some(&old));
            app.settings.dirty_since = Some(now);
        }
    }
    // Closing the window writes at once.
    let due = app
        .settings
        .dirty_since
        .is_some_and(|since| !app.settings.open || now - since >= DEBOUNCE);
    if due {
        app.settings.save(now);
        if app.settings.rescan_after_save && app.settings.save_error.is_none() {
            app.settings.rescan_after_save = false;
            app.rescan_projects();
        }
    }
    let waiting = app.settings.dirty_since.is_some()
        || app.settings.saved_at.is_some_and(|at| now - at < NOTE);
    if waiting {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }
}

fn act(app: &mut CellworksApp, actions: Actions) {
    if actions.sign_in_jira {
        app.tickets.start_sign_in(SourceId::Jira);
    }
    if actions.check_now {
        app.updates.check_now();
    }
    if let Some(source) = actions.test {
        let config = app.settings.tickets_config(&app.settings.committed);
        app.settings.tests.insert(source, Test::Running);
        let tx = app.settings.results.0.clone();
        std::thread::spawn(move || {
            let _ = tx.send((source, test_connection(&config, source)));
        });
    }
    if let Some(key) = actions.store_key {
        match app.settings.keychain.set(&key) {
            Ok(()) => {
                app.settings.linear_key = Some(key);
                app.settings.key_note = Some("Stored in the keychain.".to_owned());
                let old = app.settings.committed.clone();
                apply_key(app, &old);
            }
            Err(reason) => app.settings.key_note = Some(reason),
        }
    }
}

/// The key changed: the facade is rebuilt with it.
fn apply_key(app: &mut CellworksApp, _old: &Settings) {
    let config = app.settings.tickets_config(&app.settings.committed);
    app.tickets.reconfigure(config);
    if app.settings.list_on_change {
        app.tickets.all();
    }
}

// --------------------------------------------------------------- window

fn window(
    ctx: &egui::Context,
    w: &mut SettingsWindow,
    desk: &mut dyn TicketDesk,
    updates: &mut Updater,
    now: f64,
    actions: &mut Actions,
) {
    desk.poll();
    while let Ok((source, result)) = w.results.1.try_recv() {
        w.tests.insert(source, Test::Done(result));
    }
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("settings-scrim"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(160));
        });
    let size = vec2(
        WIDTH.min(screen.width() - 40.0),
        HEIGHT.min(screen.height() - 40.0),
    );
    egui::Window::new("Settings")
        .title_bar(false)
        .order(Order::Foreground)
        .fixed_size(size)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .frame(
            theme::frame()
                .fill(theme::PANEL)
                .stroke(egui::Stroke::new(1.0, LINE_BRIGHT))
                .inner_margin(0),
        )
        .show(ctx, |ui| {
            ui.set_min_size(size);
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            header(ui, w, now);
            let rule = ui.available_rect_before_wrap();
            ui.painter()
                .hline(rule.x_range(), rule.top(), egui::Stroke::new(1.0, LINE));
            let body = ui.available_size();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.allocate_ui_with_layout(
                    vec2(NAV_WIDTH, body.y),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_size(vec2(NAV_WIDTH, body.y));
                        ui.add_space(12.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for section in Section::ALL {
                            if nav_row(ui, section.name(), w.section == section).clicked() {
                                w.section = section;
                            }
                        }
                    },
                );
                let line = ui.available_rect_before_wrap();
                ui.painter()
                    .vline(line.left(), line.y_range(), egui::Stroke::new(1.0, LINE));
                ui.allocate_ui_with_layout(
                    vec2(body.x - NAV_WIDTH, body.y),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_width(body.x - NAV_WIDTH);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width() - 16.0);
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(24.0);
                                    ui.vertical(|ui| {
                                        ui.set_max_width(ui.available_width() - 24.0);
                                        ui.spacing_mut().item_spacing.y = 10.0;
                                        form(ui, w, desk, updates, actions);
                                    });
                                });
                                ui.add_space(20.0);
                            });
                    },
                );
            });
        });
}

fn header(ui: &mut Ui, w: &mut SettingsWindow, now: f64) {
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), 44.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.add_space(18.0);
            let title = theme::tracked(ui.ctx(), "SETTINGS", 14.0, INK, false);
            ui.label(title);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                if theme::tab(ui, "Close", false, false).clicked() {
                    w.open = false;
                }
                let note = if let Some(error) = &w.save_error {
                    Some((format!("not saved: {error}"), DANGER))
                } else if w.dirty_since.is_some() {
                    Some(("saving…".to_owned(), DIM))
                } else if w.saved_at.is_some_and(|at| now - at < NOTE) {
                    Some(("saved".to_owned(), OK))
                } else {
                    None
                };
                if let Some((text, colour)) = note {
                    ui.label(theme::tracked(
                        ui.ctx(),
                        &text.to_uppercase(),
                        11.0,
                        colour,
                        false,
                    ));
                }
            });
        },
    );
}

fn nav_row(ui: &mut Ui, text: &str, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(NAV_WIDTH - 24.0, 30.0), Sense::click());
    let rect = rect.translate(vec2(12.0, 0.0));
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, selected, text));
    let (fill, ink) = if selected {
        (YELLOW_SOFT, YELLOW)
    } else if response.hovered() {
        (PANEL_RAISED, INK)
    } else {
        (Color32::TRANSPARENT, DIM)
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
            0.0,
            YELLOW,
        );
    }
    let galley = theme::eyebrow(ui.ctx(), text, ink);
    ui.painter().galley(
        egui::pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
    response
}

// ----------------------------------------------------------------- form

fn form(
    ui: &mut Ui,
    w: &mut SettingsWindow,
    desk: &mut dyn TicketDesk,
    updates: &mut Updater,
    actions: &mut Actions,
) {
    let problems = w.draft.problems();
    let env = w.env.clone();
    title(ui, w.section.name());
    match w.section {
        Section::General => general(ui, w),
        Section::Projects => {
            note(
                ui,
                "Folders walked for git repositories, one per line. ~/ is your home folder. Saving rescans.",
            );
            let error = problems.get("projects.folders");
            let response = ui.add(
                egui::TextEdit::multiline(&mut w.folders_text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .font(theme::font(ui.ctx(), 13.0, true)),
            );
            if response.changed() {
                w.draft.project_folders = w
                    .folders_text
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            if let Some(error) = error {
                ui.label(egui::RichText::new(error).color(DANGER).size(12.0));
            }
            note(
                ui,
                "Folders added by hand (added_folders) and per-project tables in settings.toml are kept as they are.",
            );
        }
        Section::Tickets => tickets(ui, w, desk, &problems, &env, actions),
        Section::Map => map(ui, w),
        Section::Editor => editor(ui, w, &problems, &env),
        Section::Updates => updates_section(ui, w, updates, actions),
    }
}

fn general(ui: &mut Ui, w: &mut SettingsWindow) {
    toggle(ui, "Theme words", &mut w.draft.theme_words);
    note(
        ui,
        "The launcher's Warhammer wording. Off shows plain words.",
    );
    ui.add_space(8.0);
    label(ui, "Settings file");
    let path = w
        .store
        .as_ref()
        .map_or("(not saved: no settings file)".to_owned(), |s| {
            s.path().display().to_string()
        });
    ui.label(
        egui::RichText::new(path)
            .color(INK)
            .font(theme::font(ui.ctx(), 12.0, true)),
    );
    if let Some(problem) = &w.problem {
        ui.label(
            egui::RichText::new(format!("The file could not be read, defaults are shown and nothing is saved over it: {problem}"))
                .color(DANGER)
                .size(12.0),
        );
    }
    note(
        ui,
        "Changes save as you make them. The first write keeps your original as settings.toml.bak, since comments do not survive.",
    );
}

fn tickets(
    ui: &mut Ui,
    w: &mut SettingsWindow,
    desk: &mut dyn TicketDesk,
    problems: &BTreeMap<&'static str, String>,
    env: &Env,
    actions: &mut Actions,
) {
    let states = desk.sources();
    let eff = w.draft.effective(env);
    let offline = eff.jira.offline;

    // Jira
    let state = states.iter().find(|s| s.id == SourceId::Jira);
    card(ui, "Jira", |ui| {
        toggle(ui, "Jira enabled", &mut w.draft.jira.enabled);
        status(ui, w.draft.jira.enabled, state, offline, None);
        env_text(
            ui,
            "Site (part of the address)",
            "tickets.jira.site",
            env,
            &eff.jira.site,
            &mut w.draft.jira.site,
            problems,
        );
        env_toggle(
            ui,
            "Jira offline",
            "tickets.jira.offline",
            env,
            &mut w.draft.jira.offline,
        );
        ui.horizontal(|ui| {
            if theme::tab(ui, "Jira sign in", false, false).clicked() {
                actions.sign_in_jira = true;
            }
            if let Some(state) = state {
                if state.sign_in == SignIn::Pending {
                    ui.label(
                        egui::RichText::new("waiting for the browser")
                            .color(CYAN)
                            .size(12.0),
                    );
                }
            }
        });
    });

    // Cockpit
    let state = states.iter().find(|s| s.id == SourceId::Cockpit);
    card(ui, "Cockpit", |ui| {
        toggle(ui, "Cockpit enabled", &mut w.draft.cockpit.enabled);
        status(ui, w.draft.cockpit.enabled, state, offline, None);
        env_text(
            ui,
            "URL",
            "tickets.cockpit.url",
            env,
            &eff.cockpit.url,
            &mut w.draft.cockpit.url,
            problems,
        );
        test_button(ui, SourceId::Cockpit, "Test Cockpit", &w.tests, actions);
    });

    // GitHub
    let state = states.iter().find(|s| s.id == SourceId::GitHub);
    card(ui, "GitHub", |ui| {
        toggle(ui, "GitHub enabled", &mut w.draft.github.enabled);
        status(ui, w.draft.github.enabled, state, offline, None);
        let before = w.repos_text.clone();
        let eff_repos = eff.github.repos.join(", ");
        env_text(
            ui,
            "Repositories besides those assigned to you, owner/repo, comma separated",
            "tickets.github.repos",
            env,
            &eff_repos,
            &mut w.repos_text,
            problems,
        );
        if w.repos_text != before {
            w.draft.github.repos = w
                .repos_text
                .split(',')
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_owned)
                .collect();
        }
        test_button(ui, SourceId::GitHub, "Test GitHub", &w.tests, actions);
    });

    // Linear
    let state = states.iter().find(|s| s.id == SourceId::Linear);
    let has_key = env.linear_key().is_some() || w.linear_key.is_some();
    card(ui, "Linear", |ui| {
        toggle(ui, "Linear enabled", &mut w.draft.linear_enabled);
        let missing = (!has_key).then_some("needs an API key");
        status(ui, w.draft.linear_enabled, state, offline, missing);
        if let Some(var) = env.overriding("tickets.linear.key") {
            note(
                ui,
                &format!("The API key comes from ${var}; the keychain is not used."),
            );
        } else {
            label(
                ui,
                "Personal API key (stored in the macOS Keychain, never in settings.toml)",
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut w.key_input)
                        .password(true)
                        .hint_text(if has_key {
                            "a key is stored"
                        } else {
                            "lin_api_…"
                        })
                        .desired_width(300.0),
                );
                if theme::tab(ui, "Store key", false, false).clicked()
                    && !w.key_input.trim().is_empty()
                {
                    actions.store_key = Some(w.key_input.trim().to_owned());
                    w.key_input.clear();
                }
            });
            if let Some(note_text) = &w.key_note {
                note(ui, note_text);
            }
        }
        test_button(ui, SourceId::Linear, "Test Linear", &w.tests, actions);
    });
}

fn map(ui: &mut Ui, w: &mut SettingsWindow) {
    note(
        ui,
        "What a project opens with when you have not chosen otherwise for it. V in a project switches its view and remembers that.",
    );
    label(ui, "Default view");
    ui.horizontal(|ui| {
        for (text, view) in [
            ("City view", DefaultView::City),
            ("Flat view", DefaultView::Flat),
        ] {
            if theme::tab(ui, text, w.draft.map.default_view == view, true).clicked() {
                w.draft.map.default_view = view;
            }
        }
    });
    label(ui, "Default lens");
    ui.horizontal_wrapped(|ui| {
        for lens in LENSES {
            if theme::tab(ui, lens, w.draft.map.default_lens == lens, true).clicked() {
                w.draft.map.default_lens = lens.to_owned();
            }
        }
    });
    note(
        ui,
        "A lens a project cannot be seen through falls back to folders; the cells lens needs the domain model, read after the map, so it does too.",
    );
    ui.add_space(6.0);
    toggle(ui, "Show ignored files", &mut w.draft.map.show_ignored);
    toggle(ui, "Show import lanes", &mut w.draft.map.import_lanes);
    toggle(ui, "Show call lanes", &mut w.draft.map.call_lanes);
    toggle(ui, "Start underground", &mut w.draft.map.underground);
}

fn editor(
    ui: &mut Ui,
    w: &mut SettingsWindow,
    problems: &BTreeMap<&'static str, String>,
    env: &Env,
) {
    let eff = w.draft.effective(env);
    env_text(
        ui,
        "Editor command (empty: the system's text editor)",
        "editor.command",
        env,
        &eff.editor,
        &mut w.draft.editor,
        problems,
    );
    env_text(
        ui,
        "Cases folder (empty: ~/.cellworks/cases)",
        "cases.folder",
        env,
        &eff.cases_folder,
        &mut w.draft.cases_folder,
        problems,
    );
    env_text(
        ui,
        "Skills folder (empty: this source tree's skills)",
        "skills.folder",
        env,
        &eff.skills_folder,
        &mut w.draft.skills_folder,
        problems,
    );
}

fn updates_section(ui: &mut Ui, w: &mut SettingsWindow, updates: &Updater, actions: &mut Actions) {
    label(ui, "Running version");
    ui.label(
        egui::RichText::new(env!("CARGO_PKG_VERSION"))
            .color(INK)
            .font(theme::font(ui.ctx(), 13.0, true)),
    );
    toggle(ui, "Check for updates every hour", &mut w.draft.auto_check);
    ui.horizontal(|ui| {
        if theme::tab(ui, "Check now", false, false).clicked() {
            actions.check_now = true;
        }
        ui.add_space(6.0);
        let (text, colour) = match &updates.state {
            State::Unsupported => (
                "Updates are checked only when running from the .app bundle.".to_owned(),
                DIM,
            ),
            State::Idle => ("Idle".to_owned(), DIM),
            State::Checking => ("Checking…".to_owned(), CYAN),
            State::UpToDate => ("Up to date".to_owned(), OK),
            State::Available(m) => (format!("Version {} is available", m.version), WARN),
            State::Downloading(m) => (format!("Downloading {}…", m.version), CYAN),
            State::Ready(m) => (
                format!("{} is ready: restart from the corner badge", m.version),
                OK,
            ),
            State::Failed(reason) => (format!("Failed: {reason}"), DANGER),
        };
        ui.label(egui::RichText::new(text).color(colour).size(12.0));
    });
}

// -------------------------------------------------------------- widgets

fn title(ui: &mut Ui, text: &str) {
    ui.label(theme::tracked(
        ui.ctx(),
        &text.to_uppercase(),
        15.0,
        YELLOW,
        false,
    ));
}

fn label(ui: &mut Ui, text: &str) {
    ui.label(theme::tracked(
        ui.ctx(),
        &text.to_uppercase(),
        10.0,
        DIM,
        false,
    ));
}

fn note(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(DIM).size(12.0));
}

fn card(ui: &mut Ui, name: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(PANEL_RAISED)
        .stroke(egui::Stroke::new(1.0, LINE))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(theme::tracked(
                ui.ctx(),
                &name.to_uppercase(),
                12.0,
                INK,
                false,
            ));
            add(ui);
        });
}

/// A switch with its label; the label is what a screen reader says.
fn toggle(ui: &mut Ui, text: &str, on: &mut bool) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), theme::font(ui.ctx(), 13.0, false), INK);
    let size = vec2(38.0 + 10.0 + galley.size().x, 20.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let value = *on;
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, value, text));
    let track = egui::Rect::from_min_size(rect.min, vec2(38.0, 20.0));
    let enabled = ui.is_enabled();
    let (fill, edge, knob) = if value {
        (YELLOW_SOFT, YELLOW, YELLOW)
    } else {
        (theme::WELL, LINE_BRIGHT, DIM)
    };
    let p = ui.painter();
    p.add(egui::Shape::convex_polygon(
        theme::chamfer(track, 5.0),
        fill,
        egui::Stroke::new(1.0, edge),
    ));
    let x = if value {
        track.right() - 14.0
    } else {
        track.left() + 4.0
    };
    p.rect_filled(
        egui::Rect::from_min_size(egui::pos2(x, track.top() + 4.0), vec2(10.0, 12.0)),
        0.0,
        knob,
    );
    let ink = if enabled { INK } else { DIM };
    p.galley(
        egui::pos2(
            track.right() + 10.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        ink,
    );
    response
}

/// A toggle an environment variable may have taken over.
fn env_toggle(ui: &mut Ui, text: &str, key: &str, env: &Env, on: &mut bool) {
    match env.overriding(key) {
        Some(var) => {
            let mut shown = true;
            ui.add_enabled_ui(false, |ui| toggle(ui, text, &mut shown));
            note(ui, &format!("Set by ${var}; the file's value is ignored."));
        }
        None => {
            toggle(ui, text, on);
        }
    }
}

/// A single-line field an environment variable may have taken over: then
/// `effective` is shown, disabled, with the variable's name.
fn env_text(
    ui: &mut Ui,
    name: &str,
    key: &'static str,
    env: &Env,
    effective: &str,
    value: &mut String,
    problems: &BTreeMap<&'static str, String>,
) {
    label(ui, name);
    let font = theme::font(ui.ctx(), 13.0, true);
    match env.overriding(key) {
        Some(var) => {
            let mut shown = effective.to_owned();
            ui.add_enabled(
                false,
                egui::TextEdit::singleline(&mut shown)
                    .desired_width(f32::INFINITY)
                    .font(font),
            );
            note(ui, &format!("Set by ${var}; the file's value is ignored."));
        }
        None => {
            ui.add(
                egui::TextEdit::singleline(value)
                    .desired_width(f32::INFINITY)
                    .font(font),
            );
            if let Some(error) = problems.get(key) {
                ui.label(egui::RichText::new(error).color(DANGER).size(12.0));
            }
        }
    }
}

/// One line of a source's state, from the facade.
fn status(
    ui: &mut Ui,
    enabled: bool,
    state: Option<&SourceState>,
    offline: bool,
    missing: Option<&str>,
) {
    let (text, colour): (String, Color32) = if !enabled {
        ("off".into(), DIM)
    } else if let Some(missing) = missing {
        (missing.into(), WARN)
    } else if let Some(state) = state {
        match (&state.sign_in, &state.failure) {
            (SignIn::Needed, _) => ("signed out".into(), WARN),
            (SignIn::Pending, _) => ("waiting for the browser".into(), CYAN),
            (SignIn::Failed(reason), _) => (format!("sign-in failed: {reason}"), DANGER),
            (_, Some(reason)) => (format!("failing: {reason}"), DANGER),
            _ if state.fetching => ("syncing…".into(), CYAN),
            _ => match state.synced {
                Some(synced) => (synced.label(), OK),
                None => ("not synced yet".into(), DIM),
            },
        }
    } else if offline {
        ("idle while Jira is offline".into(), DIM)
    } else {
        ("starting".into(), DIM)
    };
    ui.label(egui::RichText::new(text).color(colour).size(12.0));
}

fn test_button(
    ui: &mut Ui,
    source: SourceId,
    text: &str,
    tests: &BTreeMap<SourceId, Test>,
    actions: &mut Actions,
) {
    ui.horizontal(|ui| {
        if theme::tab(ui, text, false, false).clicked() {
            actions.test = Some(source);
        }
        match tests.get(&source) {
            Some(Test::Running) => {
                ui.label(egui::RichText::new("testing…").color(CYAN).size(12.0));
            }
            Some(Test::Done(Ok(n))) => {
                ui.label(
                    egui::RichText::new(format!("works: {n} tickets listed"))
                        .color(OK)
                        .size(12.0),
                );
            }
            Some(Test::Done(Err(reason))) => {
                ui.label(
                    egui::RichText::new(format!("failed: {reason}"))
                        .color(DANGER)
                        .size(12.0),
                );
            }
            None => {}
        }
    });
}
