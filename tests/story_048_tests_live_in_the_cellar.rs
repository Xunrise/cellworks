//! STORY-048, tests live in the cellar: tests hang below the ground under
//! their block, a file of only tests lies in the district's cellar, and the
//! cellar is out of reach unless the view is underground.
//!
//! One test per scenario of issue #72, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory. No test runs
//! git, reads `$HOME` or opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::city;
use cellworks_map::ui::DrawnTower;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Recorded, Tree, map, open, tower};

const PRICING: &str = "\
fn discount(price: u32) -> u32 {
    let reduced = price - 1;
    reduced
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discount_reduces() {
        let price = discount(2);
        assert_eq!(price, 1);
    }
}
";

const TESTS: &str = "\
#[test]
fn a_cart_starts_empty() {
    let items = 0;
    assert_eq!(items, 0);
}

#[test]
fn a_cart_counts_its_items() {
    let items = 2;
    assert_eq!(items, 2);
}
";

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open as a city; "pricing.rs" holds "discount"
/// and the test "discount_reduces", and "tests.rs" holds only tests.
fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/pricing.rs", PRICING),
            ("catalog/tests.rs", TESTS),
            ("orders/order.rs", "fn place() {\n    let order = 1;\n}\n"),
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn path(&self, relative: &str) -> PathBuf {
        self.tree.root.join(relative)
    }

    fn drawn(&self, relative: &str) -> DrawnTower {
        tower(&self.harness, &self.path(relative)).clone()
    }

    /// Where the front edge of `relative`'s ground starts on screen.
    fn ground_of(&self, relative: &str) -> Pos2 {
        let map = map(&self.harness);
        let item = map.item(&self.path(relative)).expect("on the map");
        let view = self.harness.state().view().expect("drawn");
        view.to_screen(city::front(item, 0.0), 0.0)
    }

    /// A point on a line of `relative` clear of the district's name, which
    /// lies over the ground in front of the plaza.
    fn on(&self, relative: &str) -> Pos2 {
        let name = self
            .harness
            .get_by_label("catalog")
            .accesskit_node()
            .bounding_box();
        let name = name.map(|r| {
            egui::Rect::from_min_max(
                egui::pos2(r.x0 as f32, r.y0 as f32),
                egui::pos2(r.x1 as f32, r.y1 as f32),
            )
        });
        self.drawn(relative)
            .floors
            .iter()
            .map(|line| line.start + (line.end - line.start) * 0.5)
            .find(|point| name.is_none_or(|name| !name.expand(2.0).contains(*point)))
            .expect("a line clear of the district's name")
    }

    /// Zooms in over `relative`, close but not so close that the camera
    /// turns to a facade.
    fn zoom_onto(&mut self, relative: &str) {
        for _ in 0..12 {
            let scale = self.harness.state().view().expect("drawn").scale();
            if scale * 1.25 >= 4.0 {
                return;
            }
            let at = self.ground_of(relative);
            self.harness.hover_at(at);
            self.harness.event(Event::Zoom(1.25));
            self.harness.run();
        }
    }

    fn click(&mut self, at: Pos2, modifiers: Modifiers) {
        self.harness.hover_at(at);
        for pressed in [true, false] {
            self.harness.event(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers,
            });
        }
        self.harness.run();
    }

    fn selection(&self) -> Option<PathBuf> {
        let selection = self.harness.state().navigation().selection();
        selection.map(Path::to_path_buf)
    }
}

const TEST: &str = "catalog/pricing.rs/tests::discount_reduces";

/// A test hangs underground under its block
#[test]
fn a_test_hangs_underground_under_its_block() {
    let shop = shop();

    let test = shop.drawn(TEST);
    let ground = shop.ground_of(TEST);
    assert!(test.cellar);
    assert!(
        test.floors.iter().all(|floor| floor.start.y > ground.y),
        "{:?} is not below {ground:?}",
        test.floors[0]
    );

    let discount = shop.drawn("catalog/pricing.rs/discount");
    let ground = shop.ground_of("catalog/pricing.rs/discount");
    assert!(!discount.cellar);
    assert!(discount.floors.iter().all(|floor| floor.start.y < ground.y));
}

/// A file of only tests lies in the district's cellar
#[test]
fn a_file_of_only_tests_lies_in_the_districts_cellar() {
    let shop = shop();
    let map = map(&shop.harness);
    let catalog = map.find(&shop.path("catalog")).expect("catalog").area;
    let block = map
        .find(&shop.path("catalog/tests.rs"))
        .expect("tests.rs")
        .area;

    assert!(
        (block.center() - catalog.center()).length() < 1.0,
        "{block:?} is not under the plaza at {:?}",
        catalog.center()
    );
    for name in ["a_cart_starts_empty", "a_cart_counts_its_items"] {
        assert!(
            shop.drawn(&format!("catalog/tests.rs/{name}")).cellar,
            "{name}"
        );
    }
    let pricing = map
        .find(&shop.path("catalog/pricing.rs"))
        .expect("pricing.rs")
        .area;
    assert!(
        !pricing.contains(catalog.center()),
        "the plaza is left free"
    );
}

/// The cellar is out of reach by default
#[test]
fn the_cellar_is_out_of_reach_by_default() {
    let mut shop = shop();
    shop.zoom_onto(TEST);
    let at = shop.on(TEST);

    shop.click(at, Modifiers::NONE);

    assert_ne!(shop.selection(), Some(shop.path(TEST)));
}

/// The underground view brings the cellar within reach
#[test]
fn the_underground_view_brings_the_cellar_within_reach() {
    let mut shop = shop();
    shop.zoom_onto(TEST);
    shop.harness.key_press(Key::U);
    shop.harness.run();
    let at = shop.on(TEST);

    shop.click(at, Modifiers::NONE);

    assert_eq!(shop.selection(), Some(shop.path(TEST)));
}

/// Holding the underground key brings the cellar within reach while held
#[test]
fn holding_the_underground_key_brings_the_cellar_within_reach_while_held() {
    let mut shop = shop();
    shop.zoom_onto(TEST);
    shop.harness
        .event(Event::ModifiersChanged(Modifiers::SHIFT));
    shop.harness.run();
    let at = shop.on(TEST);

    shop.click(at, Modifiers::SHIFT);

    assert_eq!(shop.selection(), Some(shop.path(TEST)));
}
