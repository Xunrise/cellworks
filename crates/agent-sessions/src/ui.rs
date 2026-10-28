//! The sessions sidebar and the map overlay: drawn with egui, so covered by
//! the binary's acceptance tests (`tests/story_030_sessions_walk_to_the_towers_they_work_on.rs`)
//! rather than by unit tests here.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use egui::{Pos2, Rect, pos2, vec2};

use crate::transcript::{Entry, EntryKind};
use crate::{SessionState, place};

/// One session as the overlay drew it this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnSession {
    pub id: String,
    pub tower: PathBuf,
    pub dot: Pos2,
    pub label: Rect,
    pub waiting: bool,
    pub writing: bool,
}

/// The `egui::Id` the search field is given, so `search_focused` can check
/// the same id `sidebar` builds its `TextEdit` with.
const SEARCH_ID: &str = "cellworks-sessions-search";

/// The `egui::Id` a terminal pane's interact area is given, so `pane` can
/// request and check its own keyboard focus the same way the sidebar's
/// search field does.
const TERMINAL_ID: &str = "cellworks-session-terminal";

/// The default height of a freshly opened session pane (design.md
/// requirement 2).
const PANE_HEIGHT: f32 = 340.0;

/// Height added per marked floor while a session writes on top of a tower
/// (requirement 8); purely cosmetic, so not read from `cellworks_map::city`.
const WRITTEN_FLOOR_HEIGHT: f32 = 6.0;

/// How long the eased walk along x then y lasts, in seconds.
const WALK_SECONDS: f64 = 1.5;

/// How long the climb up to the tower's working height lasts, in seconds.
const CLIMB_SECONDS: f64 = 0.4;

/// How long a walking session's trail fades for, in seconds.
const TRAIL_SECONDS: f64 = 0.7;

/// One session's animation state, kept across frames.
struct Anim {
    /// The tower it is currently walking to or standing on.
    tower: PathBuf,
    from: Pos2,
    to: Pos2,
    /// `ui.input(|i| i.time)` when the walk toward `to` began.
    started: f64,
    /// A per-session offset so the bustle motion is not synchronized.
    phase: f32,
    /// Ground points behind a walking session, with the time they were left.
    trail: VecDeque<(Pos2, f64)>,
}

/// The sidebar and overlay's own view model: what it watches, what it last
/// drew, and which session is selected.
pub struct SessionsView {
    watch: Option<Receiver<crate::SessionEvent>>,
    sessions: Vec<crate::Session>,
    error: Option<String>,
    selected: Option<String>,
    query: String,
    anims: HashMap<String, Anim>,
    has_sources: bool,
    /// The `claude` program a freshly opened pane attaches to, stops and
    /// continues with.
    claude: PathBuf,
    /// Where a read-only pane tails its session's transcript from.
    transcripts: PathBuf,
    /// The session pane currently open, if any (design.md requirement 2).
    pane: Option<crate::SessionPane>,
    pane_top: Option<f32>,
    dock_right: bool,
    collapsed: bool,
    hosted: bool,
    /// Set the moment a pane is (re)opened, so the terminal grabs keyboard
    /// focus on its very first draw, before any click into it; cleared once
    /// `pane` consumes it.
    want_focus: bool,
    /// Whether the open pane's terminal had ended, as of the last call to
    /// `pane` - [`SessionPane::has_ended`] needs `&mut self`, so `pane`
    /// caches it here once per frame for `pane_text`'s `&self` to read.
    pane_ended: bool,
}

impl SessionsView {
    /// Nothing to watch and nothing to show: no project is open, or the app
    /// has no session sources.
    pub fn none() -> SessionsView {
        SessionsView {
            watch: None,
            sessions: Vec::new(),
            error: None,
            selected: None,
            query: String::new(),
            anims: HashMap::new(),
            has_sources: false,
            claude: PathBuf::new(),
            transcripts: PathBuf::new(),
            pane: None,
            pane_top: None,
            dock_right: false,
            collapsed: false,
            hosted: false,
            want_focus: false,
            pane_ended: false,
        }
    }

    /// Watches every running session on the machine, from `sources`.
    pub fn watching(sources: crate::SessionSources) -> SessionsView {
        let claude = sources.claude.clone();
        let transcripts = sources.transcripts.clone();
        SessionsView {
            watch: Some(crate::start_watch(sources)),
            has_sources: true,
            claude,
            transcripts,
            ..SessionsView::none()
        }
    }

    /// Showing `sessions`, watching nothing further and attaching through a
    /// plain `claude` on the `PATH` with no transcripts root - the test seam
    /// `CellworksApp::with_sessions` uses, for tests that never open a pane.
    pub fn with_sessions(sessions: Vec<crate::Session>) -> SessionsView {
        Self::with_sessions_and_sources(sessions, PathBuf::from("claude"), PathBuf::new())
    }

    /// Showing `sessions`, watching nothing further, pointed at `claude` and
    /// `transcripts` for a pane to use - the test seam
    /// `CellworksApp::with_pane_sources` uses.
    pub fn with_sessions_and_sources(
        sessions: Vec<crate::Session>,
        claude: PathBuf,
        transcripts: PathBuf,
    ) -> SessionsView {
        SessionsView {
            sessions,
            claude,
            transcripts,
            has_sources: true,
            ..SessionsView::none()
        }
    }

