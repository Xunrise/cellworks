//! PROTOTYPE (stakeholder demo): everything here is gated on the
//! `CELLWORKS_DEMO` environment variable, so normal mode is unchanged.
//!
//! The demo runs on a throwaway world under `${TMPDIR}cellworks-demo`, built
//! by `demo/setup.sh`: the sample project, two decoys, Jira seeds, case
//! snapshots and a transcripts folder the scripted agents write into. A guided
//! tour (chapters 0 to 11) lies over the app; its Next button runs the
//! chapter's drive lines, one beat at a time. The same commands work from a
//! `CELLWORKS_DRIVE` file.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cellworks_agent_sessions::{Agent, AgentList, ClaudeUnavailable, SessionSources, parse_agents};
use cellworks_workspace::{HostProbe, Project, Sources};
use serde_json::{Value, json};

use crate::app::CellworksApp;

/// A widget reports its place, by id, for the tutorial's highlight.
pub fn mark(ctx: &egui::Context, id: &str, rect: egui::Rect) {
    cellworks_workspace::mark(ctx, id, rect);
}

/// The gap between a floating overlay and its neighbours.
const GAP: f32 = 12.0;

/// What the side panels, the Explorer, the right column and the dock leave
/// of the map this frame; `None` while a case shows its own chrome.
pub fn free_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    cellworks_workspace::marked(ctx, "map.free")
}

/// The rects the floating map overlays hold: lens caption, lane legend,
/// minimap, history bar and the Factory strip.
pub fn overlay_rects(ctx: &egui::Context) -> Vec<egui::Rect> {
    let mut rects = cellworks_workspace::marked_all(ctx, "slot.");
    rects.extend(cellworks_workspace::marked(ctx, "pipeline"));
    rects
}

/// Moves `rect` up (or down) past every rect in `taken` it overlaps.
fn clear_of(mut rect: egui::Rect, taken: &[egui::Rect], up: bool) -> egui::Rect {
    // ponytail: a few passes settle the handful of overlays the map has.
    for _ in 0..taken.len() + 1 {
        let Some(hit) = taken.iter().find(|t| t.intersects(rect)) else {
            break;
        };
        let dy = if up {
            hit.top() - GAP - rect.bottom()
        } else {
            hit.bottom() + GAP - rect.top()
        };
        rect = rect.translate(egui::vec2(0.0, dy));
    }
    rect
}

/// Where a `size` overlay centred on the top (or bottom) edge of `free` goes,
/// clear of the overlays named in `others`.
fn slot(ctx: &egui::Context, free: egui::Rect, size: egui::Vec2, top: bool, others: &[&str]) -> egui::Rect {
    let x = (free.center().x - size.x / 2.0)
        .min(free.right() - GAP - size.x)
        .max(free.left() + GAP);
    let y = if top {
        free.top() + GAP
    } else {
        free.bottom() - GAP - size.y
    };
    let taken: Vec<egui::Rect> = others
        .iter()
        .filter_map(|id| cellworks_workspace::marked(ctx, id))
        .collect();
    clear_of(egui::Rect::from_min_size(egui::pos2(x, y), size), &taken, !top)
}

pub fn is_demo() -> bool {
    std::env::var_os("CELLWORKS_DEMO").is_some()
}

/// The demo world.
pub fn root() -> PathBuf {
    let tmp = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned());
    let dir = PathBuf::from(tmp.trim_end_matches('/')).join("cellworks-demo");
    dir.canonicalize().unwrap_or(dir)
}

