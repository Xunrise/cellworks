//! STORY-081, steer a session from its session pane.
//!
//! One test per scenario of issue #121, named after it. The background has
//! the project "shop" open as a city (`common::shop_city`); every session
//! here is seeded by hand through `CellworksApp::with_pane_sources`, pointed
//! at a fake `claude` - a real POSIX shell script behind a real
//! pseudo-terminal and a real clean environment, never a mock (design.md's
//! Decisions section): `attach <id>` prints a recognisable banner then
//! `exec cat`s, so typed keystrokes really round-trip; `stop <id>` appends
//! its invocation to a log whose path is baked into the script's own text,
//! since the clean environment carries no variable a test could pass it
//! through; `--resume <id> --bg -n <name>` prints a "backgrounded <id>"
//! line naming a fresh copy (or, for the failing variant, an error on
//! stderr and a non-zero exit). No test here runs the real `claude`, opens
//! a window or reads `$HOME`.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cellworks::app::CellworksApp;
use cellworks_agent_sessions::{FileEvent, Session};
use egui::{Event, Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Recorded, Tree, double_click, shop_city};

/// A fake `claude`, and the folders a session pane needs around it: the
/// program itself, the transcripts root a read-only conversation is read
/// from, and the log `stop` invocations are appended to.
struct Env {
    _claude_dir: tempfile::TempDir,
    _transcripts_dir: tempfile::TempDir,
    claude: PathBuf,
    transcripts: PathBuf,
    stop_log: PathBuf,
}

/// A fake `claude` whose `--resume ... --bg` always starts a copy.
fn env() -> Env {
    env_with(None)
}

/// A fake `claude` whose `--resume ... --bg` instead fails, printing
/// `failure` to stderr and exiting non-zero - scenario 8's fixture.
fn env_with_failing_resume(failure: &str) -> Env {
    env_with(Some(failure))
}

fn env_with(failure: Option<&str>) -> Env {
    let claude_dir = tempfile::tempdir().expect("a temporary directory");
    let transcripts_dir = tempfile::tempdir().expect("a temporary directory");
    let stop_log = claude_dir.path().join("stop.log");
    let claude = claude_dir.path().join("claude");

    let resume_body = match failure {
        Some(failure) => format!("echo \"{failure}\" 1>&2\n    exit 1"),
        None => "newid=\"$2-copy\"\n    echo \"started a copy as $newid\"\n    \
                  echo \"backgrounded $newid\""
            .to_owned(),
    };
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\n  \
         attach)\n    echo \"SESSION SCREEN $2\"\n    exec cat\n    ;;\n  \
         stop)\n    echo \"$@\" >> {log}\n    ;;\n  \
         --resume)\n    {resume_body}\n    ;;\n\
         esac\n",
        log = stop_log.display(),
    );
    fs::write(&claude, script).expect("write the fake claude script");
    let mut perms = fs::metadata(&claude)
        .expect("the script's metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&claude, perms).expect("make the fake claude executable");

    Env {
        claude,
        transcripts: transcripts_dir.path().to_path_buf(),
        stop_log,
        _claude_dir: claude_dir,
        _transcripts_dir: transcripts_dir,
    }
}

/// A session named `id`: a background one when `attach_id` is `Some`, an
/// interactive one started outside Cellworks when it is `None`.
fn session(id: &str, attach_id: Option<&str>, events: Vec<FileEvent>) -> Session {
    Session {
        id: id.to_owned(),
        attach_id: attach_id.map(str::to_owned),
        name: id.to_owned(),
        cwd: PathBuf::new(),
        status: "busy".to_owned(),
        state: "working".to_owned(),
        repo: "shop".to_owned(),
        branch: "main".to_owned(),
        summary: String::new(),
        last_active: Some(0),
        events,
    }
}

/// A read of `path`, with no checkout - the fixtures here are not git
/// checkouts, so a path prefix against the city's own root places it.
fn read_of(path: PathBuf, line: Option<usize>) -> FileEvent {
    FileEvent {
        kind: cellworks_agent_sessions::ActionKind::Read,
        checkout: None,
        path,
        line,
        added: 0,
        ..FileEvent::default()
    }
}

/// Opens `tree`'s project (as "shop") seeded with `sessions`, pointed at
/// `env`'s fake `claude` and transcripts root.
fn open_with_pane(
    tree: &Tree,
    sessions: Vec<Session>,
    env: &Env,
) -> Harness<'static, CellworksApp> {
    let project = tree.as_project(None);
    let name = project.name.clone();
    let app = common::app(vec![project], Recorded::listing(&[])).with_pane_sources(
        sessions,
        env.claude.clone(),
        env.transcripts.clone(),
    );
    let mut harness = common::harness(app);
    double_click(&mut harness, &name);
    common::await_read(&mut harness);
    harness
}