    /// Drains whatever the watch has reported so far. Never blocks.
    pub fn drain(&mut self) {
        let Some(rx) = &self.watch else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(crate::SessionEvent::Listed(sessions)) => {
                    self.sessions = sessions;
                    self.error = None;
                }
                Ok(crate::SessionEvent::ListingFailed(message)) => {
                    self.error = Some(message);
                }
                // The main watch's own channel never sends a pane's variants
                // (`ConversationRead`, `CopyStarted`, `CopyFailed`, `Stopped`,
                // `StopFailed`) - those travel on a `SessionPane`'s own
                // channel instead - but `SessionEvent` is one type shared by
                // both, so this match must still be exhaustive over it.
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.watch = None;
                    break;
                }
            }
        }
    }

    /// Whether this view has a session source: a running background watch,
    /// or sessions seeded directly for a test.
    pub fn has_sources(&self) -> bool {
        self.has_sources
    }

    pub fn sessions(&self) -> &[crate::Session] {
        &self.sessions
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Clears the session selection; `true` when one was selected.
    pub fn deselect(&mut self) -> bool {
        self.selected.take().is_some()
    }

    /// The ordered list `step`, and the sidebar's "in this city" count, both
    /// walk: every session in "Needs you", "Working", "Idle", in that order,
    /// each already sorted newest-first by `groups`.
    fn in_city<'a>(
        &'a self,
        map: &cellworks_map::ProjectMap,
        common_dir: Option<&std::path::Path>,
    ) -> Vec<&'a crate::Session> {
        crate::groups(&self.sessions, map, common_dir, "")
            .into_iter()
            .filter(|(heading, _)| *heading != "Other projects")
            .flat_map(|(_, rows)| rows)
            .collect()
    }

    /// Steps to the next (`forward`) or previous session in this city;
    /// the tower centre to recentre the camera on, when the newly selected
    /// session stands on one.
    pub fn step(
        &mut self,
        map: &cellworks_map::ProjectMap,
        common_dir: Option<&std::path::Path>,
        forward: bool,
    ) -> Option<Pos2> {
        let list = self.in_city(map, common_dir);
        if list.is_empty() {
            return None;
        }
        let len = list.len();
        let current = list
            .iter()
            .position(|s| Some(s.id.as_str()) == self.selected.as_deref());
        let next = match (current, forward) {
            (None, true) => 0,
            (None, false) => len - 1,
            (Some(i), true) => (i + 1) % len,
            (Some(i), false) => (i + len - 1) % len,
        };
        let centre = place(map, common_dir, list[next]).map(|p| p.centre);
        self.selected = Some(list[next].id.clone());
        centre
    }

    /// Whether the sidebar's search field currently has keyboard focus (so
    /// the caller can suppress the map's letter/bracket shortcuts).
    pub fn search_focused(&self, ui: &egui::Ui) -> bool {
        ui.memory(|m| m.has_focus(egui::Id::new(SEARCH_ID)))
    }

    /// Opens a pane on `session` (design.md requirement 2): a no-op when
    /// `session` is already shown, otherwise replaces whatever pane was open
    /// and arranges for the terminal to grab keyboard focus the moment it is
    /// next drawn, so a typed key reaches it with no extra click.
    pub fn open_pane(&mut self, session: &crate::Session) {
        if self
            .pane
            .as_ref()
            .is_some_and(|pane| pane.session_id() == session.id)
        {
            return;
        }
        self.pane = Some(crate::SessionPane::open(
            session,
            &self.claude,
            &self.transcripts,
        ));
        self.want_focus = true;
    }

    /// Detaches without stopping anything (design.md requirement 5's "Close
    /// (detach)"; the Esc half of that requirement is `src/app.rs`'s
    /// concern, gated by [`SessionsView::has_keyboard`]).
    pub fn close_pane(&mut self) {
        self.pane = None;
    }

    /// Prototype: types `bytes` into the open pane's terminal.
    pub fn send_to_pane(&mut self, bytes: &[u8]) {
        if let Some(pane) = self.pane.as_mut() {
            pane.send(bytes);
        }
    }

    /// Prototype: the id of the session the pane shows.
    pub fn pane_session(&self) -> Option<&str> {
        self.pane.as_ref().map(|pane| pane.session_id())
    }

    pub fn pane_open(&self) -> bool {
        self.pane.is_some()
    }

    /// Whether an open pane's terminal currently holds keyboard focus. A
    /// read-only conversation has nothing to focus, so this is always
    /// `false` for it - `src/app.rs`'s Escape-closes-pane logic treats
    /// "a terminal pane with no focus" and "a conversation pane" the same
    /// way, by this one check.
    pub fn has_keyboard(&self, ui: &egui::Ui) -> bool {
        self.pane.as_ref().is_some_and(|pane| {
            pane.is_terminal() && ui.memory(|m| m.has_focus(egui::Id::new(TERMINAL_ID)))
        })
    }

    /// Test-only observability: the open pane's title, banner and body
    /// joined into one string - a terminal's screen rows, or a conversation's
    /// entries, one per line, `"> "`-prefixed for a prompt. `None` when no
    /// pane is open. Exists so acceptance tests can assert on the pane's
    /// content without reaching into egui's render tree; nothing else calls
    /// it.
    pub fn pane_text(&self) -> Option<String> {
        let pane = self.pane.as_ref()?;
        let mut lines = vec![format!("Session: {}", pane.name())];
        if let Some(banner) = pane.banner() {
            lines.push(banner.to_owned());
        }
        if let Some(screen) = pane.screen() {
            let cols = screen.size().1.get();
            lines.extend(screen.rows(0, cols));
            if self.pane_ended {
                lines.push("(attach ended)".to_owned());
            }
        } else if let Some(entries) = pane.entries() {
            for entry in entries {
                let prefix = if entry.kind == EntryKind::Prompt {
                    "> "
                } else {
                    ""
                };
                lines.push(format!("{prefix}{}", entry.text));
            }
        }
        Some(lines.join("\n"))
    }

    /// Draws the open pane, if any, as a resizable bottom panel (design.md
    /// requirements 2-9): its header (name, "Close (detach)", "Stop" for a
    /// terminal), an optional banner, and either a live terminal or a
    /// read-only conversation filling the rest.
    pub fn pane(&mut self, ui: &mut egui::Ui) {
        if self.pane.is_none() {
            self.pane_top = None;
            return;
        }
        // Prototype: docked right while someone hosts it, it is drawn there
        // by `pane_at` instead.
        if self.dock_right && self.hosted {
            self.pane_top = None;
            return;
        }

        let mut close = false;
        if self.dock_right {
            egui::Panel::right("session-pane-right")
                .frame(pane_frame())
                .resizable(true)
                .default_size(480.0)
                .show(ui, |ui| self.pane_body(ui, &mut close));
            self.pane_top = None;
        } else {
            let panel = egui::Panel::bottom("session-pane").frame(pane_frame());
            let panel = if self.collapsed {
                panel.resizable(false).exact_size(66.0)
            } else {
                panel.resizable(true).default_size(PANE_HEIGHT)
            };
            let shown = panel.show(ui, |ui| self.pane_body(ui, &mut close));
            self.pane_top = Some(shown.response.rect.top());
        }

        if close {
            self.pane = None;
        }
    }

    /// Prototype: draws the pane into `rect`, for a host that docks it as a
    /// column (the iterate stage, under its rounds).
    pub fn pane_at(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        if self.pane.is_none() {
            return;
        }
        let mut close = false;
        chamfer(
            ui.painter(),
            rect,
            cellworks_map::ui::case::PANEL,
            cellworks_map::ui::case::LINE,
            10.0,
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(12.0, 10.0))),
            |ui| {
                ui.set_clip_rect(rect);
                self.pane_body(ui, &mut close);
            },
        );
        if close {
            self.pane = None;
        }
    }

    /// Prototype: whether a host draws the pane when it is docked right.
    pub fn set_hosted(&mut self, hosted: bool) {
        self.hosted = hosted;
    }

    pub fn toggle_dock(&mut self) {
        self.dock_right = !self.dock_right;
    }

    pub fn toggle_collapsed(&mut self) {
        self.collapsed = !self.collapsed;
    }

    pub fn dock_right(&self) -> bool {
        self.dock_right
    }

    pub fn collapsed(&self) -> bool {
        self.collapsed
    }

    fn pane_body(&mut self, ui: &mut egui::Ui, close: &mut bool) {
        let pane = self.pane.as_mut().expect("checked by the caller");
        pane.drain();
        let ended = pane.is_terminal() && pane.has_ended();

        ui.horizontal(|ui| {
            use cellworks_map::ui::case::{FLOW, INK, INK_DIM, OK};
            let fold = if self.collapsed { "+" } else { "-" };
            if nr_ctl(ui, fold, Kind::Plain)
                .on_hover_text(if self.collapsed { "Expand" } else { "Collapse" })
                .clicked()
            {
                self.collapsed = !self.collapsed;
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("SESSION")
                    .font(egui::FontId::new(
                        11.0,
                        egui::FontFamily::Name("semibold".into()),
                    ))
                    .extra_letter_spacing(1.6)
                    .color(INK_DIM),
            );
            ui.label(
                egui::RichText::new(pane.name())
                    .font(egui::FontId::new(
                        13.0,
                        egui::FontFamily::Name("mono-bold".into()),
                    ))
                    .color(INK),
            );
            let (state, colour) = if ended {
                ("ENDED", INK_DIM)
            } else if pane.is_terminal() {
                ("ATTACHED", OK)
            } else {
                ("TRANSCRIPT", FLOW)
            };
            ui.label(
                egui::RichText::new(format!("● {state}"))
                    .monospace()
                    .size(11.0)
                    .color(colour),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if pane.is_terminal() && nr_ctl(ui, "STOP", Kind::Danger).clicked() {
                    pane.stop();
                    *close = true;
                }
                if nr_ctl(ui, "DETACH", Kind::Plain).clicked() {
                    *close = true;
                }
                let dock = if self.dock_right {
                    "DOCK BOTTOM"
                } else {
                    "DOCK RIGHT"
                };
                if nr_ctl(ui, dock, Kind::Plain).clicked() {
                    self.dock_right = !self.dock_right;
                }
            });
        });
        ui.add_space(4.0);

        if self.collapsed {
            // Folded: the session's last line only.
            let last = pane
                .screen()
                .map(|screen| screen.contents())
                .and_then(|text| {
                    text.lines()
                        .rev()
                        .map(str::trim)
                        .find(|line| {
                            !line.is_empty()
                                && !line.starts_with('─')
                                && !line.starts_with('>')
                                && !line.starts_with('❯')
                        })
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            ui.add(egui::Label::new(egui::RichText::new(last).monospace()).truncate());
        } else {
            if let Some(banner) = pane.banner().map(str::to_owned) {
                ui.horizontal(|ui| {
                    ui.label(&banner);
                    if !pane.is_terminal() && ui.button("Continue in the background").clicked() {
                        pane.continue_in_background();
                    }
                });
            }

            if pane.screen().is_some() {
                terminal_body(ui, pane, &mut self.want_focus);
            } else if let Some(entries) = pane.entries() {
                conversation_body(ui, entries);
            }
        }

        ui.ctx()
            .request_repaint_after(Duration::from_millis(if pane.is_terminal() {
                33
            } else {
                500
            }));

        self.pane_ended = ended;
    }

    /// Prototype: where the pane's top edge was last drawn, while one is open.
    pub fn pane_top(&self) -> Option<f32> {
        self.pane.as_ref().and(self.pane_top)
    }

    /// Draws the sidebar: heading, summary (or the listing error), search,
    /// groups. Returns the session clicked, and - when it stands in this
    /// city - the tower centre to recentre the camera on.
    pub fn sidebar(
        &mut self,
        ui: &mut egui::Ui,
        map: &cellworks_map::ProjectMap,
        common_dir: Option<&std::path::Path>,
    ) -> Option<(String, Option<Pos2>)> {
        ui.heading("Sessions");

        if let Some(message) = &self.error {
            ui.colored_label(
                egui::Color32::from_rgb(255, 120, 120),
                format!("Could not list sessions: {message}"),
            );
        } else {
            let running = self.sessions.len();
            let needs_you = self
                .sessions
                .iter()
                .filter(|s| s.session_state() == SessionState::Waiting)
                .count();
            let in_city: usize = crate::groups(&self.sessions, map, common_dir, "")
                .into_iter()
                .filter(|(heading, _)| *heading != "Other projects")
                .map(|(_, rows)| rows.len())
                .sum();
            ui.label(format!(
                "{running} running · {needs_you} need you · {in_city} in this city"
            ));
        }

        let mut query = self.query.clone();
        ui.add(
            egui::TextEdit::singleline(&mut query)
                .id(egui::Id::new(SEARCH_ID))
                .hint_text("search name, project, branch")
                .desired_width(f32::INFINITY),
        );
        self.query = query;

        ui.separator();

        let now_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());

        let mut clicked: Option<(String, Option<Pos2>)> = None;
        // Deferred until `groups` (borrowed from `self.sessions`) is dropped
        // below: `open_pane` needs `&mut self` as a whole, which the active
        // shared borrow of `self.sessions` would not allow yet.
        let mut to_open: Option<crate::Session> = None;
        let groups = crate::groups(&self.sessions, map, common_dir, &self.query);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (heading, rows) in groups {
                ui.label(egui::RichText::new(heading).strong());
                for session in rows {
                    let (row_clicked, row_opened) =
                        session_row(ui, session, self.selected.as_deref(), now_seconds);
                    if row_clicked {
                        self.selected = Some(session.id.clone());
                        let centre = place(map, common_dir, session).map(|p| p.centre);
                        clicked = Some((session.id.clone(), centre));
                    }
                    if row_opened {
                        to_open = Some(session.clone());
                    }
                }
            }
        });

        if let Some(session) = to_open {
            self.open_pane(&session);
        }

        clicked
    }

    /// Draws every session over the already-drawn map; returns what was
    /// drawn, the id of a session whose dot was clicked, and whether a
    /// double-click on a dot opened a pane this frame (so `src/app.rs` can
    /// stop that same double-click from also stepping into the tower
    /// underneath it, design.md requirement 2).
    pub fn paint(
        &mut self,
        ui: &egui::Ui,
        map: &cellworks_map::ProjectMap,
        common_dir: Option<&std::path::Path>,
        view: &cellworks_map::View,
    ) -> (Vec<DrawnSession>, Option<String>, bool) {
        let now = ui.input(|i| i.time);
        let painter = ui.painter();

        let placed: Vec<(&crate::Session, crate::Placement)> = self
            .sessions
            .iter()
            .filter_map(|session| place(map, common_dir, session).map(|p| (session, p)))
            .collect();

        let towers: Vec<PathBuf> = placed.iter().map(|(_, p)| p.tower.clone()).collect();
        let fans = crate::fan(&towers);

        let click = ui.input(|i| {
            i.pointer
                .primary_clicked()
                .then(|| i.pointer.interact_pos())
                .flatten()
        });
        let double_click = ui.input(|i| {
            let primary = egui::PointerButton::Primary;
            (i.pointer.button_double_clicked(primary) || i.pointer.button_triple_clicked(primary))
                .then(|| i.pointer.interact_pos())
                .flatten()
        });

        let mut drawn = Vec::new();
        let mut clicked = None;
        // Deferred, the same way `sidebar` defers it: `open_pane` needs
        // `&mut self`, which the loop's active borrow of `self.sessions`
        // (through `placed`) does not allow yet.
        let mut to_open: Option<crate::Session> = None;
        let mut anything_is_animating = !placed.is_empty();

        for ((session, placement), (k, n)) in placed.iter().zip(fans) {
            let anim = self
                .anims
                .entry(session.id.clone())
                .or_insert_with(|| Anim {
                    tower: placement.tower.clone(),
                    from: placement.centre,
                    to: placement.centre,
                    started: now - 100.0,
                    phase: session.id.bytes().map(f32::from).sum(),
                    trail: VecDeque::new(),
                });

            if anim.tower != placement.tower {
                let (current, _, _) = walk(anim, now, 0.0);
                anim.from = current;
                anim.to = placement.centre;
                anim.tower = placement.tower.clone();
                anim.started = now;
                anim.trail.clear();
            } else {
                anim.to = placement.centre;
            }

            let (ground, mut z, walking) = walk(anim, now, placement.z);
            if walking {
                anim.trail.push_back((ground, now));
            }
            while anim
                .trail
                .front()
                .is_some_and(|(_, t)| now - t > TRAIL_SECONDS)
            {
                anim.trail.pop_front();
            }

            for (g, t) in &anim.trail {
                let fade = (1.0 - (now - t) / TRAIL_SECONDS).clamp(0.0, 1.0) as f32;
                painter.circle_filled(
                    view.to_screen(*g, 0.0),
                    3.0 * fade + 1.0,
                    egui::Color32::from_white_alpha((120.0 * fade) as u8),
                );
            }

            if !walking && placement.writing {
                z += placement.added as f32 * WRITTEN_FLOOR_HEIGHT;
            }

            let base = view.to_screen(ground, z);
            let mut at = if n > 1 {
                let a = std::f32::consts::TAU * k as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                base + vec2(13.0 * a.cos(), 7.0 * a.sin())
            } else {
                base
            };

            if !walking && session.session_state() != SessionState::Waiting {
                let speed = if placement.writing { 2.2 } else { 0.7 };
                let ang = anim.phase + now as f32 * speed;
                at += vec2(9.0 * ang.cos(), 5.0 * ang.sin() + 2.0 * (ang * 1.7).sin());
                if placement.writing {
                    let tick = (now * 14.0) as u64;
                    for i in 0..3u64 {
                        let h = tick
                            .wrapping_mul(2654435761)
                            .wrapping_add(i * 97 + (anim.phase * 100.0) as u64)
                            .wrapping_mul(40503);
                        if !(h >> 7).is_multiple_of(3) {
                            let d = vec2((h % 13) as f32 - 6.0, -(((h >> 4) % 10) as f32) - 5.0);
                            painter.circle_filled(
                                at + d,
                                1.6,
                                egui::Color32::from_rgb(255, 220, 60),
                            );
                        }
                    }
                } else {
                    painter.circle_filled(
                        at,
                        13.0,
                        egui::Color32::from_rgba_unmultiplied(90, 200, 255, 40),
                    );
                }
            }

            let waiting = session.session_state() == SessionState::Waiting;
            if waiting {
                let pulse = (now * 3.0).sin() as f32 * 0.5 + 0.5;
                painter.circle_stroke(
                    at,
                    10.0 + 8.0 * pulse,
                    egui::Stroke::new(2.0, egui::Color32::YELLOW),
                );
            }

            let colour = if waiting {
                egui::Color32::YELLOW
            } else if placement.writing {
                egui::Color32::from_rgb(255, 90, 90)
            } else {
                egui::Color32::from_rgb(90, 200, 255)
            };

            if Some(session.id.as_str()) == self.selected.as_deref() {
                painter.circle_stroke(at, 10.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
            }
            painter.circle_filled(at, 4.5, colour);

            if click.is_some_and(|c| (c - at).length() <= 9.0) {
                clicked = Some(session.id.clone());
            }
            if double_click.is_some_and(|c| (c - at).length() <= 9.0) {
                to_open = Some((*session).clone());
            }

            let what = if waiting {
                "needs you"
            } else if placement.writing {
                "writing"
            } else {
                "reading"
            };
            let label_at = base + vec2(24.0, (k as f32 - (n as f32 - 1.0) / 2.0) * 15.0);
            let galley = painter.layout_no_wrap(
                format!("{} · {what}", session.name),
                egui::FontId::proportional(12.0),
                egui::Color32::WHITE,
            );
            let label_rect = egui::Align2::LEFT_CENTER
                .anchor_size(label_at, galley.size())
                .expand2(vec2(4.0, 1.0));
            painter.line_segment(
                [at, label_rect.left_center()],
                egui::Stroke::new(1.0, colour.gamma_multiply(0.6)),
            );
            painter.rect_filled(label_rect, 3.0, egui::Color32::from_black_alpha(170));
            painter.galley(
                label_rect.min + vec2(5.0, 1.0),
                galley,
                egui::Color32::WHITE,
            );

            anything_is_animating |= walking || !waiting;

            drawn.push(DrawnSession {
                id: session.id.clone(),
                tower: placement.tower.clone(),
                dot: at,
                label: label_rect,
                waiting,
                writing: placement.writing,
            });
        }

        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(if anything_is_animating {
                33
            } else {
                500
            }));

        if clicked.is_some() {
            self.selected = clicked.clone();
        }

        let opened_a_pane = to_open.is_some();
        if let Some(session) = to_open {
            self.open_pane(&session);
        }

        (drawn, clicked, opened_a_pane)
    }
}

