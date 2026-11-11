//! STORY-003, see a code project as districts, sub-districts, groups and
//! nodes: what the engineer sees once they open a project.
//!
//! The ten scenarios of issue #5 come first, one test each, named after the
//! scenario. Each raises a real folder tree in a temporary directory, shows it
//! in the universe, double-clicks it, and reads the frame. git is a recorded
//! listing, so no test runs git, reads `$HOME` or opens a window. The tests
//! after the scenarios pin the frame's requirements in `spec.md`, each naming
//! its requirement in its doc comment.

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use cellworks::app::CellworksApp;
use cellworks_code_analysis::{GitListing, GitUnavailable};
use egui::accesskit::Role;
use egui::{Modifiers, Rect};
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

use common::{Recorded, Tree, app, await_read, double_click, harness, labels, map, open};

const READING: &str = "Reading the project…";
const EMPTY: &str = "This project has no files to show.";
const FAILED: &str = "Could not read this project.";

/// A git that does not answer until the test lets go of it.
struct Blocking(Mutex<mpsc::Receiver<()>>);

impl GitListing for Blocking {
    fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        let _ = self.0.lock().map(|waiting| waiting.recv());
        Ok(Vec::new())
    }
}

/// A git whose read thread dies.
struct Panicking;

impl GitListing for Panicking {
    fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        panic!("the read thread goes away");
    }
}

fn district_names(harness: &Harness<'_, CellworksApp>) -> Vec<String> {
    map(harness)
        .districts
        .iter()
        .map(|district| district.name.clone())
        .collect()
}

fn strictly_inside(inner: Rect, outer: Rect) -> bool {
    outer.contains_rect(inner) && inner != outer
}

fn shows(harness: &Harness<'_, CellworksApp>, label: &str) -> bool {
    labels(harness).iter().any(|shown| shown == label)
}

/// By default, top folders below the common prefix become districts
#[test]
fn by_default_top_folders_below_the_common_prefix_become_districts() {
    let tree = Tree::project("shop", &["src/catalog/item.rs", "src/ordering/order.rs"]);

    let harness = open(&tree, None, Recorded::listing(&[]));

    assert_eq!(district_names(&harness), ["catalog", "ordering"]);
    assert!(shows(&harness, "catalog") && shows(&harness, "ordering"));
}

/// The next folder level becomes sub-districts, deeper folders become groups
#[test]
fn the_next_folder_level_becomes_sub_districts_deeper_folders_become_groups() {
    let tree = Tree::project(
        "shop",
        &[
            "src/catalog/pricing/rules/discount.rs",
            "src/catalog/pricing/price.rs",
        ],
    );

    let harness = open(&tree, None, Recorded::listing(&[]));

    let map = map(&harness);
    let catalog = &map.districts[0];
    assert_eq!(catalog.name, "catalog");
    let pricing = &catalog.sub_districts[0];
    assert_eq!(pricing.name, "pricing");
    assert!(strictly_inside(pricing.area, catalog.area));
    let rules = &pricing.groups[0];
    assert_eq!(rules.name, "rules");
    assert!(strictly_inside(rules.area, pricing.area));
    assert_eq!(rules.nodes[0].name, "discount.rs");
    assert!(strictly_inside(rules.nodes[0].area, rules.area));
    assert_eq!(pricing.nodes[0].name, "price.rs");
    assert!(strictly_inside(pricing.nodes[0].area, pricing.area));
    for label in ["catalog", "pricing", "rules", "discount.rs", "price.rs"] {
        assert!(shows(&harness, label), "{label} is drawn");
    }
}

/// Files directly in a district are its nodes
#[test]
fn files_directly_in_a_district_are_its_nodes() {
    let tree = Tree::project("shop", &["src/catalog/item.rs", "src/ordering/order.rs"]);

    let harness = open(&tree, None, Recorded::listing(&[]));

    let map = map(&harness);
    let catalog = &map.districts[0];
    assert_eq!(catalog.name, "catalog");
    assert_eq!(catalog.nodes[0].name, "item.rs");
    assert!(strictly_inside(catalog.nodes[0].area, catalog.area));
    assert!(shows(&harness, "item.rs"));
}

/// Files at the top of the project form a district named after the project
#[test]
fn files_at_the_top_of_the_project_form_a_district_named_after_the_project() {
    let tree = Tree::project("shop", &["README.md", "src/catalog/item.rs"]);

    let harness = open(&tree, None, Recorded::listing(&[]));

    let map = map(&harness);
    assert_eq!(district_names(&harness), ["shop", "src"]);
    assert_eq!(map.districts[0].nodes[0].name, "README.md");
    assert!(shows(&harness, "shop") && shows(&harness, "README.md"));
}

