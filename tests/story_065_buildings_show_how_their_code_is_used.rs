//! STORY-065, buildings show how their code is used: public methods are lit
//! doors on the street and private ones a walled courtyard behind, code
//! called from many places has a forecourt, code calling other files carries
//! masts, a type holding another is joined to it by a skybridge, buildings
//! in front hide skybridges, and pointing at a skybridge says what it joins.
//!
//! One test per scenario of issue #92, named after it:
//! - Public methods are doors on the street
//! - Code called from many places has a forecourt
//! - Code calling other files carries masts
//! - Masts of tests show only underground
//! - A type holding another is joined to it by a skybridge
//! - Buildings in front hide skybridges
//! - Pointing at a skybridge says what it joins
//!
//! The project "shop" is a real Rust crate in a temporary directory; in
//! "cart.rs" the types "Cart", "Basket" and "Wish" each hold "Line", and a
//! type in each of three other files holds a type of another file, which
//! makes no skybridge: a skybridge stays within its block. No test runs git,
//! reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::city;
use cellworks_map::ui::{DrawnBridge, DrawnBuilding, bridge_card};
use egui::Key;
use egui_kittest::Harness;

use common::{Recorded, Tree, map, open};

const MAIN: &str = "\
mod cart;
mod checkout;
mod pricing;

use checkout::Order;

struct Till {
    order: Order,
}

fn main() {
    checkout::checkout();
}
";

const CART: &str = "\
pub struct Cart {
    owner: String,
    lines: Vec<Line>,
}

pub struct Line {
    sku: String,
    name: String,
    count: u32,
    cents: u32,
    note: String,
    gift: bool,
}

pub struct Basket {
    owner: String,
    note: String,
    picked: Vec<Line>,
}

pub struct Wish {
    owner: String,
    wanted: Vec<Line>,
}

pub fn sample() -> u32 {
    let a = 1;
    let b = 2;
    a + b
}

pub fn label() -> u32 {
    let a = 3;
    a
}

impl Cart {
    pub fn new() -> Cart {
        Cart { owner: String::new(), lines: Vec::new() }
    }

    pub fn add(&mut self, line: Line) {
        self.lines.push(line);
        self.tidy();
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    fn tidy(&mut self) {
        self.lines.retain(|line| line.count > 0);
    }
}
";

const PRICING: &str = "\
use crate::cart::Line;

pub struct Rate {
    line: Line,
}

pub fn total(cents: &[u32]) -> u32 {
    let mut sum = 0;
    for c in cents {
        sum += c;
    }
    sum
}

pub fn rounded(cents: u32) -> u32 {
    cents / 5 * 5
}
";

const CHECKOUT: &str = "\
use crate::cart::Cart;
use crate::pricing::{rounded, total};

pub struct Order {
    number: u32,
    cart: Cart,
}

pub fn checkout() -> u32 {
    let cart = Cart::new();
    let sum = total(&[cart.lines().len() as u32]);
    rounded(sum)
}

pub fn quote() -> u32 {
    total(&[2])
}

pub fn invoice() -> u32 {
    let net = total(&[3]);
    let tax = total(&[4]);
    net + tax
}

pub fn refund() -> u32 {
    total(&[5])
}

#[cfg(test)]
mod tests {
    use crate::cart::Cart;

    #[test]
    fn adds_an_item() {
        let cart = Cart::new();
        assert!(cart.lines().is_empty());
    }
}
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
            ("src/pricing.rs", PRICING),
            ("src/checkout.rs", CHECKOUT),
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

    /// The skybridge drawn from the building at `from` to the one at `to`.
    fn bridge(&self, from: &PathBuf, to: &PathBuf) -> DrawnBridge {
        self.harness
            .state()
            .bridges()
            .iter()
            .find(|bridge| bridge.from == *from && bridge.to == *to)
            .cloned()
            .unwrap_or_else(|| panic!("no skybridge from {}", from.display()))
    }

    /// Turns the view underground.
    fn look_underground(&mut self) {
        self.harness.key_press(Key::U);
        self.harness.run();
    }
}

