//! STORY-045, the camera turns to face a facade: close up, each zoom step
//! turns the camera from the corner towards a straight front view of one
//! facade, and zooming out turns it back.
//!
//! One test per scenario of issue #69, named after it. The project "shop" is
//! a real folder tree of Rust sources in a temporary directory
//! (`common::shop_city`). What the camera does is read off the screen: from
//! the corner a tower's lines run down to the right, facing it they run
//! level. No test runs git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::ui::DrawnTower;
use egui::{Event, Pos2, pos2};
use egui_kittest::Harness;

use common::{Tree, shop_city, tower};

/// Where the camera starts to turn, and where it faces the facade.
const TURN_FROM: f32 = 4.0;
const TURN_TO: f32 = 16.0;

/// How steeply a line runs seen from the corner: 0.5 down for 0.866 across.
const FROM_THE_CORNER: f32 = 0.577;

/// The middle of the map's panel: the window, under the breadcrumb.
const MIDDLE_X: f32 = 600.0;

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

fn shop() -> Shop {
    let (tree, harness) = shop_city();
    Shop { tree, harness }
}

impl Shop {
    fn at(&self, name: &str) -> PathBuf {
        self.tree.root.join("catalog/pricing.rs").join(name)
    }

    /// The background: "total" is selected.
    fn select_total(&mut self) {
        let total = self.at("total");
        self.harness.state_mut().jump(total);
        self.harness.run();
    }

    fn drawn(&self, name: &str) -> DrawnTower {
        tower(&self.harness, &self.at(name)).clone()
    }

    fn scale(&self) -> f32 {
        self.harness
            .state()
            .view()
            .expect("the city is drawn")
            .scale()
    }

    /// How steeply the longest line of `name` runs on screen: down per across.
    fn slope(&self, name: &str) -> f32 {
        let tower = self.drawn(name);
        let line = tower
            .floors
            .iter()
            .max_by(|a, b| (a.end.x - a.start.x).total_cmp(&(b.end.x - b.start.x)))
            .expect("a floor");
        (line.end.y - line.start.y) / (line.end.x - line.start.x)
    }

    /// The middle of `name` on screen.
    fn middle_of(&self, name: &str) -> Pos2 {
        let tower = self.drawn(name);
        let (mut min, mut max) = (pos2(f32::MAX, f32::MAX), pos2(f32::MIN, f32::MIN));
        for floor in &tower.floors {
            for point in [floor.start, floor.end] {
                min = min.min(point);
                max = max.max(point);
            }
        }
        min + (max - min) / 2.0
    }

    /// One zoom step by `factor` with the pointer at `at`, and the frames it
    /// takes the camera to settle.
    fn zoom(&mut self, at: Pos2, factor: f32) {
        self.harness.hover_at(at);
        self.harness.event(Event::Zoom(factor));
        self.harness.run();
    }

    /// Zooms in over the middle of the view until the scale passes `until`.
    fn zoom_in_until(&mut self, until: f32) {
        while self.scale() < until {
            self.zoom(pos2(MIDDLE_X, 450.0), 1.15);
        }
    }
}

/// Zooming in turns the camera towards the facade
#[test]
fn zooming_in_turns_the_camera_towards_the_facade() {
    let mut shop = shop();
    shop.select_total();
    assert!((shop.slope("total") - FROM_THE_CORNER).abs() < 0.01);

    shop.zoom_in_until(TURN_TO);

    assert!(
        shop.slope("total").abs() < 0.01,
        "slope {}",
        shop.slope("total")
    );
}

/// Each zoom step turns the camera a little further
#[test]
fn each_zoom_step_turns_the_camera_a_little_further() {
    let mut shop = shop();
    shop.select_total();
    shop.zoom_in_until(TURN_FROM * 0.99);
    let before = shop.slope("total");

    shop.zoom_in_until(TURN_FROM * 1.4);

    let after = shop.slope("total");
    assert!(after < before - 0.01, "{before} → {after}");
    assert!(after > 0.01, "already facing it: {after}");
}

/// Zooming out turns the camera back
#[test]
fn zooming_out_turns_the_camera_back() {
    let mut shop = shop();
    shop.select_total();
    shop.zoom_in_until(TURN_TO);

    while shop.scale() > TURN_FROM * 0.9 {
        shop.zoom(pos2(MIDDLE_X, 450.0), 1.0 / 1.15);
    }

    assert!((shop.slope("total") - FROM_THE_CORNER).abs() < 0.01);
}

/// The facade turned to stays in the middle of the view
#[test]
fn the_facade_turned_to_stays_in_the_middle_of_the_view() {
    let mut shop = shop();
    shop.select_total();

    shop.zoom_in_until(TURN_TO);

    let middle = shop.middle_of("total");
    assert!((middle.x - MIDDLE_X).abs() < 40.0, "total at {middle:?}");
}

/// Without a selection the camera turns to the tower pointed at
#[test]
fn without_a_selection_the_camera_turns_to_the_tower_pointed_at() {
    let mut shop = shop();
    assert!(shop.harness.state().navigation().selection().is_none());

    while shop.scale() < TURN_TO {
        // Keep pointing at "discount" wherever it is drawn.
        let line = shop.drawn("discount").floors[0];
        let at = line.start + (line.end - line.start) * 0.5;
        shop.zoom(at, 1.15);
    }

    assert!(
        shop.slope("discount").abs() < 0.01,
        "slope {}",
        shop.slope("discount")
    );
    assert_eq!(
        shop.harness.state().navigation().facing(),
        Some(shop.at("discount").as_path())
    );
}

/// From afar the camera does not turn
#[test]
fn from_afar_the_camera_does_not_turn() {
    let mut shop = shop();
    shop.select_total();
    assert!(shop.scale() * 1.15 < TURN_FROM);

    shop.zoom(pos2(MIDDLE_X, 450.0), 1.15);

    assert!((shop.slope("total") - FROM_THE_CORNER).abs() < 0.01);
    assert!(shop.harness.state().navigation().facing().is_none());
}
