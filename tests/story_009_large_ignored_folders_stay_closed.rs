//! STORY-009, large ignored folders stay closed when ignored files are shown:
//! a well-known heavy folder, or one over the large-folder limit, is one node
//! that says how many files it holds.
//!
//! One test per scenario of issue #27, named after it; the scenario outline is
//! one test looping over its examples. Each project is a real folder tree in a
//! temporary directory with a `.git` folder, and git is a recorded listing of
//! the files it does not ignore. No test runs git, reads `$HOME` or opens a
//! window.

mod common;

use cellworks::app::CellworksApp;
use cellworks_map::{Element, ProjectMap};
use egui::Key;
use egui_kittest::Harness;

use common::{Recorded, Tree, await_read, labels, map, open};

/// The Background's file, which git lists.
const ITEM: &str = "src/catalog/item.rs";

/// A repository holding the Background's file and `files`, with git listing
/// `listed` besides it; opened, with ignored files switched on.
fn shown(files: &[&str], listed: &[&str]) -> (Tree, Harness<'static, CellworksApp>) {
    let mut all = vec![ITEM];
    all.extend_from_slice(files);
    let tree = Tree::repository("shop", &all);
    let mut git = vec![ITEM];
    git.extend_from_slice(listed);
    let mut harness = open(&tree, None, Recorded::listing(&git));
    harness.key_press(Key::I);
    harness.run();
    await_read(&mut harness);
    (tree, harness)
}

/// `folder` is one node on the map, labelled with `label`, and nothing inside
/// it is on the map.
fn assert_closed(tree: &Tree, harness: &Harness<'_, CellworksApp>, folder: &str, label: &str) {
    let map = map(harness);
    let path = tree.root.join(folder);
    let found = map
        .find(&path)
        .unwrap_or_else(|| panic!("{folder} is not on the map"));
    assert!(
        found.kind == Element::Node,
        "{folder} is a folder, not one closed node"
    );
    let shown = labels(harness);
    assert!(shown.iter().any(|l| l == label), "{label} not in {shown:?}");
    let inside: Vec<_> = node_paths(&map)
        .into_iter()
        .filter(|node| node.starts_with(&path) && node != &path)
        .collect();
    assert!(inside.is_empty(), "inside {folder}: {inside:?}");
}

fn node_paths(map: &ProjectMap) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for district in &map.districts {
        paths.extend(district.nodes.iter().map(|n| n.path.clone()));
        for sub in &district.sub_districts {
            paths.extend(sub.nodes.iter().map(|n| n.path.clone()));
            let mut pending: Vec<_> = sub.groups.iter().collect();
            while let Some(group) = pending.pop() {
                paths.extend(group.nodes.iter().map(|n| n.path.clone()));
                pending.extend(group.groups.iter());
            }
        }
    }
    paths
}

/// The names of the nodes the district `name` holds directly.
fn district_nodes(map: &ProjectMap, name: &str) -> Vec<String> {
    let district = map
        .districts
        .iter()
        .find(|district| district.name == name)
        .unwrap_or_else(|| panic!("no district {name}"));
    district.nodes.iter().map(|n| n.name.clone()).collect()
}

/// A folder with a well-known heavy name stays closed
#[test]
fn a_folder_with_a_well_known_heavy_name_stays_closed() {
    for folder in ["node_modules", "target", ".venv", "src/web/node_modules"] {
        let files: Vec<String> = ["a", "b/c", "b/d"]
            .iter()
            .map(|file| format!("{folder}/{file}"))
            .collect();
        let files: Vec<&str> = files.iter().map(String::as_str).collect();

        let (tree, harness) = shown(&files, &[]);

        let name = folder.rsplit('/').next().expect("a name");
        assert_closed(&tree, &harness, folder, &format!("{name} (3 files)"));
    }
}

/// Any ignored folder with more files than the limit stays closed
#[test]
fn any_ignored_folder_with_more_files_than_the_limit_stays_closed() {
    let files: Vec<String> = (0..1001).map(|i| format!("logs/{i}.log")).collect();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();

    let (tree, harness) = shown(&files, &[]);

    assert_closed(&tree, &harness, "logs", "logs (1001 files)");
}

/// A small ignored folder opens as before
#[test]
fn a_small_ignored_folder_opens_as_before() {
    let (_tree, harness) = shown(&["logs/a.log", "logs/b.log"], &[]);

    assert_eq!(district_nodes(&map(&harness), "logs"), ["a.log", "b.log"]);
}

/// A folder git does not ignore is never closed
#[test]
fn a_folder_git_does_not_ignore_is_never_closed() {
    let (_tree, harness) = shown(&["build/plan.rs"], &["build/plan.rs"]);

    assert_eq!(district_nodes(&map(&harness), "build"), ["plan.rs"]);
}
