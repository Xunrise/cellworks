//! Painting one project map.
//!
//! The only module of this crate that touches an egui widget. Every name is a
//! `Label` placed at its rectangle rather than painted text, so the frame's
//! accessibility tree carries one entry per district, sub-district, group and
//! node. A click on a name is reported back; the selected element is outlined
//! and its label marked selected, so the selection is visible both on screen
//! and in the accessibility tree.

use std::path::{Path, PathBuf};

use egui::emath::RectTransform;
use egui::{Rect, Sense, Stroke, StrokeKind};

use crate::Framing;
use crate::graph::{Group, Node, ProjectMap};
use crate::layout::TITLE;

/// What one frame of the map is drawn with: where it goes and what is
/// selected. The clicked element is written back to `clicked`.
struct Painter<'a> {
    to_screen: RectTransform,
    stroke: Stroke,
    selected: Option<&'a Path>,
    /// The folder whose folder view is drawn; everything outside it is not.
    folder_view: Option<&'a Path>,
    clicked: Option<PathBuf>,
}

/// One frame of the map as drawn.
pub struct Drawn {
    /// The path of the element whose name was clicked, if any.
    pub clicked: Option<PathBuf>,
    /// From the part of the map in view, at the layout's scale, to the space
    /// it was drawn in on screen.
    pub view: RectTransform,
}

/// Draws `map` with `framing` fitted to the space left in `ui`, outlines the
/// element at `selected`, and reports the element whose name was clicked this
/// frame. With a `folder_view`, only that folder and what it holds are drawn.
/// Draws nothing for an empty map.
pub fn project(
    ui: &mut egui::Ui,
    map: &ProjectMap,
    framing: Framing,
    selected: Option<&Path>,
    folder_view: Option<&Path>,
) -> Drawn {
    let panel = ui.available_rect_before_wrap();
    let to_screen = fit(framing, panel);
    let view = RectTransform::from_to(to_screen.inverse().transform_rect(panel), panel);
    if map.is_empty() {
        return Drawn {
            clicked: None,
            view,
        };
    }
    let mut painter = Painter {
        to_screen,
        stroke: ui.visuals().widgets.noninteractive.fg_stroke,
        selected,
        folder_view,
        clicked: None,
    };

    for district in &map.districts {
        let shown = painter.boxed(ui, district.area, &district.name, &district.path, false);
        for sub in &district.sub_districts {
            let shown = painter.boxed(ui, sub.area, &sub.name, &sub.path, shown);
            for group in &sub.groups {
                painter.group_with_children(ui, group, shown);
            }
            painter.nodes(ui, &sub.nodes, shown);
        }
        painter.nodes(ui, &district.nodes, shown);
    }
    Drawn {
        clicked: painter.clicked,
        view,
    }
}

/// The transform that scales `framing.area` uniformly to fill `framing.fill`
/// of `panel`, centred. Never scales up past the layout's own size unless
/// `framing.scale_up` allows it.
pub(crate) fn fit(framing: Framing, panel: Rect) -> RectTransform {
    let area = framing.area;
    let mut scale = (panel.width() * framing.fill / area.width())
        .min(panel.height() * framing.fill / area.height());
    if !framing.scale_up {
        scale = scale.min(1.0);
    }
    let target = Rect::from_center_size(panel.center(), area.size() * scale);
    RectTransform::from_to(area, target)
}

impl Painter<'_> {
    fn group_with_children(&mut self, ui: &mut egui::Ui, group: &Group, inside: bool) {
        let shown = self.boxed(ui, group.area, &group.name, &group.path, inside);
        for inner in &group.groups {
            self.group_with_children(ui, inner, shown);
        }
        self.nodes(ui, &group.nodes, shown);
    }

    /// Draws the folder at `path` if it is shown: when its parent is shown
    /// (`inside`), it is the folder view, or there is no folder view. Returns
    /// whether it was, which is whether what it holds is shown.
    fn boxed(
        &mut self,
        ui: &mut egui::Ui,
        area: Rect,
        name: &str,
        path: &Path,
        inside: bool,
    ) -> bool {
        let shown = inside || self.folder_view.is_none_or(|folder| folder == path);
        if !shown {
            return false;
        }
        let rect = self.to_screen.transform_rect(area);
        ui.painter()
            .rect_stroke(rect, 4.0, self.stroke, StrokeKind::Inside);
        let title = Rect::from_min_size(
            rect.min,
            egui::vec2(rect.width(), TITLE * self.to_screen.scale().y),
        );
        self.label(ui, rect, title, name, path);
        true
    }

    fn nodes(&mut self, ui: &mut egui::Ui, nodes: &[Node], shown: bool) {
        if !shown {
            return;
        }
        let fill = ui.visuals().faint_bg_color;
        for node in nodes {
            let rect = self.to_screen.transform_rect(node.area);
            ui.painter().rect_filled(rect, 2.0, fill);
            self.label(ui, rect, rect, &node.name, &node.path);
        }
    }

    /// Puts `name` at `at`, outlines `outline` when `path` is selected, and
    /// notes a click on it.
    fn label(&mut self, ui: &mut egui::Ui, outline: Rect, at: Rect, name: &str, path: &Path) {
        // The sense goes on the response, not the widget, so the entry stays a
        // label in the accessibility tree.
        let response = ui
            .put(at, egui::Label::new(name).truncate())
            .interact(Sense::click());
        if self.selected == Some(path) {
            ui.painter().rect_stroke(
                outline,
                4.0,
                ui.visuals().selection.stroke,
                StrokeKind::Outside,
            );
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_selected(true));
        }
        if response.clicked() {
            self.clicked = Some(path.to_path_buf());
        }
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

        let to_screen = fit(
            Framing {
                area: map.extent,
                fill: 1.0,
                scale_up: false,
            },
            panel,
        );

        for district in &map.districts {
            let drawn = to_screen.transform_rect(district.area);
            assert!(
                panel.expand(0.01).contains_rect(drawn),
                "{drawn:?} is outside {panel:?}"
            );
        }
    }

    #[test]
    fn a_framing_that_may_scale_up_fills_its_share_of_the_panel() {
        let panel = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 500.0));
        let area = Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(100.0, 20.0));

        let close = fit(
            Framing {
                area,
                fill: 0.5,
                scale_up: true,
            },
            panel,
        )
        .transform_rect(area);
        let capped = fit(
            Framing {
                area,
                fill: 0.5,
                scale_up: false,
            },
            panel,
        )
        .transform_rect(area);

        assert!((close.width() - 500.0).abs() < 0.01, "{close:?}");
        assert_eq!(close.center(), panel.center());
        assert!((capped.width() - 100.0).abs() < 0.01, "{capped:?}");
    }
}
