//! STORY-089, navigate a project from the Explorer: the Explorer lists the
//! open project's tree beside the map, follows the active lens, moves the
//! camera as the cursor moves, follows a selection made on the map, filters
//! by name and marks folders and files with uncommitted changes.
//!
//! One test per scenario of issue #142, named after it. The project "shop" is
//! a real folder tree in a temporary directory with the district "catalog"
//! (its sub-district "pricing", holding "base.rs" and the group "rules",
//! holding "discount.rs", plus the file "item.rs" directly in "catalog") and
//! the district "orders" (`order.rs`): a genuine folder/subfolder/file depth
//! of three, with no top-level loose file, shaped like
//! `cellworks_navigation`'s own `shop()` fixture. No test runs git, reads
//! `$HOME` or opens a window.

mod common;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use cellworks::app::CellworksApp;
use cellworks_map::{Group, Lens, ProjectMap};
use cellworks_navigation::Layer;
use cellworks_navigation::explorer::{Change, Explorer};
use egui::Key;
use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::{Harness, Node};

use common::{Recorded, Tree, open};

struct Shop {
    tree: Tree,
    harness: Harness<'static, CellworksApp>,
}

/// The background: "shop" is open with the Explorer beside the map.
fn shop() -> Shop {
    let tree = Tree::project(
        "shop",
        &[
            "catalog/pricing/base.rs",
            "catalog/pricing/rules/discount.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    Shop { tree, harness }
}

/// The rows of the Explorer, in order: the buttons that can be highlighted.
fn rows<'t>(root: Node<'t>) -> Vec<Node<'t>> {
    root.children_recursive()
        .filter(|node| {
            let node = node.accesskit_node();
            node.role() == Role::Button
                && node.toggled().is_some()
                && node.label().as_deref() != Some("lens panel")
        })
        .collect()
}

fn name(node: &Node<'_>) -> String {
    node.accesskit_node()
        .label()
        .or_else(|| node.accesskit_node().value())
        .map(|text| text.to_string())
        .unwrap_or_default()
}

/// Types `text`, letter by letter, into whatever already has the keyboard
/// focus (the filter field, once "/" has focused it).
fn type_into_focused(harness: &mut Harness<'_, CellworksApp>, text: &str) {
    for letter in text.chars() {
        harness
            .input_mut()
            .events
            .push(egui::Event::Text(letter.to_string()));
    }
    harness.run();
}

/// Every node path the lensed `map` holds, however deep, counted once per
/// occurrence: used to check a file appears under exactly one group.
fn node_paths(map: &ProjectMap) -> Vec<PathBuf> {
    fn in_groups(groups: &[Group], into: &mut Vec<PathBuf>) {
        for group in groups {
            into.extend(group.nodes.iter().map(|node| node.path.clone()));
            in_groups(&group.groups, into);
        }
    }
    let mut paths = Vec::new();
    for district in &map.districts {
        paths.extend(district.nodes.iter().map(|node| node.path.clone()));
        for sub in &district.sub_districts {
            paths.extend(sub.nodes.iter().map(|node| node.path.clone()));
            in_groups(&sub.groups, &mut paths);
        }
    }
    paths
}

impl Shop {
    fn at(&self, relative: &str) -> PathBuf {
        self.tree.root.join(relative)
    }

    fn press(&mut self, key: Key) {
        self.harness.key_press(key);
        self.harness.run();
    }

    fn press_cmd_b(&mut self) {
        self.harness
            .key_press_modifiers(egui::Modifiers::COMMAND, Key::B);
        self.harness.run();
    }

    fn listed(&self) -> Vec<String> {
        rows(self.harness.root()).iter().map(name).collect()
    }

    fn highlighted(&self) -> Option<String> {
        rows(self.harness.root())
            .iter()
            .find(|row| row.accesskit_node().toggled() == Some(Toggled::True))
            .map(name)
    }

    fn selection(&self) -> Option<PathBuf> {
        self.harness
            .state()
            .navigation()
            .selection()
            .map(Path::to_path_buf)
    }
}

/// The Explorer is open beside the map by default
#[test]
fn the_explorer_is_open_beside_the_map_by_default() {
    let shop = shop();

    assert!(shop.harness.state().explorer_open());
    assert_eq!(shop.listed(), ["catalog", "orders"]);
    shop.harness.get_by_label("4 files");
    let map = common::map(&shop.harness);
    for lens in Lens::ALL {
        if lens.offered(&map) {
            shop.harness.get_by_label(lens.name());
        }
    }
    assert!(
        Lens::Folders.offered(&map),
        "a plain project offers folders"
    );
}

