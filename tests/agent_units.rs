//! Agents as units: a recorded session's tool calls, replayed through the
//! real transcript watch, make its unit walk from tower to tower and work
//! at each - reading, editing, running a command, searching - then wait
//! for the engineer and finish. Drives `claude agents --json` through a
//! fake listing and a transcript written to a temporary folder; runs no
//! `claude` and reads nothing under `$HOME`.

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cellworks::app::CellworksApp;
use cellworks_agent_sessions::ui::DrawnSession;
use cellworks_agent_sessions::units::Pose;
use cellworks_agent_sessions::{ActionKind, Agent, AgentList, ClaudeUnavailable, SessionSources};
use egui_kittest::Harness;
use serde_json::json;

/// One listed session whose `status`/`state` the test changes as it goes.
struct Listing {
    cwd: PathBuf,
    state: Mutex<(String, String)>,
}

impl AgentList for Listing {
    fn list(&self) -> Result<Vec<Agent>, ClaudeUnavailable> {
        let (status, state) = self.state.lock().expect("the listing").clone();
        Ok(vec![Agent {
            session_id: "sid".to_owned(),
            id: String::new(),
            name: "pricing-fix".to_owned(),
            cwd: self.cwd.display().to_string(),
            status,
            state,
        }])
    }
}

/// The transcript of session "sid", appended to line by line.
struct Transcript {
    file: PathBuf,
    cwd: PathBuf,
    n: usize,
}

impl Transcript {
    fn append(&mut self, line: serde_json::Value) {
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)
            .expect("the transcript");
        writeln!(out, "{line}").expect("append");
    }

    /// The assistant calls `tool`; returns its id.
    fn call(&mut self, tool: &str, input: serde_json::Value) -> String {
        self.n += 1;
        let id = format!("toolu_{}", self.n);
        self.append(json!({
            "type": "assistant", "cwd": self.cwd.display().to_string(),
            "timestamp": format!("2026-11-20T10:00:{:02}.000Z", self.n), "isSidechain": false,
            "message": {"role": "assistant", "content": [
                {"type": "tool_use", "id": id, "name": tool, "input": input}
            ]},
        }));
        id
    }

    fn result(&mut self, id: &str, failed: bool) {
        self.append(json!({
            "type": "user", "cwd": self.cwd.display().to_string(), "isSidechain": false,
            "message": {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": id, "content": "", "is_error": failed}
            ]},
        }));
    }
}

