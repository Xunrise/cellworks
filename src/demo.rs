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

pub fn is_demo() -> bool {
    std::env::var_os("CELLWORKS_DEMO").is_some()
}

/// The demo world.
pub fn root() -> PathBuf {
    let tmp = std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".to_owned());
    let dir = PathBuf::from(tmp.trim_end_matches('/')).join("cellworks-demo");
    dir.canonicalize().unwrap_or(dir)
}

/// Builds the world when it is missing, and points the Jira and case readers
/// at it. Called once, before any window or thread exists.
pub fn init() {
    if !is_demo() {
        return;
    }
    if !root().join("projects").is_dir() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo/setup.sh");
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

struct Step {
    title: &'static str,
    body: &'static str,
    /// Drive lines; a `--` line splits them into beats, one per press.
    lines: &'static [&'static str],
}

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

const STEPS: [Step; 12] = [
    Step {
        title: "What Cellworks is and where it is going",
        body: "One map for your projects, code, tickets, agents and servers.",
        lines: &[],
    },
    Step {
        title: "Pick a project",
        body: "The launcher lists every project on this machine and the hosts you connected. Shopfront is a small web shop: its preview shows the code, the docs, the open tickets and the outside systems it touches. The two others are here to show the list is real. Select shopfront and press Enter.",
        lines: &["select {ROOT}/projects/shopfront"],
    },
    Step {
        title: "Read the city",
        body: "Each folder of code is a district. Every file is a tower and every function a floor stack; floors are lines, and the cellar holds the tests. Lines on the ground are import lanes, arcs in the air are calls. Documents stand as Libraries, Terraform as Plants, YAML as Switchboards and the Dockerfile as a Depot. Press E for the tree and L to switch the lens. The small figure walking between towers is an agent reading the code.",
        lines: &[
            "open {ROOT}/projects/shopfront",
            "wait 2500",
            "agent explorer src/catalog/search.ts",
            "wait 3500",
            "agent explorer src/cart/totals.ts",
        ],
    },
    Step {
        title: "Reshape the city for your question",
        body: "The city above answers a general question. A lens answers yours. Lenses belong to one project and one person: you ask in the lens panel, Claude reads the project's facts and writes a small definition file through the cellworks-lens skill, and the map redraws. The same definition can also reshape how the project is read in the first place. Ask whether the backend is hexagonal, see how the frontend talks to the backend, tell Claude what to hide, then look at the project from a ticket's perspective.",
        lines: &[
            "lens-ask \"Is the backend hexagonal?\" hexagonal",
            "--",
            "lens-ask \"Show me how the frontend talks to the backend\" front-back",
            "--",
            "lens-ask \"Hide the tests and group the adapters by the system they talk to\" hexagonal-v2",
            "--",
            "lens-ask \"Look at this project from SHOP-42's perspective\" shop-42",
        ],
    },
    Step {
        title: "A ticket arrives",
        body: "Open the tickets and pick SHOP-42, Apply a promo code at checkout. The case shows the problem in plain words, and a schematic of the code and the outside systems it touches: the checkout pod, the orders database and the payment secret. Press INGEST to let the case be written. The map takes on the ticket's perspective: only what SHOP-42 touches stays lit.",
        lines: &["lens-show shop-42"],
    },
    Step {
        title: "Iterate with the app running",
        body: "PROTOTYPE makes a worktree and a branch for the ticket. The shop starts in its own window and the agent builds the promo code while you watch the towers rise. Pin the checkout view, capture the app window and send round 1: show the discount on the total.",
        lines: &[
            "lens-reset",
            "devserver",
            "wait 4000",
            "app-window",
            "wait 1500",
            "write round-1",
            "agent shop-42 src/cart/promo.ts",
            "wait 2500",
            "agent shop-42 src/ui/checkoutView.ts",
            "--",
            "write round-2",
            "agent shop-42 src/cart/totals.ts",
            "wait 2500",
            "agent shop-42 src/ui/checkoutView.ts",
        ],
    },
    Step {
        title: "Freeze the prototype",
        body: "When the behaviour is right, freeze it. The session is distilled into a prototype note and the work is handed to the pipeline. Nothing is built here from now on.",
        lines: &["case-file prototyped"],
    },
    Step {
        title: "Plan",
        body: "The plan comes back as steps, each with the files it touches, the evidence and the risks. Read the risks, then approve.",
        lines: &["case-file planned"],
    },
    Step {
        title: "The Factory builds",
        body: "The pipeline builds, checks, delivers and reviews. Three agents commit in turn and the buildings rise: the promo rules, then the order, then the docs.",
        lines: &[
            "pipeline build",
            "write factory/step-1",
            "agent factory-1 src/cart/promo.ts",
            "wait 2000",
            "commit feat(cart): apply promo codes\\n\\nRefs: STORY-042",
            "--",
            "pipeline checks",
            "write factory/step-2",
            "agent factory-2 src/checkout/order.ts",
            "wait 2000",
            "commit feat(checkout): charge the discounted total\\n\\nRefs: STORY-042",
            "--",
            "pipeline deliver",
            "write factory/step-3",
            "agent factory-3 docs/domain/contexts/Cart/glossary.md",
            "wait 2000",
            "commit docs(cart): promo code in the glossary and story\\n\\nRefs: STORY-042",
            "pipeline review",
            "case-file fixed",
        ],
    },
    Step {
        title: "Review",
        body: "The commits column lists what the agents did. Open each file to see its diff against main, then mark the case reviewed.",
        lines: &[],
    },
    Step {
        title: "Deploy",
        body: "The reviewed change goes to the staging server and its version moves on. The app window reloads from staging.",
        lines: &["deploy"],
    },
    Step {
        title: "History",
        body: "Scrub through the commits and watch the city grow. Per-line history says why a line exists, and it leads back to SHOP-42.",
        lines: &[
            "history 0",
            "wait 2500",
            "history 3",
            "wait 2500",
            "history 6",
            "wait 2500",
            "history last",
        ],
    },
];

