//! STORY-019, a tower explains why it was built: the selected tower names
//! its bounded context, the pull requests that built its floors with their
//! shares and the stories under them, and the decisions that bind it; inside
//! the tower each floor names the pull request and story that last wrote it;
//! a tower with no story behind it says so.
//!
//! One test per scenario of issue #41, named after it. The project "shop"
//! documents the bounded context "Pricing", implemented by the crate
//! "catalog"; the pull request "#12" implementing "STORY-012: Loyalty
//! discount" built most floors of the tower "discount", "#80" the rest; the
//! decision "ADR-0003" binds "Pricing". No test runs git, reads `$HOME` or
//! opens a window.
//!
//! The scenarios that need per-line history hand-build the `ProjectMap` the
//! loaders would deliver and draw the Map context's own panel, as
//! `design.md`'s Proof says: recording a git log would mean widening
//! `cellworks-code-analysis`, which requirement 11 leaves alone. The
//! scenarios that need only the Jarvis docs run through the whole binary,
//! from a real folder tree.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::ui::{DrawnTower, ViewOptions};
use cellworks_map::{
    BoundedContext, BoundedContexts, Commit, Declaration, Directories, Floor, Framing, Item,
    ItemKind, Lanes, LineHistory, ProjectMap, StoryText, Why, default_prefix_rule, why_of,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

use common::{Recorded, Tree};

// ---------------------------------------------------------------------------
// The project "shop" as its loaders would deliver it, history and all.
// ---------------------------------------------------------------------------

const ROOT: &str = "/p";

/// The tower "discount": nine floors, the first two written by "#80" and the
/// other seven by "#12".
const DISCOUNT: &str = "catalog/discount.rs";

/// The tower "legacy_helper": three floors, written before any story existed.
const LEGACY: &str = "catalog/legacy_helper.rs";

fn commit(sha: &str, summary: &str, pull_request: Option<u32>, stories: &[&str]) -> Commit {
    Commit {
        sha: sha.to_owned(),
        time: 0,
        summary: summary.to_owned(),
        pull_request,
        stories: stories.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn item(file: &Path, name: &str, lines: usize) -> Item {
    Item {
        name: name.to_owned(),
        path: file.join(name),
        kind: ItemKind::Function,
        tests: false,
        floors: (1..=lines)
            .map(|number| Floor {
                number,
                text: format!("    let a{number} = price;"),
                indent: 4,
                width: 24,
                tokens: Vec::new(),
                calls: Vec::new(),
                imports: Vec::new(),
            })
            .collect(),
        area: egui::Rect::NOTHING,
        base: 0.0,
        declaration: Declaration::default(),
    }
}

fn story(title: &str, so_that: &str) -> StoryText {
    StoryText {
        title: title.to_owned(),
        so_that: so_that.to_owned(),
    }
}

/// The map of "shop" the loaders deliver: two towers in the crate "catalog",
/// which implements the bounded context "Pricing", with the per-line history
/// of STORY-018 and the domain knowledge of DomainKnowledge already on it.
fn shop() -> ProjectMap {
    let root = Path::new(ROOT);
    let files: Vec<PathBuf> = [DISCOUNT, LEGACY].iter().map(PathBuf::from).collect();
    let items = BTreeMap::from([
        (
            PathBuf::from(DISCOUNT),
            vec![item(&root.join(DISCOUNT), "discount", 9)],
        ),
        (
            PathBuf::from(LEGACY),
            vec![item(&root.join(LEGACY), "legacy_helper", 3)],
        ),
    ]);
    let strategy = Directories {
        prefix: default_prefix_rule(),
    };
    let mut map = ProjectMap::of_code("shop", root, &files, &strategy, items, &BTreeMap::new());
    map.history = LineHistory {
        commits: vec![
            commit(
                "c1",
                "feat: loyalty discount (#12)",
                Some(12),
                &["STORY-012"],
            ),
            commit(
                "c2",
                "feature: relationship lenses (#80)",
                Some(80),
                &["STORY-015", "STORY-016"],
            ),
            commit("c3", "chore: initial import (#2)", Some(2), &[]),
        ],
        lines: BTreeMap::from([
            // Two floors from "#80", seven from "#12".
            (
                root.join(DISCOUNT),
                (0..9)
                    .map(|line| if line < 2 { Some(1) } else { Some(0) })
                    .collect(),
            ),
            (root.join(LEGACY), vec![Some(2); 3]),
        ]),
    };
    map.contexts = Some(BoundedContexts {
        contexts: vec![BoundedContext {
            name: "Pricing".to_owned(),
            code: vec![root.join("catalog")],
            description: "How much a customer pays for an order.".to_owned(),
        }],
        stories: BTreeMap::from([
            (
                "STORY-012".to_owned(),
                story("Loyalty discount", "regular customers come back"),
            ),
            (
                "STORY-015".to_owned(),
                story("Relationship lenses", "I see what connects the code"),
            ),
            (
                "STORY-016".to_owned(),
                story("The cells lens", "the domain model is on the map"),
            ),
        ]),
        ..Default::default()
    });
    map
}

fn tower(name: &str, file: &str) -> PathBuf {
    Path::new(ROOT).join(file).join(name)
}

/// Why the tower `name` of "shop" was built.
fn why(map: &ProjectMap, name: &str, file: &str) -> Why {
    why_of(map, &tower(name, file)).unwrap_or_else(|| panic!("a why for the tower {name}"))
}

/// Every label the why panel writes for `why`, in order; `floor` is the
/// commit of the floor the engineer stands on, if inside the tower.
fn panel(why: &Why, floor: Option<&Commit>) -> Vec<String> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(420.0, 1400.0))
        .build_ui(|ui| cellworks_map::ui::why_panel(ui, why, floor));
    harness.run();
    labels_of(&harness)
}

fn labels_of<S>(harness: &Harness<'_, S>) -> Vec<String> {
    let root = harness.root();
    std::iter::once(root)
        .chain(root.children_recursive())
        .filter_map(|node| {
            let node = node.accesskit_node();
            match node.role() {
                Role::Label => node.value().map(|text| text.to_string()),
                _ => None,
            }
        })
        .collect()
}

/// Whether `lines` holds one that contains `text`.
fn says(lines: &[String], text: &str) -> bool {
    lines.iter().any(|line| line.contains(text))
}

/// Where `text` stands among `lines`.
fn place(lines: &[String], text: &str) -> usize {
    lines
        .iter()
        .position(|line| line.contains(text))
        .unwrap_or_else(|| panic!("no line holding {text:?} among {lines:#?}"))
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

/// The selected tower names its bounded context
#[test]
fn the_selected_tower_names_its_bounded_context() {
    let (_tree, harness) = documented_shop();
    let shown = common::labels(&harness);

    // The description is the panel's own; the name stands on the line above
    // it, which the folder named after the context on the map does not.
    let at = place(&shown, "How much a customer pays for an order.");
    assert_eq!(shown[at - 1], "Pricing", "{shown:#?}");
    assert!(
        !says(&shown, "What a customer asked for."),
        "not the other context's description, {shown:#?}"
    );
}

/// The pull requests that built the tower are listed with their share of
/// floors
#[test]
fn the_pull_requests_that_built_the_tower_are_listed_with_their_share_of_floors() {
    let map = shop();

    let shown = panel(&why(&map, "discount", DISCOUNT), None);

    assert!(
        says(&shown, "#12 · 7 of 9 floors (78 %)"),
        "most floors, {shown:#?}"
    );
    assert!(says(&shown, "#80 · 2 of 9 floors (22 %)"), "{shown:#?}");
    assert!(
        place(&shown, "#12 ·") < place(&shown, "#80 ·"),
        "the largest share first"
    );
    assert!(
        says(&shown, "STORY-012: Loyalty discount"),
        "the story under it, {shown:#?}"
    );
    assert!(
        says(&shown, "So that regular customers come back"),
        "with its \"so that\", {shown:#?}"
    );
    assert!(
        place(&shown, "#12 ·") < place(&shown, "STORY-012"),
        "the story stands under its pull request"
    );
}

/// A pull request that implemented several stories lists all of them
#[test]
fn a_pull_request_that_implemented_several_stories_lists_all_of_them() {
    let map = shop();

    let shown = panel(&why(&map, "discount", DISCOUNT), None);

    assert!(says(&shown, "#80 · 2 of 9 floors"), "{shown:#?}");
    assert!(says(&shown, "STORY-015: Relationship lenses"), "{shown:#?}");
    assert!(says(&shown, "STORY-016: The cells lens"), "{shown:#?}");
    assert!(
        place(&shown, "#80 ·") < place(&shown, "STORY-015"),
        "both stand under the one pull request"
    );
    assert!(place(&shown, "#80 ·") < place(&shown, "STORY-016"));
}

/// Decisions that bind the tower are listed
#[test]
fn decisions_that_bind_the_tower_are_listed() {
    let (_tree, harness) = documented_shop();
    let shown = common::labels(&harness);

    assert!(
        says(&shown, "ADR-0003: Round prices to the nearest cent"),
        "{shown:#?}"
    );
    assert!(
        says(&shown, "We round every price to the nearest cent."),
        "its decision, {shown:#?}"
    );
    assert!(
        !says(&shown, "ADR-0006"),
        "not the one scoped to another context, {shown:#?}"
    );
}

/// Inside the tower each floor names the pull request and story that last
/// wrote it
#[test]
fn inside_the_tower_each_floor_names_the_pull_request_and_story_that_last_wrote_it() {
    let map = shop();
    let discount = tower("discount", DISCOUNT);

    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui(|ui| {
            let panel = ui.available_rect_before_wrap();
            cellworks_map::ui::inside(ui, &map, &discount, 0, panel);
        });
    harness.run();

    let rows: Vec<String> = harness
        .root()
        .children_recursive()
        .map(|node| node.accesskit_node())
        .filter(|node| node.toggled().is_some())
        .filter_map(|node| node.label())
        .collect();
    assert_eq!(rows.len(), 9, "one row per floor, {rows:#?}");
    assert!(
        rows[0].contains("#80 STORY-015 +1"),
        "the first floor, written by the epic pull request: {:?}",
        rows[0]
    );
    assert!(
        rows[2].contains("#12 STORY-012"),
        "the third floor: {:?}",
        rows[2]
    );
    assert!(
        rows.iter().filter(|row| row.contains("#12")).count() == 7,
        "seven floors from #12, {rows:#?}"
    );

    // Selecting a floor explains it in the panel: its commit's subject and
    // each of its stories' "so that".
    let why = why(&map, "discount", DISCOUNT);
    let floor = cellworks_map::floor_commit(&map, map.item(&discount).expect("the tower"), 0);
    let shown = panel(&why, floor);
    assert!(
        says(&shown, "feature: relationship lenses (#80)"),
        "{shown:#?}"
    );
    assert!(
        says(&shown, "So that I see what connects the code"),
        "{shown:#?}"
    );
}

/// A tower with no story behind it says so
#[test]
fn a_tower_with_no_story_behind_it_says_so() {
    let map = shop();

    let shown = panel(&why(&map, "legacy_helper", LEGACY), None);

    assert!(
        says(&shown, cellworks_map::ui::NO_STORY),
        "built by commits without a story, {shown:#?}"
    );
    assert!(
        says(&shown, "chore: initial import (#2)"),
        "those commits are listed, {shown:#?}"
    );
    assert!(!says(&shown, "STORY-"), "no story to name, {shown:#?}");
}

// ---------------------------------------------------------------------------
// The rest of the design's proof
// ---------------------------------------------------------------------------

/// Nothing or a district selected shows no panel.
#[test]
fn no_panel_without_a_selected_tower() {
    let map = shop();

    assert_eq!(why_of(&map, Path::new(ROOT)), None, "the project");
    assert_eq!(
        why_of(&map, &Path::new(ROOT).join("catalog")),
        None,
        "a district"
    );
    assert_eq!(
        why_of(&map, &Path::new(ROOT).join(DISCOUNT)),
        None,
        "a block"
    );

    // And the app draws no why panel once the selection is no tower.
    let (tree, mut harness) = documented_shop();
    let explained = common::labels(&harness);
    assert!(says(&explained, "How much a customer pays for an order."));

    harness.state_mut().jump(tree.root.join("catalog"));
    harness.run();

    assert!(harness.state().why().is_none(), "a district is no tower");
    let shown = common::labels(&harness);
    assert!(
        !says(&shown, "How much a customer pays for an order."),
        "the panel is gone, {shown:#?}"
    );
}

/// The selected tower's floors are coloured by their pull request, and every
/// other tower is dimmed.
#[test]
fn other_towers_are_dimmed_while_one_is_selected() {
    let map = shop();
    let discount = tower("discount", DISCOUNT);
    let why = why(&map, "discount", DISCOUNT);

    let drawn = |tint: Option<&[usize]>| -> Vec<DrawnTower> {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1200.0, 900.0))
            .build_ui_state(
                |ui, towers: &mut Vec<DrawnTower>| {
                    *towers = cellworks_map::ui::project(
                        ui,
                        &map,
                        ViewOptions {
                            framing: Framing {
                                area: map.extent,
                                fill: 0.9,
                                closest: 8.0,
                                headroom: 0.0,
                            },
                            selected: Some(&discount),
                            folder_view: None,
                            pivot: None,
                            underground: false,
                            lanes: Lanes::default(),
                            picked: None,
                            network: None,
                            cables: false,
                            tint,
                            review: None,
                            within: None,
                        },
                    )
                    .towers;
                },
                Vec::new(),
            );
        harness.run();
        harness.state().clone()
    };

    let light = |towers: &[DrawnTower], name: &str, file: &str| -> f32 {
        let path = tower(name, file);
        towers
            .iter()
            .find(|drawn| drawn.path == path)
            .unwrap_or_else(|| panic!("no tower {name}"))
            .light
    };

    let before = drawn(None);
    let after = drawn(Some(&why.floors));

    assert!(
        light(&after, "legacy_helper", LEGACY) < light(&before, "legacy_helper", LEGACY),
        "the other tower steps back while one is explained"
    );
    assert_eq!(
        light(&after, "discount", DISCOUNT),
        light(&before, "discount", DISCOUNT),
        "the explained tower keeps its light"
    );
    // Its floors carry the colour of the pull request that wrote each.
    assert_eq!(why.floors, [1, 1, 0, 0, 0, 0, 0, 0, 0]);
}

