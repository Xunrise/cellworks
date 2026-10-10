//! STORY-055, each lens explains itself: the lens bar offers every lens the
//! project can have and explains each, the view says what blocks and
//! districts are, a switch is announced, the lanes shown are explained, each
//! lens draws its districts in its own outline, and a lens the project
//! cannot have is not offered.
//!
//! One test per scenario of issue #79, named after it. The project "shop"
//! has the crates "catalog" and "orders", which import and call each other,
//! documents the bounded contexts "Catalog" and "Orders", and git's history
//! changes the two crates together. No test runs git, reads `$HOME` or
//! opens a window.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cellworks::app::CellworksApp;
use cellworks_code_analysis::{GitListing, GitUnavailable};
use cellworks_map::ui::{Outline, caption_text};
use cellworks_map::{DistrictKind, Lens};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use common::{Recorded, Tree};

const FILES: [(&str, &str); 7] = [
    ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
    (
        "catalog/src/lib.rs",
        "use orders::ship;\n\npub fn price() -> u32 {\n    ship();\n    1\n}\n",
    ),
    ("orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
    (
        "orders/src/lib.rs",
        "use catalog::price;\n\npub fn ship() {\n    price();\n}\n",
    ),
    ("docs/domain/contexts/Catalog/Catalog.md", "# Catalog\n"),
    ("docs/domain/contexts/Orders/Orders.md", "# Orders\n"),
    (
        "docs/domain/context-map.md",
        "| 1 | Catalog | Orders | Published Language (PL) | confirmed |\n",
    ),
];

/// git, recorded: it lists the files and changed the two crates together.
struct History;

impl GitListing for History {
    fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        Ok(FILES.iter().map(|(file, _)| PathBuf::from(file)).collect())
    }

    fn history(&self, _: &Path) -> Result<Vec<Vec<PathBuf>>, GitUnavailable> {
        let together = vec![
            PathBuf::from("catalog/src/lib.rs"),
            PathBuf::from("orders/src/lib.rs"),
        ];
        Ok(vec![together.clone(), together])
    }
}

fn open(tree: &Tree, git: Arc<dyn GitListing + Send + Sync>) -> Harness<'static, CellworksApp> {
    let project = tree.as_project(None);
    let name = project.name.clone();
    let mut harness = common::harness(common::app(vec![project], git));
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    // The domain knowledge is read beside the code.
    for _ in 0..100 {
        harness.state_mut().apply_pending();
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    harness
}

/// The project, whose folder lasts as long as the tree, and its harness.
fn shop() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::sources("shop", &FILES);
    fs::create_dir(tree.root.join(".git")).expect("a repository");
    let harness = open(&tree, Arc::new(History));
    (tree, harness)
}

/// Every text the frame shows the accessibility tree: labels and buttons.
fn texts(harness: &Harness<'_, CellworksApp>) -> Vec<(Role, String)> {
    let root = harness.root();
    std::iter::once(root)
        .chain(root.children_recursive())
        .filter_map(|node| {
            let node = node.accesskit_node();
            let text = node
                .value()
                .or_else(|| node.label())
                .map(|t| t.to_string())?;
            Some((node.role(), text))
        })
        .collect()
}

fn buttons(harness: &Harness<'_, CellworksApp>) -> Vec<String> {
    texts(harness)
        .into_iter()
        .filter(|(role, _)| *role == Role::Button)
        .map(|(_, text)| text)
        .collect()
}

fn shows(harness: &Harness<'_, CellworksApp>, wanted: impl Fn(&str) -> bool) -> bool {
    texts(harness).iter().any(|(_, text)| wanted(text))
}

fn switch_to(harness: &mut Harness<'static, CellworksApp>, lens: Lens) {
    for _ in 0..Lens::ALL.len() {
        if harness.state().opened().map(|o| o.lens()) == Some(lens) {
            return;
        }
        harness.key_press(egui::Key::L);
        harness.run_steps(150);
    }
    assert_eq!(harness.state().opened().map(|o| o.lens()), Some(lens));
}