/// The demo's bundled files: `CELLWORKS_DEMO_ASSETS` (set by the packaged app),
/// else `demo/` in this source tree.
fn assets() -> PathBuf {
    std::env::var_os("CELLWORKS_DEMO_ASSETS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("demo"))
}

/// Builds the world when it is missing, and points the Jira and case readers
/// at it. Called once, before any window or thread exists.
pub fn init() {
    if !is_demo() {
        return;
    }
    if !root().join("projects").is_dir() {
        let script = assets().join("setup.sh");
        match Command::new("bash").arg(&script).status() {
            Ok(status) if status.success() => {}
            other => eprintln!("demo: setup.sh failed: {other:?}"),
        }
    }
    let root = root();
    // SAFETY: nothing else is running yet; this is the first thing `run` does.
    unsafe {
        std::env::set_var("CELLWORKS_JIRA_OFFLINE", "1");
        std::env::set_var("CELLWORKS_JIRA_CACHE", root.join("jira"));
        std::env::set_var("CELLWORKS_CASES", root.join("cases"));
    }
}

pub fn sources() -> Option<Sources> {
    is_demo().then(|| Sources {
        projects_folder: root().join("projects"),
        settings_file: root().join("settings.toml"),
    })
}

pub fn session_sources() -> Option<SessionSources> {
    is_demo().then(|| SessionSources {
        transcripts: root().join("transcripts"),
        agents: Arc::new(DemoAgents(root().join("agents.json"))),
        poll: Duration::from_millis(500),
        claude: PathBuf::from("false"),
    })
}

/// The `staging` host chip shows as connected.
pub fn probe() -> Option<Arc<dyn HostProbe + Send + Sync>> {
    struct Up;
    impl HostProbe for Up {
        fn connected(&self, _host: &str) -> bool {
            true
        }
    }
    is_demo().then(|| Arc::new(Up) as Arc<dyn HostProbe + Send + Sync>)
}

pub fn tour() -> Option<Tour> {
    is_demo().then(Tour::default)
}

/// The agents the tour has started, from `agents.json`.
struct DemoAgents(PathBuf);

impl AgentList for DemoAgents {
    fn list(&self) -> Result<Vec<Agent>, ClaudeUnavailable> {
        Ok(std::fs::read(&self.0)
            .map(|bytes| parse_agents(&bytes))
            .unwrap_or_default())
    }
}

// ---------------------------------------------------------------- the tour

mod tutorial;

pub use tutorial::{Info as TutorialInfo, Phase as TutorialPhase};

const CORE: [(&str, &str); 2] = [
    (
        "The Prototype Driven Development loop",
        "A ticket becomes a running prototype you steer, then a frozen plan the pipeline builds, with every step visible on the map.",
    ),
    (
        "Perspective lenses",
        "Ask Claude a question about a project and the map reshapes itself to answer it: per project, per person.",
    ),
];

#[derive(Default)]
pub struct Tour {
    tut: tutorial::State,
    queue: VecDeque<String>,
    resume: Option<Instant>,
    servers: Vec<Child>,
    /// The Factory strip: the stage running, `None` while it is hidden.
    pipeline: Option<usize>,
    pipeline_done: bool,
    /// The engineer dragged the strip: it stays where they put it.
    pipeline_moved: bool,
    /// When the deploy packet left the Depot, in egui time.
    deploy_at: Option<f64>,
    history: Option<History>,
}

const STAGES: [&str; 5] = ["BUILD", "CHECKS", "DELIVER", "REVIEW", "DEPLOY"];
const TRAVEL: f64 = 2.5;

/// The history scrubber: the commits of the shop's main branch, oldest first,
/// and the one `shopfront-history` shows.
struct History {
    commits: Vec<(String, String)>,
    idx: usize,
    applied: Option<usize>,
    last: Instant,
}

impl Drop for Tour {
    fn drop(&mut self) {
        for server in &mut self.servers {
            let _ = Command::new("kill")
                .args(["-TERM", &format!("-{}", server.id())])
                .status();
            let _ = server.kill();
        }
    }
}

/// Test driver: what the tutorial shows now.
pub fn tutorial_info(app: &CellworksApp) -> Option<tutorial::Info> {
    app.demo.as_ref()?.tut.info.clone()
}

/// Test driver: the chapter's title.
pub fn chapter_title(chapter: usize) -> &'static str {
    tutorial::CHAPTERS[chapter].title
}

pub fn chapter_count() -> usize {
    tutorial::CHAPTERS.len()
}

/// Test driver: case stage, open project and lens, as one line.
pub fn status(app: &CellworksApp) -> String {
    let project = app
        .opened()
        .and_then(|o| o.root().map(|r| r.display().to_string()));
    format!(
        "case stage {:?}, project {}, lens {:?}, lens panel {}",
        app.case.as_ref().map(|c| c.stage()),
        project.as_deref().unwrap_or("-"),
        app.opened().map(|o| o.lens()),
        app.lens_panel.open
    )
}

/// Test driver: nothing queued and no `wait` pending.
pub fn idle(app: &CellworksApp) -> bool {
    app.demo
        .as_ref()
        .is_none_or(|t| t.queue.is_empty() && t.resume.is_none())
}

