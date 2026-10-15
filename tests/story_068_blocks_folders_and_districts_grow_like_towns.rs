//! STORY-068, blocks, folders and districts grow like towns: buildings and
//! blocks bound by their code stand together, the building with the most
//! doors is the centrepiece of its block, a folder's documents gather, every
//! block, folder and district is outlined round what it holds, and the same
//! code always lays out the same town.
//!
//! One test per scenario of issue #95, named after it:
//! - Buildings joined by a skybridge stand side by side
//! - The type with the most doors is the centrepiece of its block
//! - Blocks whose code reaches each other stand together
//! - Documents of a folder gather in one quarter
//! - Blocks, folders and districts are outlined round what they hold
//! - The same code lays out the same town
//!
//! The project "shop" is a real folder tree in a temporary directory, cut by
//! the district rule "none" so "app" is the district: "app/src" holds the
//! code, "app/docs" three documents and two code files. In "cart.rs" the
//! struct "Cart" has five public methods and holds "Line"; "cart.rs" imports
//! from and calls "price.rs" often, "notes.rs" never. No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::{ProjectMap, city};
use egui::{Pos2, Rect};
use egui_kittest::Harness;

use common::{Recorded, Tree, map, open};

const MAIN: &str = "\
mod cart;
mod notes;
mod price;

fn main() {
    let started = true;
}
";

const CART: &str = "\
use crate::price::{rate, Price};

pub struct Cart {
    lines: Vec<Line>,
}

pub struct Line {
    price: Price,
}

pub struct Coupon {
    code: u32,
}

impl Cart {
    pub fn add(&mut self) {
        rate();
    }

    pub fn remove(&mut self) {
        rate();
    }

    pub fn total(&self) -> u32 {
        rate()
    }

    pub fn count(&self) -> usize {
        self.lines.len()
    }

    pub fn clear(&mut self) {
        rate();
    }
}

impl Coupon {
    pub fn apply(&self) -> u32 {
        self.code
    }
}

fn audit() {
    let checked = true;
}
";

const PRICE: &str = "\
pub struct Price {
    cents: u32,
}

pub fn rate() -> u32 {
    1
}
";

const NOTES: &str = "\
pub fn note() -> u32 {
    let written = 2;
    written
}
";

const DOCUMENT: &str = "# Guide\n\nHow it works.\n\n## Details\n\nMore of it.\n";

const TOOL: &str = "fn tool() {\n    let made = true;\n}\n";

/// The street the widest between two buildings on a block, with the open
/// ground the centrepiece keeps.
const BESIDE: f32 = 2.0 + 3.0 + 6.0;

/// The street the widest between two blocks of a folder.
const BLOCKS_BESIDE: f32 = 14.0 * 1.2;

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open as a city in the folders lens.
fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("app/Cargo.toml", "[package]\nname = \"app\"\n"),
            ("app/src/main.rs", MAIN),
            ("app/src/cart.rs", CART),
            ("app/src/price.rs", PRICE),
            ("app/src/notes.rs", NOTES),
            ("app/docs/guide.md", DOCUMENT),
            ("app/docs/setup.md", DOCUMENT),
            ("app/docs/faq.md", DOCUMENT),
            ("app/docs/build.rs", TOOL),
            ("app/docs/gen.rs", TOOL),
        ],
    );
    let harness = open(&tree, Some("none"), Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn path(&self, relative: &str) -> PathBuf {
        self.tree.root.join(relative)
    }

    /// The ground the element at `relative` stands on.
    fn ground(&self, relative: &str) -> Rect {
        map(&self.harness)
            .find(&self.path(relative))
            .unwrap_or_else(|| panic!("{relative} is on the map"))
            .area
    }

    /// The outline the element at `relative` was drawn by.
    fn outline(&self, relative: &str) -> Vec<Pos2> {
        let path = self.path(relative);
        self.harness
            .state()
            .outlines()
            .iter()
            .find(|(drawn, _)| *drawn == path)
            .map(|(_, outline)| outline.clone())
            .unwrap_or_else(|| panic!("{relative} is outlined"))
    }
}

/// How far apart the grounds `a` and `b` are: 0 when they touch.
fn apart(a: Rect, b: Rect) -> f32 {
    let x = (a.min.x - b.max.x).max(b.min.x - a.max.x).max(0.0);
    let y = (a.min.y - b.max.y).max(b.min.y - a.max.y).max(0.0);
    x.max(y)
}

/// Whether `at` lies inside the convex `outline`, or on its edge.
fn within(outline: &[Pos2], at: Pos2) -> bool {
    let n = outline.len();
    let side = |i: usize| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        (b - a).x * (at - a).y - (b - a).y * (at - a).x
    };
    (0..n).all(|i| side(i) >= -1e-2) || (0..n).all(|i| side(i) <= 1e-2)
}

fn corners(r: Rect) -> [Pos2; 4] {
    [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ]
}

/// Buildings joined by a skybridge stand side by side
#[test]
fn buildings_joined_by_a_skybridge_stand_side_by_side() {
    let shop = shop();

    let cart = shop.ground("app/src/cart.rs/Cart");
    let line = shop.ground("app/src/cart.rs/Line");

    assert!(
        shop.harness.state().bridges().iter().any(|bridge| {
            bridge.from == shop.path("app/src/cart.rs/Cart")
                && bridge.to == shop.path("app/src/cart.rs/Line")
        }),
        "a skybridge joins them"
    );
    assert!(
        apart(cart, line) <= BESIDE,
        "Cart {cart:?} and Line {line:?} stand {} apart",
        apart(cart, line)
    );
}

