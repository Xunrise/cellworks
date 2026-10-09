//! STORY-004, move one layer in and out: Enter goes one layer in, Esc one
//! layer out, and the selection is outlined on every layer.
//!
//! One test per scenario of issue #6, named after it; a scenario outline is
//! one test looping over its examples. The universe holds the code projects
//! "shop" and "billing", raised as real folder trees in temporary
//! directories. A click selects, as the story leaves choosing the selection
//! by keyboard to EPIC-003. No test runs git, reads `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_navigation::Layer;
use egui::Key;
use egui::accesskit::{Rect, Role};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Recorded, Tree, app, await_read, harness, labels, map};

/// The window the harness draws into.
const WIDTH: f64 = 1200.0;
const HEIGHT: f64 = 900.0;

/// The background: "shop" with the district "catalog", its sub-district
/// "pricing", the group "rules" and the node "discount.rs"; and "billing".
struct Universe {
    shop: Tree,
    _billing: Tree,
    harness: Harness<'static, CellworksApp>,
}

fn universe() -> Universe {
    let shop = Tree::project(
        "shop",
        &[
            "catalog/pricing/rules/discount.rs",
            "catalog/pricing/rules/vat.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ],
    );
    let billing = Tree::project("billing", &["ledger/entry.rs"]);
    let harness = harness(app(
        vec![shop.as_project(None), billing.as_project(None)],
        Recorded::listing(&[]),
    ));
    Universe {
        shop,
        _billing: billing,
        harness,
    }
}

impl Universe {
    fn select(&mut self, name: &str) {
        self.harness.get_by_label(name).click();
        self.harness.run();
    }

    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    fn layer(&self) -> Layer {
        self.harness.state().navigation().layer()
    }

    /// The project "shop" is open and its map drawn.
    fn open_shop(&mut self) {
        self.select("shop");
        self.press(Key::Enter);
        await_read(&mut self.harness);
    }

    /// Opens "shop" and frames each of `names` in turn with Enter.
    fn frame(&mut self, names: &[&str]) {
        self.open_shop();
        for name in names {
            self.select(name);
            self.press(Key::Enter);
        }
    }

    fn rect_of(&self, name: &str) -> Rect {
        self.harness
            .get_by_label(name)
            .accesskit_node()
            .bounding_box()
            .expect("a label is laid out")
    }

    /// The labels marked selected in the accessibility tree.
    fn outlined(&self) -> Vec<String> {
        let root = self.harness.root();
        root.children_recursive()
            .map(|node| node.accesskit_node())
            .filter(|node| node.role() == Role::Label && node.is_selected() == Some(true))
            .filter_map(|node| node.value().map(|text| text.to_string()))
            .collect()
    }

    /// The names of everything the framed folder at `name` holds.
    fn held_by(&self, name: &str) -> Vec<String> {
        let map = map(&self.harness);
        let path = match name {
            "catalog" => self.shop.root.join("catalog"),
            "pricing" => self.shop.root.join("catalog/pricing"),
            "rules" => self.shop.root.join("catalog/pricing/rules"),
            other => panic!("no folder {other} in the background"),
        };
        let mut held = Vec::new();
        for district in &map.districts {
            for sub in &district.sub_districts {
                for group in &sub.groups {
                    if group.path.starts_with(&path) {
                        held.push(group.name.clone());
                        held.extend(group.nodes.iter().map(|node| node.name.clone()));
                    }
                }
                if sub.path.starts_with(&path) {
                    held.push(sub.name.clone());
                    held.extend(sub.nodes.iter().map(|node| node.name.clone()));
                }
            }
            if district.path.starts_with(&path) {
                held.extend(district.nodes.iter().map(|node| node.name.clone()));
            }
        }
        held
    }

    /// `name`'s label is the framed thing: inside the window and spanning a
    /// good part of it, with everything it holds inside the window too.
    fn assert_framed(&self, name: &str) {
        let framed = self.rect_of(name);
        assert!(in_window(framed), "{name} at {framed:?}");
        assert!(
            framed.width() > WIDTH * 0.4,
            "{name} at {framed:?} is not framed"
        );
        for held in self.held_by(name) {
            let rect = self.rect_of(&held);
            assert!(in_window(rect), "{held}, inside {name}, is at {rect:?}");
        }
    }

    /// Every project of the universe is listed in the window.
    fn assert_all_projects_shown(&self) {
        let mut shown = labels(&self.harness);
        shown.sort();
        assert_eq!(shown, ["billing", "shop"]);
        assert!(in_window(self.rect_of("shop")) && in_window(self.rect_of("billing")));
    }
}

fn in_window(rect: Rect) -> bool {
    rect.x0 >= 0.0 && rect.y0 >= 0.0 && rect.x1 <= WIDTH && rect.y1 <= HEIGHT
}

/// Enter on a project opens it
#[test]
fn enter_on_a_project_opens_it() {
    let mut universe = universe();
    universe.select("shop");

    universe.press(Key::Enter);
    await_read(&mut universe.harness);

    assert_eq!(universe.layer(), Layer::Project);
    let shown = labels(&universe.harness);
    assert!(shown.iter().any(|label| label == "catalog"), "{shown:?}");
    assert!(
        !shown
            .iter()
            .any(|label| label == "shop" || label == "billing"),
        "only the project shop is shown: {shown:?}"
    );
    for label in shown {
        let rect = universe.rect_of(&label);
        assert!(in_window(rect), "{label} at {rect:?} is outside the view");
    }
}

/// Enter on a folder frames it with what it holds
#[test]
fn enter_on_a_folder_frames_it_with_what_it_holds() {
    for (folder, path_to_it) in [
        ("catalog", &[][..]),
        ("pricing", &["catalog"][..]),
        ("rules", &["catalog", "pricing"][..]),
    ] {
        let mut universe = universe();
        universe.frame(path_to_it);
        universe.select(folder);

        universe.press(Key::Enter);

        assert_eq!(universe.layer(), Layer::Directory, "{folder}");
        universe.assert_framed(folder);
    }
}

/// Enter on a node frames it at inspection distance
#[test]
fn enter_on_a_node_frames_it_at_inspection_distance() {
    let mut universe = universe();
    universe.frame(&["catalog", "pricing", "rules"]);
    universe.select("discount.rs");

    universe.press(Key::Enter);

    assert_eq!(universe.layer(), Layer::File);
    let node = universe.rect_of("discount.rs");
    assert!(in_window(node), "{node:?}");
    // Wide enough for its name many times over: readable at a glance.
    assert!(
        node.width() > WIDTH * 0.4,
        "{node:?} is not close enough to read"
    );
}

/// Enter on a framed node changes nothing yet
#[test]
fn enter_on_a_framed_node_changes_nothing_yet() {
    let mut universe = universe();
    universe.frame(&["catalog", "pricing", "rules", "discount.rs"]);
    let before = universe.rect_of("discount.rs");

    universe.press(Key::Enter);

    assert_eq!(universe.layer(), Layer::File);
    assert_eq!(universe.rect_of("discount.rs"), before);
}

/// Esc moves one level out
#[test]
fn esc_moves_one_level_out() {
    for (from, path_to_it, to, selected) in [
        (
            "discount.rs",
            &["catalog", "pricing", "rules", "discount.rs"][..],
            "rules",
            "rules",
        ),
        (
            "rules",
            &["catalog", "pricing", "rules"][..],
            "pricing",
            "pricing",
        ),
        ("pricing", &["catalog", "pricing"][..], "catalog", "catalog"),
        ("catalog", &["catalog"][..], "shop", "catalog"),
        ("shop", &[][..], "the universe", "shop"),
    ] {
        let mut universe = universe();
        universe.frame(path_to_it);
        if from == "shop" {
            // The project is framed with the selection on it: Esc leaves it
            // whatever inside it was selected.
            universe.select("catalog");
        }

        universe.press(Key::Escape);

        match to {
            "shop" => assert_eq!(universe.layer(), Layer::Project, "from {from}"),
            "the universe" => {
                assert_eq!(universe.layer(), Layer::Universe, "from {from}");
                universe.assert_all_projects_shown();
            }
            folder => {
                assert_eq!(universe.layer(), Layer::Directory, "from {from}");
                universe.assert_framed(folder);
            }
        }
        assert_eq!(universe.outlined(), [selected], "from {from}");
    }
}

/// Esc in the universe frames all projects
#[test]
fn esc_in_the_universe_frames_all_projects() {
    let mut universe = universe();
    assert_eq!(universe.layer(), Layer::Universe);

    universe.press(Key::Escape);

    assert_eq!(universe.layer(), Layer::Universe);
    universe.assert_all_projects_shown();
}

/// The selection is visible on every layer
#[test]
fn the_selection_is_visible_on_every_layer() {
    let mut universe = universe();
    universe.frame(&["catalog", "pricing", "rules", "discount.rs"]);
    assert_eq!(universe.outlined(), ["discount.rs"]);

    for selected in ["rules", "pricing", "catalog", "catalog", "shop"] {
        universe.press(Key::Escape);
        assert_eq!(universe.outlined(), [selected]);
    }
}
