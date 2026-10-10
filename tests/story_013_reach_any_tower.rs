//! STORY-013, reach any tower, even behind other towers: towers in front of
//! the one pointed at turn to glass, a line picks its own tower, Tab cycles
//! the towers of a block, and empty ground selects nothing.
//!
//! One test per scenario of issue #35, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`). The issue's background has "total" stand in front
//! of "discount"; since STORY-047 a block is a skyline, so the shorter
//! "discount" stands in front of the taller "total". The tests keep the
//! scenarios and swap the two names: [`FRONT`] stands in front of
//! [`BEHIND`]. No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::ui::DrawnTower;
use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Tree, map, shop_city, tower};

/// The tower standing in front, and the one behind it.
const FRONT: &str = "discount";
const BEHIND: &str = "total";

/// How near a line a point must be to be on it, and how far from every other
/// line to be clear of it, on screen.
const ON: f32 = 0.5;
const CLEAR: f32 = 5.0;

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open as a city, and [`FRONT`] stands in front
/// of [`BEHIND`].
fn shop() -> Shop {
    let (tree, harness) = shop_city();
    let shop = Shop { tree, harness };
    let (front, behind) = (shop.drawn(FRONT), shop.drawn(BEHIND));
    let order = |path: &Path| {
        let towers = shop.harness.state().towers();
        towers.iter().position(|t| t.path == path)
    };
    assert!(
        order(&front.path) > order(&behind.path),
        "{FRONT} is drawn after {BEHIND}"
    );
    shop
}

impl Shop {
    fn at(&self, name: &str) -> PathBuf {
        self.tree.root.join("catalog/pricing.rs").join(name)
    }

    fn drawn(&self, name: &str) -> DrawnTower {
        tower(&self.harness, &self.at(name)).clone()
    }

    /// A point on a line of `name` that no other tower's line comes near.
    fn clear_point_of(&self, name: &str) -> Pos2 {
        let target = self.drawn(name);
        let others: Vec<&DrawnTower> = self
            .harness
            .state()
            .towers()
            .iter()
            .filter(|t| t.path != target.path)
            .collect();
        for floor in &target.floors {
            for step in 0..=20 {
                let point = floor.start + (floor.end - floor.start) * (step as f32 / 20.0);
                let clear = others
                    .iter()
                    .all(|t| t.floors.iter().all(|f| f.distance(point) > CLEAR));
                if floor.distance(point) <= ON && clear {
                    return point;
                }
            }
        }
        panic!("no line of {name} is clear of the others");
    }

    /// A point of the ground of "catalog" off every block and every line.
    fn empty_ground(&self) -> Pos2 {
        let map = map(&self.harness);
        let view = self.harness.state().view().expect("the city is drawn");
        let catalog = map
            .find(&self.tree.root.join("catalog"))
            .expect("catalog")
            .area;
        let blocks: Vec<egui::Rect> = map.districts[0].nodes.iter().map(|n| n.area).collect();
        for ring in 0..12 {
            for step in 0..24 {
                let angle = step as f32 / 24.0 * std::f32::consts::TAU;
                let ground = catalog.center() + ring as f32 * egui::vec2(angle.cos(), angle.sin());
                let point = view.to_screen(ground, 0.0);
                let off_blocks = blocks.iter().all(|b| !b.expand(1.0).contains(ground));
                let off_lines = self
                    .harness
                    .state()
                    .towers()
                    .iter()
                    .all(|t| t.floors.iter().all(|f| f.distance(point) > CLEAR));
                if off_blocks && off_lines {
                    return point;
                }
            }
        }
        panic!("no empty ground on the plaza of catalog");
    }

    fn hover(&mut self, at: Pos2) {
        self.harness.hover_at(at);
        self.harness.run();
    }

    fn click(&mut self, at: Pos2) {
        self.harness.hover_at(at);
        for pressed in [true, false] {
            self.harness.event(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::default(),
            });
        }
        self.harness.run();
    }

    fn selection(&self) -> Option<PathBuf> {
        let selection = self.harness.state().navigation().selection();
        selection.map(Path::to_path_buf)
    }
}

/// Towers in front of the hovered tower turn transparent
#[test]
fn towers_in_front_of_the_hovered_tower_turn_transparent() {
    let mut shop = shop();
    let point = shop.clear_point_of(BEHIND);

    shop.hover(point);

    assert!(shop.drawn(FRONT).glass, "{FRONT} stays solid");
    assert!(!shop.drawn(BEHIND).glass, "{BEHIND} turned to glass");
}

/// Pointing at a line selects the tower that line belongs to
#[test]
fn pointing_at_a_line_selects_the_tower_that_line_belongs_to() {
    let mut shop = shop();
    let point = shop.clear_point_of(BEHIND);

    shop.click(point);

    assert_eq!(shop.selection(), Some(shop.at(BEHIND)));
}

/// Cycling within a block reaches every tower of it
#[test]
fn cycling_within_a_block_reaches_every_tower_of_it() {
    let mut shop = shop();
    let point = shop.clear_point_of("total");
    shop.click(point);
    assert_eq!(shop.selection(), Some(shop.at("total")));

    shop.harness.key_press(Key::Tab);
    shop.harness.run();

    assert_eq!(shop.selection(), Some(shop.at("discount")));
}

/// Pointing at empty ground selects nothing
#[test]
fn pointing_at_empty_ground_selects_nothing() {
    let mut shop = shop();
    shop.harness.get_by_label("pricing.rs").click_accesskit();
    shop.harness.run();
    assert!(shop.selection().is_some());
    let point = shop.empty_ground();

    shop.click(point);

    assert_eq!(shop.selection(), None);
}