/// The type with the most doors is the centrepiece of its block
#[test]
fn the_type_with_the_most_doors_is_the_centrepiece_of_its_block() {
    let shop = shop();
    let block = shop.ground("app/src/cart.rs");
    let cart = shop.ground("app/src/cart.rs/Cart");

    assert!(
        (cart.center() - block.center()).length() < 1e-2,
        "Cart {cart:?} stands in the middle of {block:?}"
    );
    let map = map(&shop.harness);
    let items = map
        .items_of(&shop.path("app/src/cart.rs"))
        .expect("cart.rs");
    let others: Vec<&cellworks_map::Item> = items
        .iter()
        .filter(|item| item.declaration.part_of.is_none() && !city::in_cellar(item))
        .filter(|item| item.path != shop.path("app/src/cart.rs/Cart"))
        .collect();
    assert!(others.len() >= 3, "{others:?}");
    // Open ground around it: no building stands as close to it as the
    // narrowest street.
    for other in others {
        assert!(
            apart(cart, other.area) >= 2.0 + 0.6 * 5.0 - 1e-2,
            "{} stands {} from Cart",
            other.name,
            apart(cart, other.area)
        );
    }
}

/// Blocks whose code reaches each other stand together
#[test]
fn blocks_whose_code_reaches_each_other_stand_together() {
    let shop = shop();

    let cart = shop.ground("app/src/cart.rs");
    let price = shop.ground("app/src/price.rs");
    let notes = shop.ground("app/src/notes.rs");

    assert!(apart(cart, price) <= BLOCKS_BESIDE, "{cart:?} {price:?}");
    let distance = |a: Rect, b: Rect| (a.center() - b.center()).length();
    assert!(
        distance(price, cart) < distance(notes, cart),
        "price {price:?} notes {notes:?} cart {cart:?}"
    );
}

/// Documents of a folder gather in one quarter
#[test]
fn documents_of_a_folder_gather_in_one_quarter() {
    let shop = shop();
    let documents: Vec<Rect> = ["guide.md", "setup.md", "faq.md"]
        .iter()
        .map(|name| shop.ground(&format!("app/docs/{name}")))
        .collect();

    // Each stands beside another, and together they are one quarter.
    let mut reached = vec![0];
    while let Some(next) = (0..documents.len()).find(|j| {
        !reached.contains(j)
            && reached
                .iter()
                .any(|i| apart(documents[*i], documents[*j]) <= BLOCKS_BESIDE)
    }) {
        reached.push(next);
    }
    assert_eq!(reached.len(), 3, "{documents:?}");
}

/// Blocks, folders and districts are outlined round what they hold
#[test]
fn blocks_folders_and_districts_are_outlined_round_what_they_hold() {
    let shop = shop();
    let map = map(&shop.harness);

    let mut checked = 0;
    for (relative, held) in [
        (
            "app/src/cart.rs",
            held_items(&map, &shop.path("app/src/cart.rs")),
        ),
        ("app/src", held_blocks(&map, &shop.path("app/src"))),
        ("app/docs", held_blocks(&map, &shop.path("app/docs"))),
        ("app", held_blocks(&map, &shop.path("app"))),
    ] {
        let outline = shop.outline(relative);
        // Round what it holds.
        assert!(!held.is_empty(), "{relative} holds something");
        for ground in &held {
            for corner in corners(*ground) {
                assert!(
                    within(&outline, corner),
                    "{relative}: {corner:?} is outside"
                );
            }
        }
        // No rectangle: more corners than four, and none of the box around
        // it on it.
        let around = Rect::from_points(&outline);
        assert!(outline.len() > 4, "{relative}: {outline:?}");
        for corner in corners(around) {
            assert!(
                outline.iter().all(|p| (*p - corner).length() > 0.5),
                "{relative} has the corner {corner:?}"
            );
        }
        // No circle: its points lie at different distances from its middle.
        let middle = around.center();
        let distances: Vec<f32> = outline.iter().map(|p| (*p - middle).length()).collect();
        let (near, far) = distances
            .iter()
            .fold((f32::MAX, 0.0_f32), |(lo, hi), d| (lo.min(*d), hi.max(*d)));
        assert!(
            far - near > 0.05 * far,
            "{relative} is round: {distances:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 4);
    // Every block with buildings is drawn by its outline.
    let drawn = shop.harness.state().outlines();
    for node in ["app/src/main.rs", "app/src/notes.rs", "app/docs/guide.md"] {
        assert!(
            drawn.iter().any(|(path, _)| *path == shop.path(node)),
            "{node} is outlined"
        );
    }
}

/// The ground of the items standing on the block at `node`.
fn held_items(map: &ProjectMap, node: &std::path::Path) -> Vec<Rect> {
    map.items_of(node)
        .expect("the block")
        .iter()
        .map(|item| item.area)
        .collect()
}

/// The ground of the buildings on every block under the folder at `folder`.
fn held_blocks(map: &ProjectMap, folder: &std::path::Path) -> Vec<Rect> {
    map.items_under(folder)
        .iter()
        .map(|item| item.area)
        .collect()
}

/// The same code lays out the same town
#[test]
fn the_same_code_lays_out_the_same_town() {
    let mut shop = shop();
    let before = map(&shop.harness);

    // Read again unchanged: ignored files shown, then hidden again.
    for _ in 0..2 {
        shop.harness.key_press(egui::Key::I);
        shop.harness.step();
        common::await_read(&mut shop.harness);
    }

    let after = map(&shop.harness);
    assert_eq!(after.districts, before.districts);
}
