//! STORY-006, the layer follows the zoom, unless the layer is locked.
//!
//! One test per scenario of issue #24, named after it. The project "shop" is a
//! real folder tree in a temporary directory: the district "catalog" holds the
//! group "rules" with the node "discount.rs". The user zooms by pinching over
//! a point of the map, sent to the harness as zoom events; a click selects. No
//! test runs git, reads `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_navigation::Layer;
use egui::accesskit::Role;
use egui::{Event, Key, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{BREADCRUMB, Recorded, Tree, open};

/// The window the harness draws into.
const WIDTH: f32 = 1200.0;
const HEIGHT: f32 = 900.0;

/// How far one pinch zooms.
const PINCH: f32 = 1.25;

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open on the Project layer.
fn shop() -> Shop {
    let tree = Tree::project(
        "shop",
        &[
            "catalog/rules/discount.rs",
            "catalog/rules/vat.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn select(&mut self, name: &str) {
        self.harness.get_by_label(name).click();
        self.harness.run();
    }

    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    /// The node "discount.rs" is framed on the File layer.
    fn frame_discount(&mut self) {
        for name in ["catalog", "rules", "discount.rs"] {
            self.select(name);
            self.press(Key::Enter);
        }
        assert_eq!(self.layer(), Layer::File);
    }

    fn layer(&self) -> Layer {
        self.harness.state().navigation().layer()
    }

    fn selection(&self) -> Option<String> {
        let selected = self.harness.state().navigation().selection()?;
        let relative = selected.strip_prefix(&self.tree.root).ok()?;
        Some(relative.to_string_lossy().into_owned())
    }

    fn rect_of(&self, name: &str) -> egui::Rect {
        let rect = self
            .harness
            .get_by_label(name)
            .accesskit_node()
            .bounding_box()
            .expect("a label is laid out");
        egui::Rect::from_min_max(
            pos2(rect.x0 as f32, rect.y0 as f32),
            pos2(rect.x1 as f32, rect.y1 as f32),
        )
    }

    /// One pinch by `factor` with the pointer at `at`.
    fn pinch(&mut self, at: Pos2, factor: f32) {
        self.harness.hover_at(at);
        self.harness.event(Event::Zoom(factor));
        self.harness.run();
    }

    /// Zooms in over the node "discount.rs" until it fills most of the view.
    fn zoom_into_discount(&mut self) {
        for _ in 0..40 {
            let node = self.rect_of("discount.rs");
            if node.width() > WIDTH * 0.6 {
                return;
            }
            self.pinch(node.center(), PINCH);
        }
        panic!("discount.rs never filled the view");
    }

    /// The breadcrumb above the map.
    fn breadcrumb(&self) -> String {
        self.harness
            .root()
            .children_recursive()
            .map(|node| node.accesskit_node())
            .filter(|node| node.role() == Role::Label)
            .filter_map(|node| node.value().map(|text| text.to_string()))
            .find(|text| text.starts_with(BREADCRUMB))
            .expect("a breadcrumb")
    }
}

/// Zooming into a node puts the user on the File layer
#[test]
fn zooming_into_a_node_puts_the_user_on_the_file_layer() {
    let mut shop = shop();
    assert_eq!(shop.layer(), Layer::Project);

    shop.zoom_into_discount();

    assert_eq!(shop.layer(), Layer::File);
    assert_eq!(
        shop.selection().as_deref(),
        Some("catalog/rules/discount.rs")
    );
}

/// Zooming out puts the user back on the outer layer
#[test]
fn zooming_out_puts_the_user_back_on_the_outer_layer() {
    let mut shop = shop();
    shop.frame_discount();

    let middle = pos2(WIDTH / 2.0, HEIGHT / 2.0);
    for _ in 0..30 {
        shop.pinch(middle, 1.0 / PINCH);
    }

    assert_eq!(shop.layer(), Layer::Project);
    assert_eq!(shop.selection().as_deref(), Some("catalog"));
    for name in ["catalog", "orders", "discount.rs", "order.rs"] {
        let rect = shop.rect_of(name);
        assert!(
            rect.min.x >= 0.0 && rect.max.x <= WIDTH && rect.max.y <= HEIGHT,
            "{name} at {rect:?} is outside the view"
        );
    }
}

/// Zooming into empty space keeps the selection
#[test]
fn zooming_into_empty_space_keeps_the_selection() {
    let mut shop = shop();
    shop.select("catalog");
    // The whole project is drawn at its own size in the middle of the window,
    // so the bottom-left corner of the window is empty space.
    let empty = pos2(10.0, HEIGHT - 10.0);

    for _ in 0..20 {
        shop.pinch(empty, PINCH);
    }

    assert_eq!(shop.layer(), Layer::Project);
    assert_eq!(shop.selection().as_deref(), Some("catalog"));
}

/// A locked layer stays put while zooming
#[test]
fn a_locked_layer_stays_put_while_zooming() {
    let mut shop = shop();
    shop.press(Key::Z);
    shop.select("catalog");

    shop.zoom_into_discount();

    assert_eq!(shop.layer(), Layer::Project);
    assert_eq!(shop.selection().as_deref(), Some("catalog"));
    let node = shop.rect_of("discount.rs");
    assert!(
        node.min.x >= 0.0 && node.max.x <= WIDTH && node.width() > WIDTH * 0.6,
        "the name of discount.rs is not shown: {node:?}"
    );
}

/// Switching the behaviour keeps the current place
#[test]
fn switching_the_behaviour_keeps_the_current_place() {
    let mut shop = shop();
    shop.frame_discount();
    let before = shop.rect_of("discount.rs");

    for locked in [true, false] {
        shop.press(Key::Z);

        assert_eq!(shop.harness.state().navigation().is_locked(), locked);
        assert_eq!(shop.layer(), Layer::File);
        assert_eq!(
            shop.selection().as_deref(),
            Some("catalog/rules/discount.rs")
        );
        assert_eq!(shop.rect_of("discount.rs"), before);
    }
}

/// The breadcrumb names the path and the behaviour
#[test]
fn the_breadcrumb_names_the_path_and_the_behaviour() {
    let mut shop = shop();
    shop.frame_discount();

    assert_eq!(
        shop.breadcrumb(),
        "Universe › shop › catalog › rules › discount.rs"
    );
    shop.harness
        .get_by_label("Layer follows the zoom (Z to lock)");

    shop.press(Key::Z);
    shop.harness
        .get_by_label("Layer locked (Z to follow the zoom)");
}
