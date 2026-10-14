//! STORY-007, the folder view is the Directory layer: stepping into a folder
//! shows only what it holds, and Esc has one step per layer.
//!
//! One test per scenario of issue #25, named after it. The project "shop" is a
//! real folder tree in a temporary directory: the district "catalog" holds the
//! group "rules" (under the sub-district "pricing") with the node
//! "discount.rs", and the district "orders" holds "order.rs". The map has no
//! lanes yet, so the ring of outside connections and the import of the
//! background are not asserted; they arrive with the lanes. A click selects;
//! the jump is made through the app, as no key reaches it yet. No test runs
//! git, reads `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_navigation::Layer;
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Recorded, Tree, labels, open};

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open on the Project layer.
fn shop() -> Shop {
    let tree = Tree::project(
        "shop",
        &[
            "catalog/pricing/base.rs",
            "catalog/pricing/rules/discount.rs",
            "catalog/pricing/rules/vat.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn select(&mut self, name: &str) {
        self.harness.get_by_label(name).click_accesskit();
        self.harness.run();
    }

    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    /// Selects each of `names` in turn and steps into it with Enter.
    fn step_into(&mut self, names: &[&str]) {
        for name in names {
            self.select(name);
            self.press(Key::Enter);
        }
    }

    fn layer(&self) -> Layer {
        self.harness.state().navigation().layer()
    }

    fn shown(&self) -> Vec<String> {
        let mut shown = labels(&self.harness);
        shown.sort();
        shown
    }

    /// The labels marked selected in the accessibility tree.
    fn outlined(&self) -> Vec<String> {
        self.harness
            .root()
            .children_recursive()
            .map(|node| node.accesskit_node())
            .filter(|node| node.role() == Role::Label && node.is_selected() == Some(true))
            .filter_map(|node| node.value().map(|text| text.to_string()))
            .collect()
    }
}

/// Enter on a folder shows its folder view
#[test]
fn enter_on_a_folder_shows_its_folder_view() {
    let mut shop = shop();
    shop.select("catalog");

    shop.press(Key::Enter);

    assert_eq!(shop.layer(), Layer::Directory);
    assert_eq!(
        shop.shown(),
        [
            "base.rs",
            "catalog",
            "discount.rs",
            "item.rs",
            "pricing",
            "rules",
            "vat.rs"
        ]
    );
}

/// One Esc leaves the folder and the layer together
#[test]
fn one_esc_leaves_the_folder_and_the_layer_together() {
    let mut shop = shop();
    shop.step_into(&["catalog", "pricing", "rules"]);
    assert_eq!(shop.layer(), Layer::Directory);

    shop.press(Key::Escape);

    assert_eq!(shop.layer(), Layer::Project);
    assert_eq!(
        shop.shown(),
        [
            "base.rs",
            "catalog",
            "discount.rs",
            "item.rs",
            "order.rs",
            "orders",
            "pricing",
            "rules",
            "vat.rs"
        ]
    );
}

/// Esc from a node returns to its folder view
#[test]
fn esc_from_a_node_returns_to_its_folder_view() {
    let mut shop = shop();
    shop.step_into(&["catalog", "pricing", "rules", "discount.rs"]);
    assert_eq!(shop.layer(), Layer::File);

    shop.press(Key::Escape);

    assert_eq!(shop.layer(), Layer::Directory);
    assert_eq!(shop.shown(), ["discount.rs", "rules", "vat.rs"]);
}

/// A jump to a node outside the folder view opens its folder
#[test]
fn a_jump_to_a_node_outside_the_folder_view_opens_its_folder() {
    let mut shop = shop();
    shop.step_into(&["catalog", "pricing", "rules"]);

    let order = shop.tree.root.join("orders/order.rs");
    shop.harness.state_mut().jump(order.clone());
    shop.harness.run();

    assert_eq!(shop.layer(), Layer::Directory);
    assert_eq!(shop.shown(), ["order.rs", "orders"]);
    assert_eq!(
        shop.harness.state().navigation().selection(),
        Some(order.as_path())
    );
    assert_eq!(shop.outlined(), ["order.rs"]);
}
