//! STORY-008, the tree lists the current layer: the tree beside the map lists
//! what the current layer holds and follows the map's selection.
//!
//! One test per scenario of issue #26, named after it. The project "shop" is a
//! real folder tree in a temporary directory with the districts "catalog" and
//! "orders". A folder built from files always holds something, so the empty
//! district of the last scenario is a hand-built map drawn by the tree alone.
//! No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::{District, ProjectMap};
use cellworks_navigation::{Layer, Navigation};
use egui::Key;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, Node};

use common::{Recorded, Tree, open};

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open on the Project layer with the districts
/// "catalog" and "orders".
fn shop() -> Shop {
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
    Shop { tree, harness }
}

/// The rows of the tree, in order: the buttons that can be highlighted.
fn rows<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    root.children_recursive()
        .filter(|node| {
            let node = node.accesskit_node();
            node.role() == Role::Button && node.toggled().is_some()
        })
        .collect()
}

fn name(node: &Node<'_>) -> String {
    node.accesskit_node()
        .label()
        .or_else(|| node.accesskit_node().value())
        .map(|text| text.to_string())
        .unwrap_or_default()
}

impl Shop {
    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    /// The user opens the tree, and highlights `name` with a click on its row.
    fn open_tree_on(&mut self, name: &str) {
        self.press(Key::E);
        assert!(self.harness.state().tree_open());
        let row = rows(self.harness.root())
            .into_iter()
            .find(|row| self::name(row) == name)
            .expect("a row");
        row.click();
        self.harness.run();
        assert_eq!(self.highlighted().as_deref(), Some(name));
    }

    fn listed(&self) -> Vec<String> {
        rows(self.harness.root()).iter().map(name).collect()
    }

    fn highlighted(&self) -> Option<String> {
        rows(self.harness.root())
            .iter()
            .find(|row| row.accesskit_node().toggled() == Some(Toggled::True))
            .map(name)
    }

    fn selection(&self) -> Option<PathBuf> {
        self.harness
            .state()
            .navigation()
            .selection()
            .map(Path::to_path_buf)
    }
}

/// The tree lists the nodes of the current layer
#[test]
fn the_tree_lists_the_nodes_of_the_current_layer() {
    let mut shop = shop();

    shop.press(Key::E);

    assert_eq!(shop.listed(), ["catalog", "orders"]);
}

/// Moving in the tree moves the selection on the map
#[test]
fn moving_in_the_tree_moves_the_selection_on_the_map() {
    let mut shop = shop();
    shop.open_tree_on("catalog");

    shop.press(Key::ArrowDown);

    assert_eq!(shop.selection(), Some(shop.tree.root.join("orders")));
    assert_eq!(shop.highlighted().as_deref(), Some("orders"));
}

/// Moving on the map moves the tree
#[test]
fn moving_on_the_map_moves_the_tree() {
    let mut shop = shop();
    shop.press(Key::E);

    // The click lands on the map's label, which is a label; the tree's row is
    // not.
    shop.harness
        .get_all_by_label("orders")
        .find(|node| node.accesskit_node().role() == Role::Label)
        .expect("the map's label")
        .click();
    shop.harness.run();

    assert_eq!(shop.selection(), Some(shop.tree.root.join("orders")));
    assert_eq!(shop.highlighted().as_deref(), Some("orders"));
}

/// Stepping in from the tree changes the layer for both
#[test]
fn stepping_in_from_the_tree_changes_the_layer_for_both() {
    let mut shop = shop();
    shop.open_tree_on("catalog");

    shop.press(Key::Enter);

    assert_eq!(shop.harness.state().navigation().layer(), Layer::Directory);
    assert_eq!(shop.listed(), ["rules", "item.rs"]);
}

/// An empty layer shows an empty tree, not the previous one
#[test]
fn an_empty_layer_shows_an_empty_tree_not_the_previous_one() {
    let root = PathBuf::from("/p/shop");
    let catalog = root.join("catalog");
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200.0, 100.0));
    let map = ProjectMap {
        project: "shop".to_owned(),
        root: root.clone(),
        districts: vec![District {
            name: "catalog".to_owned(),
            path: catalog.clone(),
            sub_districts: Vec::new(),
            nodes: Vec::new(),
            area,
            kind: Default::default(),
            elevation: 0.0,
        }],
        extent: area,
        co_changes: Vec::new(),
        history: Default::default(),
        lens: Default::default(),
        homes: Default::default(),
        hub: None,
        stair: Vec::new(),
        cycles: Default::default(),
        contexts: None,
        roads: Vec::new(),
        arrangement: Default::default(),
    };
    let mut navigation = Navigation::default();
    navigation.opened(root);
    navigation.select(catalog);

    navigation.enter(Some(&map));

    let harness = Harness::new_ui_state(
        |ui, navigation: &mut Navigation| {
            cellworks_navigation::ui::tree(ui, &map, navigation);
        },
        navigation,
    );
    assert_eq!(harness.state().layer(), Layer::Directory);
    harness.get_by_label("catalog holds nothing.");
    assert!(rows(harness.root()).is_empty());
}
