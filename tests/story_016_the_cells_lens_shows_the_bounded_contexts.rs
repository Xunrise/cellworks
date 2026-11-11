//! STORY-016, the cells lens shows the bounded contexts: each bounded
//! context is a district holding the code that implements it, the context
//! map's relationships are roads named by their type, related contexts
//! stand near each other, code of no context has its own district, and a
//! project without a domain model is not offered the lens.
//!
//! One test per scenario of issue #38, named after it. The project "shop"
//! documents the contexts "Pricing", "Ordering" and "Billing"; its context
//! map says "Pricing" is upstream of "Ordering" as a published language;
//! its settings say "catalog" implements "Pricing", "orders" "Ordering" and
//! "billing" "Billing"; "src" implements none. No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use cellworks::app::CellworksApp;
use cellworks_map::{DistrictKind, Lens, ProjectMap};
use egui_kittest::Harness;

use common::{Recorded, Tree};

const CONTEXT_MAP: &str = "\
| # | Upstream | Downstream | Type | Clarity |
|---|---|---|---|---|
| 1 | Pricing | Ordering | Published Language (PL) | confirmed |
";

/// Opens `tree` with `contexts` in its settings, and lets both reads end.
fn open(tree: &Tree, contexts: BTreeMap<String, Vec<PathBuf>>) -> Harness<'static, CellworksApp> {
    let mut project = tree.as_project(None);
    project.contexts = contexts;
    let name = project.name.clone();
    let mut harness = common::harness(common::app(vec![project], Recorded::listing(&[])));
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    // The domain knowledge is read beside the code.
    for _ in 0..200 {
        harness.state_mut().apply_pending();
        harness.step();
        if harness
            .state()
            .opened()
            .is_some_and(|o| o.offers(Lens::Cells))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    harness
}

fn shop() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::sources(
        "shop",
        &[
            ("docs/domain/contexts/Pricing/Pricing.md", "# Pricing\n"),
            ("docs/domain/contexts/Ordering/Ordering.md", "# Ordering\n"),
            ("docs/domain/contexts/Billing/Billing.md", "# Billing\n"),
            ("docs/domain/context-map.md", CONTEXT_MAP),
            ("catalog/pricing.rs", "pub fn price() {}\n"),
            ("catalog/discount.rs", "pub fn discount() {}\n"),
            ("orders/order.rs", "pub fn place() {}\n"),
            ("billing/invoice.rs", "pub fn bill() {}\n"),
            ("src/main.rs", "fn main() {}\n"),
        ],
    );
    let contexts = BTreeMap::from([
        ("Pricing".to_owned(), vec![PathBuf::from("catalog")]),
        ("Ordering".to_owned(), vec![PathBuf::from("orders")]),
        ("Billing".to_owned(), vec![PathBuf::from("billing")]),
    ]);
    let harness = open(&tree, contexts);
    (tree, harness)
}

/// The code architect switches to the cells lens.
fn to_cells(harness: &mut Harness<'static, CellworksApp>) -> ProjectMap {
    for _ in 0..Lens::ALL.len() {
        if harness.state().opened().map(|o| o.lens()) == Some(Lens::Cells) {
            break;
        }
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::L);
        common::await_lens(harness);
        harness.run_steps(120);
    }
    assert_eq!(
        harness.state().opened().map(|o| o.lens()),
        Some(Lens::Cells)
    );
    common::map(harness)
}

fn district<'m>(map: &'m ProjectMap, name: &str) -> (usize, &'m cellworks_map::District) {
    map.districts
        .iter()
        .enumerate()
        .find(|(_, d)| d.name == name)
        .unwrap_or_else(|| panic!("no district {name}"))
}

/// Each bounded context is a district
#[test]
fn each_bounded_context_is_a_district() {
    let (tree, mut harness) = shop();

    let map = to_cells(&mut harness);

    let blocks = |name: &str| -> Vec<PathBuf> {
        district(&map, name)
            .1
            .nodes
            .iter()
            .map(|n| n.path.clone())
            .collect()
    };
    let catalog = tree.root.join("catalog");
    assert_eq!(
        blocks("Pricing"),
        [catalog.join("discount.rs"), catalog.join("pricing.rs")]
    );
    assert_eq!(blocks("Ordering"), [tree.root.join("orders/order.rs")]);
}

/// Relationships from the context map are roads named by their type
#[test]
fn relationships_from_the_context_map_are_roads_named_by_their_type() {
    let (_tree, mut harness) = shop();

    let map = to_cells(&mut harness);

    let (pricing, ordering) = (district(&map, "Pricing").0, district(&map, "Ordering").0);
    assert_eq!(
        map.roads,
        [(pricing, ordering, "published language".to_owned())],
        "upstream first, pointing downstream"
    );
}

/// Related contexts stand near each other
#[test]
fn related_contexts_stand_near_each_other() {
    let (_tree, mut harness) = shop();

    let map = to_cells(&mut harness);

    let centre = |name: &str| district(&map, name).1.area.center();
    let apart = |a: &str, b: &str| (centre(a) - centre(b)).length();
    assert!(apart("Pricing", "Ordering") < apart("Pricing", "Billing"));
}

/// Code outside any bounded context has its own district
#[test]
fn code_outside_any_bounded_context_has_its_own_district() {
    let (tree, mut harness) = shop();

    let map = to_cells(&mut harness);

    let outside = map
        .districts
        .iter()
        .find(|d| d.kind == DistrictKind::NoContext)
        .expect("a district of no context");
    assert!(
        outside
            .nodes
            .iter()
            .any(|n| n.path == tree.root.join("src/main.rs"))
    );
    let contexts = ["Pricing", "Ordering", "Billing"].map(|name| district(&map, name).1);
    assert!(contexts.iter().all(|d| {
        d.nodes
            .iter()
            .all(|n| !n.path.starts_with(tree.root.join("src")))
    }));
}

/// A project without a domain model offers no cells lens
#[test]
fn a_project_without_a_domain_model_offers_no_cells_lens() {
    let tree = Tree::sources(
        "notes",
        &[
            ("notes.md", "# notes\n"),
            ("src/lib.rs", "pub fn note() {}\n"),
        ],
    );
    let mut harness = open(&tree, BTreeMap::new());

    let mut seen = Vec::new();
    for _ in 0..2 * Lens::ALL.len() {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::L);
        common::await_lens(&mut harness);
        harness.run_steps(120);
        seen.extend(harness.state().opened().map(|o| o.lens()));
    }

    assert!(!seen.contains(&Lens::Cells), "{seen:?}");
    assert!(!harness.state().opened().expect("open").offers(Lens::Cells));
}
