//! STORY-005, show or hide ignored files on the map: I switches git-ignored
//! files and hidden folders on and off while a project is open.
//!
//! One test per scenario of issue #16, named after it. Each project is a real
//! folder tree in a temporary directory with a `.git` folder, and git is a
//! recorded listing of the files it does not ignore. No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_map::ProjectMap;
use cellworks_navigation::Layer;
use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Recorded, Tree, await_read, labels, map, open};

/// The user switches ignored files on or off, and the read again finishes.
fn switch(harness: &mut Harness<'_, CellworksApp>) {
    harness.key_press(Key::I);
    harness.run();
    await_read(harness);
}

/// The names of every node the district `name` holds, at any depth.
fn held_by(map: &ProjectMap, name: &str) -> Vec<String> {
    let district = map
        .districts
        .iter()
        .find(|district| district.name == name)
        .unwrap_or_else(|| panic!("no district {name}"));
    let mut held: Vec<String> = district.nodes.iter().map(|n| n.name.clone()).collect();
    for sub in &district.sub_districts {
        held.extend(sub.nodes.iter().map(|n| n.name.clone()));
        for group in &sub.groups {
            held.extend(group.nodes.iter().map(|n| n.name.clone()));
        }
    }
    held
}

fn catalog() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::repository("shop", &["src/catalog/item.rs", "src/catalog/item.log"]);
    let harness = open(&tree, None, Recorded::listing(&["src/catalog/item.rs"]));
    (tree, harness)
}

/// Ignored files stay out of the way by default
#[test]
fn ignored_files_stay_out_of_the_way_by_default() {
    let (_tree, harness) = catalog();

    let held = held_by(&map(&harness), "catalog");

    assert_eq!(held, ["item.rs"]);
}

/// Showing ignored files puts them where they belong
#[test]
fn showing_ignored_files_puts_them_where_they_belong() {
    let (_tree, mut harness) = catalog();

    switch(&mut harness);

    let mut held = held_by(&map(&harness), "catalog");
    held.sort();
    assert_eq!(held, ["item.log", "item.rs"]);
    // Like any other node: drawn, selectable, and Enter frames it.
    assert!(labels(&harness).iter().any(|label| label == "item.log"));
    harness.get_by_label("item.log").click();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().navigation().layer(), Layer::File);
}

/// An ignored folder comes back with its contents
#[test]
fn an_ignored_folder_comes_back_with_its_contents() {
    let tree = Tree::repository("shop", &["src/catalog/item.rs", "target/build.rs"]);
    let mut harness = open(&tree, None, Recorded::listing(&["src/catalog/item.rs"]));

    switch(&mut harness);

    assert_eq!(held_by(&map(&harness), "target"), ["build.rs"]);
}

/// Hiding them again clears them
#[test]
fn hiding_them_again_clears_them() {
    let tree = Tree::repository(
        "shop",
        &[
            "src/catalog/item.rs",
            "src/catalog/item.log",
            "target/build.rs",
            ".cache/stale.bin",
        ],
    );
    let mut harness = open(&tree, None, Recorded::listing(&["src/catalog/item.rs"]));
    switch(&mut harness);

    switch(&mut harness);

    let map = map(&harness);
    let districts: Vec<&str> = map.districts.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(districts, ["catalog"]);
    assert_eq!(held_by(&map, "catalog"), ["item.rs"]);
}

/// Hiding them again while one is framed lets go of it: the project is framed
/// whole, nothing is selected, and Esc closes the project.
#[test]
fn hiding_them_again_while_one_is_framed_frames_the_project() {
    let tree = Tree::repository("shop", &["src/catalog/item.rs", "target/build.rs"]);
    let mut harness = open(&tree, None, Recorded::listing(&["src/catalog/item.rs"]));
    switch(&mut harness);
    for name in ["target", "build.rs"] {
        harness.get_by_label(name).click();
        harness.run();
        harness.key_press(Key::Enter);
        harness.run();
    }
    assert_eq!(harness.state().navigation().layer(), Layer::File);

    switch(&mut harness);

    assert_eq!(harness.state().navigation().layer(), Layer::Project);
    assert_eq!(harness.state().navigation().selection(), None);
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().navigation().layer(), Layer::Universe);
}
