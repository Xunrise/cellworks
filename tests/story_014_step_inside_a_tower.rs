//! STORY-014, step inside a tower and read its floors: one row per line, in
//! source order, with the calls it makes; j and k move a floor; Esc returns
//! to the city with the tower still selected; no editor opens.
//!
//! One test per scenario of issue #36, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`). No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_navigation::Layer;
use egui::Key;
use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

use common::{Tree, shop_city};

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open as a city and the tower "discount" is
/// selected.
fn shop() -> Shop {
    let (tree, mut harness) = shop_city();
    let discount = tree.root.join("catalog/pricing.rs/discount");
    harness.state_mut().jump(discount.clone());
    harness.run();
    assert_eq!(
        harness.state().navigation().selection(),
        Some(discount.as_path())
    );
    Shop { tree, harness }
}

impl Shop {
    fn discount(&self) -> PathBuf {
        self.tree.root.join("catalog/pricing.rs/discount")
    }

    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    /// The rows inside the tower, in order: the selectable labels.
    fn rows(&self) -> Vec<(String, bool)> {
        self.harness
            .root()
            .children_recursive()
            .map(|node| node.accesskit_node())
            .filter(|node| node.toggled().is_some())
            .filter_map(|node| {
                let text = node.label().or_else(|| node.value())?.to_string();
                Some((text, node.toggled() == Some(Toggled::True)))
            })
            .filter(|(text, _)| text.trim_start().starts_with(|c: char| c.is_ascii_digit()))
            .collect()
    }

    fn selected_row(&self) -> usize {
        self.rows()
            .iter()
            .position(|(_, selected)| *selected)
            .expect("a selected row")
    }
}

/// Stepping inside shows the lines of the function
#[test]
fn stepping_inside_shows_the_lines_of_the_function() {
    let mut shop = shop();

    shop.press(Key::Enter);

    assert_eq!(shop.harness.state().navigation().layer(), Layer::Item);
    let rows = shop.rows();
    assert_eq!(rows.len(), 12, "{rows:?}");
    let numbers: Vec<usize> = rows
        .iter()
        .map(|(text, _)| text.split_whitespace().next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(numbers, (1..=12).collect::<Vec<_>>());
    assert!(
        rows[0].0.contains("fn discount(price: u32) -> u32 {"),
        "{:?}",
        rows[0]
    );
    assert!(
        rows[2].0.contains("        let deeper = base;"),
        "{:?}",
        rows[2]
    );
}

/// Floors that call other functions say what they call
#[test]
fn floors_that_call_other_functions_say_what_they_call() {
    let mut shop = shop();

    shop.press(Key::Enter);

    let rows = shop.rows();
    assert!(rows[3].0.ends_with("calls round_price"), "{:?}", rows[3]);
    assert!(!rows[2].0.contains("calls"), "{:?}", rows[2]);
}

/// Moving between floors moves the selection by one line
#[test]
fn moving_between_floors_moves_the_selection_by_one_line() {
    let mut shop = shop();
    shop.press(Key::Enter);
    assert_eq!(shop.selected_row(), 0);

    shop.press(Key::J);
    assert_eq!(shop.selected_row(), 1);

    shop.press(Key::K);
    assert_eq!(shop.selected_row(), 0);
}

/// Stepping out returns to the city with the tower still selected
#[test]
fn stepping_out_returns_to_the_city_with_the_tower_still_selected() {
    let mut shop = shop();
    shop.press(Key::Enter);

    shop.press(Key::Escape);

    assert_ne!(shop.harness.state().navigation().layer(), Layer::Item);
    assert!(
        !shop.harness.state().towers().is_empty(),
        "the city is shown"
    );
    let selection = shop.harness.state().navigation().selection();
    assert_eq!(selection, Some(shop.discount().as_path()));
}

/// Stepping inside never opens the editor by itself
#[test]
fn stepping_inside_never_opens_the_editor_by_itself() {
    let mut shop = shop();

    shop.press(Key::Enter);

    let editors = shop
        .harness
        .root()
        .children_recursive()
        .filter(|node| {
            matches!(
                node.accesskit_node().role(),
                Role::TextInput | Role::MultilineTextInput
            )
        })
        .count();
    assert_eq!(editors, 0, "an editor is open");
    assert_eq!(shop.harness.state().navigation().layer(), Layer::Item);
}
