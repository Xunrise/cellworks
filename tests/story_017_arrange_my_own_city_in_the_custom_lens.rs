//! STORY-017, arrange my own city in the custom lens: it starts as the
//! folder city, a moved block stays where it was put with its towers, a
//! district grows to keep a block moved out of it, a moved district takes
//! its blocks along, dragging empty ground moves the view, the arrangement
//! is there the next time, and no file of the project changes.
//!
//! One test per scenario of issue #39, named after it. The project "shop"
//! has the districts "catalog" and "orders" in a temporary directory; the
//! arrangement is kept in a temporary folder of its own. No test runs git,
//! reads `$HOME` or opens a window.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cellworks::app::CellworksApp;
use cellworks_map::{Lens, ProjectMap};
use cellworks_workspace::ArrangementStore;
use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2, vec2};
use egui_kittest::Harness;

use common::{Recorded, Tree};

const CODE: &str = "pub fn work() {\n    let done = true;\n}\n";

fn tree() -> Tree {
    Tree::sources(
        "shop",
        &[
            ("catalog/pricing.rs", CODE),
            ("catalog/discount.rs", CODE),
            ("orders/order.rs", CODE),
            ("orders/ship.rs", CODE),
        ],
    )
}

/// Opens `tree`, its arrangement kept in `kept`, and switches to the
/// custom lens.
fn open(tree: &Tree, kept: &Path) -> Harness<'static, CellworksApp> {
    let project = tree.as_project(None);
    let name = project.name.clone();
    let app = common::app(vec![project], Recorded::listing(&[]))
        .with_arrangements(ArrangementStore::at(kept.to_path_buf()));
    let mut harness = common::harness(app);
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    for _ in 0..Lens::ALL.len() {
        if harness.state().opened().map(|o| o.lens()) == Some(Lens::Custom) {
            break;
        }
        harness.key_press(egui::Key::L);
        common::await_lens(&mut harness);
        harness.run_steps(120);
    }
    assert_eq!(
        harness.state().opened().map(|o| o.lens()),
        Some(Lens::Custom)
    );
    harness
}

fn area(map: &ProjectMap, path: &Path) -> Rect {
    map.find(path)
        .unwrap_or_else(|| panic!("no {}", path.display()))
        .area
}

/// Where the ground point `ground` is on screen.
fn on_screen(harness: &Harness<'_, CellworksApp>, ground: Pos2) -> Pos2 {
    harness
        .state()
        .view()
        .expect("a view")
        .to_screen(ground, 0.0)
}

/// The engineer drags from `from` to `to` on screen.
fn drag(harness: &mut Harness<'_, CellworksApp>, from: Pos2, to: Pos2) {
    harness.event(Event::PointerMoved(from));
    harness.step();
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    harness.event(button(from, true));
    harness.step();
    for k in 1..=10 {
        harness.event(Event::PointerMoved(from + (to - from) * (k as f32 / 10.0)));
        harness.step();
    }
    harness.event(button(to, false));
    harness.step();
    harness.run_steps(5);
}

/// How far the engineer drags on screen to move something `by` on the
/// ground.
fn screen_delta(harness: &Harness<'_, CellworksApp>, at: Pos2, by: Vec2) -> Vec2 {
    on_screen(harness, at + by) - on_screen(harness, at)
}

/// Where the engineer grabs `block`: by its back corner, the part highest on
/// screen, clear of a district's name lying over the front of its district.
fn grip(block: Rect) -> Pos2 {
    block.min + vec2(1.5, 1.5)
}

fn near(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1.0
}

/// The custom lens starts as the folder city
#[test]
fn the_custom_lens_starts_as_the_folder_city() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let folders = {
        let mut harness = open(&tree, kept.path());
        harness.key_press(egui::Key::L);
        common::await_lens(&mut harness);
        harness.run_steps(120);
        common::map(&harness)
    };

    let harness = open(&tree, kept.path());

    let map = common::map(&harness);
    for district in &folders.districts {
        assert_eq!(
            area(&map, &district.path),
            district.area,
            "{}",
            district.name
        );
        for node in &district.nodes {
            assert_eq!(area(&map, &node.path), node.area, "{}", node.name);
        }
    }
}

/// Moving a block keeps it where it was put
#[test]
fn moving_a_block_keeps_it_where_it_was_put() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let mut harness = open(&tree, kept.path());
    let pricing = tree.root.join("catalog/pricing.rs");
    let tower = pricing.join("work");
    let before = common::map(&harness);
    let (block, building) = (area(&before, &pricing), area(&before, &tower));

    let east = vec2(30.0, 0.0);
    let from = on_screen(&harness, grip(block));
    let to = from + screen_delta(&harness, block.center(), east);
    drag(&mut harness, from, to);

    let after = common::map(&harness);
    let moved = area(&after, &pricing).center() - block.center();
    assert!(near(moved, east), "moved by {moved:?}");
    assert!(
        near(area(&after, &tower).center() - building.center(), moved),
        "its towers moved with it"
    );
}

