//! The sessions sidebar and the map overlay: drawn with egui, so covered by
//! the binary's acceptance tests (`tests/story_030_sessions_walk_to_the_towers_they_work_on.rs`)
//! rather than by unit tests here.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::{Pos2, Rect, pos2, vec2};

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
        }
    }

    /// Watches every running session on the machine, from `sources`.
    pub fn watching(sources: crate::SessionSources) -> SessionsView {
        SessionsView {
            watch: Some(crate::start_watch(sources)),
            has_sources: true,
            ..SessionsView::none()
        }
    }

    /// Showing `sessions`, watching nothing further - the test seam
    /// `CellworksApp::with_sessions` uses.
    pub fn with_sessions(sessions: Vec<crate::Session>) -> SessionsView {
        SessionsView {
            sessions,
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
        let groups = crate::groups(&self.sessions, map, common_dir, &self.query);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (heading, rows) in groups {
                ui.label(egui::RichText::new(heading).strong());
                for session in rows {
                    if session_row(ui, session, self.selected.as_deref(), now_seconds) {
                        self.selected = Some(session.id.clone());
                        let centre = place(map, common_dir, session).map(|p| p.centre);
                        clicked = Some((session.id.clone(), centre));
                    }
                }
            }
        });

        clicked
    }

    /// Draws every session over the already-drawn map; returns what was
    /// drawn and the id of a session whose dot was clicked.
    pub fn paint(
        &mut self,
        ui: &egui::Ui,
        map: &cellworks_map::ProjectMap,
        common_dir: Option<&std::path::Path>,
        view: &cellworks_map::View,
    ) -> (Vec<DrawnSession>, Option<String>) {
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

        let mut drawn = Vec::new();
        let mut clicked = None;
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

        (drawn, clicked)
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
/// the last assistant text, state and age. Returns whether it was clicked.
fn session_row(
    ui: &mut egui::Ui,
    session: &crate::Session,
    selected: Option<&str>,
    now_seconds: u64,
) -> bool {
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

    response.clicked()
}