#[derive(Default)]
pub struct Tour {
    step: usize,
    beat: usize,
    queue: VecDeque<String>,
    resume: Option<Instant>,
    servers: Vec<Child>,
    /// The Factory strip: the stage running, `None` while it is hidden.
    pipeline: Option<usize>,
    pipeline_done: bool,
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
            let _ = server.kill();
        }
    }
}

impl Tour {
    fn beats(&self) -> Vec<&'static [&'static str]> {
        STEPS[self.step]
            .lines
            .split(|line| *line == "--")
            .filter(|beat| !beat.is_empty())
            .collect()
    }

    /// Runs the next beat of this chapter, or moves to the next chapter.
    fn advance(&mut self) {
        let beats = self.beats();
        if let Some(beat) = beats.get(self.beat) {
            self.queue.extend(beat.iter().map(|l| (*l).to_owned()));
            self.beat += 1;
        } else if self.step + 1 < STEPS.len() {
            self.step += 1;
            self.beat = 0;
        }
    }

    fn back(&mut self) {
        self.step = self.step.saturating_sub(1);
        self.beat = 0;
        self.queue.clear();
        self.resume = None;
    }
}

/// Runs what the tour has queued, and draws its card: chapter, title, body,
/// Back and Next (keys B and N).
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
    let mut next = !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::N));
    let mut back = !typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::B));
    let step = &STEPS[tour.step];
    let beats = tour.beats().len();
    egui::Window::new("Cellworks tour")
        .title_bar(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
        .resizable(false)
        .collapsible(false)
        .show(&ctx, |ui| {
            ui.set_max_width(400.0);
            ui.weak(format!("Chapter {}/{}", tour.step, STEPS.len() - 1));
            ui.heading(step.title);
            ui.label(step.body);
            if tour.step == 0 {
                for (head, text) in CORE {
                    ui.add_space(4.0);
                    ui.strong(head);
                    ui.label(text);
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                back |= ui
                    .add_enabled(tour.step > 0, egui::Button::new("Back (B)"))
                    .clicked();
                let label = if tour.beat < beats {
                    format!("Do it {}/{} (N)", tour.beat + 1, beats)
                } else {
                    "Next (N)".to_owned()
                };
                next |= ui.button(label).clicked();
            });
        });
    pipeline_strip(ui, tour);
    let close = history_bar(ui, tour);
    if back {
        tour.back();
    } else if next {
        tour.advance();
    }
    for line in due {
        crate::app::drive_line(ui, app, &line);
    }
    if close {
        history_close(app);
    }
    history_tick(ui, app);
}