/// Hidden folders, and folders git ignores, are left out
#[test]
fn hidden_folders_and_folders_git_ignores_are_left_out() {
    let tree = Tree::repository(
        "shop",
        &[".cache/stale.bin", "build/output.o", "src/catalog/item.rs"],
    );

    let harness = open(
        &tree,
        None,
        Recorded::listing(&[".cache/stale.bin", "src/catalog/item.rs"]),
    );

    let shown = labels(&harness);
    for left_out in [".cache", "stale.bin", "build", "output.o"] {
        assert!(
            !shown.iter().any(|l| l == left_out),
            "{left_out}: {shown:?}"
        );
    }
    assert!(shows(&harness, "item.rs"));
}

/// A folder that is not a git repository leaves out only hidden folders
#[test]
fn a_folder_that_is_not_a_git_repository_leaves_out_only_hidden_folders() {
    let tree = Tree::project("shop", &[".cache/stale.bin", "build/output.o"]);

    let harness = open(&tree, None, Recorded::listing(&[]));

    assert!(!shows(&harness, ".cache"));
    assert_eq!(district_names(&harness), ["build"]);
    assert!(shows(&harness, "build"));
}

/// A project without files still opens
#[test]
fn a_project_without_files_still_opens() {
    let tree = Tree::project("shop", &[]);

    let harness = open(&tree, None, Recorded::listing(&[]));

    assert_eq!(labels(&harness), [EMPTY]);
}

/// A project set to the longest shared prefix cuts districts below every
/// folder its files share
#[test]
fn a_project_set_to_the_longest_shared_prefix_cuts_districts_below_every_folder_its_files_share() {
    let tree = Tree::project(
        "shop",
        &[
            "src/main/java/acme/billing/Invoice.java",
            "src/main/java/acme/orders/Order.java",
        ],
    );

    let harness = open(&tree, Some("longest-shared-prefix"), Recorded::listing(&[]));

    assert_eq!(district_names(&harness), ["billing", "orders"]);
}

/// A project set to no prefix shows its top folders as districts
#[test]
fn a_project_set_to_no_prefix_shows_its_top_folders_as_districts() {
    let tree = Tree::project("shop", &["src/catalog/item.rs", "src/ordering/order.rs"]);

    let harness = open(&tree, Some("none"), Recorded::listing(&[]));

    assert_eq!(district_names(&harness), ["src"]);
}

/// An unknown district prefix falls back to the default and says so
#[test]
fn an_unknown_district_prefix_falls_back_to_the_default_and_says_so() {
    let tree = Tree::project("shop", &["src/catalog/item.rs", "src/ordering/order.rs"]);

    let harness = open(&tree, Some("sideways"), Recorded::listing(&[]));

    assert_eq!(district_names(&harness), ["catalog", "ordering"]);
    assert!(
        labels(&harness)
            .iter()
            .any(|label| label.contains("\"sideways\" is unknown")),
        "{:?}",
        labels(&harness)
    );
}

/// Requirements 1 and 2: a double-click opens one project and hides the
/// others; Esc shows the universe again with the projects it had.
#[test]
fn a_double_click_opens_one_project_and_esc_goes_back() {
    let alpha = Tree::project("alpha", &["src/catalog/item.rs", "src/ordering/o.rs"]);
    let beta = Tree::project("beta", &["lib/x.rs"]);
    let mut harness = harness(app(
        vec![alpha.as_project(None), beta.as_project(None)],
        Recorded::listing(&[]),
    ));

    double_click(&mut harness, "alpha");
    await_read(&mut harness);

    let opened = labels(&harness);
    assert!(opened.iter().any(|l| l == "catalog"), "{opened:?}");
    assert!(
        !opened.iter().any(|l| l == "alpha" || l == "beta"),
        "{opened:?}"
    );

    harness.key_press(egui::Key::Escape);
    harness.run();

    // STORY-078 surrounds the same project list with a header, a locations
    // column and an inspector, so a label is asked for rather than the whole
    // of what is drawn.
    let shown = labels(&harness);
    for name in ["alpha", "beta"] {
        assert!(shown.contains(&name.to_owned()), "{shown:?}");
    }
    assert!(
        !shown.iter().any(|label| label == "catalog"),
        "the project closed on Esc is no longer shown: {shown:?}"
    );
}

