//! STORY-049, the layer is locked by default: zooming only moves the camera
//! until the engineer lets the layer follow the zoom.
//!
//! One test per scenario of issue #73, named after it. The project "shop" is
//! a real folder tree in a temporary directory. No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_navigation::Layer;
use egui::{Event, Key, Rect, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Recorded, Tree, map, open};

/// The background: "shop" is open on the Project layer.
fn shop() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::project(
        "shop",
        &[
            "catalog/rules/discount.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    assert_eq!(harness.state().navigation().layer(), Layer::Project);
    (tree, harness)
}

fn width_of(harness: &Harness<'_, CellworksApp>, label: &str) -> f32 {
    let rect = harness
        .get_by_label(label)
        .accesskit_node()
        .bounding_box()
        .expect("laid out");
    Rect::from_min_max(
        pos2(rect.x0 as f32, rect.y0 as f32),
        pos2(rect.x1 as f32, rect.y1 as f32),
    )
    .width()
}

/// Zooms in over the node "discount.rs" until its name spans most of the view.
fn zoom_into_discount(tree: &Tree, harness: &mut Harness<'static, CellworksApp>) {
    for _ in 0..40 {
        if width_of(harness, "discount.rs") > 1200.0 * 0.6 {
            return;
        }
        let found = map(harness)
            .find(&tree.root.join("catalog/rules/discount.rs"))
            .expect("the node")
            .area;
        let over = harness
            .state()
            .view()
            .expect("drawn")
            .to_screen(found.center(), 0.0);
        harness.hover_at(over);
        harness.event(Event::Zoom(1.25));
        harness.run();
    }
    panic!("discount.rs never filled the view");
}

/// Zooming keeps the layer by default
#[test]
fn zooming_keeps_the_layer_by_default() {
    let (tree, mut harness) = shop();

    zoom_into_discount(&tree, &mut harness);

    assert_eq!(harness.state().navigation().layer(), Layer::Project);
}

/// The layer can be made to follow the zoom
#[test]
fn the_layer_can_be_made_to_follow_the_zoom() {
    let (tree, mut harness) = shop();
    harness.key_press(Key::Z);
    harness.run();

    zoom_into_discount(&tree, &mut harness);

    assert_eq!(harness.state().navigation().layer(), Layer::File);
}
