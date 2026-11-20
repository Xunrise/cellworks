//! The flat view: an open project switches between the 3D city and a 2D
//! factory floor seen from straight above, per project, keeping the
//! selection and navigation across the switch.
//!
//! The project "shop" is a real folder tree in a temporary directory (see
//! `common::shop_tree_sources`). No test runs git, reads `$HOME` or opens a
//! window; the PNG renders at the end are ignored by default.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use cellworks::app::CellworksApp;
use cellworks::opened::ViewMode;
use cellworks_workspace::{ArrangementStore, Project};
use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Recorded, Tree, labels, shop_city};

fn mode(harness: &Harness<'_, CellworksApp>) -> ViewMode {
    harness
        .state()
        .opened()
        .expect("a project is open")
        .view_mode()
}

/// A primary click at `at`, one frame per event.
fn click_at(harness: &mut Harness<'_, CellworksApp>, at: egui::Pos2) {
    harness.hover_at(at);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }
    harness.run();
}

fn flat(harness: &mut Harness<'_, CellworksApp>) {
    harness.key_press(Key::V);
    harness.run();
    assert_eq!(mode(harness), ViewMode::Flat);
}

/// V switches the open project to the flat view and back; the view drawn is
/// seen from straight above only in the flat view.
#[test]
fn v_switches_between_the_city_and_the_flat_view() {
    let (_tree, mut harness) = shop_city();
    assert_eq!(mode(&harness), ViewMode::City);
    assert!(!harness.state().view().expect("drawn").is_flat());

    flat(&mut harness);
    assert!(harness.state().view().expect("drawn").is_flat());

    harness.key_press(Key::V);
    harness.run();
    assert_eq!(mode(&harness), ViewMode::City);
    assert!(!harness.state().view().expect("drawn").is_flat());
}

/// The breadcrumb's "flat view" tab switches the mode as V does.
#[test]
fn the_breadcrumb_tab_switches_the_mode() {
    let (_tree, mut harness) = shop_city();
    harness.get_by_label("flat view").click();
    harness.run();
    assert_eq!(mode(&harness), ViewMode::Flat);

    harness.get_by_label("flat view").click();
    harness.run();
    assert_eq!(mode(&harness), ViewMode::City);
}

/// The flat view draws a labelled machine for every file of the project and
/// names its districts.
#[test]
fn the_flat_view_draws_labelled_machines() {
    let (_tree, mut harness) = shop_city();
    flat(&mut harness);
    let names = labels(&harness);
    for name in ["catalog", "orders", "pricing.rs", "mod.rs", "order.rs"] {
        assert!(
            names.iter().any(|label| label == name),
            "no label {name} in {names:?}"
        );
    }
}

/// Each camera is kept: zooming the flat view leaves the city's camera where
/// it was, and switching back finds the flat view as it was left.
#[test]
fn each_mode_keeps_its_own_camera() {
    let (_tree, mut harness) = shop_city();
    let city = harness.state().view().expect("drawn").visible;
    flat(&mut harness);
    harness.hover_at(egui::pos2(600.0, 450.0));
    harness.event(egui::Event::Zoom(1.6));
    harness.run();
    let zoomed = harness.state().view().expect("drawn").visible;

    harness.key_press(Key::V);
    harness.run();
    assert_eq!(harness.state().view().expect("drawn").visible, city);

    harness.key_press(Key::V);
    harness.run();
    let back = harness.state().view().expect("drawn").visible;
    assert!(
        (back.center() - zoomed.center()).length() < 1e-3
            && (back.width() - zoomed.width()).abs() < 1e-3,
        "the flat camera moved: {zoomed:?} became {back:?}"
    );
}