/// `(ground position, height, still walking)` at `now`, for `anim`'s current
/// walk toward `anim.to`: a 1.5 s eased walk along x then y at ground level,
/// then a 0.4 s climb to `work_z`.
fn walk(anim: &Anim, now: f64, work_z: f32) -> (Pos2, f32, bool) {
    let smoothstep = |t: f64| (t * t * (3.0 - 2.0 * t)) as f32;
    let w = ((now - anim.started) / WALK_SECONDS).clamp(0.0, 1.0);
    if w < 1.0 {
        let (dx, dy) = (anim.to.x - anim.from.x, anim.to.y - anim.from.y);
        let d = smoothstep(w) * (dx.abs() + dy.abs());
        let ground = if d <= dx.abs() {
            pos2(anim.from.x + dx.signum() * d, anim.from.y)
        } else {
            pos2(anim.to.x, anim.from.y + dy.signum() * (d - dx.abs()))
        };
        (ground, 0.0, true)
    } else {
        let c = ((now - anim.started - WALK_SECONDS) / CLIMB_SECONDS).clamp(0.0, 1.0);
        (anim.to, work_z * smoothstep(c), false)
    }
}

/// One sidebar row: status dot, name, "repo · branch", up to two lines of
/// the last assistant text, state and age. Returns `(clicked, double_clicked)`
/// - a double-click opens the row's session in a pane (design.md
///   requirement 2).
fn session_row(
    ui: &mut egui::Ui,
    session: &crate::Session,
    selected: Option<&str>,
    now_seconds: u64,
) -> (bool, bool) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 60.0), egui::Sense::click());
    let state = session.session_state();
    let fill = if Some(session.id.as_str()) == selected {
        egui::Color32::from_white_alpha(28)
    } else if response.hovered() {
        egui::Color32::from_white_alpha(12)
    } else {
        egui::Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, fill);

    let dot_colour = match state {
        SessionState::Waiting => egui::Color32::YELLOW,
        SessionState::Working => egui::Color32::from_rgb(80, 200, 100),
        SessionState::Idle => egui::Color32::GRAY,
    };
    painter.circle_filled(rect.left_top() + vec2(10.0, 12.0), 5.0, dot_colour);

    painter.text(
        rect.left_top() + vec2(22.0, 2.0),
        egui::Align2::LEFT_TOP,
        &session.name,
        egui::FontId::proportional(13.0),
        egui::Color32::WHITE,
    );
    painter.text(
        rect.left_top() + vec2(22.0, 18.0),
        egui::Align2::LEFT_TOP,
        format!("{} · {}", session.repo, session.branch),
        egui::FontId::proportional(11.0),
        egui::Color32::GRAY,
    );

    let snippet: String = session.summary.chars().take(110).collect();
    let mut job = egui::text::LayoutJob::simple(
        snippet,
        egui::FontId::proportional(11.0),
        egui::Color32::LIGHT_GRAY,
        (rect.width() - 28.0).max(1.0),
    );
    job.wrap.max_rows = 2;
    let galley = painter.layout_job(job);
    painter.galley(
        rect.left_top() + vec2(22.0, 33.0),
        galley,
        egui::Color32::WHITE,
    );

    let state_word = match state {
        SessionState::Waiting => "needs you",
        SessionState::Working => "working",
        SessionState::Idle => "idle",
    };
    painter.text(
        rect.right_top() + vec2(-6.0, 2.0),
        egui::Align2::RIGHT_TOP,
        format!(
            "{state_word} · {}",
            crate::ago(session.last_active, now_seconds)
        ),
        egui::FontId::proportional(11.0),
        dot_colour,
    );

    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Button);
        node.set_label(session.name.clone());
    });

    // A triple click counts too: egui counts a second click on the row, soon
    // after an earlier one elsewhere, as the third of a run.
    (
        response.clicked(),
        response.double_clicked() || response.triple_clicked(),
    )
}