/// The Explorer can be hidden and shown again
#[test]
fn the_explorer_can_be_hidden_and_shown_again() {
    let mut shop = shop();
    shop.press(Key::ArrowDown);
    let selected = shop.highlighted().expect("a row is highlighted");

    shop.press_cmd_b();

    assert!(!shop.harness.state().explorer_open());
    assert!(
        rows(shop.harness.root()).is_empty(),
        "no Explorer row while it is hidden"
    );

    shop.press_cmd_b();

    assert!(shop.harness.state().explorer_open());
    assert_eq!(
        shop.highlighted(),
        Some(selected),
        "the same row is still selected"
    );
}

/// Moving the cursor selects the row and frames it on the map (row: folder, place: district)
#[test]
fn moving_the_cursor_to_a_folder_row_frames_its_district() {
    let mut shop = shop();

    shop.press(Key::ArrowDown);

    let catalog = shop.at("catalog");
    assert_eq!(shop.selection(), Some(catalog.clone()));
    let map = common::map(&shop.harness);
    assert_eq!(
        shop.harness.state().navigation().framing(&map).area,
        map.find(&catalog).unwrap().area
    );
}

/// Moving the cursor selects the row and frames it on the map (row: subfolder, place: sub-district)
#[test]
fn moving_the_cursor_to_a_subfolder_row_frames_its_sub_district() {
    let mut shop = shop();
    shop.press(Key::ArrowDown); // catalog
    shop.press(Key::ArrowRight); // unfold catalog
    shop.press(Key::ArrowDown); // pricing: the first child, folders first

    let pricing = shop.at("catalog/pricing");
    assert_eq!(shop.selection(), Some(pricing.clone()));
    let map = common::map(&shop.harness);
    assert_eq!(
        shop.harness.state().navigation().framing(&map).area,
        map.find(&pricing).unwrap().area
    );
}

/// Moving the cursor selects the row and frames it on the map (row: file, place: building)
#[test]
fn moving_the_cursor_to_a_file_row_frames_its_building() {
    let mut shop = shop();
    shop.press(Key::ArrowDown); // catalog
    shop.press(Key::ArrowRight); // unfold catalog
    shop.press(Key::ArrowDown); // pricing
    shop.press(Key::ArrowDown); // item.rs: the next sibling, a file

    let item = shop.at("catalog/item.rs");
    assert_eq!(shop.selection(), Some(item.clone()));
    let map = common::map(&shop.harness);
    assert_eq!(
        shop.harness.state().navigation().framing(&map).area,
        map.find(&item).unwrap().area
    );
}

/// Selecting on the map selects the matching row
#[test]
fn selecting_on_the_map_selects_the_matching_row() {
    let mut shop = shop();
    // "catalog" (and so "base.rs"'s folder) is folded: the default.
    assert_eq!(shop.listed(), ["catalog", "orders"]);

    shop.harness.get_by_label("base.rs").click_accesskit();
    shop.harness.run();

    let base = shop.at("catalog/pricing/base.rs");
    assert_eq!(shop.selection(), Some(base));
    assert_eq!(
        shop.highlighted().as_deref(),
        Some("base.rs"),
        "the Explorer unfolded to the file's row"
    );
}