/// Steps the harness, as a frame every 1/60 s, until `done` holds or ten
/// seconds of real time pass (the watch polls on its own thread), keeping
/// every frame's drawn unit.
fn until(
    harness: &mut Harness<'_, CellworksApp>,
    frames: &mut Vec<DrawnSession>,
    done: impl Fn(&DrawnSession) -> bool,
) -> DrawnSession {
    let started = std::time::Instant::now();
    loop {
        harness.state_mut().apply_pending();
        harness.step();
        if let Some(unit) = harness.state().sessions_drawn().first().cloned() {
            frames.push(unit.clone());
            if done(&unit) {
                return unit;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "never reached; last frames: {:?}",
            &frames[frames.len().saturating_sub(3)..]
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// "shop" opened with its sessions watched from `transcripts` as `listing`
/// lists them; with `motion` off, as when the system asks for reduced
/// motion (the harness itself sets no animation time).
fn watching(
    tree: &common::Tree,
    transcripts: &Path,
    listing: Arc<Listing>,
    motion: bool,
) -> Harness<'static, CellworksApp> {
    let project = tree.as_project(None);
    let name = project.name.clone();
    let app = common::app(vec![project], common::Recorded::listing(&[])).with_session_sources(
        SessionSources {
            transcripts: transcripts.to_owned(),
            agents: listing,
            poll: Duration::from_millis(5),
            claude: PathBuf::from("false"),
        },
    );
    let mut harness = common::harness(app);
    if motion {
        harness
            .ctx
            .all_styles_mut(|style| style.animation_time = 1.0 / 12.0);
    }
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    harness
}

fn working(kind: ActionKind) -> impl Fn(&DrawnSession) -> bool {
    move |unit| unit.pose == Pose::Working(kind)
}

#[test]
fn a_replayed_session_walks_and_works_per_tool_call() {
    let (tree, _plain) = common::shop_city();
    let transcripts = tempfile::tempdir().expect("a transcripts folder");
    std::fs::create_dir_all(transcripts.path().join("shop")).expect("project folder");
    let listing = Arc::new(Listing {
        cwd: tree.root.clone(),
        state: Mutex::new(("busy".to_owned(), "working".to_owned())),
    });
    let mut transcript = Transcript {
        file: transcripts.path().join("shop/sid.jsonl"),
        cwd: tree.root.clone(),
        n: 0,
    };
    let pricing = tree.root.join("catalog/pricing.rs");
    let order = tree.root.join("orders/order.rs");

    // The session has read "discount" before Cellworks sees it.
    let read = transcript.call("Read", json!({"file_path": pricing, "offset": 3}));
    transcript.result(&read, false);

    let mut harness = watching(&tree, transcripts.path(), listing.clone(), true);
    let mut frames = Vec::new();

    let first = until(&mut harness, &mut frames, |_| true);
    assert_eq!(
        first.tower,
        pricing.join("discount"),
        "seen for the first time, it stands at its last action"
    );
    assert_eq!(first.action, "reading pricing.rs · discount");

    // It edits "place" in another folder: it walks there, then builds.
    let edit = transcript.call(
        "Edit",
        json!({"file_path": order, "old_string": "x", "new_string": "let order = 1;\nlet more = 2;"}),
    );
    let before = until(&mut harness, &mut frames, |u| u.pose == Pose::Moving);
    let built = until(&mut harness, &mut frames, working(ActionKind::Write));
    assert_eq!(built.tower, order.join("place"));
    assert_eq!(built.action, "editing order.rs · place");
    assert!(built.writing);
    let walked: Vec<_> = frames
        .iter()
        .filter(|u| u.pose == Pose::Moving)
        .map(|u| u.dot)
        .collect();
    assert!(
        walked.len() > 10,
        "a visible walk, not a jump: {} frames",
        walked.len()
    );
    assert!(
        (walked[walked.len() / 2] - before.dot).length() > 1.0
            && (walked[walked.len() / 2] - built.dot).length() > 1.0,
        "half way it stands between the two towers"
    );
    transcript.result(&edit, false);

    // A failing test run happens at the project's gate, below the map.
    let run = transcript.call("Bash", json!({"command": "cargo test"}));
    let running = until(&mut harness, &mut frames, working(ActionKind::Run));
    assert_eq!(running.action, "testing · cargo test");
    assert!(
        running.dot.y > built.dot.y,
        "the gate is in front of the city"
    );
    transcript.result(&run, true);

    // A search sweeps the catalog folder.
    transcript.call("Grep", json!({"pattern": "discount", "path": "catalog"}));
    let searching = until(&mut harness, &mut frames, working(ActionKind::Search));
    assert_eq!(searching.tower, tree.root.join("catalog"));
    assert_eq!(searching.action, "searching · discount");

    // It asks the engineer, then finishes.
    *listing.state.lock().expect("the listing") = ("busy".to_owned(), "waiting".to_owned());
    until(&mut harness, &mut frames, |u| u.pose == Pose::Waiting);
    *listing.state.lock().expect("the listing") = ("idle".to_owned(), "done".to_owned());
    until(&mut harness, &mut frames, |u| u.pose == Pose::Done);

    let mut poses: Vec<Pose> = frames.iter().map(|u| u.pose).collect();
    poses.dedup();
    poses.retain(|pose| *pose != Pose::Thinking);
    poses.dedup();
    assert_eq!(
        poses,
        [
            Pose::Working(ActionKind::Read),
            Pose::Moving,
            Pose::Working(ActionKind::Write),
            Pose::Moving,
            Pose::Working(ActionKind::Run),
            Pose::Moving,
            Pose::Working(ActionKind::Search),
            Pose::Waiting,
            Pose::Done,
        ]
    );
}

/// A burst of calls faster than the unit can show them is compressed: it
/// skips to the newest few rather than falling behind.
#[test]
fn a_burst_of_calls_never_leaves_the_unit_far_behind() {
    let (tree, _plain) = common::shop_city();
    let transcripts = tempfile::tempdir().expect("a transcripts folder");
    std::fs::create_dir_all(transcripts.path().join("shop")).expect("project folder");
    let listing = Arc::new(Listing {
        cwd: tree.root.clone(),
        state: Mutex::new(("busy".to_owned(), "working".to_owned())),
    });
    let mut transcript = Transcript {
        file: transcripts.path().join("shop/sid.jsonl"),
        cwd: tree.root.clone(),
        n: 0,
    };
    let files = ["catalog/pricing.rs", "orders/order.rs", "catalog/mod.rs"];
    transcript.call("Read", json!({"file_path": tree.root.join(files[0])}));

    let mut harness = watching(&tree, transcripts.path(), listing, true);
    let mut frames = Vec::new();
    until(&mut harness, &mut frames, |_| true);

    for i in 0..30 {
        transcript.call("Read", json!({"file_path": tree.root.join(files[i % 3])}));
    }
    let last: &Path = &tree.root.join(files[29 % 3]);
    let started = frames.len();
    until(&mut harness, &mut frames, |u| {
        u.pose == Pose::Working(ActionKind::Read) && u.tower.starts_with(last)
    });
    // Let the queue drain.
    for _ in 0..600 {
        harness.state_mut().apply_pending();
        harness.step();
    }
    let unit = harness.state().sessions_drawn()[0].clone();
    assert!(
        unit.tower.starts_with(last),
        "it ends at the newest call: {unit:?}"
    );
    let seconds = (frames.len() - started) as f32 / 60.0;
    assert!(
        seconds < 12.0,
        "caught up within {seconds} s of virtual time"
    );
}

/// With reduced motion a unit steps straight to its next action: no walk.
#[test]
fn with_reduced_motion_a_unit_steps_without_walking() {
    let (tree, _plain) = common::shop_city();
    let transcripts = tempfile::tempdir().expect("a transcripts folder");
    std::fs::create_dir_all(transcripts.path().join("shop")).expect("project folder");
    let listing = Arc::new(Listing {
        cwd: tree.root.clone(),
        state: Mutex::new(("busy".to_owned(), "working".to_owned())),
    });
    let mut transcript = Transcript {
        file: transcripts.path().join("shop/sid.jsonl"),
        cwd: tree.root.clone(),
        n: 0,
    };
    transcript.call(
        "Read",
        json!({"file_path": tree.root.join("catalog/pricing.rs")}),
    );
    let mut harness = watching(&tree, transcripts.path(), listing, false);
    let mut frames = Vec::new();
    until(&mut harness, &mut frames, |_| true);

    let order = tree.root.join("orders/order.rs");
    transcript.call("Write", json!({"file_path": order}));
    until(&mut harness, &mut frames, |u| u.tower.starts_with(&order));
    assert!(
        frames.iter().all(|u| u.pose != Pose::Moving),
        "no frame shows it walking"
    );
}

/// PNG frame sequences of a unit on this repository at 1600x1000, in the
/// city and in the flat view: it reads `view.rs`, walks to `city.rs` and
/// builds on `floor_z`, runs the map crate's tests (failing) and searches
/// `ui/`. Written to target/agent-units-shots/{city,flat}/NNN-<pose>.png.
#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn unit_shots() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/agent-units-shots");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the repository");
    let only = std::env::var("AGENT_SHOTS").unwrap_or_default();
    for flat in [false, true] {
        if (only == "city" && flat) || (only == "flat" && !flat) {
            continue;
        }
        let dir = out.join(if flat { "flat" } else { "city" });
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the shots folder");

        let transcripts = tempfile::tempdir().expect("a transcripts folder");
        std::fs::create_dir_all(transcripts.path().join("repo")).expect("project folder");
        let listing = Arc::new(Listing {
            cwd: root.clone(),
            state: Mutex::new(("busy".to_owned(), "working".to_owned())),
        });
        let mut transcript = Transcript {
            file: transcripts.path().join("repo/sid.jsonl"),
            cwd: root.clone(),
            n: 0,
        };
        let read = transcript.call(
            "Read",
            json!({"file_path": root.join("crates/map/src/view.rs"), "offset": 186}),
        );
        transcript.result(&read, false);

        let project = cellworks_workspace::Project::at(root.clone());
        let app = common::app(
            vec![project.clone()],
            Arc::new(cellworks_code_analysis::GitCli),
        )
        .with_session_sources(SessionSources {
            transcripts: transcripts.path().to_owned(),
            agents: listing.clone(),
            poll: Duration::from_millis(5),
            claude: PathBuf::from("false"),
        });
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1600.0, 1000.0))
            .with_step_dt(1.0 / 20.0)
            .with_max_steps(1)
            .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
        harness.ctx.set_theme(egui::Theme::Dark);
        cellworks_workspace::theme::apply(&harness.ctx);
        harness
            .ctx
            .all_styles_mut(|style| style.animation_time = 1.0 / 12.0);
        harness.state_mut().open(project);
        let started = std::time::Instant::now();
        while (harness
            .state()
            .opened()
            .is_none_or(|o| o.is_reading() || o.is_preparing())
            || harness.state().sessions_drawn().is_empty())
            && started.elapsed() < Duration::from_secs(120)
        {
            harness.state_mut().apply_pending();
            harness.step();
            std::thread::sleep(Duration::from_millis(20));
        }
        if flat {
            harness.key_press(egui::Key::V);
        }
        // Framed on the map crate's sources, the Explorer folded away.
        harness.state_mut().jump(root.join("crates/map/src"));
        for _ in 0..40 {
            harness.state_mut().apply_pending();
            harness.step();
        }
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
        for _ in 0..20 {
            harness.state_mut().apply_pending();
            harness.step();
        }

        let mut frame = 0;
        let mut shoot = |harness: &mut Harness<'_, CellworksApp>, frames: usize| {
            for i in 0..frames {
                harness.state_mut().apply_pending();
                harness.step();
                std::thread::sleep(Duration::from_millis(5));
                if i % 3 == 0 {
                    let pose = harness
                        .state()
                        .sessions_drawn()
                        .first()
                        .map(|u| u.pose.name().replace(' ', "-"))
                        .unwrap_or_default();
                    let path = dir.join(format!("{frame:03}-{pose}.png"));
                    harness.render().expect("render").save(&path).expect("save");
                    frame += 1;
                }
            }
        };
        shoot(&mut harness, 9);
        let edit = transcript.call(
            "Edit",
            json!({"file_path": root.join("crates/map/src/city.rs"),
                "old_string": "", "new_string": "pub fn floor_z(item: &Item, index: usize) -> f32 {"}),
        );
        shoot(&mut harness, 75);
        transcript.result(&edit, false);
        let run = transcript.call("Bash", json!({"command": "cargo test -p cellworks-map"}));
        shoot(&mut harness, 45);
        transcript.result(&run, true);
        shoot(&mut harness, 15);
        transcript.call(
            "Grep",
            json!({"pattern": "fn flat", "path": "crates/map/src/ui"}),
        );
        shoot(&mut harness, 45);
        *listing.state.lock().expect("the listing") = ("busy".to_owned(), "waiting".to_owned());
        shoot(&mut harness, 12);
        println!("{}", dir.display());
    }
}
