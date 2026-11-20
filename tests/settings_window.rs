//! The settings window (Cmd+,): it opens and closes from the keyboard, shows
//! one section at a time, saves what is changed to a settings file, puts the
//! change to work, and leaves the user's own settings file and keychain alone.
//!
//! Every test points the app at a settings file in a temporary directory and
//! an empty environment; none runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use cellworks::app::CellworksApp;
use cellworks::opened::ViewMode;
use cellworks_sources::tickets::SourceId;
use cellworks_workspace::{Settings, SettingsStore};
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Recorded, Tree};

/// A settings file in a fresh temporary directory, holding `text` if any.
fn settings_file(text: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("Cellworks").join("settings.toml");
    if let Some(text) = text {
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        std::fs::write(&path, text).expect("write the settings");
    }
    (dir, path)
}

fn launcher_with_settings(path: &std::path::Path) -> Harness<'static, CellworksApp> {
    let app = common::app(vec![], Recorded::listing(&[])).with_settings_file(path.to_owned());
    common::harness(app)
}

/// Frames enough for the debounce (0.3 s at 60 frames a second) to pass.
fn let_it_save(harness: &mut Harness<'_, CellworksApp>) {
    for _ in 0..40 {
        harness.step();
    }
}

#[test]
fn cmd_comma_opens_the_window_and_escape_closes_it() {
    let (_dir, path) = settings_file(None);
    let mut harness = launcher_with_settings(&path);
    assert!(!harness.state().settings_open());

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings_open());
    assert!(harness.query_by_label("Editor & cases").is_some());

    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings_open());

    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(!harness.state().settings_open(), "Cmd+, toggles");
}

#[test]
fn the_launcher_header_has_a_settings_tab() {
    let (_dir, path) = settings_file(None);
    let mut harness = launcher_with_settings(&path);

    harness.get_by_label("Settings").click();
    harness.run();

    assert!(harness.state().settings_open());
}

#[test]
fn a_section_shows_its_own_form() {
    let (_dir, path) = settings_file(None);
    let mut harness = launcher_with_settings(&path);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();

    harness.get_by_label("Tickets").click();
    harness.run();
    assert!(harness.query_by_label("Cockpit enabled").is_some());
    assert!(harness.query_by_label("Show import lanes").is_none());

    harness.get_by_label("Map").click();
    harness.run();
    assert!(harness.query_by_label("Show import lanes").is_some());
    assert!(harness.query_by_label("Cockpit enabled").is_none());

    harness.get_by_label("Updates").click();
    harness.run();
    assert!(harness.query_by_label("Check now").is_some());
}

#[test]
fn a_map_default_is_saved_to_the_settings_file() {
    let (_dir, path) = settings_file(Some("future_key = 7\n"));
    let mut harness = launcher_with_settings(&path);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    harness.get_by_label("Map").click();
    harness.run();

    harness.get_by_label("Show import lanes").click();
    harness.run();
    let_it_save(&mut harness);

    assert!(harness.state().settings().map.import_lanes);
    let saved = SettingsStore::at(&path).load().settings;
    assert!(saved.map.import_lanes, "the toggle reached the file");
    let text = std::fs::read_to_string(&path).expect("the file");
    assert!(
        text.contains("future_key"),
        "a key the window does not own survives"
    );
}

#[test]
fn closing_the_window_saves_at_once() {
    let (_dir, path) = settings_file(None);
    let mut harness = launcher_with_settings(&path);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    harness.get_by_label("Map").click();
    harness.run();
    harness.get_by_label("Start underground").click();
    harness.step();
    harness.key_press(Key::Escape);
    harness.run();

    assert!(SettingsStore::at(&path).load().settings.map.underground);
}

