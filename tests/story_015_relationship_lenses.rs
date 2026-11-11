//! STORY-015, relationship lenses rebuild the blocks and districts: the
//! imports, calls and co-change lenses group the code by its connections,
//! towers keep their floors and their crate's colour, districts are named
//! after what holds them, tests hang under the code they test, unconnected
//! code stands in the outskirts, and a switch of lens flies the towers.
//!
//! One test per scenario of issue #37, named after it. The project "shop" is
//! two crates in a temporary directory: in "catalog", "discount" and
//! "charge" of "orders" call each other often; "tests/pricing.rs" tests
//! "discount"; "tests/smoke.rs" tests nothing of "shop"; "unused_helper" is
//! called by nothing. git is recorded: its history changes the two crates'
//! files together. No test runs git, reads `$HOME` or opens a window.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cellworks::app::CellworksApp;
use cellworks::opened::ProjectState;
use cellworks_code_analysis::{GitListing, GitUnavailable};
use cellworks_map::ui::Outline;
use cellworks_map::{DistrictKind, Lens, ProjectMap};
use egui_kittest::Harness;

use common::{Tree, map, open, tower};

const CATALOG: &str = "\
use orders::charge;

pub fn discount() -> u32 {
    charge();
    1
}

pub fn price() -> u32 {
    discount()
}

pub fn total() -> u32 {
    discount() + price()
}

pub fn unused_helper() {
    let nothing = 0;
}
";

const ORDERS: &str = "\
use catalog::discount;

pub fn charge() {
    discount();
}

pub fn ship() {
    charge();
}
";

const FILES: [&str; 8] = [
    "catalog/Cargo.toml",
    "catalog/src/lib.rs",
    "catalog/tests/pricing.rs",
    "catalog/tests/smoke.rs",
    "orders/Cargo.toml",
    "orders/src/lib.rs",
    "orders/src/notes.md",
    "README.md",
];

/// git, recorded: it lists the files, and its history changed the two
/// crates' code together three times.
struct History;

impl GitListing for History {
    fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        Ok(FILES.iter().map(PathBuf::from).collect())
    }

    fn history(&self, _: &Path) -> Result<Vec<Vec<PathBuf>>, GitUnavailable> {
        let together = vec![
            PathBuf::from("catalog/src/lib.rs"),
            PathBuf::from("orders/src/lib.rs"),
        ];
        Ok(vec![
            together.clone(),
            together.clone(),
            together,
            vec![PathBuf::from("README.md")],
        ])
    }
}

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

fn shop() -> Shop {
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
            ("catalog/src/lib.rs", CATALOG),
            (
                "catalog/tests/pricing.rs",
                "#[test]\nfn prices() {\n    catalog::discount();\n}\n",
            ),
            (
                "catalog/tests/smoke.rs",
                "#[test]\nfn boots() {\n    let started = true;\n}\n",
            ),
            ("orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
            ("orders/src/lib.rs", ORDERS),
            ("orders/src/notes.md", "# orders\n"),
            ("README.md", "# shop\n"),
        ],
    );
    fs::create_dir(tree.root.join(".git")).expect("a repository");
    let harness = open(&tree, None, Arc::new(History));
    Shop { tree, harness }
}

impl Shop {
    fn item(&self, file: &str, name: &str) -> PathBuf {
        self.tree.root.join(file).join(name)
    }

    fn discount(&self) -> PathBuf {
        self.item("catalog/src/lib.rs", "discount")
    }

    fn charge(&self) -> PathBuf {
        self.item("orders/src/lib.rs", "charge")
    }

    /// The code architect switches to `lens`, and the towers land.
    fn switch_to(&mut self, lens: Lens) {
        for _ in 0..Lens::ALL.len() {
            if self.harness.state().opened().map(|o| o.lens()) == Some(lens) {
                break;
            }
            self.harness
                .key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::L);
            common::await_lens(&mut self.harness);
            // Traffic on the lanes keeps asking for frames: two seconds of
            // them, enough for every tower to land.
            self.harness.run_steps(120);
        }
        assert_eq!(self.harness.state().opened().map(|o| o.lens()), Some(lens));
    }

    fn map(&self) -> ProjectMap {
        map(&self.harness)
    }
}

/// The block an item stands on, and the district holding that block.
fn standing(map: &ProjectMap, item: &Path) -> (PathBuf, PathBuf) {
    let block = map
        .find(item)
        .and_then(|found| found.parent)
        .expect("on a block")
        .to_path_buf();
    let district = map
        .districts
        .iter()
        .find(|district| district.nodes.iter().any(|node| node.path == block))
        .expect("in a district");
    (block, district.path.clone())
}

fn district_named(map: &ProjectMap, kind: DistrictKind) -> &cellworks_map::District {
    map.districts
        .iter()
        .find(|d| d.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} district"))
}

/// The calls lens puts towers that call each other on one block
#[test]
fn the_calls_lens_puts_towers_that_call_each_other_on_one_block() {
    let mut shop = shop();
    let crates = [
        shop.tree.root.join("catalog"),
        shop.tree.root.join("orders"),
    ];

    shop.switch_to(Lens::Calls);

    let map = shop.map();
    assert_eq!(
        standing(&map, &shop.discount()).0,
        standing(&map, &shop.charge()).0
    );
    assert!(
        map.districts.iter().all(|d| !crates.contains(&d.path)),
        "the districts are not the crates"
    );
}

