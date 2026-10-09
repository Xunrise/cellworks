//! Painting one project map.
//!
//! The only module of this crate that touches an egui widget. Every name is a
//! `Label` placed at its rectangle rather than painted text, so the frame's
//! accessibility tree carries one entry per district, sub-district, group and
//! node.

use egui::emath::RectTransform;
use egui::{Rect, Stroke, StrokeKind};

use crate::graph::{Group, Node, ProjectMap};
use crate::layout::TITLE;

/// Draws `map` scaled uniformly to fit the space left in `ui`, centred.
/// Draws nothing for an empty map, which has no extent to fit.
pub fn project(ui: &mut egui::Ui, map: &ProjectMap) {
    if map.is_empty() {
        return;
    }
    let panel = ui.available_rect_before_wrap();
    let to_screen = fit(map.extent, panel);
    let stroke = ui.visuals().widgets.noninteractive.fg_stroke;

    for district in &map.districts {
        boxed(ui, &to_screen, district.area, &district.name, stroke);
        for sub in &district.sub_districts {
            boxed(ui, &to_screen, sub.area, &sub.name, stroke);
            for group in &sub.groups {
                group_with_children(ui, &to_screen, group, stroke);
            }
            nodes(ui, &to_screen, &sub.nodes);
        }
        nodes(ui, &to_screen, &district.nodes);
    }
}

/// The transform that scales `extent` uniformly into `panel` and centres it.
/// Never scales up past the layout's own size.
pub(crate) fn fit(extent: Rect, panel: Rect) -> RectTransform {
    let scale = (panel.width() / extent.width())
        .min(panel.height() / extent.height())
        .min(1.0);
    let target = Rect::from_center_size(panel.center(), extent.size() * scale);
    RectTransform::from_to(extent, target)
}

fn group_with_children(
    ui: &mut egui::Ui,
    to_screen: &RectTransform,
    group: &Group,
    stroke: Stroke,
) {
    boxed(ui, to_screen, group.area, &group.name, stroke);
    for inner in &group.groups {
        group_with_children(ui, to_screen, inner, stroke);
    }
    nodes(ui, to_screen, &group.nodes);
}

fn boxed(ui: &mut egui::Ui, to_screen: &RectTransform, area: Rect, name: &str, stroke: Stroke) {
    let rect = to_screen.transform_rect(area);
    ui.painter()
        .rect_stroke(rect, 4.0, stroke, StrokeKind::Inside);
    let title = Rect::from_min_size(
        rect.min,
        egui::vec2(rect.width(), TITLE * to_screen.scale().y),
    );
    ui.put(title, egui::Label::new(name).truncate());
}

fn nodes(ui: &mut egui::Ui, to_screen: &RectTransform, nodes: &[Node]) {
    let fill = ui.visuals().faint_bg_color;
    for node in nodes {
        let rect = to_screen.transform_rect(node.area);
        ui.painter().rect_filled(rect, 2.0, fill);
        ui.put(rect, egui::Label::new(node.name.as_str()).truncate());
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{Directories, default_prefix_rule};

    #[test]
    fn every_district_lies_inside_the_panel_it_is_fitted_to() {
        let files: Vec<PathBuf> = (0..300)
            .map(|n| PathBuf::from(format!("src/d{}/s{}/f{n}.rs", n % 7, n % 3)))
            .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of("shop", Path::new("/p/shop"), &files, &strategy);
        let panel = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(400.0, 300.0));

        let to_screen = fit(map.extent, panel);

        for district in &map.districts {
            let drawn = to_screen.transform_rect(district.area);
            assert!(
                panel.expand(0.01).contains_rect(drawn),
                "{drawn:?} is outside {panel:?}"
            );
        }
    }
}
