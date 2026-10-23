//! STORY-057, every file stands as a tower: a file in a language no parser
//! reads stands as one tower of its lines, a file a parser reads keeps its
//! towers, and a file without text stays a bare block.
//!
//! One test per scenario of issue #82, named after it. The project "fleet"
//! is a real folder tree in a temporary directory. No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use std::fs;

use cellworks::app::CellworksApp;
use cellworks_map::ItemKind;
use egui_kittest::Harness;

use common::{Recorded, Tree, map, open, tower};

const MAIN: &str = "\
fn main() {
    start();
}

fn start() {
    let ready = true;
}
";

/// The background: "fleet" holds "src/main.rs" with two functions and
/// "web/app.rb", 40 lines no parser reads, and is open as a city.
fn fleet(more: &[(&str, &[u8])]) -> (Tree, Harness<'static, CellworksApp>) {
    let app: String = (1..=40)
        .map(|n| format!("export const line{n} = {n};\n"))
        .collect();
    let tree = Tree::sources("fleet", &[("src/main.rs", MAIN), ("web/app.rb", &app)]);
    for (file, bytes) in more {
        fs::write(tree.root.join(file), bytes).expect("write the file");
    }
    let harness = open(&tree, None, Recorded::listing(&[]));
    (tree, harness)
}

/// A file no parser reads stands as one tower
#[test]
fn a_file_no_parser_reads_stands_as_one_tower() {
    let (tree, harness) = fleet(&[]);

    let map = map(&harness);
    let app = tree.root.join("web/app.rb");
    let items = map.items_of(&app).expect("a block for app.rb");
    assert_eq!(items.len(), 1, "one tower");
    assert_eq!(items[0].name, "app.rb");
    assert_eq!(items[0].kind, ItemKind::Text);
    assert_eq!(items[0].floors.len(), 40, "a floor per line");
    assert_eq!(
        tower(&harness, &items[0].path).floors.len(),
        40,
        "it is drawn"
    );
}

/// A file a parser reads keeps its towers
#[test]
fn a_file_a_parser_reads_keeps_its_towers() {
    let (tree, harness) = fleet(&[]);

    let map = map(&harness);
    let main = tree.root.join("src/main.rs");
    let names: Vec<&str> = map
        .items_of(&main)
        .expect("a block for main.rs")
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, ["main", "start"]);
}

/// A file without text stands as a bare block
#[test]
fn a_file_without_text_stands_as_a_bare_block() {
    let png: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0xff];
    let (tree, harness) = fleet(&[("web/logo.png", png)]);

    let map = map(&harness);
    let logo = tree.root.join("web/logo.png");
    assert_eq!(
        map.items_of(&logo).map(<[_]>::len),
        Some(0),
        "a block, no tower"
    );
}