/// Runs what the tour has queued, and draws the tutorial: N does the
/// current step, B goes back a chapter, Esc hides or restores the overlay.
pub fn overlay(ui: &mut egui::Ui, app: &mut CellworksApp) {
    let Some(tour) = app.demo.as_mut() else {
        return;
    };
    let mut due = Vec::new();
    if tour.resume.is_none_or(|at| Instant::now() >= at) {
        tour.resume = None;
        while let Some(line) = tour.queue.pop_front() {
            if let Some(ms) = line.strip_prefix("wait ") {
                tour.resume =
                    Some(Instant::now() + Duration::from_millis(ms.trim().parse().unwrap_or(0)));
                break;
            }
            due.push(line.replace("{ROOT}", &root().display().to_string()));
        }
    }

    let ctx = ui.ctx().clone();
    let typing = ctx.egui_wants_keyboard_input();
    let key = |k| !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
    let next = key(egui::Key::N);
    let back = key(egui::Key::B);
    let esc = !typing && ui.input(|i| i.key_pressed(egui::Key::Escape));
    // Run the lines first: a step that was just prepared sees their effect.
    for line in due {
        crate::app::drive_line(ui, app, &line);
    }
    tutorial::run(&ctx, app, next, back, esc);
    let Some(tour) = app.demo.as_mut() else {
        return;
    };
    pipeline_strip(ui, tour);
    let close = history_bar(ui, tour);
    if close {
        history_close(app);
    }
    history_tick(ui, app);
}

/// Top-centre strip: BUILD, CHECKS, DELIVER, REVIEW, DEPLOY. It takes the
/// top of the free map, below the lens caption when both do not fit side by
/// side, and the engineer can drag it anywhere on the free map.
fn pipeline_strip(ui: &egui::Ui, tour: &mut Tour) {
    let Some(running) = tour.pipeline else {
        return;
    };
    let ctx = ui.ctx();
    let pulse = ((ctx.input(|i| i.time) * 4.0).sin() * 0.5 + 0.5) as f32;
    let free = free_rect(ctx).unwrap_or(ctx.content_rect());
    let size = cellworks_workspace::marked(ctx, "pipeline")
        .map_or(egui::vec2(640.0, 34.0), |r| r.size());
    let mut area = egui::Area::new(egui::Id::new("demo-pipeline"))
        .order(egui::Order::Foreground)
        .movable(true)
        .constrain_to(free);
    if !tour.pipeline_moved {
        area = area.current_pos(slot(ctx, free, size, true, &["slot.caption"]).min);
    }
    let strip = area.show(ctx, |ui| {
            use cellworks_workspace::theme;
            theme::frame().show(ui, |ui| {
                // Narrower than the strip, the free map wraps it.
                ui.set_max_width(free.width() - 2.0 * GAP - 24.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    theme::eyebrow_label(ui, "The Factory", theme::YELLOW);
                    theme::rule(ui);
                    for (i, name) in STAGES.iter().enumerate() {
                        let (text, colour) = if tour.pipeline_done || i < running {
                            (format!("\u{2714} {name}"), theme::OK)
                        } else if i == running {
                            (
                                format!("\u{25CF} {name}"),
                                theme::YELLOW.gamma_multiply(0.6 + 0.4 * pulse),
                            )
                        } else {
                            (format!("\u{25CB} {name}"), theme::TERRAIN_INK)
                        };
                        // Egui's own family: it falls back to the glyphs for the marks.
                        ui.label(
                            egui::RichText::new(text)
                                .size(11.5)
                                .extra_letter_spacing(1.5)
                                .color(colour),
                        );
                    }
                });
            });
        });
    // A click without movement leaves it in its slot.
    if strip.response.dragged() && strip.response.drag_delta() != egui::Vec2::ZERO {
        tour.pipeline_moved = true;
    }
    if strip.response.hovered() {
        ctx.set_cursor_icon(egui::CursorIcon::Grab);
    }
    mark(ctx, "pipeline", strip.response.rect);
    if !tour.pipeline_done {
        ctx.request_repaint_after(Duration::from_millis(60));
    }
}

fn shop() -> PathBuf {
    root().join("projects/shopfront")
}

fn history_dir() -> PathBuf {
    root().join("projects/shopfront-history")
}

