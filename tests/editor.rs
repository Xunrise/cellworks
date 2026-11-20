//! Entering a file opens it in one editor over the map, in the city and the
//! flat view alike: a double-click on a tower or a crate opens its file at
//! its first line, the editor stays open across V, a popped-out editor stays
//! open while the next file opens, and Esc leaves the editor before the map.
//!
//! Tests read files as text (`Editors::read_only`); they never start Neovim
//! and never open a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_navigation::Layer;
use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{map, shop_city};

/// Double-clicks the first tower or crate whose path holds `name`. It is clicked
/// once first: selecting it opens the why panel, which moves the map.
fn enter(harness: &mut Harness<'_, CellworksApp>, name: &str) {
    click_at(harness, drawn(harness, name), 1);
    click_at(harness, drawn(harness, name), 2);
}

/// `clicks` primary clicks at `at`, one frame per event.
fn click_at(harness: &mut Harness<'_, CellworksApp>, at: egui::Pos2, clicks: usize) {
    harness.hover_at(at);
    for _ in 0..clicks {
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            harness.step();
        }
    }
    harness.run();
}

/// A point on the first tower or crate whose path holds `name`: in the city a
/// tower is picked by its lines, in the flat view a crate by its outline.
fn drawn(harness: &Harness<'_, CellworksApp>, name: &str) -> egui::Pos2 {
    let tower = harness
        .state()
        .towers()
        .iter()
        .find(|tower| tower.path.to_string_lossy().contains(name))
        .unwrap_or_else(|| panic!("{name} is not drawn"));
    if tower.floors.is_empty() {
        return tower.outline.center();
    }
    // A point on one of its lines, clear of every other tower's.
    let others: Vec<_> = harness
        .state()
        .towers()
        .iter()
        .filter(|other| other.path != tower.path)
        .collect();
    for floor in &tower.floors {
        for step in 0..=20 {
            let point = floor.start + (floor.end - floor.start) * (step as f32 / 20.0);
            let clear = others
                .iter()
                .all(|other| other.floors.iter().all(|f| f.distance(point) > 6.0));
            if floor.distance(point) <= 1.0 && clear {
                return point;
            }
        }
    }
    panic!("no line of {name} is clear of the others")
}

#[test]
fn entering_a_file_opens_it_in_one_editor_that_stays_across_the_views() {
    let (tree, mut harness) = shop_city();
    let total = tree.root.join("catalog/pricing.rs/total");
    let line = map(&harness).item(&total).expect("total").floors[0].number;

    enter(&mut harness, "total");

    let open = harness.state().editors.shown();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].path, tree.root.join("catalog/pricing.rs"));
    assert_eq!(
        open[0].line, line,
        "the file opens at the tower's first line"
    );
    assert!(!open[0].popped);
    assert!(harness.state().editors.has_keyboard());

    // The map underneath keeps its own keys only once the editor lets go.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().editors.has_keyboard());

    // The flat view shows the same editor.
    harness.key_press(Key::V);
    harness.run();
    assert_eq!(harness.state().editors.shown(), open);

    // Popped out, it stays open while the next file opens over the map.
    harness.state_mut().editors.pop_out();
    harness.run();
    harness.state_mut().jump(tree.root.join("orders/order.rs"));
    harness.run();
    // A file is entered with Enter as well: the first frames it, the second
    // opens it.
    if harness.state().navigation().layer() != Layer::File {
        harness.key_press(Key::Enter);
        harness.run();
    }
    assert_eq!(harness.state().navigation().layer(), Layer::File);
    harness.key_press(Key::Enter);
    harness.run();
    let open = harness.state().editors.shown();
    assert_eq!(open.len(), 2, "{open:?}");
    assert!(open[0].path.ends_with("orders/order.rs") && !open[0].popped);
    assert!(open[1].path.ends_with("catalog/pricing.rs") && open[1].popped);

    // And both are still there back in the city.
    harness.key_press(Key::Escape);
    harness.key_press(Key::V);
    harness.run();
    assert_eq!(harness.state().editors.shown(), open);
}

#[test]
fn esc_leaves_the_editor_before_it_moves_the_map() {
    let (_tree, mut harness) = shop_city();
    enter(&mut harness, "total");
    let layer = harness.state().navigation().layer();

    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(
        harness.state().navigation().layer(),
        layer,
        "the first Esc only leaves the editor"
    );

    harness.key_press(Key::Escape);
    harness.run();
    assert_ne!(harness.state().navigation().layer(), layer);
    assert_ne!(layer, Layer::Universe);
}

