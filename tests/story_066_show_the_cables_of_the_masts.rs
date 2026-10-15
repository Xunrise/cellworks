//! STORY-066, show the cables of the masts: with M the code architect
//! switches on a cable from each mast to the roof of the building it calls,
//! and follows where a building reaches without the call lanes.
//!
//! One test per scenario of issue #93, named after it:
//! - Cables are hidden at first
//! - Switching cables on shows a cable from each mast to the building it calls
//! - A selection shows only its own cables
//! - Cables stay shown across a lens switch
//! - Only buildings with masts have cables
//!
//! The project "shop" is a real Rust crate in a temporary directory:
//! "checkout" calls "total" in "price.rs" and "ship" in "orders.rs", and
//! "dispatch" in "orders.rs" calls "total" too, so a building other than
//! "checkout" carries a mast. No test runs git, reads `$HOME` or opens a
//! window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use egui::Key;
use egui_kittest::Harness;

use common::{Recorded, Tree, await_lens, open};

const MAIN: &str = "\
mod checkout;
mod orders;
mod price;

fn main() {
    checkout::checkout();
}
";

const CHECKOUT: &str = "\
use crate::orders::ship;
use crate::price::total;

pub fn checkout() -> u32 {
    let sum = total(&[1, 2]);
    ship(sum)
}
";

const PRICE: &str = "\
pub fn total(cents: &[u32]) -> u32 {
    let mut sum = 0;
    for c in cents {
        sum += c;
    }
    sum
}
";

const ORDERS: &str = "\
use crate::price::total;

pub fn ship(sum: u32) -> u32 {
    sum + 1
}

pub fn dispatch() -> u32 {
    ship(total(&[3]))
}
";

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: the project "shop" is open as a city, and "checkout"
/// carries masts towards "total" in "price.rs" and "ship" in "orders.rs".
fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("Cargo.toml", "[package]\nname = \"shop\"\n"),
            ("src/main.rs", MAIN),
            ("src/checkout.rs", CHECKOUT),
            ("src/price.rs", PRICE),
            ("src/orders.rs", ORDERS),
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn at(&self, file: &str, name: &str) -> PathBuf {
        self.tree.root.join("src").join(file).join(name)
    }

    fn cables(&self) -> Vec<(PathBuf, PathBuf)> {
        self.harness.state().cables().to_vec()
    }

    /// The code architect shows the cables.
    fn show_cables(&mut self) {
        self.harness.key_press(Key::M);
        self.harness.run();
    }
}

/// Cables are hidden at first
#[test]
fn cables_are_hidden_at_first() {
    let shop = shop();

    assert!(!shop.harness.state().navigation().shows_cables());
    assert!(shop.cables().is_empty(), "{:?}", shop.cables());
}

/// Switching cables on shows a cable from each mast to the building it calls
#[test]
fn switching_cables_on_shows_a_cable_from_each_mast_to_the_building_it_calls() {
    let mut shop = shop();

    shop.show_cables();

    let checkout = shop.at("checkout.rs", "checkout");
    let cables = shop.cables();
    for called in [shop.at("price.rs", "total"), shop.at("orders.rs", "ship")] {
        assert!(
            cables.contains(&(checkout.clone(), called.clone())),
            "a cable from checkout to {}: {cables:?}",
            called.display()
        );
    }
}

/// A selection shows only its own cables
#[test]
fn a_selection_shows_only_its_own_cables() {
    let mut shop = shop();
    shop.show_cables();
    let checkout = shop.at("checkout.rs", "checkout");
    let dispatch = shop.at("orders.rs", "dispatch");
    assert!(
        shop.cables().iter().any(|(from, _)| *from == dispatch),
        "dispatch has a cable of its own: {:?}",
        shop.cables()
    );

    shop.harness.state_mut().jump(checkout.clone());
    shop.harness.run();

    let cables = shop.cables();
    assert_eq!(cables.len(), 2, "{cables:?}");
    assert!(
        cables.iter().all(|(from, _)| *from == checkout),
        "{cables:?}"
    );
}

/// Cables stay shown across a lens switch
#[test]
fn cables_stay_shown_across_a_lens_switch() {
    let mut shop = shop();
    shop.show_cables();
    let before = shop.harness.state().opened().map(|opened| opened.lens());

    shop.harness.key_press(Key::L);
    await_lens(&mut shop.harness);
    shop.harness.run_steps(150);

    let after = shop.harness.state().opened().map(|opened| opened.lens());
    assert_ne!(after, before, "the lens switched");
    assert!(shop.harness.state().navigation().shows_cables());
    assert!(!shop.cables().is_empty(), "the cables are still shown");
}

/// Only buildings with masts have cables
#[test]
fn only_buildings_with_masts_have_cables() {
    let mut shop = shop();

    shop.show_cables();

    let total = shop.at("price.rs", "total");
    let cables = shop.cables();
    assert!(!cables.is_empty());
    assert!(
        cables.iter().all(|(from, _)| *from != total),
        "total calls no code in another file: {cables:?}"
    );
}