/// Bottom slider over the commits of main. `true` when it was closed.
fn history_bar(ui: &egui::Ui, tour: &mut Tour) -> bool {
    let Some(h) = &mut tour.history else {
        return false;
    };
    let mut close = false;
    let ctx = ui.ctx();
    // Bottom centre of the free map, right above the dock; lifted over the
    // lane legend or the minimap when the map is too narrow to pass them.
    let free = free_rect(ctx).unwrap_or(ctx.content_rect());
    let width = (free.width() - 2.0 * GAP - 24.0).clamp(240.0, 560.0);
    let size = cellworks_workspace::marked(ctx, "slot.history")
        .map_or(egui::vec2(width + 24.0, 96.0), |r| egui::vec2(width + 24.0, r.height()));
    let at = slot(ctx, free, size, false, &["slot.minimap", "slot.legend"]);
    let shown = egui::Window::new("History")
        .title_bar(false)
        .order(egui::Order::Foreground)
        .fixed_pos(at.min)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            let last = h.commits.len().saturating_sub(1);
            ui.set_width(width);
            use cellworks_workspace::theme;
            ui.horizontal(|ui| {
                theme::eyebrow_label(ui, "History", theme::YELLOW);
                ui.label(
                    egui::RichText::new(format!("{}/{}", h.idx + 1, h.commits.len()))
                        .monospace()
                        .size(11.0)
                        .color(theme::DIM),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::tab(ui, "Close", false, false).clicked() {
                        close = true;
                    }
                });
            });
            ui.spacing_mut().slider_width = width - 20.0;
            ui.visuals_mut().widgets.inactive.bg_fill = theme::LINE_BRIGHT;
            ui.visuals_mut().widgets.inactive.fg_stroke.color = theme::YELLOW;
            let slider = ui.add(egui::Slider::new(&mut h.idx, 0..=last).show_value(false));
            mark(ui.ctx(), "history.slider", slider.rect);
            if last > 0 {
                let r = slider.rect;
                let y = r.bottom() + 1.0;
                for i in 0..=last {
                    let x = r.left() + 8.0 + (r.width() - 16.0) * i as f32 / last as f32;
                    let colour = if i == h.idx {
                        theme::YELLOW
                    } else {
                        theme::LINE_BRIGHT
                    };
                    ui.painter().line_segment(
                        [egui::pos2(x, y), egui::pos2(x, y + 5.0)],
                        egui::Stroke::new(1.0, colour),
                    );
                }
                ui.add_space(6.0);
            }
            if let Some((sha, subject)) = h.commits.get(h.idx) {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&sha[..sha.len().min(7)])
                            .monospace()
                            .color(theme::CYAN),
                    );
                    ui.label(
                        egui::RichText::new(subject.lines().next().unwrap_or("")).color(theme::INK),
                    );
                });
            }
        });
    if let Some(shown) = shown {
        mark(ctx, "slot.history", shown.response.rect);
    }
    close
}

/// Checks the chosen commit out into `shopfront-history` and shows it, at
/// most every 300 ms so a drag does not queue reads.
fn history_tick(ui: &egui::Ui, app: &mut CellworksApp) {
    let Some(h) = app.demo.as_mut().and_then(|t| t.history.as_mut()) else {
        return;
    };
    if h.applied == Some(h.idx) {
        return;
    }
    if h.last.elapsed() < Duration::from_millis(300) {
        ui.ctx().request_repaint_after(Duration::from_millis(100));
        return;
    }
    let dir = history_dir();
    let sha = h.commits[h.idx].0.clone();
    h.applied = Some(h.idx);
    h.last = Instant::now();
    git(&dir, &["checkout", "-q", "--detach", &sha]);
    open_or_rescan(app, &dir);
}

fn open_or_rescan(app: &mut CellworksApp, dir: &Path) {
    let open = app
        .opened()
        .as_ref()
        .and_then(|o| o.project().id.local_path().map(Path::to_owned));
    if open.as_deref() == Some(dir) {
        app.rescan();
    } else {
        app.open(Project::at(dir.to_owned()));
    }
}

fn history_close(app: &mut CellworksApp) {
    if let Some(tour) = &mut app.demo {
        tour.history = None;
    }
    app.open(Project::at(shop()));
}

