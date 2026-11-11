//! STORY-052, a lens shows its lanes: import lanes run along the ground and
//! bundle into a highway between two districts, call lanes arc through the
//! air, every lane carries the colour of the building it starts from, the
//! overview is quiet, and each lens shows its own lanes.
//!
//! One test per scenario of issue #76, named after it. The project "shop" is
//! two crates in a temporary directory: "orders" imports "Price" from
//! "catalog/src/pricing.rs" in "checkout.rs", "refund.rs" and "invoice.rs",
//! and its function "charge" calls "discount" in "pricing.rs". No test runs
//! git, reads `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::ui::{DrawnLane, colour_of};
use cellworks_map::{LaneKind, Lens};
use egui_kittest::Harness;

use common::{Recorded, Tree, open};

const IMPORTING: &str =
    "use catalog::pricing::Price;\n\nfn use_it(price: Price) {\n    let kept = price;\n}\n";

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
            ("catalog/src/lib.rs", "pub mod pricing;\n"),
            (
                "catalog/src/pricing.rs",
                "pub struct Price;\n\npub fn discount() -> u32 {\n    1\n}\n",
            ),
            ("orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
            (
                "orders/src/lib.rs",
                "mod checkout;\nmod invoice;\nmod refund;\n\npub fn charge() {\n    catalog::pricing::discount();\n}\n",
            ),
            ("orders/src/checkout.rs", IMPORTING),
            ("orders/src/invoice.rs", IMPORTING),
            ("orders/src/refund.rs", IMPORTING),
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

impl Shop {
    fn file(&self, path: &str) -> PathBuf {
        self.tree.root.join(path)
    }

    /// The engineer presses `key`, and a moment passes.
    fn press(&mut self, key: egui::Key) {
        self.harness.key_press(key);
        self.harness.run_steps(120);
    }

    fn switch_to(&mut self, lens: Lens) {
        for _ in 0..Lens::ALL.len() {
            if self.harness.state().opened().map(|o| o.lens()) == Some(lens) {
                return;
            }
            self.harness
                .key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::L);
            self.harness.run_steps(120);
        }
        assert_eq!(self.harness.state().opened().map(|o| o.lens()), Some(lens));
    }

    fn lanes(&self) -> Vec<DrawnLane> {
        self.harness.state().lanes().to_vec()
    }

    fn import_lane(&self, from: &str) -> DrawnLane {
        let (from, to) = (self.file(from), self.file("catalog/src/pricing.rs"));
        self.lanes()
            .into_iter()
            .find(|lane| lane.kind == LaneKind::Imports && lane.from == from && lane.to == to)
            .unwrap_or_else(|| panic!("no import lane from {}", from.display()))
    }
}

/// Import lanes run along the ground
#[test]
fn import_lanes_run_along_the_ground() {
    let mut shop = shop();

    shop.press(egui::Key::G);

    let lane = shop.import_lane("orders/src/checkout.rs");
    let view = shop.harness.state().view().expect("a view");
    assert!(!lane.ground.is_empty());
    for (on_screen, on_ground) in lane.screen.iter().zip(&lane.ground) {
        assert!(
            (*on_screen - view.to_screen(*on_ground, 0.0)).length() < 1e-3,
            "at ground height"
        );
    }
}

/// Import lanes between two districts bundle into a highway
#[test]
fn import_lanes_between_two_districts_bundle_into_a_highway() {
    let mut shop = shop();

    shop.press(egui::Key::G);

    let lanes: Vec<DrawnLane> = ["checkout", "invoice", "refund"]
        .iter()
        .map(|file| shop.import_lane(&format!("orders/src/{file}.rs")))
        .collect();
    assert!(lanes.iter().all(|lane| lane.highway));
    // Side by side along the highway, each its own lane.
    for (i, one) in lanes.iter().enumerate() {
        for other in &lanes[i + 1..] {
            let apart = (one.ground[2] - other.ground[2]).length();
            assert!(apart > 1.0 && apart < 40.0, "{apart}");
        }
    }
}

/// Call lanes arc through the air
#[test]
fn call_lanes_arc_through_the_air() {
    let mut shop = shop();

    shop.press(egui::Key::A);

    let (charge, discount) = (
        shop.file("orders/src/lib.rs/charge"),
        shop.file("catalog/src/pricing.rs/discount"),
    );
    let lane = shop
        .lanes()
        .into_iter()
        .find(|lane| lane.kind == LaneKind::Calls && lane.from == charge && lane.to == discount)
        .expect("a call lane from charge to discount");
    assert_eq!(lane.line, Some(6), "from the line that calls");
    let (start, end) = (lane.screen[0], *lane.screen.last().expect("an end"));
    let top = lane
        .screen
        .iter()
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min);
    assert!(top < start.y.min(end.y) - 5.0, "it rises above both ends");
}

/// A lane carries the colour of the building it starts from
#[test]
fn a_lane_carries_the_colour_of_the_building_it_starts_from() {
    let mut shop = shop();

    shop.press(egui::Key::G);

    let lane = shop.import_lane("orders/src/checkout.rs");
    assert_eq!(lane.colour, colour_of(&shop.file("orders/src/checkout.rs")));
    assert_ne!(lane.colour, shop.import_lane("orders/src/refund.rs").colour);
}

/// Lanes are quiet in the overview
#[test]
fn lanes_are_quiet_in_the_overview() {
    let mut shop = shop();
    shop.press(egui::Key::G);
    shop.press(egui::Key::A);

    let mut frames = 0;
    let mut moving = std::collections::HashMap::<(PathBuf, PathBuf), usize>::new();
    for _ in 0..600 {
        shop.harness.step();
        frames += 1;
        for lane in shop.lanes() {
            assert!(!lane.lit, "nothing is selected, so no lane stands out");
            *moving
                .entry((lane.from.clone(), lane.to.clone()))
                .or_default() += usize::from(lane.streak);
        }
    }

    assert!(moving.values().any(|n| *n > 0), "traffic moves");
    for (lane, n) in moving {
        // Now and then: every lane also rests.
        assert!(
            n < frames * 4 / 5,
            "{lane:?} moved in {n} of {frames} frames"
        );
    }
}

/// Each lens shows its own lanes, and both can be switched
#[test]
fn each_lens_shows_its_own_lanes_and_both_can_be_switched() {
    let examples = [
        (Lens::Imports, (true, false)),
        (Lens::Calls, (false, true)),
        (Lens::CoChange, (true, false)),
        (Lens::Folders, (false, false)),
    ];
    let mut shop = shop();
    for (lens, (imports, calls)) in examples {
        shop.switch_to(lens);

        let kinds = |shop: &Shop, kind| shop.lanes().iter().any(|lane| lane.kind == kind);
        assert_eq!(
            kinds(&shop, LaneKind::Imports),
            imports,
            "{lens:?} import lanes"
        );
        assert_eq!(kinds(&shop, LaneKind::Calls), calls, "{lens:?} call lanes");

        shop.press(egui::Key::G);
        shop.press(egui::Key::A);
        assert_eq!(
            kinds(&shop, LaneKind::Imports),
            !imports,
            "{lens:?} G switches the import lanes"
        );
        assert_eq!(
            kinds(&shop, LaneKind::Calls),
            !calls,
            "{lens:?} A switches the call lanes"
        );
        shop.press(egui::Key::G);
        shop.press(egui::Key::A);
    }
}