#[test]
fn switching_a_tickets_source_off_reconfigures_the_facade() {
    let (_dir, path) = settings_file(None);
    let mut harness = launcher_with_settings(&path);
    assert!(
        harness
            .state()
            .ticket_sources()
            .contains(&SourceId::Cockpit)
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    harness.get_by_label("Tickets").click();
    harness.run();

    harness.get_by_label("Cockpit enabled").click();
    harness.run();
    assert!(
        !harness
            .state()
            .ticket_sources()
            .contains(&SourceId::Cockpit)
    );
    assert!(harness.state().ticket_sources().contains(&SourceId::GitHub));

    harness.get_by_label("Cockpit enabled").click();
    harness.run();
    assert!(
        harness
            .state()
            .ticket_sources()
            .contains(&SourceId::Cockpit)
    );
}

#[test]
fn a_field_the_environment_has_taken_over_says_so() {
    let (_dir, path) = settings_file(None);
    let env = cellworks_workspace::Env::of([("CELLWORKS_COCKPIT_URL", "http://127.0.0.1:1")]);
    let app = common::app(vec![], Recorded::listing(&[])).with_settings(
        path,
        env,
        std::sync::Arc::new(cellworks_workspace::MemoryKeychain::default()),
    );
    let mut harness = common::harness(app);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    harness.get_by_label("Tickets").click();
    harness.run();

    assert!(
        harness
            .query_by_label_contains("CELLWORKS_COCKPIT_URL")
            .is_some()
    );
}

#[test]
fn a_new_project_opens_with_the_map_defaults() {
    let (_dir, path) = settings_file(Some(
        "[map]\ndefault_view = \"flat\"\ndefault_lens = \"imports\"\ncall_lanes = true\n",
    ));
    let tree = Tree::project("shop", &["src/a.rs", "src/b.rs"]);
    let project = tree.as_project(None);
    let name = project.name.clone();
    let app = common::app(vec![project], Recorded::listing(&["src/a.rs", "src/b.rs"]))
        .with_settings_file(path);
    let mut harness = common::harness(app);
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);

    let opened = harness.state().opened().expect("a project is open");
    assert_eq!(opened.view_mode(), ViewMode::Flat);
    assert_eq!(opened.lens(), cellworks_map::Lens::Imports);
    assert!(opened.lanes().calls && opened.lanes().imports);
}

#[test]
fn a_projects_own_saved_view_beats_the_default() {
    let (_dir, path) = settings_file(Some("[map]\ndefault_view = \"flat\"\n"));
    let tree = Tree::project("shop", &["src/a.rs"]);
    let project = tree.as_project(None);
    let name = project.name.clone();
    let arrangements = tempfile::tempdir().expect("a temporary directory");
    let store = cellworks_workspace::ArrangementStore::at(arrangements.path().to_owned());
    store.keep_flat(&project.id, false).expect("kept");
    let app = common::app(vec![project], Recorded::listing(&["src/a.rs"]))
        .with_settings_file(path)
        .with_arrangements(store);
    let mut harness = common::harness(app);
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);

    let opened = harness.state().opened().expect("a project is open");
    assert_eq!(opened.view_mode(), ViewMode::City);
}

#[test]
fn a_garbled_settings_file_is_not_overwritten() {
    let (_dir, path) = settings_file(Some("this is [not toml"));
    let mut harness = launcher_with_settings(&path);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    harness.get_by_label("Map").click();
    harness.run();
    harness.get_by_label("Show import lanes").click();
    harness.run();
    let_it_save(&mut harness);

    assert_eq!(
        std::fs::read_to_string(&path).expect("the file"),
        "this is [not toml"
    );
    assert_eq!(*harness.state().settings(), {
        let mut expected = Settings::default();
        expected.map.import_lanes = true;
        expected
    });
}

/// PNGs of every section at 1600x1000, under `target/settings-shots/`.
#[test]
#[ignore = "renders PNGs: cargo test --test settings_window settings_shots -- --ignored"]
fn settings_shots() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/settings-shots");
    std::fs::create_dir_all(&out).expect("the output folder");
    let (_dir, path) = settings_file(None);
    let app = common::app(vec![], Recorded::listing(&[])).with_settings_file(path);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / 20.0)
        .with_max_steps(1)
        .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
    harness.ctx.set_theme(egui::Theme::Dark);
    cellworks_workspace::theme::apply(&harness.ctx);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    for _ in 0..5 {
        harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    for (n, section) in [
        "General",
        "Projects",
        "Tickets",
        "Map",
        "Editor & cases",
        "Updates",
    ]
    .into_iter()
    .enumerate()
    {
        harness.get_by_label(section).click();
        for _ in 0..5 {
            harness.step();
        }
        let file = out.join(format!(
            "{}-{}.png",
            n + 1,
            section.split(' ').next().unwrap().to_lowercase()
        ));
        harness.render().expect("render").save(&file).expect("save");
        println!("{}", file.display());
        if section == "Tickets" {
            // The rest of the form, below the fold.
            harness.hover_at(egui::pos2(900.0, 600.0));
            harness.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -600.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            });
            for _ in 0..5 {
                harness.step();
            }
            let file = out.join("3b-tickets-linear.png");
            harness.render().expect("render").save(&file).expect("save");
            println!("{}", file.display());
        }
    }
}