/// Top-centre strip: BUILD, CHECKS, DELIVER, REVIEW, DEPLOY.
fn pipeline_strip(ui: &egui::Ui, tour: &Tour) {
    let Some(running) = tour.pipeline else {
        return;
    };
    let ctx = ui.ctx();
    let pulse = ((ctx.input(|i| i.time) * 4.0).sin() * 0.5 + 0.5) as f32;
    egui::Area::new(egui::Id::new("demo-pipeline"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 12.0])
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("The Factory");
                    for (i, name) in STAGES.iter().enumerate() {
                        ui.label("·");
                        let text = if tour.pipeline_done || i < running {
                            egui::RichText::new(format!("\u{2714} {name}"))
                                .color(egui::Color32::from_rgb(90, 200, 110))
                        } else if i == running {
                            egui::RichText::new(format!("\u{25CF} {name}"))
                                .strong()
                                .color(
                                    egui::Color32::from_rgb(255, 190, 60)
                                        .gamma_multiply(0.6 + 0.4 * pulse),
                                )
                        } else {
                            egui::RichText::new(*name).weak()
                        };
                        ui.label(text);
                    }
                });
            });
        });
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
    egui::Window::new("History")
        .title_bar(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, [-120.0, -16.0])
        .resizable(false)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            let last = h.commits.len().saturating_sub(1);
            ui.set_width(560.0);
            ui.horizontal(|ui| {
                ui.strong("History");
                ui.weak(format!("{}/{}", h.idx + 1, h.commits.len()));
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
            ui.spacing_mut().slider_width = 540.0;
            let slider = ui.add(egui::Slider::new(&mut h.idx, 0..=last).show_value(false));
            if last > 0 {
                let r = slider.rect;
                let y = r.bottom() + 1.0;
                for i in 0..=last {
                    let x = r.left() + 8.0 + (r.width() - 16.0) * i as f32 / last as f32;
                    let colour = if i == h.idx {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::GRAY
                    };
                    ui.painter().line_segment(
                        [egui::pos2(x, y), egui::pos2(x, y + 5.0)],
                        egui::Stroke::new(1.0, colour),
                    );
                }
                ui.add_space(6.0);
            }
            if let Some((sha, subject)) = h.commits.get(h.idx) {
                ui.label(format!(
                    "{}  {}",
                    &sha[..sha.len().min(7)],
                    subject.lines().next().unwrap_or("")
                ));
            }
        });
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
                tour.advance();
            }
        }
        "tour-back" => {
            if let Some(tour) = &mut app.demo {
                tour.back();
            }
        }
        "rescan" => app.rescan(),
        "agent" => {
            let (name, file) = arg.split_once(' ').unwrap_or((arg, ""));
            agent(name, file);
        }
        "write" => {
            let payload = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("demo/agent")
                .join(arg);
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
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("demo/lenses")
        .join(format!("{name}.lens.json"));
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
    Command::new("npm")
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
    let url = "http://localhost:5173";
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
    fn next_runs_the_beats_then_moves_on() {
        let mut tour = Tour::default();
        tour.advance();
        assert_eq!(tour.step, 1);
        tour.step = 5;
        tour.advance();
        assert_eq!(tour.beat, 1);
        assert!(!tour.queue.is_empty());
        tour.advance();
        tour.advance();
        assert_eq!((tour.step, tour.beat), (6, 0));
    }
}
