//! Drawing the tree, the current layer as a list beside the map, and the hint
//! bar naming where each arrow leads from a faced facade.

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

/// The bar at the bottom of `panel` naming where each arrow leads from the
/// faced facade: `·` where a direction ends.
pub fn hints(ui: &mut egui::Ui, panel: egui::Rect, hints: &crate::Hints) {
    let name = |to: &Option<String>| to.clone().unwrap_or_else(|| "·".to_owned());
    let text = format!(
        "← {}    [ {} ]    {} →      ↑ behind: {}    ↓ in front: {}",
        name(&hints.left),
        hints.here,
        name(&hints.right),
        name(&hints.behind),
        name(&hints.front),
    );
    let painter = ui.painter_at(panel);
    let ink = egui::Color32::from_rgb(0xd7, 0xe2, 0xea);
    let galley = painter.layout_no_wrap(text.clone(), egui::FontId::monospace(12.0), ink);
    let at = egui::pos2(panel.center().x - galley.size().x / 2.0, panel.max.y - 34.0);
    let bar = egui::Rect::from_min_size(at, galley.size()).expand2(egui::vec2(12.0, 6.0));
    let fill = egui::Color32::from_rgb(0x0d, 0x11, 0x17).gamma_multiply(0.92);
    let edge = egui::Stroke::new(1.0, egui::Color32::from_rgb(0x34, 0x50, 0x6a));
    painter.rect(bar, 0.0, fill, edge, egui::StrokeKind::Inside);
    painter.galley(at, galley, ink);
    // Named for the accessibility tree, so it can be read out.
    let response = ui.interact(bar, ui.id().with("hints"), egui::Sense::hover());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Status);
        node.set_label(text);
        let bounds = egui::accesskit::Rect {
            x0: bar.min.x.into(),
            y0: bar.min.y.into(),
            x1: bar.max.x.into(),
            y1: bar.max.y.into(),
        };
        node.set_bounds(bounds);
    });
}
