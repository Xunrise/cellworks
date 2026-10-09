//! The binary's own share of STORY-002: its frame body renders the Workspace
//! universe, and the frame loop drains the scan without ever waiting for it.
//!
//! This replaces `tests/empty_window.rs`, which proved STORY-001's empty
//! central panel — the seam STORY-001's own spec said the map canvas would
//! replace, and which is false the moment the universe draws a project.
//!
//! The frame runs through `cellworks::draw` and an `egui_kittest` harness, so
//! it needs neither a GPU adapter nor a display, and it never opens a window:
//! nothing here constructs `CellworksApp` through eframe or touches winit.
//! `Sources::for_this_user` is never called, so no test reads the user's
//! home directory.

use std::path::PathBuf;
use std::sync::mpsc;

use cellworks::app::CellworksApp;
use cellworks_workspace::{Project, Sources, Universe, WorkspaceEvent};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

/// Shown until the first result arrives.
const SCANNING: &str = "Scanning for projects…";

/// Two paths with nothing behind them: no frame below reads the filesystem.
fn sources() -> Sources {
    Sources {
        projects_folder: PathBuf::from("/Users/engineer/Projects"),
        settings_file: PathBuf::from(
            "/Users/engineer/Library/Application Support/Cellworks/settings.toml",
        ),
    }
}

/// The text of every labelled entry one frame of `cellworks::draw` put on the
/// screen. egui carries a label's text in the node's value.
fn labels(app: &mut CellworksApp) -> Vec<String> {
    let mut harness = Harness::new_ui(|ui| cellworks::draw(ui, app));
    harness.run();

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

#[test]
fn the_frame_body_renders_the_workspace_universe() {
    let (sender, receiver) = mpsc::channel();
    let mut app = CellworksApp::over(Universe::scanning(&sources()), receiver);

    assert_eq!(
        labels(&mut app),
        vec![SCANNING.to_owned()],
        "before anything is drained the frame shows the scanning state"
    );

    sender
        .send(WorkspaceEvent::ProjectsDiscovered(vec![Project::at(
            PathBuf::from("/Users/engineer/Projects/alpha"),
        )]))
        .expect("the app is still listening");
    app.apply_pending();

    assert_eq!(
        labels(&mut app),
        vec!["alpha".to_owned()],
        "once the scan has reported, the frame lists its projects"
    );
}

#[test]
fn draining_an_empty_channel_returns_rather_than_waiting() {
    let (sender, receiver) = mpsc::channel::<WorkspaceEvent>();
    let mut app = CellworksApp::over(Universe::scanning(&sources()), receiver);

    // The sender is alive and silent: a `recv` here would never return.
    app.apply_pending();

    assert_eq!(labels(&mut app), vec![SCANNING.to_owned()]);
    drop(sender);
}

#[test]
fn a_scan_thread_that_goes_away_before_reporting_leaves_the_scanning_state() {
    let (sender, receiver) = mpsc::channel::<WorkspaceEvent>();
    let mut app = CellworksApp::over(Universe::scanning(&sources()), receiver);
    drop(sender);

    app.apply_pending();

    assert_eq!(
        labels(&mut app),
        vec!["Could not finish scanning for projects.".to_owned()],
        "rather than showing \"Scanning\" for ever"
    );
}