/// `history` toggles the bar; `history <n>` or `history last` opens it at that commit.
fn history(app: &mut CellworksApp, arg: &str) {
    let open = app.demo.as_ref().is_some_and(|t| t.history.is_some());
    if open && arg.is_empty() {
        history_close(app);
        return;
    }
    let Some(tour) = &mut app.demo else {
        return;
    };
    if tour.history.is_none() {
        let out = Command::new("git")
            .current_dir(shop())
            .args(["log", "--reverse", "--format=%H%x09%s", "main"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        let commits: Vec<(String, String)> = out
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        if commits.is_empty() {
            return;
        }
        tour.history = Some(History {
            idx: commits.len() - 1,
            commits,
            applied: None,
            last: Instant::now() - Duration::from_secs(1),
        });
    }
    if let Some(h) = &mut tour.history {
        if arg == "last" {
            h.idx = h.commits.len() - 1;
        } else if let Ok(n) = arg.parse::<usize>() {
            h.idx = n.min(h.commits.len() - 1);
        }
        h.last = Instant::now() - Duration::from_secs(1);
    }
}

/// Paints what only the demo has in the city: the staging server and the
/// deploy packet, in the map's own projection. Called over the map.
pub fn paint_world(
    ui: &egui::Ui,
    tour: &mut Tour,
    map: &cellworks_map::ProjectMap,
    view: &cellworks_map::View,
) {
    use egui::{Color32, Stroke, pos2, vec2};
    if map.root.file_name().is_none_or(|n| n != "shopfront") {
        return;
    }
    let now = ui.input(|i| i.time);
    let depot = map
        .find(&map.root.join("Dockerfile"))
        .map(|f| f.area.center())
        .unwrap_or_else(|| map.extent.left_center());
    let ground = pos2(map.extent.right() + 30.0, depot.y);
    let painter = ui.painter();

    // Packet: depot to server, with a short arc and a fading trail.
    let at = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        let e = (t * t * (3.0 - 2.0 * t)) as f32;
        let g = depot + (ground - depot) * e;
        view.to_screen(g, 30.0 * (std::f32::consts::PI * e).sin())
    };
    let progress = tour.deploy_at.map(|start| (now - start) / TRAVEL);
    let arrived = progress.is_some_and(|p| p >= 1.0);
    if let Some(p) = progress
        && (0.0..1.0).contains(&p)
    {
        for k in (1..14).rev() {
            let fade = 1.0 - k as f32 / 14.0;
            painter.circle_filled(
                at(p - k as f64 * 0.012),
                4.0 * fade + 1.0,
                Color32::from_white_alpha((150.0 * fade) as u8),
            );
        }
        painter.circle_filled(
            at(p),
            9.0,
            Color32::from_rgba_unmultiplied(90, 200, 255, 60),
        );
        painter.circle_filled(at(p), 5.0, Color32::from_rgb(180, 240, 255));
    }
    if arrived && tour.pipeline == Some(4) {
        tour.pipeline_done = true;
    }
    let since = progress.map_or(f64::MAX, |p| (p - 1.0) * TRAVEL);

    // Server: a rack tower with blinking lights.
    let base = view.to_screen(ground, 0.0);
    let w = (view.to_screen(ground + vec2(7.0, 0.0), 0.0) - base)
        .length()
        .max(10.0);
    let h = (base - view.to_screen(ground, 28.0)).length().max(26.0);
    let body = egui::Rect::from_min_max(base + vec2(-w, -h), base + vec2(w, 0.0));
    mark(ui.ctx(), "server", body.expand(6.0));
    painter.rect_filled(
        body.translate(vec2(5.0, 3.0)),
        2.0,
        Color32::from_black_alpha(70),
    );
    painter.rect_filled(body, 2.0, Color32::from_rgb(38, 44, 56));
    painter.rect_stroke(
        body,
        2.0,
        Stroke::new(1.5, Color32::from_rgb(120, 140, 170)),
        egui::StrokeKind::Inside,
    );
    for i in 0..5 {
        let y = body.top() + h * (i as f32 + 0.5) / 5.0;
        painter.line_segment(
            [
                pos2(body.left() + 3.0, y - h / 10.0),
                pos2(body.right() - 3.0, y - h / 10.0),
            ],
            Stroke::new(1.0, Color32::from_rgb(70, 80, 100)),
        );
        let on = ((now * 3.0 + i as f64 * 1.9).sin() > -0.3) as u8;
        let colour = if on == 1 {
            if i % 2 == 0 {
                Color32::from_rgb(90, 220, 120)
            } else {
                Color32::from_rgb(255, 190, 60)
            }
        } else {
            Color32::from_rgb(50, 60, 70)
        };
        painter.circle_filled(pos2(body.left() + w * 0.5, y), 2.2, colour);
        painter.circle_filled(
            pos2(body.left() + w * 0.5 + 7.0, y),
            2.2,
            Color32::from_rgb(90, 220, 120).gamma_multiply(0.6),
        );
    }

    // Version label, flashing when the deploy lands.
    let version = if arrived { "v1.4.0" } else { "v1.3.0" };
    let flash = (1.0 - since / 0.9).clamp(0.0, 1.0) as f32;
    let top = body.center_top() - vec2(0.0, 14.0);
    if flash > 0.0 {
        painter.circle_stroke(
            body.center_top(),
            12.0 + 40.0 * (1.0 - flash),
            Stroke::new(
                2.0,
                Color32::from_rgba_unmultiplied(120, 255, 160, (200.0 * flash) as u8),
            ),
        );
    }
    painter.text(
        top,
        egui::Align2::CENTER_BOTTOM,
        version,
        egui::FontId::proportional(15.0),
        if flash > 0.0 {
            Color32::from_rgb(160, 255, 190)
        } else {
            Color32::WHITE
        },
    );
    painter.text(
        base + vec2(0.0, 4.0),
        egui::Align2::CENTER_TOP,
        "staging",
        egui::FontId::proportional(12.0),
        Color32::from_rgb(170, 180, 200),
    );
    ui.ctx().request_repaint_after(Duration::from_millis(60));
}

// ---------------------------------------------------------------- commands

/// The demo's own drive commands. `true` when `line` was one of them.
pub fn command(ui: &mut egui::Ui, app: &mut CellworksApp, line: &str) -> bool {
    let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
    match cmd {
        "tour-next" => {
            if let Some(tour) = &mut app.demo {
                tour.tut.press_n = true;
            }
        }
        "tour-go" => {
            if let (Some(tour), Ok(chapter)) = (&mut app.demo, arg.trim().parse::<usize>()) {
                tour.tut.go_chapter(chapter);
            }
        }
        "fit" => app.navigation.fit(),
        "tree" => app.tree_open = !app.tree_open,
        "lens-next" => {
            let now = ui.input(|i| i.time);
            if let Some(opened) = &mut app.opened {
                let next = opened.next_lens();
                opened.set_lens(next, now);
            }
        }
        "lanes" => {
            if let Some(opened) = &mut app.opened {
                if arg.trim() == "imports" {
                    opened.toggle_import_lanes();
                } else {
                    opened.toggle_call_lanes();
                }
            }
        }
        "tickets" => app.tickets_open = !app.tickets_open,
        "lens-panel" => app.lens_panel.open = arg.trim() == "on",
        "lens-question" => app.lens_panel.question = arg.to_owned(),
        "case-freeze" | "case-approve" | "case-review" | "case-ingest" => {
            let now = ui.input(|i| i.time);
            if let Some(case) = &mut app.case {
                match cmd {
                    "case-freeze" => case.freeze(now),
                    "case-approve" => case.approve_act(now),
                    "case-ingest" => case.ingest_act(now),
                    _ => case.review_act(now),
                }
            }
        }
        "rescan" => app.rescan(),
        "agent" => {
            let (name, file) = arg.split_once(' ').unwrap_or((arg, ""));
            agent(name, file);
        }
        "write" => {
            let payload = assets().join("agent").join(arg);
            run(Command::new("cp")
                .arg("-R")
                .arg(format!("{}/.", payload.display()))
                .arg(workdir()));
            app.rescan();
        }
        "commit" => {
            let dir = workdir();
            git(&dir, &["add", "-A"]);
            git(
                &dir,
                &[
                    "-c",
                    "user.name=Factory Agent",
                    "-c",
                    "user.email=factory@shopfront.example",
                    "commit",
                    "-q",
                    "-m",
                    &arg.replace("\\n", "\n"),
                ],
            );
            app.rescan();
        }
        "round-close" => {
            if let Some(it) = &mut app.iterate {
                it.close_round();
            }
        }
        "case-open" => {
            let now = ui.input(|i| i.time);
            app.open_case(arg.trim(), now);
            // The lens panel would stand behind the case sheets.
            app.lens_panel.open = false;
        }
        "case-close" => app.case = None,
        "merge-case" => git(
            &root().join("projects/shopfront"),
            &[
                "merge",
                "--no-ff",
                "-q",
                "-m",
                "Merge SHOP-42: apply a promo code at checkout",
                "proto/shop-42-shoppers-cannot-apply-promo",
            ],
        ),
        "case-file" => {
            let from = root().join(format!("case-snapshots/SHOP-42.{arg}.json"));
            if let Err(error) = std::fs::copy(&from, root().join("cases/SHOP-42.json")) {
                eprintln!("demo: case-file {arg}: {error}");
            }
        }
        "devserver" => {
            if let Some(tour) = &mut app.demo
                && tour.servers.is_empty()
                && let Some(child) = devserver()
            {
                tour.servers.push(child);
            }
        }
        "app-window" => app_window(),
        "pipeline" => {
            if let Some(tour) = &mut app.demo {
                tour.pipeline_done = arg == "done";
                tour.pipeline = Some(
                    STAGES
                        .iter()
                        .position(|s| s.eq_ignore_ascii_case(arg))
                        .unwrap_or(0),
                );
            }
        }
        "deploy" => {
            if let Some(tour) = &mut app.demo {
                tour.deploy_at = Some(ui.input(|i| i.time));
                tour.pipeline = Some(4);
                tour.pipeline_done = false;
            }
        }
        "history" => history(app, arg),
        "lens-ask" => {
            // lens-ask "<question>" <name>
            let (question, name) = match arg.strip_prefix('"').and_then(|a| a.rsplit_once('"')) {
                Some((q, n)) => (Some(q), n.trim()),
                None => (None, arg.trim()),
            };
            lens_ask(ui, app, question, name);
        }
        "lens-show" => lens_ask(ui, app, None, arg.trim()),
        "lens-reset" => {
            let now = ui.input(|i| i.time);
            if let Some(opened) = &mut app.opened {
                opened.set_lens(cellworks_map::Lens::Folders, now);
            }
        }
        _ => return false,
    }
    true
}

/// What Claude says once a lens is built, by lens name. `{files}` and
/// `{districts}` come from the real build report.
fn lens_reply(name: &str) -> &'static [&'static str] {
    match name {
        "hexagonal" => &[
            "Reading facts... {files} files, {districts} districts.",
            "Imports run adapters -> ports -> domain, except one.",
            "Writing hexagonal.lens.json",
            "Built. One violation is flagged: server/domain/order.ts imports adapters/payments/stripeGateway.ts for the fee rate. The domain must not know the payment provider; put the rate behind the PaymentGateway port.",
        ],
        "front-back" => &[
            "Reading facts... {files} files, {districts} districts.",
            "src/core/http.ts calls /api/orders; server/adapters/http/routes.ts serves it. Pairing them by path.",
            "Writing front-back.lens.json",
            "Built. Frontend on one side, backend on the other, one lane between them: the orders API.",
        ],
        "hexagonal-v2" => &[
            "Updating hexagonal.lens.json: tests out, adapters split by external system.",
            "Writing hexagonal-v2.lens.json",
            "Built. {files} files, {districts} districts: postgres, payments, http, plus domain, ports and api. The violation is still flagged.",
        ],
        _ => &[
            "Reading facts... {files} files, {districts} districts.",
            "Lighting what SHOP-42 touches: cart totals, checkout, the checkout view and the server promo rules. Everything else lies underground.",
            "Writing shop-42.lens.json",
            "Built. The ticket's footprint is the lit part; the rest is context.",
        ],
    }
}