/// Requirement 1: the click that selects a project leaves its row where it
/// was, so the second click of the double-click lands on it and opens it.
/// The folder's path is wide enough to wrap once anything narrows the list
/// (STORY-078's inspector appearing on selection did), moving the row below.
#[test]
fn a_double_click_opens_a_project_under_a_long_folder_path() {
    let path =
        PathBuf::from("/private/var/folders/jw/8nnwftsn3b9_3jpgxvmhq_3m0000gn/T/.tmppqmQOC/shop");
    let mut harness = harness(app(
        vec![cellworks_workspace::Project::at(path)],
        Recorded::listing(&[]),
    ));

    double_click(&mut harness, "shop");

    assert!(harness.state().opened().is_some(), "{:?}", labels(&harness));
}

/// Requirements 3 and 4: while git has not answered, frames keep running and
/// the project says it is being read, not that it is empty.
#[test]
fn a_project_says_it_is_being_read_while_frames_keep_running() {
    let tree = Tree::repository("shop", &["src/a.rs"]);
    let (release, waiting) = mpsc::channel();
    let mut harness = harness(app(
        vec![tree.as_project(None)],
        Arc::new(Blocking(Mutex::new(waiting))),
    ));

    double_click(&mut harness, "shop");
    harness.state_mut().apply_pending();
    harness.run();
    harness.run();

    assert_eq!(labels(&harness), [READING]);
    drop(release);
}

/// Requirement 4: a read that goes away without a map says the project could
/// not be read, rather than "reading" for ever.
#[test]
fn a_read_that_goes_away_says_the_project_could_not_be_read() {
    let tree = Tree::repository("shop", &["src/a.rs"]);

    let harness = open(&tree, None, Arc::new(Panicking));

    assert_eq!(labels(&harness), [FAILED]);
}

/// Requirement 10: when git cannot answer, the folder is walked and the map
/// is drawn with a notice beside it.
#[test]
fn when_git_cannot_answer_the_map_is_drawn_with_a_notice() {
    let tree = Tree::repository("shop", &["build/output.o"]);

    let harness = open(
        &tree,
        None,
        Arc::new(Recorded(Err(GitUnavailable("git: not found".to_owned())))),
    );

    assert_eq!(district_names(&harness), ["build"]);
    assert!(
        labels(&harness)
            .iter()
            .any(|label| label.contains("ignored files may be on the map")),
        "{:?}",
        labels(&harness)
    );
}

/// Requirements 24 and 25: the whole project fits the panel, and every
/// district, sub-district, group and node carries its name as a label.
#[test]
fn the_whole_project_fits_the_panel_and_every_element_is_labelled() {
    let files: Vec<String> = (0..60)
        .map(|n| format!("src/d{}/s{}/g{}/f{n}.rs", n % 4, n % 3, n % 2))
        .collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let tree = Tree::project("shop", &files);

    let mut harness = open(&tree, None, Recorded::listing(&[]));
    // This requirement is about the project's own districts, sub-districts,
    // groups and nodes, not the Explorer beside them (STORY-089); hidden, the
    // labels this test reads are exactly the map's own.
    harness.key_press_modifiers(Modifiers::COMMAND, egui::Key::B);
    harness.run();

    let root = harness.root();
    for node in root.children_recursive() {
        let node = node.accesskit_node();
        if node.role() == Role::Label {
            let drawn = node.bounding_box().expect("a label is laid out");
            assert!(
                drawn.x0 >= 0.0 && drawn.y0 >= 0.0 && drawn.x1 <= 1200.0 && drawn.y1 <= 900.0,
                "{:?} at {drawn:?} is outside the window",
                node.value()
            );
        }
    }
    let map = map(&harness);
    fn groups(groups: &[cellworks_map::Group], into: &mut Vec<String>) {
        for group in groups {
            into.push(group.name.clone());
            groups_and_nodes(&group.groups, &group.nodes, into);
        }
    }
    fn groups_and_nodes(
        inner: &[cellworks_map::Group],
        nodes: &[cellworks_map::Node],
        into: &mut Vec<String>,
    ) {
        groups(inner, into);
        into.extend(nodes.iter().map(|node| node.name.clone()));
    }
    let mut expected = Vec::new();
    for district in &map.districts {
        expected.push(district.name.clone());
        for sub in &district.sub_districts {
            expected.push(sub.name.clone());
            groups_and_nodes(&sub.groups, &sub.nodes, &mut expected);
        }
        expected.extend(district.nodes.iter().map(|node| node.name.clone()));
    }
    let mut shown = labels(&harness);
    shown.sort();
    expected.sort();
    assert_eq!(shown, expected);
}

/// Requirement 26: "no files" is counted after the exclusions.
#[test]
fn a_project_whose_every_file_is_excluded_says_it_has_no_files() {
    let tree = Tree::repository("shop", &[".cache/x", "build/output.o"]);

    let harness = open(&tree, None, Recorded::listing(&[".cache/x"]));

    assert_eq!(labels(&harness), [EMPTY]);
}
