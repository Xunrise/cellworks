//! The window's content is empty: one frame of the app's frame body puts no
//! widget on the screen.
//!
//! The frame runs through `cellworks::draw` and an `egui_kittest` harness, so
//! it needs neither a GPU adapter nor a display, and it never opens a window:
//! nothing here constructs `CellworksApp` through eframe or touches winit.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::NodeT;

/// Structure rather than content: the window itself and the containers egui
/// wraps a panel in. Any other role in the tree would be a widget.
const CONTAINER_ROLES: [Role; 2] = [Role::Window, Role::GenericContainer];

#[test]
fn one_frame_of_the_app_contains_no_widget() {
    let mut harness = Harness::new_ui(cellworks::draw);
    harness.run();

    let root = harness.root();
    let mut found: Vec<String> = Vec::new();

    for node in std::iter::once(root).chain(root.children_recursive()) {
        let node = node.accesskit_node();

        if !CONTAINER_ROLES.contains(&node.role()) {
            found.push(format!("{:?}", node.role()));
        }
        assert_eq!(node.label(), None, "a node carries a label");
        assert_eq!(node.value(), None, "a node carries a value");
    }

    assert!(
        found.is_empty(),
        "the frame is empty but for its containers; found {found:?}"
    );
}