/// `lens-ask`: a scripted Claude turn in the lens panel. The definition is
/// pre-written in `demo/lenses`; the build and the redraw are real.
fn lens_ask(ui: &egui::Ui, app: &mut CellworksApp, question: Option<&str>, name: &str) {
    let now = ui.input(|i| i.time);
    let Some(opened) = &mut app.opened else {
        return;
    };
    let Some(repo) = opened.root().map(Path::to_path_buf) else {
        return;
    };
    let source = assets().join("lenses").join(format!("{name}.lens.json"));
    let dir = repo.join(".cellworks/lenses");
    let copied = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::copy(&source, dir.join(format!("{name}.lens.json"))));
    let panel = &mut app.lens_panel;
    panel.open = true;
    if let Some(question) = question {
        panel.say(format!("» {question}"));
    }
    if let Err(error) = copied {
        panel.say(format!("demo: {name}: {error}"));
        return;
    }
    let report = crate::tools::lens_build(&repo.display().to_string(), name)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or_default();
    let files = report["files"].as_u64().unwrap_or(0).to_string();
    let districts = report["districts"]
        .as_array()
        .map_or(0, Vec::len)
        .to_string();
    for line in lens_reply(name) {
        panel.say(
            line.replace("{files}", &files)
                .replace("{districts}", &districts),
        );
    }
    panel.show_now(opened, name, now);
}