/// A double-click on `session_id`'s dot on the map, the way `double_click`
/// double-clicks a sidebar row or a launcher entry: two real pointer clicks
/// close enough in (simulated) time to register as one.
fn double_click_dot(harness: &mut Harness<'_, CellworksApp>, session_id: &str) {
    let dot = harness
        .state()
        .sessions_drawn()
        .iter()
        .find(|drawn| drawn.id == session_id)
        .unwrap_or_else(|| panic!("session {session_id} is not drawn on the map"))
        .dot;
    for _ in 0..2 {
        harness.event(Event::PointerMoved(dot));
        for pressed in [true, false] {
            harness.event(Event::PointerButton {
                pos: dot,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::default(),
            });
        }
    }
    harness.run();
}

/// Runs frames, sleeping a little real time between them, until `done` is
/// satisfied or 5 seconds pass; `true` only if `done` was ever satisfied.
/// The pane's own work (the pty's reading thread, the conversation's 1s
/// background re-read) happens off the frame loop, so a test must let real
/// time pass for it to show up - the same patience-loop style as
/// `tests/common/mod.rs`'s `await_read`.
fn wait_for(
    harness: &mut Harness<'_, CellworksApp>,
    mut done: impl FnMut(&CellworksApp) -> bool,
) -> bool {
    let started = Instant::now();
    loop {
        harness.run();
        if done(harness.state()) {
            return true;
        }
        if started.elapsed() > Duration::from_secs(5) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

const TS: &str = "2026-10-19T10:00:00.000Z";

/// One transcript line of a user prompt (a `text` block).
fn prompt_line(text: &str) -> String {
    format!(
        r#"{{"timestamp":"{TS}","cwd":"/repo","isSidechain":false,"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"{text}"}}]}}}}"#
    )
}

/// One transcript line of a plain text message of `role`.
fn text_line(role: &str, text: &str) -> String {
    format!(
        r#"{{"timestamp":"{TS}","cwd":"/repo","isSidechain":false,"type":"{role}","message":{{"role":"{role}","content":[{{"type":"text","text":"{text}"}}]}}}}"#
    )
}

/// One sidechain transcript line, of either kind - it must never surface,
/// whatever it carries.
fn sidechain_line(text: &str) -> String {
    format!(
        r#"{{"timestamp":"{TS}","cwd":"/repo","isSidechain":true,"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"{text}"}}]}}}}"#
    )
}

/// One transcript line of a single `tool_use` block.
fn tool_line(tool: &str, input_json: &str) -> String {
    format!(
        r#"{{"timestamp":"{TS}","cwd":"/repo","isSidechain":false,"type":"assistant","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"{tool}","input":{input_json}}}]}}}}"#
    )
}

/// Opening a background session shows its own screen
#[test]
fn opening_a_background_session_shows_its_own_screen() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", Some("atch-pf"), vec![event]);
    let mut harness = open_with_pane(&tree, vec![pricing_fix], &env);

    // From the sidebar row.
    double_click(&mut harness, "pricing-fix");
    assert!(harness.state().pane_open(), "the row opened the pane");
    let shown = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN atch-pf"))
    });
    assert!(
        shown,
        "the pane shows pricing-fix's own screen, not blank: {:?}",
        harness.state().pane_text()
    );

    harness.get_by_label("DETACH").click();
    harness.run();
    assert!(!harness.state().pane_open());

    // From the dot.
    double_click_dot(&mut harness, "pricing-fix");
    assert!(harness.state().pane_open(), "the dot opened the pane");
    let shown_again = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN atch-pf"))
    });
    assert!(shown_again, "{:?}", harness.state().pane_text());
    assert!(
        harness.state().navigation().inside().is_none(),
        "a double-click on the dot never also steps into the tower underneath it"
    );
}

/// The engineer answers a session in its pane
#[test]
fn the_engineer_answers_a_session_in_its_pane() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", Some("atch-pf"), vec![event]);
    let mut harness = open_with_pane(&tree, vec![pricing_fix], &env);

    double_click(&mut harness, "pricing-fix");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN"))
    }));

    let explorer_open_before = harness.state().explorer_open();
    let inside_before = harness.state().navigation().inside().is_some();

    for letter in ["y", "e", "s"] {
        harness
            .input_mut()
            .events
            .push(Event::Text(letter.to_owned()));
    }
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();

    let answered = wait_for(&mut harness, |app| {
        app.pane_text().is_some_and(|text| text.contains("yes"))
    });
    assert!(
        answered,
        "pricing-fix receives the typed answer and continues: {:?}",
        harness.state().pane_text()
    );
    assert_eq!(
        harness.state().explorer_open(),
        explorer_open_before,
        "typing 'e' into the session did not also toggle the map's Explorer"
    );
    assert_eq!(
        harness.state().navigation().inside().is_some(),
        inside_before,
        "the Enter that answered the session did not also step into a tower"
    );
}

