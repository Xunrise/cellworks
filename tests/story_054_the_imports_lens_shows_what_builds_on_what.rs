//! STORY-054, the imports lens shows what builds on what: the foundation
//! stands on the ground and what imports it a step higher, imports both
//! ways lift by their net direction, files importing each other in a loop
//! share one block marked as an import cycle, within a district what is
//! imported stands behind what imports it, and import lanes walk the steps.
//!
//! One test per scenario of issue #78, named after it. The projects are
//! crates in a temporary directory, seen through the imports lens. No test
//! runs git, reads `$HOME` or opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::{LaneKind, Lens, ProjectMap};
use egui_kittest::Harness;

use common::{Recorded, Tree, open};

/// One step of the staircase, in layout points.
const LEVEL: f32 = 90.0;

/// "catalog" imports from no other crate; "orders" imports from it. With
/// `looped`, "cart.rs" and "price.rs" import each other.
fn shop(looped: bool) -> (Tree, Harness<'static, CellworksApp>) {
    let price = if looped {
        "use crate::cart::Cart;\nuse crate::tax::Rate;\npub struct Price;\n"
    } else {
        "use crate::tax::Rate;\npub struct Price;\n"
    };
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
            (
                "catalog/src/lib.rs",
                "pub mod cart;\npub mod price;\npub mod tax;\n",
            ),
            ("catalog/src/price.rs", price),
            ("catalog/src/tax.rs", "pub struct Rate;\n"),
            (
                "catalog/src/cart.rs",
                "use crate::price::Price;\nuse crate::tax::Rate;\npub struct Cart;\n",
            ),
            ("orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
            (
                "orders/src/lib.rs",
                "pub mod invoice;\npub mod order;\npub mod ship;\n",
            ),
            (
                "orders/src/order.rs",
                "use crate::invoice::Invoice;\nuse crate::ship::Ship;\npub struct Order;\n",
            ),
            (
                "orders/src/invoice.rs",
                "use crate::ship::Ship;\nuse catalog::price::Price;\npub struct Invoice;\n",
            ),
            ("orders/src/ship.rs", "pub struct Ship;\n"),
        ],
    );
    let harness = imports_lens(&tree);
    (tree, harness)
}

/// Opens `tree` and switches to the imports lens.
fn imports_lens(tree: &Tree) -> Harness<'static, CellworksApp> {
    let mut harness = open(tree, None, Recorded::listing(&[]));
    while harness.state().opened().map(|o| o.lens()) != Some(Lens::Imports) {
        harness.key_press(egui::Key::L);
        harness.run_steps(120);
    }
    harness
}

/// The district standing under the file `file`, and its index.
fn district_of(map: &ProjectMap, file: &Path) -> usize {
    map.districts
        .iter()
        .position(|district| {
            district
                .nodes
                .iter()
                .any(|node| node.items.iter().any(|item| item.path.starts_with(file)))
        })
        .unwrap_or_else(|| panic!("no district holds {}", file.display()))
}

/// The block holding the items of `file`.
fn block_of<'m>(map: &'m ProjectMap, file: &Path) -> &'m cellworks_map::Node {
    map.districts
        .iter()
        .flat_map(|district| &district.nodes)
        .find(|node| node.items.iter().any(|item| item.path.starts_with(file)))
        .unwrap_or_else(|| panic!("no block holds {}", file.display()))
}

/// The foundation stands on the ground and what imports it stands higher
#[test]
fn the_foundation_stands_on_the_ground_and_what_imports_it_stands_higher() {
    let (tree, harness) = shop(false);
    let map = common::map(&harness);

    let catalog = &map.districts[district_of(&map, &tree.root.join("catalog/src/tax.rs"))];
    let orders = &map.districts[district_of(&map, &tree.root.join("orders/src/invoice.rs"))];

    assert_ne!(catalog.path, orders.path);
    assert_eq!(catalog.elevation, 0.0, "on the ground");
    assert_eq!(orders.elevation, LEVEL, "one step higher");
    let gap = (catalog.area.center() - orders.area.center()).length()
        - (catalog.area.width() + orders.area.width()) / 2.0;
    assert!(gap.abs() < 10.0, "beside it, edge to edge: {gap}");
}