/// The case's worktree once PROTOTYPE made it, else the shop's main checkout.
fn workdir() -> PathBuf {
    let worktree = root().join("projects/shopfront-shop-42");
    if worktree.is_dir() {
        worktree
    } else {
        root().join("projects/shopfront")
    }
}

fn run(command: &mut Command) {
    if let Err(error) = command.status() {
        eprintln!("demo: {command:?}: {error}");
    }
}

fn git(dir: &Path, args: &[&str]) {
    run(Command::new("git").current_dir(dir).args(args));
}

fn now() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%S.000Z"])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// Lists agent `name` and appends a transcript line, so its session walks to
/// `file` and writes it. The conversation pane shows what it said.
fn agent(name: &str, file: &str) {
    let dir = workdir();
    let session = format!("demo-{name}");
    let listing = root().join("agents.json");
    let mut list: Vec<Value> = std::fs::read(&listing)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    list.retain(|entry| entry["sessionId"] != session);
    list.push(json!({
        "sessionId": session, "id": "", "name": name,
        "cwd": dir.display().to_string(), "status": "busy", "state": "working",
    }));
    let _ = std::fs::write(&listing, Value::Array(list).to_string());

    let full = dir.join(file);
    // The first function of the file, so the agent lands on a tower.
    let anchor = std::fs::read_to_string(&full)
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| {
                    line.starts_with("export function") || line.starts_with("export async function")
                })
                .map(str::to_owned)
        })
        .unwrap_or_default();
    let stamp = now();
    let entry = |content: Value| {
        json!({
            "timestamp": stamp, "cwd": dir.display().to_string(), "isSidechain": false,
            "type": "assistant", "message": {"role": "assistant", "content": [content]},
        })
        .to_string()
    };
    let lines = [
        entry(json!({"type": "text", "text": format!("Working on {file}.")})),
        entry(json!({"type": "tool_use", "name": "Edit",
            "input": {"file_path": full.display().to_string(), "old_string": "", "new_string": anchor}})),
    ];
    let folder = root().join("transcripts/demo");
    let _ = std::fs::create_dir_all(&folder);
    if let Ok(mut out) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(folder.join(format!("{session}.jsonl")))
    {
        use std::io::Write;
        for line in lines {
            let _ = writeln!(out, "{line}");
        }
    }
}

