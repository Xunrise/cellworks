//! PROTOTYPE (stakeholder demo): everything here is gated on the
//! `CELLWORKS_DEMO` environment variable, so normal mode is unchanged.
//!
//! The demo runs on a throwaway world under `${TMPDIR}cellworks-demo`, built
//! by `demo/setup.sh`: the sample project, two decoys, Jira seeds, case
//! snapshots and a transcripts folder the scripted agents write into. A guided
//! tour (chapters 0 to 10) lies over the app; its Next button runs the
//! chapter's drive lines, one beat at a time. The same commands work from a
//! `CELLWORKS_DRIVE` file.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cellworks_agent_sessions::{Agent, AgentList, ClaudeUnavailable, SessionSources, parse_agents};
use cellworks_workspace::{HostProbe, Sources};
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

const AVAILABLE: [&str; 8] = [
    "Projects in one place",
    "See a codebase as a city",
    "Move without getting lost",
    "Understand how code connects (lenses, lanes)",
    "Know why code exists",
    "Review a PR in the city",
    "Watch and steer AI agents",
    "TypeScript towers",
];
const IN_DEVELOPMENT: [&str; 5] = [
    "Ticket to reviewed change in one place",
    "Finish a review with confidence",
    "Code health at a glance",
    "Replay history",
    "Remote projects over SSH",
];
const PLANNED: [&str; 4] = [
    "Command agents from the map",
    "An overseer you can talk to",
    "Every language",
    "Jira straight into a case",
];

const STEPS: [Step; 11] = [
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
        title: "A ticket arrives",
        body: "Open the tickets and pick SHOP-42, Apply a promo code at checkout. The case shows the problem in plain words, and a schematic of the code and the outside systems it touches: the checkout pod, the orders database and the payment secret. Press INGEST to let the case be written.",
        lines: &[],
    },
    Step {
        title: "Iterate with the app running",
        body: "PROTOTYPE makes a worktree and a branch for the ticket. The shop starts in its own window and the agent builds the promo code while you watch the towers rise. Pin the checkout view, capture the app window and send round 1: show the discount on the total.",
        lines: &[
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
        lines: &["history 0"],
    },
];

#[derive(Default)]
pub struct Tour {
    step: usize,
    beat: usize,
    queue: VecDeque<String>,
    resume: Option<Instant>,
    servers: Vec<Child>,
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
                for (head, items) in [
                    ("Available now", &AVAILABLE[..]),
                    ("In development", &IN_DEVELOPMENT[..]),
                    ("Planned outcomes", &PLANNED[..]),
                ] {
                    ui.add_space(4.0);
                    ui.strong(head);
                    for item in items {
                        ui.label(format!("  {item}"));
                    }
                }
                ui.add_space(4.0);
                ui.label("Skins come later.");
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
    if back {
        tour.back();
    } else if next {
        tour.advance();
    }
    for line in due {
        crate::app::drive_line(ui, app, &line);
    }
}

// ---------------------------------------------------------------- commands

/// The demo's own drive commands. `true` when `line` was one of them.
pub fn command(ui: &mut egui::Ui, app: &mut CellworksApp, line: &str) -> bool {
    let (cmd, arg) = line.split_once(' ').unwrap_or((line, ""));
    let _ = ui;
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
        // ponytail: stubs until the pipeline strip, the deploy packet and the
        // history scrubber exist.
        "pipeline" | "deploy" | "history" => eprintln!("demo: {cmd} {arg} (not built yet)"),
        _ => return false,
    }
    true
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
        tour.step = 4;
        tour.advance();
        assert_eq!(tour.beat, 1);
        assert!(!tour.queue.is_empty());
        tour.advance();
        tour.advance();
        assert_eq!((tour.step, tour.beat), (5, 0));
    }
}