/// Towers keep their shape and their crate colour in every lens
#[test]
fn towers_keep_their_shape_and_their_crate_colour_in_every_lens() {
    let mut shop = shop();
    let discount = shop.discount();
    let floors = shop.map().item(&discount).expect("discount").floors.clone();
    let colour = tower(&shop.harness, &discount).colour;

    shop.switch_to(Lens::Calls);

    assert_eq!(shop.map().item(&discount).expect("discount").floors, floors);
    assert_eq!(tower(&shop.harness, &discount).colour, colour);
}

/// A district is named after what holds it together
#[test]
fn a_district_is_named_after_what_holds_it_together() {
    let mut shop = shop();

    shop.switch_to(Lens::Calls);

    let map = shop.map();
    let (_, district) = standing(&map, &shop.discount());
    let name = &map
        .districts
        .iter()
        .find(|d| d.path == district)
        .expect("the district")
        .name;
    assert_eq!(name, "around discount()");
}

/// Tests hang under the district whose code they test
#[test]
fn tests_hang_under_the_district_whose_code_they_test() {
    let mut shop = shop();
    let prices = shop.item("catalog/tests/pricing.rs", "prices");

    shop.switch_to(Lens::Imports);

    let map = shop.map();
    assert_eq!(
        standing(&map, &prices).1,
        standing(&map, &shop.discount()).1
    );
    assert!(
        map.item(&prices).expect("prices").tests,
        "it hangs in the cellar"
    );
}

/// Tests that test no placed code form the tests district
#[test]
fn tests_that_test_no_placed_code_form_the_tests_district() {
    let mut shop = shop();
    let boots = shop.item("catalog/tests/smoke.rs", "boots");

    shop.switch_to(Lens::Calls);

    let map = shop.map();
    assert_eq!(
        standing(&map, &boots).1,
        district_named(&map, DistrictKind::Tests).path
    );
}

/// Code the lens does not connect stands at the outskirts
#[test]
fn code_the_lens_does_not_connect_stands_at_the_outskirts() {
    let mut shop = shop();
    let helper = shop.item("catalog/src/lib.rs", "unused_helper");

    shop.switch_to(Lens::Calls);

    let map = shop.map();
    assert_eq!(
        standing(&map, &helper).1,
        district_named(&map, DistrictKind::Outskirts).path
    );
}

/// Switching lens moves every tower to its new place
#[test]
fn switching_lens_moves_every_tower_to_its_new_place() {
    let mut shop = shop();
    let discount = shop.discount();
    let before = shop.map().item(&discount).expect("discount").area;

    shop.harness
        .key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::L);
    shop.harness.step();
    common::await_lens(&mut shop.harness);
    // From the moment of the switch: frames pass while the new map is
    // worked out, and a tower may already be in the air.
    let started = shop
        .harness
        .state()
        .opened()
        .and_then(|o| o.switched_at())
        .expect("the lens switched");
    let at = |shop: &Shop, time: f64| {
        let shown = shop
            .harness
            .state()
            .opened()
            .and_then(|o| o.shown(time))
            .expect("a map");
        shown.item(&discount).expect("discount").area
    };
    let first = at(&shop, started);
    let middle = at(&shop, started + 0.75);
    let after = shop.map().item(&discount).expect("discount").area;

    assert!(
        (first.center() - before.center()).length() < 1.0,
        "it takes off where it stood"
    );
    assert!(
        (middle.center() - before.center()).length() > 1.0,
        "it is on its way"
    );
    assert!(
        (middle.center() - after.center()).length() > 1.0,
        "it has not landed yet"
    );
    assert!(
        (at(&shop, started + 5.0).center() - after.center()).length() < 1e-2,
        "it lands"
    );
}

/// Switching back to folders restores the folder city
#[test]
fn switching_back_to_folders_restores_the_folder_city() {
    let mut shop = shop();
    shop.switch_to(Lens::Calls);

    shop.switch_to(Lens::Folders);

    let names: Vec<String> = shop
        .map()
        .districts
        .iter()
        .map(|d| d.name.clone())
        .collect();
    assert!(
        names.contains(&"catalog".to_owned()) && names.contains(&"orders".to_owned()),
        "{names:?}"
    );
    assert!(
        shop.map()
            .districts
            .iter()
            .all(|d| d.kind == DistrictKind::Folder)
    );
}

/// Each relationship lens groups by its own kind of connection and has its own shape
#[test]
fn each_relationship_lens_groups_by_its_own_kind_of_connection_and_has_its_own_shape() {
    let examples = [
        (Lens::Imports, Outline::Step),
        (Lens::Calls, Outline::Hexagon),
        (Lens::CoChange, Outline::Orbit),
    ];
    for (lens, outline) in examples {
        let mut shop = shop();
        let (catalog, orders) = (
            shop.item("catalog/src/lib.rs", "price"),
            shop.item("orders/src/lib.rs", "ship"),
        );

        shop.switch_to(lens);

        let map = shop.map();
        let together = match lens {
            // Calls: the towers that call each other share a block.
            Lens::Calls => standing(&map, &shop.discount()).0 == standing(&map, &shop.charge()).0,
            // Imports and co-change: the files share a district.
            _ => standing(&map, &catalog).1 == standing(&map, &orders).1,
        };
        assert!(together, "{lens:?} groups by its connection");
        let (_, district) = standing(&map, &shop.discount());
        let drawn = shop
            .harness
            .state()
            .districts()
            .iter()
            .find(|(path, _)| *path == district)
            .map(|(_, o)| *o);
        assert_eq!(drawn, Some(outline), "{lens:?}");
        assert!(matches!(
            shop.harness.state().opened().map(|o| o.state()),
            Some(ProjectState::Mapped(_))
        ));
    }
}