/// `npm run dev` in the case worktree, teed to `.cellworks/run.log`. The
/// worktree has no dependencies of its own, so they are cloned from the main
/// checkout first.
fn devserver() -> Option<Child> {
    let dir = workdir();
    let modules = dir.join("node_modules");
    if !modules.exists() {
        // -c: APFS clone, instant and free.
        run(Command::new("cp")
            .arg("-Rc")
            .arg(root().join("projects/shopfront/node_modules"))
            .arg(&modules));
    }
    let _ = std::fs::create_dir_all(dir.join(".cellworks"));
    let log = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.join(".cellworks/run.log"))
        .ok()?;
    // Its own process group, so closing the tour also stops vite below npm.
    let mut command = Command::new("npm");
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
        .args(["run", "dev"])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().ok()?)
        .stderr(log)
        .spawn()
        .map_err(|error| eprintln!("demo: devserver: {error}"))
        .ok()
}

/// The shop in a window of its own, no browser chrome.
fn app_window() {
    if std::env::var_os("CELLWORKS_DRY_LAUNCH").is_some() {
        eprintln!("demo: app-window (dry run)");
        return;
    }
    let url = "http://localhost:5180";
    let opened = Command::new("open")
        .args(["-na", "Google Chrome", "--args"])
        .arg(format!("--app={url}"))
        .arg(format!(
            "--user-data-dir={}",
            root().join("chrome").display()
        ))
        .status()
        .is_ok_and(|status| status.success());
    if !opened {
        run(Command::new("open").arg(url));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_dir(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("demo")
            .join(name)
    }

    #[test]
    fn every_case_snapshot_parses_as_a_case_file() {
        for state in ["ingested", "prototyped", "planned", "fixed"] {
            let text =
                std::fs::read_to_string(demo_dir("cases").join(format!("SHOP-42.{state}.json")))
                    .unwrap();
            serde_json::from_str::<cellworks_map::ui::casefile::CaseFile>(&text)
                .unwrap_or_else(|error| panic!("{state}: {error}"));
        }
    }

    #[test]
    fn every_demo_lens_parses_and_has_a_reply() {
        for name in ["shop-42", "front-back", "hexagonal", "hexagonal-v2"] {
            let text =
                std::fs::read_to_string(demo_dir("lenses").join(format!("{name}.lens.json")))
                    .unwrap();
            serde_json::from_str::<cellworks_map::project::LensDef>(&text)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(lens_reply(name).len() >= 3);
        }
    }

    #[test]
    fn every_jira_seed_parses_as_an_issue() {
        for entry in std::fs::read_dir(demo_dir("jira")).unwrap().flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap();
            if entry.file_name() == "mine.json" {
                serde_json::from_str::<Vec<cellworks_map::ui::jira::Row>>(&text).unwrap();
            } else {
                serde_json::from_str::<cellworks_map::ui::jira::Issue>(&text).unwrap();
            }
        }
    }

    #[test]
    fn callouts_are_short_and_steps_have_an_explanation() {
        for chapter in &tutorial::CHAPTERS {
            for step in chapter.steps {
                assert!(
                    step.callout.split_whitespace().count() <= 12,
                    "{}",
                    step.callout
                );
                assert!(!step.explain.is_empty(), "{}", step.callout);
            }
        }
    }
}
