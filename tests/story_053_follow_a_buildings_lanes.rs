//! STORY-053, follow a building's lanes: a selected tower shows only its own
//! lanes, a selected district every lane in or out of it; pointing at a lane
//! reads what it carries; a picked lane stays lit; travelling along a lane
//! goes to its far end.
//!
//! One test per scenario of issue #77, named after it. The project "shop" is
//! two crates in a temporary directory: "checkout.rs" in "orders" imports
//! "Price" from "pricing.rs" in "catalog", "refund.rs" imports "discount",
//! "charge" in "orders" calls "discount", and within "catalog" its "lib.rs"
//! imports "total" and "total" calls "discount". No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::LaneKind;
use cellworks_map::ui::DrawnLane;
use egui::{Event, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;

use common::{Recorded, Tree, open};

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
            (
                "catalog/src/lib.rs",
                "pub mod pricing;\nuse pricing::total;\n",
            ),
            (
                "catalog/src/pricing.rs",
                "pub struct Price;\n\npub fn discount() -> u32 {\n    1\n}\n\npub fn total() -> u32 {\n    discount()\n}\n",
            ),
            ("orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
            (
                "orders/src/lib.rs",
                "mod checkout;\nmod refund;\n\npub fn charge() {\n    catalog::pricing::discount();\n}\n",
            ),
            (
                "orders/src/checkout.rs",
                "use catalog::pricing::Price;\n\nfn pay(price: Price) {\n    let paid = price;\n}\n",
            ),
            (
                "orders/src/refund.rs",
                "use catalog::pricing::discount;\n\nfn back() {\n    let given = 0;\n}\n",
            ),
        ],
    );
    let mut harness = open(&tree, None, Recorded::listing(&[]));
    for key in [egui::Key::G, egui::Key::A] {
        harness.key_press(key);
        harness.run_steps(10);
    }
    Shop { tree, harness }
}

impl Shop {
    fn path(&self, path: &str) -> PathBuf {
        self.tree.root.join(path)
    }

    fn settle(&mut self) {
        self.harness.run_steps(30);
    }

    fn lanes(&self) -> Vec<DrawnLane> {
        self.harness.state().lanes().to_vec()
    }

    fn lane(&self, kind: LaneKind, from: &str, to: &str) -> DrawnLane {
        let (from, to) = (self.path(from), self.path(to));
        self.lanes()
            .into_iter()
            .find(|lane| lane.kind == kind && lane.from == from && lane.to == to)
            .unwrap_or_else(|| panic!("no lane from {}", from.display()))
    }

    fn checkout_lane(&self) -> DrawnLane {
        self.lane(
            LaneKind::Imports,
            "orders/src/checkout.rs",
            "catalog/src/pricing.rs",
        )
    }

    fn charge_lane(&self) -> DrawnLane {
        self.lane(
            LaneKind::Calls,
            "orders/src/lib.rs/charge",
            "catalog/src/pricing.rs/discount",
        )
    }

    /// A point on `lane` well away from its ends.
    fn on(lane: &DrawnLane) -> Pos2 {
        let s = &lane.screen;
        match lane.kind {
            LaneKind::Calls => *s
                .iter()
                .min_by(|a, b| a.y.total_cmp(&b.y))
                .expect("a point"),
            LaneKind::Imports => s[2] + (s[3] - s[2]) * 0.5,
        }
    }

    fn point_at(&mut self, at: Pos2) {
        self.harness.event(Event::PointerMoved(at));
        self.settle();
    }

    fn click(&mut self, at: Pos2, modifiers: Modifiers) {
        self.harness.event(Event::PointerMoved(at));
        self.harness.step();
        for pressed in [true, false] {
            self.harness.event(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers,
            });
            self.harness.step();
        }
        self.settle();
    }
}

/// Selecting a tower shows only its own lanes
#[test]
fn selecting_a_tower_shows_only_its_own_lanes() {
    let mut shop = shop();

    let price = shop.path("catalog/src/pricing.rs/Price");
    shop.harness.state_mut().jump(price);
    shop.settle();

    let price = shop.checkout_lane();
    assert!(price.lit && price.shown, "the import of Price stands out");
    for lane in shop.lanes().iter().filter(|lane| lane.id != price.id) {
        assert!(!lane.shown, "{:?} has nothing to do with Price", lane.id);
    }
}

/// Selecting a district shows every lane in or out of it
#[test]
fn selecting_a_district_shows_every_lane_in_or_out_of_it() {
    let mut shop = shop();
    let orders = shop.path("orders");

    shop.harness.state_mut().jump(orders.clone());
    shop.settle();

    for lane in shop.lanes() {
        let touches = lane.from.starts_with(&orders) || lane.to.starts_with(&orders);
        assert_eq!(lane.shown, touches, "{:?}", lane.id);
        assert_eq!(lane.lit, touches, "{:?}", lane.id);
    }
    assert!(
        shop.lanes().iter().any(|lane| !lane.shown),
        "lanes within catalog are not shown"
    );
}

/// Pointing at a lane says what it carries
#[test]
fn pointing_at_a_lane_says_what_it_carries() {
    let mut shop = shop();
    let lane = shop.checkout_lane();

    shop.point_at(Shop::on(&lane));

    assert_eq!(shop.harness.state().hovered_lane(), Some(&lane.id));
    assert_eq!(
        shop.checkout_lane().card,
        ["checkout.rs imports from pricing.rs  1: Price"]
    );
}

/// Pointing at a call lane shows the calling line
#[test]
fn pointing_at_a_call_lane_shows_the_calling_line() {
    let mut shop = shop();
    let lane = shop.charge_lane();

    shop.point_at(Shop::on(&lane));

    assert_eq!(shop.harness.state().hovered_lane(), Some(&lane.id));
    assert_eq!(
        shop.charge_lane().card,
        [
            "lib.rs:5 charge calls discount (pricing.rs)",
            "    catalog::pricing::discount();"
        ]
    );
}

/// A lane picked stays lit until another click
#[test]
fn a_lane_picked_stays_lit_until_another_click() {
    let mut shop = shop();
    let lane = shop.checkout_lane();

    shop.click(Shop::on(&lane), Modifiers::NONE);
    shop.point_at(Pos2::new(5.0, 890.0));

    assert_eq!(
        shop.harness.state().navigation().picked_lane(),
        Some(&lane.id)
    );
    assert!(
        shop.checkout_lane().lit,
        "it stays lit with the pointer away"
    );
    shop.click(Pos2::new(5.0, 890.0), Modifiers::NONE);
    assert_eq!(
        shop.harness.state().navigation().picked_lane(),
        None,
        "empty ground lets it go"
    );
}

/// Travelling along a lane goes to its far end
#[test]
fn travelling_along_a_lane_goes_to_its_far_end() {
    let mut shop = shop();
    let lane = shop.checkout_lane();
    // Near the importing end, so the far end is the imported one.
    let near_checkout = lane.screen[1] + (lane.screen[2] - lane.screen[1]) * 0.1;

    shop.click(near_checkout, Modifiers::COMMAND);

    let pricing = shop.path("catalog/src/pricing.rs");
    let selected = shop
        .harness
        .state()
        .navigation()
        .selection()
        .map(PathBuf::from);
    assert_eq!(selected, Some(pricing.join("Price")));
    let map = common::map(&shop.harness);
    let framed = shop.harness.state().navigation().framing(&map).area;
    assert_eq!(framed, map.find(&pricing).expect("pricing.rs").area);
}
