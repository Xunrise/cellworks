//! STORY-002, see every project in the universe: what the engineer sees.
//!
//! The five scenarios of issue #4 come first, one test each, named after the
//! scenario: a real folder tree on disk, a scan, and what the launcher view
//! then labels. The tests after them pin the view's requirements in
//! `spec.md`, each naming its requirement in its doc comment.
//!
//! STORY-078 replaced the flat `ui::universe` with `ui::launcher`, which draws
//! a header, a locations column and an inspector around the same project
//! list, so a test here asks whether a label is *among* what was drawn rather
//! than the whole of it — the chrome around the list is this story's, not
//! STORY-002's, to pin.
//!
//! The harness pattern is the one `tests/empty_window.rs` established for
//! STORY-001, with the assertion inverted: it needs neither a GPU adapter nor
//! a display, and it never opens a window.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use cellworks_workspace::{Launcher, Project, Sources, Universe, WorkspaceEvent, scan, ui};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// Shown until the first result arrives.
const SCANNING: &str = "Scanning for projects…";

/// A `Sources` with nothing behind it: the view reads the two paths to write
/// them into the empty state, and never touches the filesystem.
fn sources() -> Sources {
    Sources {
        projects_folder: PathBuf::from("/Users/engineer/Projects"),
        settings_file: PathBuf::from(
            "/Users/engineer/Library/Application Support/Cellworks/settings.toml",
        ),
    }
}

fn a_project(name: &str) -> Project {
    Project::at(PathBuf::from("/Users/engineer/Projects").join(name))
}

/// The text of every labelled entry the frame put in the accessibility tree,
/// in tree order.
///
/// egui puts a label's text in the node's *value*, and repeats it on the text
/// runs below it, so only the `Label` nodes are read: one entry per label the
/// view drew.
fn labels_in(harness: &Harness<'_>) -> Vec<String> {
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

/// One frame of the launcher view, and what it labelled.
fn labels(universe: &Universe, launcher: &mut Launcher) -> Vec<String> {
    let mut harness = Harness::new_ui(|ui| {
        ui::launcher(ui, universe, launcher);
    });
    harness.run();
    labels_in(&harness)
}

/// What an empty universe tells the engineer, with this story's `Sources`
/// filled in. Product owner, 2026-10-08.
fn empty_state() -> String {
    "No projects yet. Put a git repository in /Users/engineer/Projects, or add a folder's path \
     to /Users/engineer/Library/Application Support/Cellworks/settings.toml."
        .to_owned()
}

/// The `Sources` of a temporary tree: `<root>/Projects` and
/// `<root>/settings.toml`. No scenario reads the user's home directory.
fn sources_in(root: &Path) -> Sources {
    Sources {
        projects_folder: root.join("Projects"),
        settings_file: root.join("settings.toml"),
    }
}

/// A git repository at `relative`: a folder holding a `.git` directory.
fn repository(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.join(".git")).expect("create a repository in the tree");
    path
}

/// A settings file adding `folders`, as the engineer would write it by hand.
fn add_folders(root: &Path, folders: &[&Path]) {
    let entries: String = folders
        .iter()
        .map(|folder| format!("  \"{}\",\n", folder.display()))
        .collect();
    fs::write(
        root.join("settings.toml"),
        format!("added_folders = [\n{entries}]\n"),
    )
    .expect("write the settings file");
}

/// What the launcher view labels once Cellworks has scanned `root`: the scan's
/// events, applied to a `Universe` and to a `Launcher` alike, the way the
/// binary forwards every drained event to both.
fn universe_of(root: &Path) -> Vec<String> {
    let sources = sources_in(root);
    let mut universe = Universe::scanning(&sources);
    let mut launcher = Launcher::new(None);
    for event in scan(&sources) {
        universe.apply(event.clone());
        launcher.apply(event);
    }
    labels(&universe, &mut launcher)
}

fn a_tree() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("a temporary directory for the tree")
}