/// Public methods are doors on the street
#[test]
fn public_methods_are_doors_on_the_street() {
    let shop = shop();

    let cart = shop.building(&shop.at("cart.rs", "Cart"));

    assert_eq!(cart.doors, 3, "new, add and lines are public");
    assert!(cart.courtyard, "tidy is private");
    let line = shop.building(&shop.at("cart.rs", "Line"));
    assert_eq!(line.doors, 0);
    assert!(!line.courtyard, "no private methods, no courtyard");
}

/// Code called from many places has a forecourt
#[test]
fn code_called_from_many_places_has_a_forecourt() {
    let shop = shop();

    assert!(
        shop.building(&shop.at("pricing.rs", "total")).forecourt,
        "total is called from five places"
    );
    assert!(
        !shop.building(&shop.at("pricing.rs", "rounded")).forecourt,
        "rounded is called from one place"
    );
    assert!(
        !shop.building(&shop.at("checkout.rs", "quote")).forecourt,
        "quote is called from none"
    );
}

/// Code calling other files carries masts
#[test]
fn code_calling_other_files_carries_masts() {
    let shop = shop();

    let checkout = shop.building(&shop.at("checkout.rs", "checkout"));

    assert_eq!(checkout.masts, [false, false], "cart.rs and pricing.rs");
    assert_eq!(
        shop.building(&shop.at("checkout.rs", "quote")).masts,
        [false],
        "quote calls pricing.rs only"
    );
}

/// Masts of tests show only underground
#[test]
fn masts_of_tests_show_only_underground() {
    let mut shop = shop();
    let test = shop.at("checkout.rs", "tests::adds_an_item");
    assert!(
        shop.building(&test).masts.is_empty(),
        "not shown above the ground"
    );

    shop.look_underground();

    assert_eq!(shop.building(&test).masts, [true], "hangs upside down");
    assert!(
        shop.building(&shop.at("checkout.rs", "checkout"))
            .masts
            .is_empty(),
        "the masts above the ground are not shown"
    );
}

/// A type holding another is joined to it by a skybridge
#[test]
fn a_type_holding_another_is_joined_to_it_by_a_skybridge() {
    let shop = shop();
    let (cart, line) = (shop.at("cart.rs", "Cart"), shop.at("cart.rs", "Line"));

    let bridge = shop.bridge(&cart, &line);

    // `lines: Vec<Line>` is the third line of `Cart`.
    let map = map(&shop.harness);
    let cart = map.item(&cart).expect("Cart is on the map");
    assert!((bridge.z - city::floor_z(cart, 2)).abs() < 1e-4);
    assert!(!bridge.pieces.is_empty());
}

/// Buildings in front hide skybridges
#[test]
fn buildings_in_front_hide_skybridges() {
    let shop = shop();

    // The skybridges of cart.rs run between the buildings of its block;
    // some building stands in front of part of one.
    let (bridges, towers) = (
        shop.harness.state().bridges(),
        shop.harness.state().towers(),
    );
    assert_eq!(bridges.len(), 3, "{bridges:?}");

    assert!(
        bridges.iter().any(|bridge| {
            let hidden = bridge.hidden_pieces(towers);
            hidden > 0 && hidden < bridge.pieces.len()
        }),
        "part of a skybridge is hidden, the rest not: {bridges:?}"
    );
}

/// Pointing at a skybridge says what it joins
#[test]
fn pointing_at_a_skybridge_says_what_it_joins() {
    let mut shop = shop();
    let (cart, line) = (shop.at("cart.rs", "Cart"), shop.at("cart.rs", "Line"));
    let pieces = shop.bridge(&cart, &line).pieces;

    // Somewhere along it no building stands in front.
    let pointed = pieces.iter().find_map(|(_, at)| {
        shop.harness.hover_at(*at);
        shop.harness.run();
        shop.harness.state().bridge_hovered().cloned()
    });

    let bridge = pointed.expect("the skybridge is pointed at");
    assert_eq!((&bridge.from, &bridge.to), (&cart, &line));
    let [head, field, meaning] = bridge_card(&bridge);
    assert_eq!(head, "Skybridge: Cart holds Line");
    assert_eq!(field, "lines: Vec<Line>    cart.rs:3");
    assert!(meaning.contains("Cart is built from Line"), "{meaning}");
}