/// A read-only conversation's body: every entry, oldest first, `"> "`-
/// prefixed for a prompt, scrolled to the newest at the bottom (design.md
/// requirement 6).
fn conversation_body(ui: &mut egui::Ui, entries: &[Entry]) {
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for entry in entries {
                let prefix = if entry.kind == EntryKind::Prompt {
                    "> "
                } else {
                    ""
                };
                ui.label(format!("{prefix}{}", entry.text));
            }
        });
}

/// A terminal pane's body (design.md requirements 3 and 4): resizes the pty
/// to the available rect's size in cells every frame, forwards every
/// keystroke to the child while focused, and paints the vt100 screen's cells,
/// colours, inverse video and cursor in monospace.
fn terminal_body(ui: &mut egui::Ui, pane: &mut crate::SessionPane, want_focus: &mut bool) {
    let font = egui::FontId::monospace(13.0);
    let cell = ui
        .painter()
        .layout_no_wrap("M".to_owned(), font.clone(), egui::Color32::WHITE)
        .size();

    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let rows = ((rect.height() / cell.y).floor() as u16).max(1);
    let cols = ((rect.width() / cell.x).floor() as u16).max(1);
    pane.resize(rows, cols);

    let id = egui::Id::new(TERMINAL_ID);
    let response = ui.interact(rect, id, egui::Sense::click());
    if response.clicked() || *want_focus {
        ui.memory_mut(|m| m.request_focus(id));
        *want_focus = false;
    }
    let focused = ui.memory(|m| m.has_focus(id));

    if focused {
        for event in ui.input(|i| i.events.clone()) {
            let bytes = key_bytes(&event);
            if !bytes.is_empty() {
                pane.send(&bytes);
            }
        }
    }

    let painter = ui.painter_at(rect);
    let default_bg = cellworks_map::ui::case::BG_DEEP;
    let default_fg = cellworks_map::ui::case::INK;
    painter.rect_filled(rect, 0.0, default_bg);

    let screen = pane
        .screen()
        .expect("terminal_body is only called once pane.screen() is Some");
    for row in 0..screen.size().0.get() {
        // A run of consecutive cells on this row sharing one fg/bg pair, so
        // one `painter.text` call draws a whole stretch of plain text rather
        // than one call per cell.
        let mut run: Option<(u16, String, egui::Color32, egui::Color32)> = None;
        let flush = |run: &mut Option<(u16, String, egui::Color32, egui::Color32)>| {
            let Some((start_col, text, fg, bg)) = run.take() else {
                return;
            };
            let at = rect.min + vec2(start_col as f32 * cell.x, row as f32 * cell.y);
            let width = text.chars().count() as f32 * cell.x;
            if bg != default_bg {
                painter.rect_filled(Rect::from_min_size(at, vec2(width, cell.y)), 0.0, bg);
            }
            if !text.trim().is_empty() {
                painter.text(at, egui::Align2::LEFT_TOP, &text, font.clone(), fg);
            }
        };

        for col in 0..screen.size().1.get() {
            let Some(vt_cell) = screen.cell(row, col) else {
                continue;
            };
            if vt_cell.is_wide_continuation() {
                continue;
            }
            let (mut fg, mut bg) = (
                xterm_color(vt_cell.fgcolor(), default_fg),
                xterm_color(vt_cell.bgcolor(), default_bg),
            );
            if vt_cell.inverse() {
                std::mem::swap(&mut fg, &mut bg);
            }
            let ch = if vt_cell.has_contents() {
                vt_cell.contents().to_owned()
            } else {
                " ".to_owned()
            };
            match &mut run {
                Some((_, text, f, b)) if *f == fg && *b == bg => text.push_str(&ch),
                _ => {
                    flush(&mut run);
                    run = Some((col, ch, fg, bg));
                }
            }
        }
        flush(&mut run);
    }

    if !screen.hide_cursor() {
        let (row, col) = screen.cursor_position();
        let at = rect.min + vec2(col as f32 * cell.x, row as f32 * cell.y);
        let block = Rect::from_min_size(at, cell);
        if focused {
            painter.rect_filled(block, 0.0, egui::Color32::from_white_alpha(150));
        } else {
            painter.rect_stroke(
                block,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::GRAY),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// A vt100 cell colour to `egui::Color32`: `default` for the terminal's own
/// default, straight through for an RGB colour, and [`xterm`]'s 256-colour
/// palette for an indexed one.
fn xterm_color(colour: vt100::Color, default: egui::Color32) -> egui::Color32 {
    match colour {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => egui::Color32::from_rgb(r, g, b),
        vt100::Color::Idx(n) => xterm(n),
    }
}

/// The xterm 256-colour palette: the 16 basic colours, the 6×6×6 colour
/// cube, then a 24-step greyscale (design.md requirement 3).
fn xterm(n: u8) -> egui::Color32 {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    match n {
        0..16 => {
            let (r, g, b) = BASE[n as usize];
            egui::Color32::from_rgb(r, g, b)
        }
        16..232 => {
            let k = n - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            egui::Color32::from_rgb(level(k / 36), level(k / 6 % 6), level(k % 6))
        }
        _ => egui::Color32::from_gray(8 + 10 * (n - 232)),
    }
}

/// Turns one egui input event into the bytes a focused terminal pane must
/// receive (design.md requirement 4): plain text and paste verbatim; Enter,
/// Backspace, Tab, Esc and the arrows as their usual control sequences;
/// Ctrl+ a single ASCII letter as that letter's control code; and egui's own
/// Copy/Cut events as Ctrl+C/Ctrl+X (0x03/0x18), since egui turns those key
/// combinations into clipboard events before this code ever sees them as
/// `Event::Key`. Every other event - including a key release, or a plain
/// unmodified letter, whose own `Event::Text` already carries it - sends
/// nothing.
fn key_bytes(event: &egui::Event) -> Vec<u8> {
    match event {
        egui::Event::Text(t) | egui::Event::Paste(t) => t.as_bytes().to_vec(),
        egui::Event::Copy => vec![0x03],
        egui::Event::Cut => vec![0x18],
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            use egui::Key::{
                ArrowDown, ArrowLeft, ArrowRight, ArrowUp, Backspace, Enter, Escape, Tab,
            };
            match key {
                Enter => b"\r".to_vec(),
                Backspace => vec![0x7f],
                Tab => b"\t".to_vec(),
                Escape => vec![0x1b],
                ArrowUp => b"\x1b[A".to_vec(),
                ArrowDown => b"\x1b[B".to_vec(),
                ArrowRight => b"\x1b[C".to_vec(),
                ArrowLeft => b"\x1b[D".to_vec(),
                _ => {
                    let name = key.name();
                    if modifiers.ctrl && name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase()
                    {
                        vec![name.as_bytes()[0] - b'A' + 1]
                    } else {
                        Vec::new()
                    }
                }
            }
        }
        _ => Vec::new(),
    }
}

/// Prototype: the pane's frame in the Netrunner skin.
fn pane_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(cellworks_map::ui::case::PANEL)
        .stroke(egui::Stroke::new(1.0, cellworks_map::ui::case::LINE))
        .inner_margin(egui::Margin::symmetric(12, 8))
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Plain,
    Danger,
}

/// Prototype: a Netrunner control: upper case, chamfered top left and bottom
/// right, outlined.
fn nr_ctl(ui: &mut egui::Ui, label: &str, kind: Kind) -> egui::Response {
    use cellworks_map::ui::case::{DANGER, DANGER_TEXT, FLOW, INK, LINE_STRONG, PANEL_RAISED};
    let font = egui::FontId::new(11.0, egui::FontFamily::Name("semibold".into()));
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, INK);
    let size = egui::vec2(galley.size().x + 22.0 + label.len() as f32 * 1.2, 26.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let (edge, ink) = match (kind, response.hovered()) {
        (Kind::Danger, _) => (DANGER, DANGER_TEXT),
        (Kind::Plain, true) => (FLOW, INK),
        (Kind::Plain, false) => (LINE_STRONG, INK),
    };
    let fill = if response.hovered() {
        PANEL_RAISED
    } else {
        egui::Color32::TRANSPARENT
    };
    chamfer(ui.painter(), rect, fill, edge, 6.0);
    let mut job = egui::text::LayoutJob::single_section(
        label.to_owned(),
        egui::TextFormat {
            font_id: egui::FontId::new(11.0, egui::FontFamily::Name("semibold".into())),
            color: ink,
            extra_letter_spacing: 1.2,
            ..Default::default()
        },
    );
    job.wrap.max_width = f32::INFINITY;
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ink);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response
}

