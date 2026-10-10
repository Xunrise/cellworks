//! STORY-012, fly the camera over the city: zoom around the pointer, turn the
//! city a quarter, fit it into view, and jump with the minimap.
//!
//! One test per scenario of issue #34, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`). No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use egui::{Event, Key, Modifiers, Pos2, Rect, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Tree, map, shop_city};

/// The window the harness draws into.
const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 900.0;

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open as a city.
fn shop() -> Shop {
    let (tree, harness) = shop_city();
    Shop { tree, harness }
}

impl Shop {
    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    fn rect_of(&self, label: &str) -> Rect {
        let rect = self
            .harness
            .get_by_label(label)
            .accesskit_node()
            .bounding_box()
            .unwrap_or_else(|| panic!("{label} is not laid out"));
        Rect::from_min_max(
            pos2(rect.x0 as f32, rect.y0 as f32),
            pos2(rect.x1 as f32, rect.y1 as f32),
        )
    }

    /// Where the middle of the block of the file at `relative` is on screen.
    fn block_centre(&self, relative: &str) -> Pos2 {
        let map = map(&self.harness);
        let block = map.find(&self.tree.root.join(relative)).expect("the block");
        let view = self.harness.state().view().expect("the city is drawn");
        view.to_screen(block.area.center(), 0.0)
    }

    fn select(&mut self, name: &str) {
        self.harness.get_by_label(name).click_accesskit();
        self.harness.run();
    }

    fn selection(&self) -> Option<PathBuf> {
        let selection = self.harness.state().navigation().selection();
        selection.map(|path| path.to_path_buf())
    }
}

fn in_window(rect: Rect) -> bool {
    rect.min.x >= 0.0 && rect.min.y >= 0.0 && rect.max.x <= WIDTH && rect.max.y <= HEIGHT
}

/// Zooming around the pointer keeps that spot under the pointer
#[test]
fn zooming_around_the_pointer_keeps_that_spot_under_the_pointer() {
    let mut shop = shop();
    let before = shop.rect_of("pricing.rs");
    let pointer = shop.block_centre("catalog/pricing.rs");

    shop.harness.hover_at(pointer);
    shop.harness.event(Event::Zoom(1.5));
    shop.harness.run();

    let after = shop.block_centre("catalog/pricing.rs");
    assert!(
        (after - pointer).length() < 1.0,
        "{after:?} left {pointer:?}"
    );
    let grown = shop.rect_of("pricing.rs");
    assert!(
        grown.width() > before.width() * 1.4,
        "{before:?} → {grown:?}"
    );
}

/// Rotating turns the city by a quarter
#[test]
fn rotating_turns_the_city_by_a_quarter() {
    let mut shop = shop();
    shop.select("pricing.rs");
    let block = shop.tree.root.join("catalog/pricing.rs");
    let before = map(&shop.harness).find(&block).expect("the block").area;

    shop.press(Key::Q);

    let after = map(&shop.harness).find(&block).expect("the block").area;
    assert!(
        (after.width() - before.height()).abs() < 1e-3,
        "{before:?} → {after:?}"
    );
    assert!(
        (after.height() - before.width()).abs() < 1e-3,
        "{before:?} → {after:?}"
    );
    assert_eq!(shop.selection(), Some(block.clone()));

    // Shift+Q turns back.
    shop.harness.key_press_modifiers(Modifiers::SHIFT, Key::Q);
    shop.harness.run();
    let back = map(&shop.harness).find(&block).expect("the block").area;
    assert!(
        (back.min - before.min).length() < 1e-2,
        "{before:?} → {back:?}"
    );
}

/// Fitting brings the whole city into view
#[test]
fn fitting_brings_the_whole_city_into_view() {
    let mut shop = shop();
    shop.select("orders");
    shop.press(Key::Enter);
    let middle = pos2(WIDTH / 2.0, HEIGHT / 2.0);
    shop.harness.hover_at(middle);
    for _ in 0..6 {
        shop.harness.event(Event::Zoom(1.5));
        shop.harness.run();
    }
    assert!(!in_window(shop.rect_of("orders")), "zoomed into orders");

    shop.press(Key::F);

    for district in ["catalog", "orders"] {
        let rect = shop.rect_of(district);
        assert!(in_window(rect), "{district} at {rect:?}");
    }
}

/// The minimap shows where the view is and jumps on request
#[test]
fn the_minimap_shows_where_the_view_is_and_jumps_on_request() {
    let mut shop = shop();
    let middle = pos2(WIDTH / 2.0, HEIGHT / 2.0);
    shop.harness.hover_at(middle);
    // Zoomed in, but not so close that the camera turns to a facade.
    for _ in 0..4 {
        let scale = shop.harness.state().view().expect("drawn").scale();
        if scale * 1.5 >= 4.0 {
            break;
        }
        shop.harness.event(Event::Zoom(1.5));
        shop.harness.run();
    }

    shop.harness.get_by_label("orders on the minimap").click();
    shop.harness.run();

    let orders = shop.rect_of("orders");
    assert!(
        (orders.center().x - WIDTH / 2.0).abs() < 2.0,
        "orders at {orders:?}"
    );
    let frame = shop.rect_of("View on the minimap");
    let spot = shop.rect_of("orders on the minimap").center();
    assert!(frame.contains(spot), "{frame:?} does not mark {spot:?}");
}
