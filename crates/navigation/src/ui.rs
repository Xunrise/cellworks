//! Drawing the tree: the current layer as a list beside the map.

use std::path::{Path, PathBuf};

use cellworks_map::ProjectMap;

use crate::Navigation;

/// Lists what the current layer of `map` holds, one row each, the selection
/// highlighted, and returns the path of the row clicked this frame. An empty
/// layer says so rather than listing nothing.
pub fn tree(ui: &mut egui::Ui, map: &ProjectMap, navigation: &Navigation) -> Option<PathBuf> {
    let listed = navigation.listed(map);
    ui.heading(listed);
    let rows = navigation.rows(map);
    if rows.is_empty() {
        ui.label(format!("{listed} holds nothing."));
        return None;
    }
    let mut clicked = None;
    for (name, path) in rows {
        let selected = navigation.selection() == Some(path);
        if ui.selectable_label(selected, name).clicked() {
            clicked = Some(Path::to_path_buf(path));
        }
    }
    clicked
}
