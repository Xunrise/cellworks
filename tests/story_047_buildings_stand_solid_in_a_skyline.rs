//! STORY-047, buildings stand solid in a skyline: the towers of a block stand
//! tallest at the back, a nearer building hides the one behind it, the first
//! line is the top floor, towers keep their width from afar, and glass and
//! fog show what stands behind.
//!
//! One test per scenario of issue #71, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`): "pricing.rs" holds "total" of 40 lines and
//! "discount" of 12. No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::city;
use cellworks_map::ui::DrawnTower;
use egui::{Event, Key, Modifiers, Pos2, pos2};
use egui_kittest::Harness;

use common::{Tree, map, shop_city, tower};

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

fn shop() -> Shop {
    let (tree, mut harness) = shop_city();
    // The Explorer, open beside the map by default (STORY-089), is not what
    // this story's skyline is about, and facing a facade while it is open
    // keeps asking for frames well past the harness's step budget.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::B);
    harness.run();
    Shop { tree, harness }
}

impl Shop {
    fn at(&self, name: &str) -> PathBuf {
        self.tree.root.join("catalog/pricing.rs").join(name)
    }

    fn drawn(&self, name: &str) -> DrawnTower {
        tower(&self.harness, &self.at(name)).clone()
    }

    fn order_of(&self, name: &str) -> usize {
        let path = self.at(name);
        let towers = self.harness.state().towers();
        towers.iter().position(|t| t.path == path).expect("drawn")
    }

    /// A point on a line of `name` that no other tower's line comes near.
    fn clear_point_of(&self, name: &str) -> Pos2 {
        let target = self.drawn(name);
        let towers = self.harness.state().towers();
        for floor in &target.floors {
            for step in 0..=20 {
                let point = floor.start + (floor.end - floor.start) * (step as f32 / 20.0);
                let clear = towers
                    .iter()
                    .filter(|t| t.path != target.path)
                    .all(|t| t.floors.iter().all(|f| f.distance(point) > 5.0));
                if clear {
                    return point;
                }
            }
        }
        panic!("no line of {name} is clear of the others");
    }
}

/// The taller tower stands behind the shorter one
#[test]
fn the_taller_tower_stands_behind_the_shorter_one() {
    let shop = shop();
    let map = map(&shop.harness);

    let total = map.item(&shop.at("total")).expect("total").area;
    let discount = map.item(&shop.at("discount")).expect("discount").area;

    // Farther back is higher up the screen: ground x and y both run down it.
    let depth = |ground: egui::Rect| ground.center().x + ground.center().y;
    assert!(
        depth(total) < depth(discount),
        "total {total:?}, discount {discount:?}"
    );
}

/// A nearer building hides the one behind it
#[test]
fn a_nearer_building_hides_the_one_behind_it() {
    let shop = shop();

    let (total, discount) = (shop.drawn("total"), shop.drawn("discount"));

    assert!(
        shop.order_of("discount") > shop.order_of("total"),
        "drawn over it"
    );
    assert!(
        discount.outline.intersects(total.outline),
        "standing in front of it"
    );
    assert!(!discount.glass && discount.light >= 1.0, "solid");
}

/// The first line of a function is its top floor
#[test]
fn the_first_line_of_a_function_is_its_top_floor() {
    let shop = shop();

    let discount = shop.drawn("discount");

    let (first, last) = (
        discount.floors[0],
        discount.floors[discount.floors.len() - 1],
    );
    assert!(
        first.start.y < last.start.y,
        "first {first:?}, last {last:?}"
    );
}

/// A tower keeps its width from afar
#[test]
fn a_tower_keeps_its_width_from_afar() {
    let shop = shop();
    let scale = shop.harness.state().view().expect("drawn").scale();
    let map = map(&shop.harness);
    let total = map.item(&shop.at("total")).expect("total");

    let drawn = shop.drawn("total");

    let wall = city::width(total);
    for (floor, span) in total.floors.iter().zip(&drawn.floors) {
        let start = floor.indent as f32 * city::COLUMN;
        let end = (floor.width as f32 * city::COLUMN).min(wall).max(start);
        let wide = (end - start) * scale;
        let length = (span.end - span.start).length();
        assert!(
            (length - wide).abs() < 0.5,
            "line {}: {length} for {wide}",
            floor.number
        );
    }
}

/// A hidden tower shows through when pointed at
#[test]
fn a_hidden_tower_shows_through_when_pointed_at() {
    let mut shop = shop();
    let point = shop.clear_point_of("total");

    shop.harness.hover_at(point);
    shop.harness.run();

    assert!(shop.drawn("discount").glass, "discount stays solid");
}

/// Facing a tower, what stands in front of it fades away
#[test]
fn facing_a_tower_what_stands_in_front_of_it_fades_away() {
    let mut shop = shop();
    let total = shop.at("total");
    shop.harness.state_mut().jump(total);
    shop.harness.run();

    while shop.harness.state().view().expect("drawn").scale() < 16.0 {
        shop.harness.hover_at(pos2(600.0, 450.0));
        shop.harness.event(Event::Zoom(1.15));
        shop.harness.run();
    }

    assert!(
        shop.drawn("discount").light < 0.1,
        "light {}",
        shop.drawn("discount").light
    );
    assert!(shop.drawn("total").light >= 1.0);
}