/// Imports both ways do not lift both districts
#[test]
fn imports_both_ways_do_not_lift_both_districts() {
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
            ("catalog/src/lib.rs", "pub mod a;\npub mod b;\npub mod c;\n"),
            ("catalog/src/a.rs", "pub struct A;\n"),
            ("catalog/src/b.rs", "use crate::a::A;\npub struct B;\n"),
            (
                "catalog/src/c.rs",
                "use crate::a::A;\nuse crate::b::B;\nuse billing::z::Z;\npub struct C;\n",
            ),
            ("billing/Cargo.toml", "[package]\nname = \"billing\"\n"),
            ("billing/src/lib.rs", "pub mod x;\npub mod y;\npub mod z;\n"),
            (
                "billing/src/x.rs",
                "use crate::y::Y;\nuse crate::z::Z;\nuse catalog::a::A;\nuse catalog::b::B;\npub struct X;\n",
            ),
            ("billing/src/y.rs", "use crate::z::Z;\npub struct Y;\n"),
            ("billing/src/z.rs", "pub struct Z;\n"),
        ],
    );
    let harness = imports_lens(&tree);
    let map = common::map(&harness);

    let billing = &map.districts[district_of(&map, &tree.root.join("billing/src/x.rs"))];
    let catalog = &map.districts[district_of(&map, &tree.root.join("catalog/src/a.rs"))];

    assert_ne!(billing.path, catalog.path, "two districts");
    assert!(
        billing.elevation > catalog.elevation,
        "{} over {}",
        billing.elevation,
        catalog.elevation
    );
}

/// Files that import each other in a loop share one block, marked as an import cycle
#[test]
fn files_that_import_each_other_in_a_loop_share_one_block_marked_as_an_import_cycle() {
    let (tree, harness) = shop(true);
    let map = common::map(&harness);

    let cart = block_of(&map, &tree.root.join("catalog/src/cart.rs"));
    let price = block_of(&map, &tree.root.join("catalog/src/price.rs"));

    assert_eq!(cart.path, price.path, "one block");
    assert!(map.cycles.contains(&cart.path), "marked as an import cycle");
    assert!(cart.name.starts_with("cycle: "), "{}", cart.name);
}

/// A file in no loop stands on its own block
#[test]
fn a_file_in_no_loop_stands_on_its_own_block() {
    let (tree, harness) = shop(true);
    let map = common::map(&harness);

    let invoice = block_of(&map, &tree.root.join("orders/src/invoice.rs"));

    let files: Vec<PathBuf> = invoice
        .items
        .iter()
        .filter_map(|item| item.path.parent().map(Path::to_path_buf))
        .collect();
    assert!(
        files.iter().all(|file| file.ends_with("invoice.rs")),
        "{files:?}"
    );
    assert!(!map.cycles.contains(&invoice.path));
}

/// Within a district, what is imported stands behind what imports it
#[test]
fn within_a_district_what_is_imported_stands_behind_what_imports_it() {
    let (tree, harness) = shop(false);
    let map = common::map(&harness);

    let (cart, tax) = (
        tree.root.join("catalog/src/cart.rs"),
        tree.root.join("catalog/src/tax.rs"),
    );
    assert_eq!(
        district_of(&map, &cart),
        district_of(&map, &tax),
        "one district"
    );
    // Behind is further from the viewer: higher up the screen, at the same
    // height, where ground x + y is smaller.
    let depth = |file: &Path| {
        let centre = block_of(&map, file).area.center();
        centre.x + centre.y
    };

    assert!(
        depth(&tax) < depth(&cart),
        "tax.rs {} behind cart.rs {}",
        depth(&tax),
        depth(&cart)
    );
}

/// Import lanes walk down the steps
#[test]
fn import_lanes_walk_down_the_steps() {
    let (tree, harness) = shop(false);
    let map = common::map(&harness);
    let (invoice, price) = (
        tree.root.join("orders/src/invoice.rs"),
        tree.root.join("catalog/src/price.rs"),
    );

    let lane = harness
        .state()
        .lanes()
        .iter()
        .find(|lane| {
            lane.kind == LaneKind::Imports
                && lane.from == block_of(&map, &invoice).path
                && lane.to == block_of(&map, &price).path
        })
        .cloned()
        .expect("an import lane from invoice.rs to price.rs");

    // Down the face of the step: one point straight over the next.
    let down = lane
        .ground
        .windows(2)
        .zip(lane.heights.windows(2))
        .any(|(g, h)| (g[1] - g[0]).length() < 10.0 && (h[0] - h[1]).abs() > 1.0);
    assert!(down, "{:?} {:?}", lane.ground, lane.heights);
    // Every point on the ground of a step, at its height.
    for (point, height) in lane.ground.iter().zip(&lane.heights) {
        let on_a_step = map.districts.iter().any(|district| {
            district.area.expand(1.0).contains(*point) && (district.elevation - height).abs() < 1e-3
        });
        assert!(on_a_step, "{point:?} at {height} is in open air");
    }
}
