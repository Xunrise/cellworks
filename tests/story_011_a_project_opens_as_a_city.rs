//! STORY-011, a project opens as a city: each function is a tower with one
//! floor per line, files are blocks and crates are districts.
//!
//! One test per scenario of issue #33, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`). No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks_map::{Element, ItemKind, ProjectMap};

use common::{map, shop_city, tower};

fn district_of(map: &ProjectMap, path: &Path) -> PathBuf {
    let mut at = path.to_path_buf();
    while let Some(parent) = map.find(&at).and_then(|found| found.parent) {
        at = parent.to_path_buf();
    }
    at
}

/// Each function is a tower with one floor per line
#[test]
fn each_function_is_a_tower_with_one_floor_per_line() {
    let (tree, harness) = shop_city();
    let pricing = tree.root.join("catalog/pricing.rs");

    assert_eq!(tower(&harness, &pricing.join("discount")).floors.len(), 12);
    assert_eq!(tower(&harness, &pricing.join("total")).floors.len(), 40);
}

/// A floor starts at its line's indentation
#[test]
fn a_floor_starts_at_its_lines_indentation() {
    let (tree, harness) = shop_city();

    let discount = tower(&harness, &tree.root.join("catalog/pricing.rs/discount"));

    assert!(discount.floors[2].start.x > discount.floors[1].start.x);
}

/// Files are blocks and crates are districts
#[test]
fn files_are_blocks_and_crates_are_districts() {
    let (tree, harness) = shop_city();
    let map = map(&harness);
    let block = tree.root.join("catalog/pricing.rs");

    for name in ["discount", "total"] {
        let found = map.find(&block.join(name)).expect("the tower");
        assert_eq!(found.kind, Element::Item);
        assert_eq!(found.parent, Some(block.as_path()), "{name}");
    }
    let found = map.find(&block).expect("the block");
    assert_eq!(found.kind, Element::Node);
    assert_eq!(found.parent, Some(tree.root.join("catalog").as_path()));
}

/// Every tower takes the colour of its crate
#[test]
fn every_tower_takes_the_colour_of_its_crate() {
    let (tree, harness) = shop_city();
    let map = map(&harness);
    let colours = |district: &str| {
        let district = tree.root.join(district);
        let mut colours: Vec<_> = harness
            .state()
            .towers()
            .iter()
            .filter(|tower| district_of(&map, &tower.path) == district)
            .map(|tower| tower.colour)
            .collect();
        colours.dedup();
        colours
    };

    let catalog = colours("catalog");
    let orders = colours("orders");

    assert_eq!(catalog.len(), 1, "{catalog:?}");
    assert_eq!(orders.len(), 1, "{orders:?}");
    assert_ne!(catalog, orders);
}

/// A file without functions still has a block
#[test]
fn a_file_without_functions_still_has_a_block() {
    let (tree, harness) = shop_city();
    let map = map(&harness);
    let block = tree.root.join("catalog/mod.rs");

    assert_eq!(
        map.find(&block).map(|found| found.kind),
        Some(Element::Node)
    );
    let uses = map
        .find(&block.join("use"))
        .expect("a tower for the use statements");
    assert_eq!(uses.parent, Some(block.as_path()));
    let low = tower(&harness, &block.join("use")).floors.len();
    let discount = tower(&harness, &tree.root.join("catalog/pricing.rs/discount"));
    assert!(low < discount.floors.len(), "{low} floors");
    let kind = map.districts[0]
        .nodes
        .iter()
        .find(|node| node.path == block)
        .map(|node| node.items[0].kind);
    assert_eq!(kind, Some(ItemKind::Uses));
}
