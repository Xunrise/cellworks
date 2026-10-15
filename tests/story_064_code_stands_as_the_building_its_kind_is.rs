//! STORY-064, code stands as the building its kind is: a type with methods
//! is a stepped skyscraper crowned by the traits it implements, an enum a
//! terrace of row houses, a constant a monument, an entry point a landmark
//! with a spire, and a type without methods a tower.
//!
//! One test per scenario of issue #91, named after it:
//! - A type with methods is a stepped skyscraper
//! - A type wears a crown for each trait it implements
//! - An enum is a terrace of row houses
//! - A constant is a monument
//! - An entry point is a landmark with a spire
//! - A type without methods stays a tower
//!
//! The project "shop" is a real Rust crate in a temporary directory; one of
//! the traits of "Cart" is implemented in another file than "Cart". No test
//! runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::ui::DrawnBuilding;
use cellworks_map::{Form, city};
use egui::{Event, pos2};
use egui_kittest::Harness;

use common::{Recorded, Tree, map, open, tower};

const MAIN: &str = "\
mod cart;
mod display;
mod layout;
mod lens;

fn main() {
    let cart = cart::Cart::default();
    cart.total();
}
";

const CART: &str = "\
pub struct Cart {
    items: Vec<u32>,
}

pub struct Price {
    cents: u32,
}

impl Cart {
    pub fn add(&mut self, item: u32) {
        self.items.push(item);
    }

    pub fn total(&self) -> u32 {
        let mut sum = 0;
        for item in &self.items {
            sum += item;
        }
        sum
    }
}

impl Default for Cart {
    fn default() -> Cart {
        Cart { items: Vec::new() }
    }
}
";

const DISPLAY: &str = "\
use std::fmt;

use crate::cart::Cart;

pub struct Receipt;

impl fmt::Display for Cart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, \"cart\")
    }
}

impl fmt::Display for Receipt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, \"receipt\")
    }
}
";

const LENS: &str = "\
pub enum Lens {
    Folders,
    Imports,
    Calls,
    CoChange,
    Cells(Vec<String>),
    Custom { name: String },
}
";

const LAYOUT: &str = "\
pub const GAP: [f32; 2] = [
    4.0,
    14.0,
];
";

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: the project "shop" is open as a city.
fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("Cargo.toml", "[package]\nname = \"shop\"\n"),
            ("src/main.rs", MAIN),
            ("src/cart.rs", CART),
            ("src/display.rs", DISPLAY),
            ("src/lens.rs", LENS),
            ("src/layout.rs", LAYOUT),
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn at(&self, file: &str, name: &str) -> PathBuf {
        self.tree.root.join("src").join(file).join(name)
    }

    /// The building drawn with its base at `path`.
    fn building(&self, path: &PathBuf) -> DrawnBuilding {
        self.harness
            .state()
            .buildings()
            .iter()
            .find(|building| building.path == *path)
            .cloned()
            .unwrap_or_else(|| panic!("no building on {}", path.display()))
    }
}

/// A type with methods is a stepped skyscraper
#[test]
fn a_type_with_methods_is_a_stepped_skyscraper() {
    let shop = shop();
    let cart = shop.at("cart.rs", "Cart");

    let building = shop.building(&cart);

    assert_eq!(building.form, Form::Skyscraper);
    for method in ["Cart::add", "Cart::total"] {
        let path = shop.at("cart.rs", method);
        assert!(building.tiers.contains(&path), "{method} is a tier");
        assert!(
            shop.harness
                .state()
                .buildings()
                .iter()
                .all(|other| other.path != path),
            "{method} is no building of its own"
        );
    }
    let map = map(&shop.harness);
    let mut below = map.item(&cart).expect("Cart is on the map");
    for tier in &building.tiers {
        let tier = map.item(tier).expect("the tier is on the map");
        assert!(
            below.area.contains_rect(tier.area) && tier.area.width() < below.area.width(),
            "{} steps back above {}",
            tier.name,
            below.name
        );
        assert!((tier.base - city::height(below)).abs() < 1e-4);
        if below.path != cart {
            assert!(
                city::reach(tier) <= city::reach(below),
                "the largest lowest"
            );
        }
        below = tier;
    }
}

/// A type wears a crown for each trait it implements
#[test]
fn a_type_wears_a_crown_for_each_trait_it_implements() {
    let shop = shop();
    let cart = shop.at("cart.rs", "Cart");

    let mut crowns = shop.building(&cart).crowns;
    crowns.sort();

    assert_eq!(crowns, ["Default", "Display"]);
    assert_eq!(
        shop.building(&shop.at("display.rs", "Receipt")).crowns,
        ["Display"],
        "the same crown"
    );
    // "Display" is implemented for "Cart" in another file: its method
    // stands on Cart's skyscraper, on Cart's block.
    let fmt = shop.at("display.rs", "Cart::fmt");
    assert!(shop.building(&cart).tiers.contains(&fmt));
    let map = map(&shop.harness);
    let block = map.find(&fmt).and_then(|found| found.parent);
    assert_eq!(block, Some(shop.tree.root.join("src/cart.rs").as_path()));
}

/// An enum is a terrace of row houses
#[test]
fn an_enum_is_a_terrace_of_row_houses() {
    let shop = shop();

    let building = shop.building(&shop.at("lens.rs", "Lens"));

    assert_eq!(building.form, Form::Terrace);
    assert_eq!(building.houses, [false, false, false, false, true, true]);
}

/// A constant is a monument
#[test]
fn a_constant_is_a_monument() {
    let shop = shop();
    let gap = shop.at("layout.rs", "GAP");

    assert_eq!(shop.building(&gap).form, Form::Monument);
    assert_eq!(
        tower(&shop.harness, &gap).floors.len(),
        1,
        "no floors to read, one span up its middle"
    );
}

/// An entry point is a landmark with a spire
#[test]
fn an_entry_point_is_a_landmark_with_a_spire() {
    let mut shop = shop();
    let main = shop.at("main.rs", "main");
    assert!(shop.building(&main).spire);

    for _ in 0..60 {
        if shop.harness.state().view().expect("drawn").scale() < 0.4 {
            break;
        }
        shop.harness.hover_at(pos2(600.0, 450.0));
        shop.harness.event(Event::Zoom(0.8));
        shop.harness.run();
    }

    assert!(shop.harness.state().view().expect("drawn").scale() < 0.4);
    assert!(shop.building(&main).spire, "still there far out");
}

/// A type without methods stays a tower
#[test]
fn a_type_without_methods_stays_a_tower() {
    let shop = shop();

    let building = shop.building(&shop.at("cart.rs", "Price"));

    assert_eq!(building.form, Form::Tower);
    assert!(building.tiers.is_empty() && building.crowns.is_empty());
}