/// Closing the pane leaves the session running
#[test]
fn closing_the_pane_leaves_the_session_running() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let pricing_fix = session("pricing-fix", Some("atch-pf"), vec![event]);
    let mut harness = open_with_pane(&tree, vec![pricing_fix], &env);

    // Closed with "DETACH".
    double_click(&mut harness, "pricing-fix");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN"))
    }));
    harness.get_by_label("DETACH").click();
    harness.run();
    assert!(!harness.state().pane_open());
    assert!(
        fs::read_to_string(&env.stop_log)
            .unwrap_or_default()
            .is_empty(),
        "\"DETACH\" only detaches; it never stops pricing-fix"
    );

    // Closed with Esc, once the terminal has lost focus to a click on the
    // map.
    double_click(&mut harness, "pricing-fix");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN"))
    }));
    let on_the_map = egui::pos2(900.0, 100.0);
    harness.event(Event::PointerMoved(on_the_map));
    for pressed in [true, false] {
        harness.event(Event::PointerButton {
            pos: on_the_map,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::default(),
        });
    }
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();

    assert!(
        !harness.state().pane_open(),
        "Esc closes the pane once the terminal has lost focus"
    );
    assert!(
        fs::read_to_string(&env.stop_log)
            .unwrap_or_default()
            .is_empty(),
        "pricing-fix is still running: stop was never recorded"
    );
}

/// A session started outside Cellworks is shown read-only
#[test]
fn a_session_started_outside_cellworks_is_shown_read_only() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let docs_run = session(
        "docs-run",
        None,
        vec![read_of(tree.root.join("catalog/pricing.rs"), None)],
    );

    let project_dir = env.transcripts.join("shop-project");
    fs::create_dir_all(&project_dir).expect("create the project's transcript folder");
    let transcript_path = project_dir.join("docs-run.jsonl");
    let initial = [
        prompt_line("What should the fix look like?"),
        sidechain_line("SIDECHAIN MARKER must never surface"),
        text_line(
            "assistant",
            "<system-reminder>skipped, starts with '<'</system-reminder>",
        ),
        text_line("assistant", "Here is my answer."),
        tool_line("Read", r#"{"file_path":"/repo/foo.rs"}"#),
    ]
    .join("\n")
        + "\n";
    fs::write(&transcript_path, &initial).expect("write the transcript fixture");

    let mut harness = open_with_pane(&tree, vec![docs_run], &env);
    double_click(&mut harness, "docs-run");

    let shown = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("Here is my answer."))
    });
    assert!(shown, "{:?}", harness.state().pane_text());
    let text = harness.state().pane_text().expect("the pane is open");
    assert!(
        text.contains("Started outside Cellworks (interactive). Read-only here."),
        "{text}"
    );
    assert!(text.contains("> What should the fix look like?"), "{text}");
    assert!(text.contains("Read /repo/foo.rs"), "{text}");
    assert!(!text.contains("SIDECHAIN MARKER"), "{text}");
    assert!(!text.contains("system-reminder"), "{text}");

    let mut appended = fs::read_to_string(&transcript_path).expect("read the fixture back");
    appended.push_str(&tool_line("Bash", r#"{"command":"echo new line"}"#));
    appended.push('\n');
    fs::write(&transcript_path, appended).expect("append a new line");

    let updated = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("Bash: echo new line"))
    });
    assert!(
        updated,
        "an appended line appears once the background re-read picks it up: {:?}",
        harness.state().pane_text()
    );
}

/// A session started outside Cellworks continues in the background
#[test]
fn a_session_started_outside_cellworks_continues_in_the_background() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let docs_run = session(
        "docs-run",
        None,
        vec![read_of(tree.root.join("catalog/pricing.rs"), None)],
    );
    let project_dir = env.transcripts.join("shop-project");
    fs::create_dir_all(&project_dir).expect("create the project's transcript folder");
    fs::write(
        project_dir.join("docs-run.jsonl"),
        prompt_line("Keep going please.") + "\n",
    )
    .expect("write the transcript fixture");

    let mut harness = open_with_pane(&tree, vec![docs_run], &env);
    double_click(&mut harness, "docs-run");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("Started outside Cellworks"))
    }));

    harness.get_by_label("Continue in the background").click();
    harness.run();

    let copied = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN docs-run-copy"))
    });
    assert!(
        copied,
        "the pane shows the copy's own screen: {:?}",
        harness.state().pane_text()
    );
    let text = harness.state().pane_text().expect("the pane is open");
    assert!(text.contains("Background copy of docs-run"), "{text}");
    assert!(text.contains("close the original"), "{text}");
    assert!(
        fs::read_to_string(&env.stop_log)
            .unwrap_or_default()
            .is_empty(),
        "the original docs-run is never stopped"
    );
}

