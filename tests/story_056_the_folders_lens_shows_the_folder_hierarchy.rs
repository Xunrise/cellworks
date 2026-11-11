//! STORY-056, the folders lens shows the folder hierarchy: each folder is
//! outlined inside the outline of the folder holding it, a folder holding
//! only one folder is one outline named after both, an outline is named once
//! it has room for its name and is selected by its name or its open ground,
//! and tests stay in their folder. Since STORY-068 a folder ring is a folder
//! outline, and issue #81 says so.
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
    let mut harness = open(&tree, Some("none"), Recorded::listing(&[]));
    // The Explorer, open beside the map by default (STORY-089), sits over
    // the left part of the map; hidden, a folder outline positioned there
    // is not covered by its panel when this story clicks it.
    harness.key_press_modifiers(Modifiers::COMMAND, egui::Key::B);
    harness.run();
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

    /// Whether the folder at `relative` was drawn by its outline, and
    /// whether its name was written on it.
    fn outline(&self, relative: &str) -> Option<bool> {
        let path = self.path(relative);
        self.harness
            .state()
            .folders()
            .iter()
            .find(|(ring, _)| *ring == path)
            .map(|(_, written)| *written)
    }

    /// A screen point on the open ground of the folder at `relative`:
    /// inside its outline, on no block and in no folder it holds.
    fn open_ground_of(&self, relative: &str) -> Pos2 {
        let map = map(&self.harness);
        let path = self.path(relative);
        let area = map.find(&path).expect("the folder").area;
        let outline = self
            .harness
            .state()
            .outlines()
            .iter()
            .find(|(drawn, _)| *drawn == path)
            .map(|(_, outline)| outline.clone())
            .expect("the folder is outlined");
        let ground = (1..20)
            .flat_map(|i| (1..20).map(move |j| (i as f32 / 20.0, j as f32 / 20.0)))
            .map(|(u, v)| area.min + egui::vec2(u * area.width(), v * area.height()))
            .find(|at| within(&outline, *at) && map.at(*at).last() == Some(&path.as_path()))
            .expect("open ground in the folder");
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

/// Whether `inner` lies inside the ground of the folder at `folder`.
fn inside(map: &ProjectMap, inner: Rect, folder: &Path) -> bool {
    map.find(folder)
        .expect("the folder")
        .area
        .contains_rect(inner)
}

/// Whether `at` lies inside the convex `outline`.
fn within(outline: &[Pos2], at: Pos2) -> bool {
    let n = outline.len();
    let side = |i: usize| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        (b - a).x * (at - a).y - (b - a).y * (at - a).x
    };
    (0..n).all(|i| side(i) >= 0.0) || (0..n).all(|i| side(i) <= 0.0)
}

/// Each folder is an outline inside the outline of the folder holding it
#[test]
fn each_folder_is_an_outline_inside_the_outline_of_the_folder_holding_it() {
    let fleet = fleet();

    assert!(
        fleet.outline("services/billing").is_some(),
        "billing is outlined"
    );
    assert!(
        fleet.outline("services/routing").is_some(),
        "routing is outlined"
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
        assert!(inside(&map, area, &billing), "{block} lies in billing");
    }
}

/// A folder holding only one folder is one outline named after both
#[test]
fn a_folder_holding_only_one_folder_is_one_outline_named_after_both() {
    let fleet = fleet();

    let map = map(&fleet.harness);
    let invoices = map
        .find(&fleet.path("services/billing/src/main/invoices"))
        .expect("one folder for the chain");
    assert_eq!(invoices.name, "src/main/invoices");
    assert_eq!(
        invoices.parent,
        Some(fleet.path("services/billing").as_path())
    );
    assert!(
        fleet
            .outline("services/billing/src/main/invoices")
            .is_some()
    );
    for own in ["services/billing/src", "services/billing/src/main"] {
        assert!(
            map.find(&fleet.path(own)).is_none(),
            "{own} is no folder of its own"
        );
        assert!(fleet.outline(own).is_none(), "{own} has no outline");
    }
}

/// A folder outline is named and can be selected
#[test]
fn a_folder_outline_is_named_and_can_be_selected() {
    let mut fleet = fleet();
    // Given its outline is wide enough on screen: closer until it is.
    for _ in 0..10 {
        if fleet.outline("services/routing") == Some(true) {
            break;
        }
        let routing = map(&fleet.harness)
            .find(&fleet.path("services/routing"))
            .expect("routing")
            .area
            .center();
        let view = fleet.harness.state().view().expect("drawn");
        fleet.harness.hover_at(view.to_screen(routing, 0.0));
        fleet.harness.event(Event::Zoom(1.25));
        fleet.harness.run();
    }
    assert_eq!(
        fleet.outline("services/routing"),
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

/// A folder outline too small on screen carries no name
#[test]
fn a_folder_outline_too_small_on_screen_carries_no_name() {
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

    let folders = fleet.harness.state().folders();
    assert!(!folders.is_empty(), "the outlines are still drawn");
    assert!(folders.iter().all(|(_, written)| !written), "{folders:?}");
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