const CALLS_FILES: [(&str, &str); 4] = [
    ("catalog/Cargo.toml", "[package]\nname = \"catalog\"\n"),
    (
        "catalog/src/lib.rs",
        "use orders::ship;\n\npub fn price() -> u32 {\n    ship();\n    1\n}\n",
    ),
    ("orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
    (
        "orders/src/lib.rs",
        "use catalog::price;\n\npub fn ship() -> u32 {\n    price()\n}\n",
    ),
];

/// The Explorer regroups by the active lens
#[test]
fn the_explorer_regroups_by_the_active_lens() {
    let tree = Tree::sources("shop", &CALLS_FILES);
    let mut harness = open(&tree, None, Recorded::listing(&[]));

    harness.get_by_label("calls").click();
    common::await_lens(&mut harness);
    harness.run_steps(150);
    assert_eq!(
        harness.state().opened().map(|opened| opened.lens()),
        Some(Lens::Calls)
    );

    let calls_map = common::map(&harness);
    let mut paths = node_paths(&calls_map);
    paths.sort();
    let mut deduped = paths.clone();
    deduped.dedup();
    assert_eq!(
        paths, deduped,
        "each file appears under exactly one group in the calls lens"
    );

    let row_names: Vec<String> = rows(harness.root()).iter().map(name).collect();
    assert!(
        !calls_map.districts.is_empty(),
        "the calls lens found at least one district"
    );
    for district in &calls_map.districts {
        assert!(
            row_names.contains(&district.name),
            "{}: {row_names:?}",
            district.name
        );
    }
}

/// Filtering narrows the rows by name
#[test]
fn filtering_narrows_the_rows_by_name() {
    let mut shop = shop();

    shop.press(Key::Slash);
    type_into_focused(&mut shop.harness, "base");

    let listed = shop.listed();
    for present in ["catalog", "pricing", "base.rs"] {
        assert!(listed.contains(&present.to_owned()), "{listed:?}");
    }
    for absent in ["item.rs", "orders", "discount.rs", "rules", "order.rs"] {
        assert!(!listed.contains(&absent.to_owned()), "{listed:?}");
    }
}

/// Rows show where each part stands
#[test]
fn rows_show_where_each_part_stands() {
    // Neither `CellworksApp` nor `OpenedProject` exposes the live Explorer or
    // the background-read changed set directly (outside the pinned
    // contract), and the accessible text of a change mark is not pinned
    // either, so this asserts through the `Explorer` model with a `changed`
    // set matching the file modified below, rather than guessing at wording.
    // `GitCli::changed` itself is pinned and tested in
    // `crates/code-analysis/src/git.rs` and its real-git test.
    let tree = Tree::project(
        "shop",
        &[
            "catalog/pricing/base.rs",
            "catalog/pricing/rules/discount.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ],
    );
    fs::create_dir(tree.root.join(".git")).expect("create .git");
    fs::write(tree.root.join("catalog/pricing/base.rs"), "changed\n").expect("modify the file");
    let git = Recorded::listing(&[
        "catalog/pricing/base.rs",
        "catalog/pricing/rules/discount.rs",
        "catalog/item.rs",
        "orders/order.rs",
    ]);
    let harness = open(&tree, None, git);
    let map = common::map(&harness);
    let changed_file = tree.root.join("catalog/pricing/base.rs");
    let changed: HashSet<PathBuf> = [changed_file.clone()].into_iter().collect();

    let mut explorer = Explorer::default();
    explorer.reveal(&map, &changed_file);
    let rows = explorer.rows(&map, &changed);
    let row = |path: &Path| {
        rows.iter()
            .find(|row| row.path == path)
            .unwrap_or_else(|| panic!("no row for {}", path.display()))
    };

    assert_eq!(row(&changed_file).change, Change::Modified);
    assert_eq!(
        row(&tree.root.join("catalog/pricing/rules")).change,
        Change::None,
        "rules holds only the untouched discount.rs"
    );
    assert_eq!(
        row(&tree.root.join("catalog/pricing")).change,
        Change::Holds
    );
    assert_eq!(row(&tree.root.join("catalog")).change, Change::Holds);
    assert_eq!(
        row(&tree.root.join("orders")).change,
        Change::None,
        "orders holds no changed file"
    );

    let catalog = row(&tree.root.join("catalog"));
    assert_eq!(
        catalog.colour,
        Some(cellworks_map::ui::district_colour_at(
            &map,
            &tree.root.join("catalog")
        )),
        "a folder row shows its district's map colour"
    );
    assert_eq!(
        catalog.count,
        Some(3),
        "a folder row shows its file count: base.rs, discount.rs, item.rs"
    );
}

/// Leaving to the whole map
#[test]
fn leaving_to_the_whole_map() {
    let mut shop = shop();
    shop.press(Key::ArrowDown); // frames "catalog" as a district
    assert_eq!(
        shop.harness.state().navigation().layer(),
        Layer::Directory,
        "a district is framed from the Explorer"
    );

    shop.press(Key::F); // fit: leave to the whole map

    let map = common::map(&shop.harness);
    assert_eq!(
        shop.harness.state().navigation().framing(&map).area,
        map.extent,
        "the whole project is framed, without being shifted beside the Explorer"
    );
}