// ---------------------------------------------------------------------------
// "shop" as a real folder tree, opened by the binary
// ---------------------------------------------------------------------------

const DECISIONS_INDEX: &str = "\
# 9. Architecture Decisions

- [ADR-0003: Round prices to the nearest cent](../adr/0003-rounding.md) — Accepted — technology · Pricing
- [ADR-0006: Keep orders in one table](../adr/0006-orders.md) — Accepted — structure · Ordering
";

/// The project "shop" as a folder tree, opened by the binary with the tower
/// "discount" selected: a real read of its Jarvis docs, no git and no window.
fn documented_shop() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::sources(
        "shop",
        &[
            (
                "docs/domain/contexts/Pricing/Pricing.md",
                "# Context: Pricing\n\nHow much a customer pays for an order.\n",
            ),
            (
                "docs/domain/contexts/Ordering/Ordering.md",
                "# Context: Ordering\n\nWhat a customer asked for.\n",
            ),
            ("docs/arc42/09-decisions.md", DECISIONS_INDEX),
            (
                "docs/adr/0003-rounding.md",
                "# ADR-0003: Round prices to the nearest cent\n\n\
                 ## Decision\nWe round every price to the nearest cent. The rest follows.\n",
            ),
            (
                "docs/adr/0006-orders.md",
                "# ADR-0006: Keep orders in one table\n\n\
                 ## Decision\nWe keep every order in one table.\n",
            ),
            (
                "catalog/discount.rs",
                "fn discount(price: u32) -> u32 {\n    price\n}\n",
            ),
        ],
    );
    let mut project = tree.as_project(None);
    project.contexts = BTreeMap::from([("Pricing".to_owned(), vec![PathBuf::from("catalog")])]);
    let name = project.name.clone();
    let mut harness = common::harness(common::app(vec![project], Recorded::listing(&[])));
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    // The domain knowledge is read beside the code, on its own thread.
    for _ in 0..200 {
        harness.state_mut().apply_pending();
        harness.step();
        if harness
            .state()
            .opened()
            .is_some_and(|opened| opened.offers(cellworks_map::Lens::Cells))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    harness
        .state_mut()
        .jump(tree.root.join("catalog/discount.rs/discount"));
    harness.run();
    assert!(
        harness.state().why().is_some(),
        "the selected tower is explained"
    );
    (tree, harness)
}