/// The lens bar offers every lens and explains each
#[test]
fn the_lens_bar_offers_every_lens_and_explains_each() {
    let (_tree, mut harness) = shop();

    let offered = buttons(&harness);
    for lens in Lens::ALL {
        assert!(
            offered.contains(&lens.name().to_owned()),
            "{} is offered: {offered:?}",
            lens.name()
        );
    }

    harness.get_by_label("calls").hover();
    harness.run_steps(60);
    assert!(
        shows(&harness, |text| text.contains(Lens::Calls.blurb())),
        "pointing at it explains it"
    );

    harness.get_by_label("calls").click();
    harness.run_steps(150);
    assert_eq!(
        harness.state().opened().map(|o| o.lens()),
        Some(Lens::Calls),
        "picking it switches to it"
    );
}

/// The current lens says what its blocks and districts are
#[test]
fn the_current_lens_says_what_its_blocks_and_districts_are() {
    let (_tree, mut harness) = shop();

    switch_to(&mut harness, Lens::Calls);

    let caption = caption_text(Lens::Calls);
    assert!(caption.contains("Block: functions that call each other."));
    assert!(caption.contains("District: blocks that call each other."));
    assert!(shows(&harness, |text| text == caption));
}

/// A switch of lens is announced
#[test]
fn a_switch_of_lens_is_announced() {
    let (_tree, mut harness) = shop();
    let banner = |harness: &Harness<'_, CellworksApp>| {
        shows(harness, |text| {
            text.starts_with("IMPORTS LENS\n") && text.contains(Lens::Imports.blurb())
        })
    };

    harness.key_press(egui::Key::L);
    harness.run_steps(10);
    assert!(banner(&harness), "the name and what it arranges by, large");

    harness.run_steps(150);
    assert!(!banner(&harness), "for a moment only");
}

/// Shown lanes are explained
#[test]
fn shown_lanes_are_explained() {
    let (_tree, mut harness) = shop();
    assert!(
        !shows(&harness, |text| text.starts_with("Lanes: ")),
        "no lanes, no legend"
    );

    harness.key_press(egui::Key::G);
    harness.run_steps(10);

    let legend = texts(&harness)
        .into_iter()
        .map(|(_, t)| t)
        .find(|text| text.starts_with("Lanes: "))
        .expect("a legend");
    for word in ["Import lanes", "highway", "Colour", "Traffic"] {
        assert!(legend.contains(word), "{word}: {legend}");
    }
}

/// Each lens draws its districts in its own outline
#[test]
fn each_lens_draws_its_districts_in_its_own_outline() {
    let (_tree, mut harness) = shop();
    let own = [
        (Lens::Folders, DistrictKind::Folder, Outline::Plaza),
        (Lens::Imports, DistrictKind::Cluster, Outline::Step),
        (Lens::Calls, DistrictKind::Cluster, Outline::Hexagon),
        (Lens::CoChange, DistrictKind::Cluster, Outline::Orbit),
        (Lens::Cells, DistrictKind::Context, Outline::Membrane),
        (Lens::Custom, DistrictKind::Folder, Outline::Own),
    ];
    for (lens, kind, outline) in own {
        switch_to(&mut harness, lens);

        let map = common::map(&harness);
        let theirs: Vec<&PathBuf> = map
            .districts
            .iter()
            .filter(|d| d.kind == kind)
            .map(|d| &d.path)
            .collect();
        assert!(!theirs.is_empty(), "{lens:?} has {kind:?} districts");
        for (path, drawn) in harness.state().districts() {
            if theirs.contains(&path) {
                assert_eq!(*drawn, outline, "{lens:?}");
            }
        }
    }
}

/// A lens the project cannot have is not offered
#[test]
fn a_lens_the_project_cannot_have_is_not_offered() {
    let tree = Tree::sources("notes", &[("src/lib.rs", "pub fn note() {}\n")]);
    let harness = open(&tree, Recorded::listing(&[]));

    let offered = buttons(&harness);

    assert!(offered.contains(&"folders".to_owned()), "{offered:?}");
    assert!(!offered.contains(&"cells".to_owned()), "{offered:?}");
}