/// A selection made in the city is kept in the flat view, and Enter, a click
/// on a crate and Esc keep working there.
#[test]
fn the_selection_and_navigation_survive_a_switch() {
    let (tree, mut harness) = shop_city();
    let pricing = tree.root.join("catalog/pricing.rs");
    harness.state_mut().jump(pricing.clone());
    harness.run();
    assert_eq!(harness.state().selection(), Some(pricing.as_path()));

    flat(&mut harness);
    assert_eq!(harness.state().selection(), Some(pricing.as_path()));

    // Enter frames the machine: read in full, its functions stand as crates.
    harness.key_press(Key::Enter);
    harness.run();
    let crate_at = harness
        .state()
        .towers()
        .iter()
        .find(|tower| tower.path.ends_with("total"))
        .map(|tower| tower.outline.center())
        .expect("the crate of `total` is drawn once its machine is framed");
    click_at(&mut harness, crate_at);
    let total = harness.state().selection().expect("a crate is selected");
    assert!(total.ends_with("total"), "selected {}", total.display());

    // Back to the city: the crate's tower is still selected.
    harness.key_press(Key::V);
    harness.run();
    assert!(harness.state().selection().unwrap().ends_with("total"));

    // Esc steps back out in the flat view too.
    flat(&mut harness);
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().opened().is_some());
}

/// Every project keeps its own mode, remembered where custom arrangements
/// are kept: a project switched to the flat view opens flat again, another
/// opens as the city.
#[test]
fn each_project_keeps_its_own_mode() {
    let kept = tempfile::tempdir().expect("a temporary directory");
    let store = ArrangementStore::at(kept.path().to_path_buf());
    let (shop, mall) = (
        Tree::project("shop", &["catalog/pricing.rs"]),
        Tree::project("mall", &["stores/shoe.rs"]),
    );
    let projects = vec![shop.as_project(None), mall.as_project(None)];
    let app =
        common::app(projects.clone(), Recorded::listing(&[])).with_arrangements(store.clone());
    let mut harness = common::harness(app);
    let open = |harness: &mut Harness<'_, CellworksApp>, project: &Project| {
        harness.state_mut().open(project.clone());
        common::await_read(harness);
    };

    open(&mut harness, &projects[0]);
    flat(&mut harness);
    let started = std::time::Instant::now();
    while !store.is_flat(&projects[0].id) && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }

    open(&mut harness, &projects[1]);
    assert_eq!(mode(&harness), ViewMode::City);
    open(&mut harness, &projects[0]);
    assert_eq!(mode(&harness), ViewMode::Flat);
}

/// The flat view draws at the whole project, a folder and a file without
/// panicking, reaching every level of detail on the way in.
#[test]
fn the_flat_view_draws_at_three_zoom_levels() {
    let (tree, mut harness) = shop_city();
    flat(&mut harness);
    let whole = harness.state().view().expect("drawn").scale();

    // A file jumped to frames the folder holding it; a function, its file.
    harness
        .state_mut()
        .jump(tree.root.join("catalog/pricing.rs"));
    harness.run();
    let folder = harness.state().view().expect("drawn").scale();

    harness
        .state_mut()
        .jump(tree.root.join("catalog/pricing.rs/total"));
    harness.run();
    let file = harness.state().view().expect("drawn").scale();
    assert!(
        whole < folder && folder <= file,
        "zoom levels {whole} {folder} {file}"
    );
    assert!(!harness.state().towers().is_empty());
}