/// Stopping a session
#[test]
fn stopping_a_session() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let pricing_fix = session(
        "pricing-fix",
        Some("atch-pf"),
        vec![read_of(tree.root.join("catalog/pricing.rs"), Some(3))],
    );
    let mut harness = open_with_pane(&tree, vec![pricing_fix], &env);

    double_click(&mut harness, "pricing-fix");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN atch-pf"))
    }));

    harness.get_by_label("STOP").click();
    harness.run();

    let stopped = wait_for(&mut harness, |app| !app.pane_open());
    assert!(stopped, "stopping a session closes its pane");

    let log = fs::read_to_string(&env.stop_log).unwrap_or_default();
    assert!(log.contains("stop atch-pf"), "{log:?}");
}

/// Opening another session replaces the pane
#[test]
fn opening_another_session_replaces_the_pane() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let file_event = read_of(tree.root.join("catalog/pricing.rs"), Some(3));
    let a = session("session-a", Some("atch-a"), vec![file_event.clone()]);
    let b = session("session-b", Some("atch-b"), vec![file_event]);
    let mut harness = open_with_pane(&tree, vec![a, b], &env);

    double_click(&mut harness, "session-a");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN atch-a"))
    }));

    double_click(&mut harness, "session-b");
    let replaced = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("SESSION SCREEN atch-b"))
    });
    assert!(replaced, "{:?}", harness.state().pane_text());
    assert!(
        !harness
            .state()
            .pane_text()
            .expect("the pane is open")
            .contains("SESSION SCREEN atch-a"),
        "opening session-b replaced session-a's pane, rather than adding to it"
    );
}

/// A failed continue shows the command's output
#[test]
fn a_failed_continue_shows_the_commands_output() {
    let long_tail = "A".repeat(200) + "ZZZ_TAIL_MARKER_SHOULD_NOT_APPEAR";
    let failure = format!("FAILURE_MARKER {long_tail}");
    let env = env_with_failing_resume(&failure);
    let (tree, _unopened) = shop_city();
    let docs_run = session(
        "docs-run",
        None,
        vec![read_of(tree.root.join("catalog/pricing.rs"), None)],
    );
    let project_dir = env.transcripts.join("shop-project");
    fs::create_dir_all(&project_dir).expect("create the project's transcript folder");
    fs::write(
        project_dir.join("docs-run.jsonl"),
        prompt_line("Hello.") + "\n",
    )
    .expect("write the transcript fixture");

    let mut harness = open_with_pane(&tree, vec![docs_run], &env);
    double_click(&mut harness, "docs-run");
    assert!(wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("Started outside Cellworks"))
    }));

    harness.get_by_label("Continue in the background").click();
    harness.run();

    let failed = wait_for(&mut harness, |app| {
        app.pane_text()
            .is_some_and(|text| text.contains("FAILURE_MARKER"))
    });
    assert!(failed, "{:?}", harness.state().pane_text());
    let text = harness.state().pane_text().expect("the pane is open");
    assert!(
        !text.contains("SESSION SCREEN"),
        "a failed continue never becomes a terminal: {text}"
    );
    assert!(
        !text.contains("ZZZ_TAIL_MARKER_SHOULD_NOT_APPEAR"),
        "the output under the banner is cut to at most 200 characters: {text:?}"
    );
}

/// The sessions panel collapses to a thin strip and expands again, from its
/// toggle and from the S key; the map gains the space while it is collapsed.
#[test]
fn the_sessions_panel_collapses_and_expands() {
    let env = env();
    let (tree, _unopened) = shop_city();
    let pricing_fix = session("pricing-fix", Some("atch-pf"), Vec::new());
    let mut harness = open_with_pane(&tree, vec![pricing_fix], &env);
    let open_width = harness.state().panel().expect("the map is drawn").width();
    assert!(harness.query_by_label("pricing-fix").is_some());

    harness.get_by_label("Collapse sessions").click();
    harness.run();
    assert!(
        harness.query_by_label("pricing-fix").is_none(),
        "the collapsed panel hides its rows"
    );
    let collapsed_width = harness.state().panel().expect("the map is drawn").width();
    assert!(
        collapsed_width > open_width + 200.0,
        "the map gains the space: {open_width} -> {collapsed_width}"
    );

    harness.get_by_label("Expand sessions").click();
    harness.run();
    assert!(harness.query_by_label("pricing-fix").is_some());
    assert_eq!(
        harness.state().panel().map(|panel| panel.width()),
        Some(open_width)
    );

    harness.key_press(Key::S);
    harness.run();
    assert!(
        harness.query_by_label("pricing-fix").is_none(),
        "S collapses it"
    );
    harness.key_press(Key::S);
    harness.run();
    assert!(
        harness.query_by_label("pricing-fix").is_some(),
        "S expands it"
    );
}
