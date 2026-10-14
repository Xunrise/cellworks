//! STORY-056, the folders lens shows the folder hierarchy: each folder is a
//! ring inside the ring of the folder holding it, a folder holding only one
//! folder is one ring named after both, a ring is named once it has room for
//! its name and is selected by its name or its open ground, and tests stay in
//! their folder.
//!
//! One test per scenario of issue #81, named after it. The project "fleet"
//! has the district "services" with the folders "billing" and "routing";
//! "billing" holds a file of its own and the chain "src/main/invoices". It
//! is a real folder tree in a temporary directory, cut by the district rule
//! "none" so "services" is the district. No test runs git, reads `$HOME` or
//! opens a window.

mod common;

use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::{Lens, ProjectMap};
use egui::accesskit::Role;
use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Recorded, Tree, map, open};

const CODE: &str = "pub fn work() {\n    let done = true;\n}\n";

const TESTS: &str = "\
#[test]
fn a_route_is_found() {
    let found = true;
    assert!(found);
}
";

struct Fleet {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "fleet" is open as a city in the folders lens.
fn fleet() -> Fleet {
    let tree = Tree::sources(
        "fleet",
        &[
            ("services/billing/lib.rs", CODE),
            ("services/billing/src/main/invoices/invoice.rs", CODE),
            ("services/routing/route.rs", CODE),
            ("services/routing/tests/routes.rs", TESTS),
        ],
    );
    let harness = open(&tree, Some("none"), Recorded::listing(&[]));
    assert_eq!(
        harness.state().opened().map(|o| o.lens()),
        Some(Lens::Folders)
    );
    Fleet { tree, harness }
}

impl Fleet {
    fn path(&self, relative: &str) -> PathBuf {
        self.tree.root.join(relative)
    }

    /// Whether the folder at `relative` was drawn as a ring, and whether its
    /// name was written on it.
    fn ring(&self, relative: &str) -> Option<bool> {
        let path = self.path(relative);
        self.harness
            .state()
            .rings()
            .iter()
            .find(|(ring, _)| *ring == path)
            .map(|(_, written)| *written)
    }

    /// A screen point on the open ground of the ring of the folder at
    /// `relative`: inside it, on no block and in no ring it holds.
    fn open_ground_of(&self, relative: &str) -> Pos2 {
        let map = map(&self.harness);
        let path = self.path(relative);
        let (centre, radius) = ring_of(map.find(&path).expect("the folder").area);
        let ground = (0..36)
            .map(|step| {
                let angle = step as f32 * std::f32::consts::TAU / 36.0;
                centre + radius * 0.85 * egui::vec2(angle.cos(), angle.sin())
            })
            .find(|at| map.at(*at).last() == Some(&path.as_path()))
            .expect("open ground in the ring");
        self.harness
            .state()
            .view()
            .expect("drawn")
            .to_screen(ground, 0.0)
    }

    /// A click of the primary button at the screen point `at`.
    fn click_at(&mut self, at: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.harness.event(Event::PointerMoved(at));
        self.harness.step();
        self.harness.event(button(true));
        self.harness.step();
        self.harness.event(button(false));
        self.harness.run();
    }