/// A district grows to keep a block moved out of it
#[test]
fn a_district_grows_to_keep_a_block_moved_out_of_it() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let mut harness = open(&tree, kept.path());
    let (catalog, pricing) = (
        tree.root.join("catalog"),
        tree.root.join("catalog/pricing.rs"),
    );
    let before = common::map(&harness);
    let block = area(&before, &pricing);
    let out = vec2(area(&before, &catalog).width(), 0.0);

    let from = on_screen(&harness, grip(block));
    let to = from + screen_delta(&harness, block.center(), out);
    drag(&mut harness, from, to);

    let after = common::map(&harness);
    let (district, moved) = (area(&after, &catalog), area(&after, &pricing));
    assert!(
        district.contains_rect(moved),
        "{moved:?} outside {district:?}"
    );
    assert_eq!(
        after.find(&pricing).and_then(|f| f.parent),
        Some(catalog.as_path())
    );
}

/// Moving a district moves all its blocks
#[test]
fn moving_a_district_moves_all_its_blocks() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let mut harness = open(&tree, kept.path());
    let orders = tree.root.join("orders");
    let before = common::map(&harness);
    let plaza = area(&before, &orders).center();

    let by = vec2(0.0, 40.0);
    let from = on_screen(&harness, plaza);
    let to = from + screen_delta(&harness, plaza, by);
    drag(&mut harness, from, to);

    let after = common::map(&harness);
    let district = before
        .districts
        .iter()
        .find(|d| d.path == orders)
        .expect("orders");
    for node in &district.nodes {
        let moved = area(&after, &node.path).center() - node.area.center();
        assert!(near(moved, by), "{} moved by {moved:?}", node.name);
    }
}

/// Moving across empty ground moves the camera, not the city
#[test]
fn moving_across_empty_ground_moves_the_camera_not_the_city() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let mut harness = open(&tree, kept.path());
    let before = common::map(&harness);
    let seen = harness.state().view().expect("a view").visible;

    drag(
        &mut harness,
        Pos2::new(40.0, 860.0),
        Pos2::new(140.0, 820.0),
    );

    assert_ne!(
        harness.state().view().expect("a view").visible,
        seen,
        "the view moved"
    );
    let after = common::map(&harness);
    for district in &before.districts {
        assert_eq!(
            area(&after, &district.path),
            district.area,
            "{}",
            district.name
        );
    }
}

/// My arrangement is there the next time
#[test]
fn my_arrangement_is_there_the_next_time() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let pricing = tree.root.join("catalog/pricing.rs");
    let put = {
        let mut harness = open(&tree, kept.path());
        let block = area(&common::map(&harness), &pricing);
        let from = on_screen(&harness, grip(block));
        let to = from + screen_delta(&harness, block.center(), vec2(25.0, 10.0));
        drag(&mut harness, from, to);
        area(&common::map(&harness), &pricing)
    };
    // The arrangement is kept off the frame loop.
    let started = Instant::now();
    while fs::read_dir(kept.path()).map_or(0, |files| files.count()) == 0
        && started.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut harness = open(&tree, kept.path());
    for _ in 0..50 {
        harness.state_mut().apply_pending();
        harness.step();
    }

    assert_eq!(area(&common::map(&harness), &pricing), put);
}

/// Moving blocks in my city changes no code
#[test]
fn moving_blocks_in_my_city_changes_no_code() {
    let tree = tree();
    let kept = tempfile::tempdir().expect("a folder for arrangements");
    let files = |root: &Path| -> BTreeMap<PathBuf, (Vec<u8>, std::time::SystemTime)> {
        let mut all = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(folder) = pending.pop() {
            for entry in fs::read_dir(&folder).expect("readable").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let modified = entry.metadata().and_then(|m| m.modified()).expect("a time");
                    all.insert(path.clone(), (fs::read(&path).expect("readable"), modified));
                }
            }
        }
        all
    };
    let before = files(&tree.root);
    let mut harness = open(&tree, kept.path());
    let pricing = tree.root.join("catalog/pricing.rs");
    let block = area(&common::map(&harness), &pricing);

    let from = on_screen(&harness, grip(block));
    let to = from + screen_delta(&harness, block.center(), vec2(20.0, 0.0));
    drag(&mut harness, from, to);
    std::thread::sleep(Duration::from_millis(100));

    assert_eq!(files(&tree.root), before);
}
