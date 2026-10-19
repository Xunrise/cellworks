//! STORY-046, walk between facades: facing a facade, the arrows step to the
//! next tower on its block, scroll a facade taller than the view, never
//! leave the block or reach a test, and a bar names where each step leads.
//!
//! One test per scenario of issue #70, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`): the block "pricing.rs" stands as a skyline, "total"
//! on the back row, "discount" and then "round_price" on the row in front.
//! No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use egui::{Event, Key, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

use common::{Tree, shop_city, shop_city_with, tower};

/// The scale at which the camera faces the facade.
const FACING: f32 = 16.0;

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

impl Shop {
    fn at(&self, name: &str) -> PathBuf {
        self.tree.root.join("catalog/pricing.rs").join(name)
    }

    /// The camera faces the facade of `name`: it is selected and the camera
    /// zoomed in until it faces it.
    fn face(&mut self, name: &str) {
        let path = self.at(name);
        self.harness.state_mut().jump(path);
        self.harness.run();
        while self.harness.state().view().expect("drawn").scale() < FACING {
            self.harness.hover_at(pos2(600.0, 450.0));
            self.harness.event(Event::Zoom(1.15));
            self.harness.run();
        }
        assert_eq!(self.facing(), Some(self.at(name)));
    }

    fn facing(&self) -> Option<PathBuf> {
        let facing = self.harness.state().navigation().facing();
        facing.map(|path| path.to_path_buf())
    }

    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }
}

/// The background: the camera faces the facade of "discount".
fn shop() -> Shop {
    let (tree, harness) = shop_city();
    let mut shop = Shop { tree, harness };
    shop.face("discount");
    shop
}

/// Moving sideways faces the next tower on the row
#[test]
fn moving_sideways_faces_the_next_tower_on_the_row() {
    let mut shop = shop();

    shop.press(Key::ArrowRight);

    assert_eq!(shop.facing(), Some(shop.at("round_price")));
}

/// Moving past the last tower of the block stays on the block
#[test]
fn moving_past_the_last_tower_of_the_block_stays_on_the_block() {
    let mut shop = shop();
    shop.face("round_price");

    shop.press(Key::ArrowRight);

    assert_eq!(shop.facing(), Some(shop.at("round_price")));
}

/// A tall facade scrolls before the next row is reached
#[test]
fn a_tall_facade_scrolls_before_the_next_row_is_reached() {
    let mut shop = shop();
    shop.face("total");
    let before = tower(&shop.harness, &shop.at("total")).floors[0].start.y;

    shop.press(Key::ArrowDown);

    let after = tower(&shop.harness, &shop.at("total")).floors[0].start.y;
    assert!(after < before - 1.0, "first line {before} → {after}");
    assert_eq!(shop.facing(), Some(shop.at("total")));
}

/// A facade arrives at its first line
#[test]
fn a_facade_arrives_at_its_first_line() {
    let mut shop = shop();

    // "total" stands on the row before "discount": left of it.
    shop.press(Key::ArrowLeft);

    assert_eq!(shop.facing(), Some(shop.at("total")));
    let first = tower(&shop.harness, &shop.at("total")).floors[0].start;
    assert!(first.y > 30.0 && first.y < 900.0, "first line at {first:?}");
}

/// Moving never reaches a test
#[test]
fn moving_never_reaches_a_test() {
    let test = "\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn discount_reduces() {\n        let price = 1;\n        assert_eq!(price, 1);\n    }\n}\n";
    let (tree, harness) = shop_city_with(test);
    let mut shop = Shop { tree, harness };
    shop.face("total");

    let mut faced = vec![shop.facing()];
    for _ in 0..6 {
        shop.press(Key::ArrowRight);
        faced.push(shop.facing());
    }

    let test = Some(shop.at("tests::discount_reduces"));
    assert!(!faced.contains(&test), "{faced:?}");
}

/// The view says where each move leads
#[test]
fn the_view_says_where_each_move_leads() {
    let shop = shop();

    let hints = shop
        .harness
        .root()
        .children_recursive()
        .filter_map(|node| node.accesskit_node().label().map(|label| label.to_string()))
        .find(|label| label.contains("[ discount ]"))
        .expect("a bar naming the moves");

    assert!(hints.contains("round_price →"), "{hints}");
    assert!(hints.contains("← total"), "{hints}");
}