    /// The labels marked selected in the accessibility tree.
    fn selected(&self) -> Vec<String> {
        self.harness
            .root()
            .children_recursive()
            .map(|node| node.accesskit_node())
            .filter(|node| node.role() == Role::Label && node.is_selected() == Some(true))
            .filter_map(|node| node.value().map(|text| text.to_string()))
            .collect()
    }
}

/// The circle of the ring `area` is the square around: centre and radius.
fn ring_of(area: Rect) -> (Pos2, f32) {
    (area.center(), area.width() / 2.0)
}

/// Whether `inner` lies inside the ring of the folder at `folder`.
fn inside(map: &ProjectMap, inner: Rect, folder: &Path) -> bool {
    let (centre, radius) = ring_of(map.find(folder).expect("the folder").area);
    let corners = [
        inner.left_top(),
        inner.right_top(),
        inner.left_bottom(),
        inner.right_bottom(),
    ];
    corners
        .iter()
        .all(|corner| (*corner - centre).length() <= radius + 1e-3)
}

/// Each folder is a ring inside the ring of the folder holding it
#[test]
fn each_folder_is_a_ring_inside_the_ring_of_the_folder_holding_it() {
    let fleet = fleet();

    assert!(
        fleet.ring("services/billing").is_some(),
        "billing is a ring"
    );
    assert!(
        fleet.ring("services/routing").is_some(),
        "routing is a ring"
    );
    let map = map(&fleet.harness);
    let billing = fleet.path("services/billing");
    let services = map.find(&fleet.path("services")).expect("services").area;
    for folder in ["services/billing", "services/routing"] {
        let area = map.find(&fleet.path(folder)).expect("the folder").area;
        assert!(services.contains_rect(area), "{folder} lies in services");
    }
    for block in [
        "services/billing/lib.rs",
        "services/billing/src/main/invoices/invoice.rs",
    ] {
        let area = map.find(&fleet.path(block)).expect("the block").area;
        assert!(
            inside(&map, area, &billing),
            "{block} lies in the ring of billing"
        );
    }
}

/// A folder holding only one folder is one ring named after both
#[test]
fn a_folder_holding_only_one_folder_is_one_ring_named_after_both() {
    let fleet = fleet();

    let map = map(&fleet.harness);
    let invoices = map
        .find(&fleet.path("services/billing/src/main/invoices"))
        .expect("one ring for the chain");
    assert_eq!(invoices.name, "src/main/invoices");
    assert_eq!(
        invoices.parent,
        Some(fleet.path("services/billing").as_path())
    );
    assert!(fleet.ring("services/billing/src/main/invoices").is_some());
    for own in ["services/billing/src", "services/billing/src/main"] {
        assert!(
            map.find(&fleet.path(own)).is_none(),
            "{own} is no folder of its own"
        );
        assert!(fleet.ring(own).is_none(), "{own} has no ring");
    }
}

/// A folder ring is named and can be selected
#[test]
fn a_folder_ring_is_named_and_can_be_selected() {
    let mut fleet = fleet();
    assert_eq!(
        fleet.ring("services/routing"),
        Some(true),
        "routing has room for its name"
    );

    fleet.harness.get_by_label("routing").click();
    fleet.harness.run();
    assert_eq!(fleet.selected(), ["routing"], "by its name");

    fleet.harness.get_by_label("billing").click();
    fleet.harness.run();
    let ground = fleet.open_ground_of("services/routing");
    fleet.click_at(ground);
    assert_eq!(fleet.selected(), ["routing"], "by its open ground");
}

/// A folder ring too small on screen carries no name
#[test]
fn a_folder_ring_too_small_on_screen_carries_no_name() {
    let mut fleet = fleet();
    let centre = fleet
        .harness
        .state()
        .view()
        .expect("drawn")
        .to_screen(map(&fleet.harness).extent.center(), 0.0);

    fleet.harness.hover_at(centre);
    for _ in 0..12 {
        fleet.harness.event(Event::Zoom(0.6));
        fleet.harness.run();
    }

    let rings = fleet.harness.state().rings();
    assert!(!rings.is_empty(), "the rings are still drawn");
    assert!(rings.iter().all(|(_, written)| !written), "{rings:?}");
}

/// Tests stay in the folder that holds them
#[test]
fn tests_stay_in_the_folder_that_holds_them() {
    let fleet = fleet();

    let map = map(&fleet.harness);
    let routes = map
        .find(&fleet.path("services/routing/tests/routes.rs"))
        .expect("the block of only tests");
    assert!(inside(
        &map,
        routes.area,
        &fleet.path("services/routing/tests")
    ));
}