/// PNGs of the editor over this repository at 1600x1000, with the Neovim and
/// configuration of whoever runs it: over the city, full-panel, over the flat
/// view, and popped out. Written to target/editor-shots.
#[test]
#[ignore = "renders PNGs through wgpu and starts nvim; run on demand"]
fn editor_shots() {
    use std::time::Duration;
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/editor-shots");
    std::fs::create_dir_all(&out).expect("create target/editor-shots");
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("the repository");
    let project = cellworks_workspace::Project::at(root.clone());
    let git = std::sync::Arc::new(cellworks_code_analysis::GitCli);
    let mut app = common::app(vec![project.clone()], git);
    app.editors = cellworks_editing::Editors::new();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / 20.0)
        .with_max_steps(1)
        .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
    harness.ctx.set_theme(egui::Theme::Dark);
    cellworks_workspace::theme::apply(&harness.ctx);
    harness.state_mut().open(project);
    let settle = |harness: &mut Harness<'_, CellworksApp>| {
        let started = std::time::Instant::now();
        loop {
            harness.state_mut().apply_pending();
            harness.step();
            std::thread::sleep(Duration::from_millis(25));
            let busy = harness
                .state()
                .opened()
                .is_some_and(|o| o.is_reading() || o.is_preparing());
            if (!busy && started.elapsed() > Duration::from_secs(2))
                || started.elapsed() > Duration::from_secs(120)
            {
                break;
            }
        }
    };
    let snap = |harness: &mut Harness<'_, CellworksApp>, name: &str| {
        let path = out.join(format!("{name}.png"));
        harness.render().expect("render").save(&path).expect("save");
        println!("{}", path.display());
    };
    settle(&mut harness);
    harness
        .state_mut()
        .jump(root.join("crates/editing/src/nvim.rs"));
    settle(&mut harness);
    if harness.state().navigation().layer() != Layer::File {
        harness.key_press(Key::Enter);
        settle(&mut harness);
    }
    harness.key_press(Key::Enter);
    settle(&mut harness);
    snap(&mut harness, "0-city-overlay");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::F);
    settle(&mut harness);
    snap(&mut harness, "1-city-full-panel");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::F);
    harness.state_mut().editors.blur();
    harness.key_press(Key::V);
    settle(&mut harness);
    snap(&mut harness, "2-flat-overlay");
    harness.state_mut().editors.pop_out();
    harness.state_mut().editors.blur();
    settle(&mut harness);
    snap(&mut harness, "3-flat-popped");
}

/// In the flat view a double-click focuses: a folder, its files in the
/// middle, then one of them; Enter opens the selected file in the editor, and
/// Esc steps back out one focus at a time to the floor.
#[test]
fn a_folder_double_clicked_in_the_flat_view_is_focused_until_esc() {
    let (tree, mut harness) = shop_city();
    // The Explorer stands over the left of the floor: out of the way.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::B);
    harness.key_press(Key::V);
    harness.run();
    let catalog = tree.root.join("catalog");
    let area = map(&harness).find(&catalog).expect("catalog").area;
    let view = harness.state().view().expect("drawn");
    // Just inside the hall's corner, clear of its machines.
    let corner = view.to_screen(area.min, 0.0) + egui::vec2(3.0, 3.0);
    click_at(&mut harness, corner, 1);
    let view = harness.state().view().expect("drawn");
    let corner = view.to_screen(area.min, 0.0) + egui::vec2(3.0, 3.0);
    click_at(&mut harness, corner, 2);
    assert_eq!(harness.state().focus.path(), Some(catalog.as_path()));

    // Its files stand in the middle, by name.
    let card = |harness: &Harness<'_, CellworksApp>| {
        let card = harness
            .get_all_by_label("pricing.rs")
            .last()
            .expect("pricing.rs in the middle");
        let at = card.accesskit_node().bounding_box().expect("laid out");
        egui::pos2(
            ((at.x0 + at.x1) / 2.0) as f32,
            ((at.y0 + at.y1) / 2.0) as f32,
        )
    };
    // Selected first: the why panel it opens moves the map.
    let at = card(&harness);
    click_at(&mut harness, at, 1);
    let at = card(&harness);
    click_at(&mut harness, at, 2);
    let pricing = tree.root.join("catalog/pricing.rs");
    assert_eq!(
        harness.state().focus.path(),
        Some(pricing.as_path()),
        "a double-click focuses the file, it does not open it"
    );
    assert!(harness.state().editors.shown().is_empty());

    // Enter opens the selected file.
    harness.key_press(Key::Enter);
    harness.run();
    let open = harness.state().editors.shown();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].path, pricing);
    assert_eq!(harness.state().focus.path(), Some(pricing.as_path()));

    // Esc leaves the editor, then steps back out one focus at a time.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().editors.has_keyboard());
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().focus.path(), Some(catalog.as_path()));
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(
        harness.state().focus.path(),
        None,
        "Esc steps back to the floor"
    );
    assert!(harness.state().opened().is_some());
}