/// Scenario: Git repositories in the projects folder appear as projects
#[test]
fn git_repositories_in_the_projects_folder_appear_as_projects() {
    let tree = a_tree();
    repository(tree.path(), "Projects/alpha");
    repository(tree.path(), "Projects/beta");

    let shown = universe_of(tree.path());

    assert!(shown.contains(&"alpha".to_owned()), "{shown:?}");
    assert!(shown.contains(&"beta".to_owned()), "{shown:?}");
}

/// Scenario: A folder that is not a git repository is not discovered
#[test]
fn a_folder_that_is_not_a_git_repository_is_not_discovered() {
    let tree = a_tree();
    repository(tree.path(), "Projects/alpha");
    fs::create_dir_all(tree.path().join("Projects/notes")).expect("create a plain folder");

    let labels = universe_of(tree.path());

    assert!(
        !labels.contains(&"notes".to_owned()),
        "a plain folder under the projects folder is not a project: {labels:?}"
    );
    assert!(labels.contains(&"alpha".to_owned()), "{labels:?}");
}

/// Scenario: An added folder appears as a project
#[test]
fn an_added_folder_appears_as_a_project() {
    let tree = a_tree();
    let gamma = tree.path().join("work/gamma");
    fs::create_dir_all(&gamma).expect("create a folder outside the projects folder");
    add_folders(tree.path(), &[&gamma]);

    assert!(universe_of(tree.path()).contains(&"gamma".to_owned()));
}

/// Scenario: The same folder in a different letter case is one project
///
/// Letter case follows the filesystem: on a case-insensitive volume, the
/// macOS default, "alpha" names the folder "Alpha" and they are one project;
/// on a case-sensitive one there is no folder "alpha" to add. Either way the
/// universe holds exactly one project for that folder.
#[test]
fn the_same_folder_in_a_different_letter_case_is_one_project() {
    let tree = a_tree();
    let alpha = repository(tree.path(), "Projects/Alpha");
    add_folders(tree.path(), &[&alpha.with_file_name("alpha")]);

    let labels = universe_of(tree.path());
    let projects = labels
        .iter()
        .filter(|label| label.eq_ignore_ascii_case("alpha"))
        .count();

    assert_eq!(projects, 1, "{labels:?}");
}

/// Scenario: No projects at all
#[test]
fn no_projects_at_all() {
    let tree = a_tree();
    let sources = sources_in(tree.path());

    let labels = universe_of(tree.path());

    // `empty_state()` names the fixed `sources()` fixture's paths, not this
    // test's own temporary tree, so the message is matched by its wording
    // here and by its two real paths below, rather than by the wrong fixed
    // text `empty_state()` would give.
    assert!(
        labels
            .iter()
            .any(|label| label.starts_with("No projects yet.")),
        "the launcher says how to add a project: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| {
            label.contains(&sources.projects_folder.display().to_string())
                && label.contains(&sources.settings_file.display().to_string())
        }),
        "naming both the projects folder and the settings file: {labels:?}"
    );
}

/// Review of #13: however many projects there are, the universe scrolls, so
/// none is cut off below the window.
#[test]
fn a_universe_taller_than_the_window_scrolls() {
    let mut universe = Universe::scanning(&sources());
    let mut launcher = Launcher::new(None);
    let discovered = WorkspaceEvent::ProjectsDiscovered(
        (0..200)
            .map(|n| a_project(&format!("project-{n:03}")))
            .collect(),
    );
    universe.apply(discovered.clone());
    launcher.apply(discovered);

    let mut harness = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui(|ui| {
            ui::launcher(ui, &universe, &mut launcher);
        });
    harness.run();

    harness.get_by_label("project-199").scroll_to_me();
    harness.run();

    let last = harness
        .get_by_label("project-199")
        .accesskit_node()
        .bounding_box()
        .expect("the last project is laid out");
    assert!(
        last.y1 <= 300.0,
        "the last project scrolls into the 300-point window, but ends at {}",
        last.y1
    );
}