fn chamfer(
    painter: &egui::Painter,
    rect: egui::Rect,
    fill: egui::Color32,
    edge: egui::Color32,
    cut: f32,
) {
    let r = rect.shrink(0.5);
    let points = vec![
        egui::pos2(r.min.x + cut, r.min.y),
        egui::pos2(r.max.x, r.min.y),
        egui::pos2(r.max.x, r.max.y - cut),
        egui::pos2(r.max.x - cut, r.max.y),
        egui::pos2(r.min.x, r.max.y),
        egui::pos2(r.min.x, r.min.y + cut),
    ];
    painter.add(egui::Shape::convex_polygon(
        points,
        fill,
        egui::Stroke::new(1.0, edge),
    ));
}

#[cfg(test)]
mod tests {
    use egui::{Event, Key, Modifiers};

    use super::key_bytes;

    fn key_event(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    /// Pins [`key_bytes`] (design.md requirement 4): plain text and Enter,
    /// Backspace, Tab, Esc and the arrows each become their own bytes; egui
    /// turning Ctrl+C and Ctrl+X into its own clipboard events still reaches
    /// the session as 0x03/0x18 (the prototype's confirmed edge case); and a
    /// plain, unmodified letter key carries no control byte of its own - the
    /// character itself arrives separately as `Event::Text`.
    #[test]
    fn keys_become_terminal_bytes() {
        assert_eq!(key_bytes(&Event::Text("q".to_owned())), b"q".to_vec());
        assert_eq!(
            key_bytes(&Event::Paste("pasted".to_owned())),
            b"pasted".to_vec()
        );

        assert_eq!(
            key_bytes(&key_event(Key::Enter, Modifiers::NONE)),
            vec![b'\r']
        );
        assert_eq!(
            key_bytes(&key_event(Key::Backspace, Modifiers::NONE)),
            vec![0x7f]
        );
        assert_eq!(
            key_bytes(&key_event(Key::ArrowUp, Modifiers::NONE)),
            b"\x1b[A".to_vec(),
            "arrows become their usual ESC [ letter sequence"
        );

        assert_eq!(
            key_bytes(&Event::Copy),
            vec![0x03],
            "Ctrl+C must still reach the session even when egui turns it into a Copy event"
        );
        assert_eq!(
            key_bytes(&Event::Cut),
            vec![0x18],
            "Ctrl+X must still reach the session even when egui turns it into a Cut event"
        );

        assert_eq!(
            key_bytes(&key_event(Key::A, Modifiers::CTRL)),
            vec![0x01],
            "Ctrl+A is that letter's control code"
        );
        assert!(
            key_bytes(&key_event(Key::A, Modifiers::NONE)).is_empty(),
            "a plain, unmodified letter key carries no control byte - its own \
             Event::Text does the typing"
        );
    }
}