/// PNGs of the flat view of this repository at 1600x1000: fit, a district
/// and a file, written to target/flat-shots.
#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn flat_shots() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/flat-shots");
    std::fs::create_dir_all(&out).expect("create target/flat-shots");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the repository");
    let project = Project::at(root.clone());
    let git = std::sync::Arc::new(cellworks_code_analysis::GitCli);
    let app = common::app(vec![project.clone()], git);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / 20.0)
        .with_max_steps(1)
        .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
    harness.ctx.set_theme(egui::Theme::Dark);
    cellworks_workspace::theme::apply(&harness.ctx);
    harness.state_mut().open(project);
    let settle = |harness: &mut Harness<'_, CellworksApp>| {
        for _ in 0..40 {
            harness.state_mut().apply_pending();
            harness.step();
            std::thread::sleep(Duration::from_millis(25));
        }
        let started = std::time::Instant::now();
        while harness
            .state()
            .opened()
            .is_some_and(|o| o.is_reading() || o.is_preparing())
            && started.elapsed() < Duration::from_secs(120)
        {
            harness.state_mut().apply_pending();
            harness.step();
            std::thread::sleep(Duration::from_millis(50));
        }
        for _ in 0..10 {
            harness.state_mut().apply_pending();
            harness.step();
        }
    };
    settle(&mut harness);
    let snap = |harness: &mut Harness<'_, CellworksApp>, name: &str| {
        let path = out.join(format!("{name}.png"));
        harness.render().expect("render").save(&path).expect("save");
        println!("{}", path.display());
    };
    snap(&mut harness, "0-city");
    harness.key_press(Key::V);
    settle(&mut harness);
    snap(&mut harness, "1-flat-fit");
    for (name, rel) in [
        ("2-flat-folder", "crates/map/src/ui/lanes.rs"),
        ("3-flat-file", "crates/map/src/view.rs/fit_flat"),
        ("4-flat-selected", "crates/map/src/ui/flat.rs/flat"),
    ] {
        harness.state_mut().jump(root.join(rel));
        settle(&mut harness);
        snap(&mut harness, name);
    }
    // The import belts of a folder, then its call belts too.
    harness
        .state_mut()
        .jump(root.join("crates/map/src/ui/lanes.rs"));
    harness.key_press(Key::G);
    settle(&mut harness);
    snap(&mut harness, "5-flat-import-belts");
    harness.key_press(Key::A);
    settle(&mut harness);
    snap(&mut harness, "6-flat-call-belts");
}

/// PNGs of the flat view's focus over this repository at 1600x1000: a folder,
/// a file and a crate focused, then with only the context lanes, written to
/// target/focus-shots.
#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn focus_shots() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/focus-shots");
    std::fs::create_dir_all(&out).expect("create target/focus-shots");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the repository");
    let project = Project::at(root.clone());
    let git = std::sync::Arc::new(cellworks_code_analysis::GitCli);
    let app = common::app(vec![project.clone()], git);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / 20.0)
        .with_max_steps(1)
        .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
    harness.ctx.set_theme(egui::Theme::Dark);
    cellworks_workspace::theme::apply(&harness.ctx);
    harness.state_mut().open(project);
    let settle = |harness: &mut Harness<'_, CellworksApp>| {
        for _ in 0..40 {
            harness.state_mut().apply_pending();
            harness.step();
            std::thread::sleep(Duration::from_millis(25));
        }
        let started = std::time::Instant::now();
        while harness
            .state()
            .opened()
            .is_some_and(|o| o.is_reading() || o.is_preparing())
            && started.elapsed() < Duration::from_secs(120)
        {
            harness.state_mut().apply_pending();
            harness.step();
            std::thread::sleep(Duration::from_millis(50));
        }
        for _ in 0..10 {
            harness.state_mut().apply_pending();
            harness.step();
        }
    };
    settle(&mut harness);
    let snap = |harness: &mut Harness<'_, CellworksApp>, name: &str| {
        let path = out.join(format!("{name}.png"));
        harness.render().expect("render").save(&path).expect("save");
        println!("{}", path.display());
    };
    snap(&mut harness, "0-city");
    harness.key_press(Key::V);
    settle(&mut harness);
    snap(&mut harness, "0-flat-fit");
    for (name, rel) in [
        ("1-focus-folder", "crates/map/src/ui"),
        ("2-focus-file", "crates/map/src/ui/flat.rs"),
        ("3-focus-crate", "crates/editing"),
    ] {
        harness.state_mut().focus.leave();
        harness.state_mut().focus.enter(root.join(rel));
        settle(&mut harness);
        snap(&mut harness, name);
    }
    harness.state_mut().focus.links.imports = false;
    harness.state_mut().focus.links.calls = false;
    settle(&mut harness);
    snap(&mut harness, "4-focus-context-only");
}