/// Requirement 18: from the first frame until the first result arrives the
/// universe says it is scanning, and the empty state does not flash on the
/// way.
#[test]
fn the_universe_says_it_is_scanning_before_the_first_result_arrives() {
    let mut universe = Universe::scanning(&sources());
    let mut launcher = Launcher::new(None);

    let while_scanning = labels(&universe, &mut launcher);
    let discovered = WorkspaceEvent::ProjectsDiscovered(Vec::new());
    universe.apply(discovered.clone());
    launcher.apply(discovered);
    let once_reported = labels(&universe, &mut launcher);

    assert!(
        while_scanning.contains(&SCANNING.to_owned()),
        "before the scan reports, the launcher says it is scanning: {while_scanning:?}"
    );
    assert!(
        !while_scanning.contains(&empty_state()),
        "the empty state must not flash on the way: {while_scanning:?}"
    );
    assert!(
        once_reported.contains(&empty_state()),
        "and it appears once the scan has reported nothing: {once_reported:?}"
    );
    assert!(
        !once_reported.contains(&SCANNING.to_owned()),
        "and scanning is no longer said: {once_reported:?}"
    );
}

/// Requirements 19 and 10: one labelled entry per project, in the order the
/// scan reported them — sorted by canonical path.
#[test]
fn every_project_the_scan_reported_has_one_labelled_entry() {
    let mut universe = Universe::scanning(&sources());
    let mut launcher = Launcher::new(None);

    let discovered = WorkspaceEvent::ProjectsDiscovered(vec![
        a_project("alpha"),
        a_project("mike"),
        a_project("zulu"),
    ]);
    universe.apply(discovered.clone());
    launcher.apply(discovered);

    let shown = labels(&universe, &mut launcher);
    let positions: Vec<usize> = ["alpha", "mike", "zulu"]
        .iter()
        .map(|name| {
            shown
                .iter()
                .position(|label| label == name)
                .unwrap_or_else(|| panic!("{name} is labelled: {shown:?}"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "reported in the scan's own order: {shown:?}"
    );
}

/// Requirement 20: a universe with nothing in it tells the engineer how to add
/// a project, rather than showing a blank screen.
#[test]
fn an_empty_universe_tells_the_engineer_how_to_add_a_project() {
    let mut universe = Universe::scanning(&sources());
    let mut launcher = Launcher::new(None);

    let discovered = WorkspaceEvent::ProjectsDiscovered(Vec::new());
    universe.apply(discovered.clone());
    launcher.apply(discovered);

    assert!(labels(&universe, &mut launcher).contains(&empty_state()));
}

/// Requirement 21: a reported failure is visible beside the folder or file it
/// belongs to, and takes nothing else down with it.
#[test]
fn a_reported_failure_is_visible_beside_the_projects_that_were_found() {
    let mut universe = Universe::scanning(&sources());
    let mut launcher = Launcher::new(None);
    let discovered = WorkspaceEvent::ProjectsDiscovered(vec![a_project("alpha")]);
    universe.apply(discovered.clone());
    launcher.apply(discovered);

    let unreadable = WorkspaceEvent::SettingsUnreadable {
        path: sources().settings_file,
        reason: "expected a value".to_owned(),
    };
    universe.apply(unreadable.clone());
    launcher.apply(unreadable);

    let labels = labels(&universe, &mut launcher);
    assert!(
        labels.contains(&"alpha".to_owned()),
        "the projects that were found are still listed: {labels:?}"
    );
    assert!(
        labels.iter().any(|label| {
            label.contains("/Users/engineer/Library/Application Support/Cellworks/settings.toml")
                && label.contains("expected a value")
        }),
        "a notice names the settings file and says why: {labels:?}"
    );
}

/// Requirement 16: the frame loop drains with `try_recv` and never waits, so
/// frames keep running while a scan that has not reported is still going.
#[test]
fn frames_keep_running_while_the_scan_has_not_reported() {
    let (sender, receiver) = mpsc::channel::<WorkspaceEvent>();
    let mut universe = Universe::scanning(&sources());
    let mut launcher = Launcher::new(None);

    {
        let mut harness = Harness::new_ui(|ui| {
            universe.drain(&receiver);
            ui::launcher(ui, &universe, &mut launcher);
        });
        harness.run();
        harness.run();

        assert!(
            labels_in(&harness).contains(&SCANNING.to_owned()),
            "the sender is still alive and silent, so the view waits without blocking: {:?}",
            labels_in(&harness)
        );
    }

    drop(sender);
}
