//! Drawing the Explorer beside the map — the open project's whole file tree,
//! grouped by the active lens, searchable, tracking folds and uncommitted
//! changes — and the hint bar naming where each arrow leads from a faced
//! facade.

use std::collections::HashSet;
use std::path::PathBuf;

use cellworks_map::ui::lens_help;
use cellworks_map::{Lens, ProjectMap};
use egui::accesskit::Toggled;
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Shape, Stroke, pos2, vec2};

use crate::Navigation;
use crate::explorer::{Badge, Change, Explorer, Row, SymbolBadge};

/// Terrain ink: the same colour [`hints`] uses for its bar, and the
/// Explorer's own key-hint footer.
const INK: Color32 = Color32::from_rgb(0xd7, 0xe2, 0xea);
const DIM: Color32 = Color32::from_rgb(0x7d, 0x8f, 0xa0);
const PANEL: Color32 = Color32::from_rgb(0x0d, 0x11, 0x17);
const LINE_STRONG: Color32 = Color32::from_rgb(0x34, 0x50, 0x6a);
const FLOW: Color32 = Color32::from_rgb(0x5e, 0xf6, 0xff);
const MODIFIED: Color32 = Color32::from_rgb(0xff, 0x9f, 0x1c);

/// How tall one row stands.
const ROW_HEIGHT: f32 = 20.0;

/// How far one depth of indent moves a row's badge and name.
const INDENT: f32 = 14.0;

/// Room left below the rows for the separator and the two-line key-hint
/// footer, so the rows' `ScrollArea` never claims that space for itself.
const FOOTER_HEIGHT: f32 = 50.0;

/// What the Explorer panel did this frame: a row was clicked (frame and
/// select it, the same way moving the cursor would), or a lens chip was
/// clicked. Folding a row and editing the filter are applied directly to the
/// `Explorer` passed to [`explorer`], since they touch nothing the binary
/// needs to orchestrate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExplorerAction {
    pub clicked: Option<PathBuf>,
    pub lens: Option<Lens>,
}

/// Draws the Explorer: the header ("EXPLORER" and the file count), one row of
/// lens chips, the filter field, the rows `explorer.rows(map, changed)` lists
/// and a two-line key-hint footer. Toggles a folder row's fold and keeps the
/// filter in `explorer` directly; returns what the binary must still act
/// on — a row picked (framed and selected the way the cursor moving there
/// would) or a lens picked. `focus_filter` asks the filter field to take the
/// keyboard focus this frame (STORY-089's own "/" key).
///
/// The whole panel is one accessibility container labelled "Explorer", so
/// `tests/common::labels()` can skip it; each row is its own toggle button,
/// labelled with its name and toggled when it is the selection.
#[allow(clippy::too_many_arguments)]
pub fn explorer(
    ui: &mut egui::Ui,
    map: &ProjectMap,
    navigation: &Navigation,
    changed: &HashSet<PathBuf>,
    lens: Lens,
    offered: &[Lens],
    files: usize,
    explorer: &mut Explorer,
    focus_filter: bool,
) -> ExplorerAction {
    let mut action = ExplorerAction::default();
    let frame = egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE_STRONG))
        .inner_margin(10.0);
    let outer = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new("EXPLORER").strong().color(INK));
            ui.label(RichText::new(format!("{files} files")).color(DIM));
        });
        ui.add_space(6.0);
        if !offered.is_empty() {
            ui.columns(offered.len(), |columns| {
                for (column, candidate) in columns.iter_mut().zip(offered) {
                    if chip(column, *candidate, *candidate == lens).clicked() {
                        action.lens = Some(*candidate);
                    }
                }
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("/").color(DIM).monospace());
            let id = ui.id().with("explorer filter");
            let mut text = explorer.filter().to_owned();
            let response = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id(id)
                    .hint_text("filter files and symbols")
                    .desired_width(ui.available_width()),
            );
            if focus_filter {
                response.request_focus();
            }
            if response.changed() {
                explorer.set_filter(text);
            }
        });
        ui.add_space(6.0);
        let scroll_to = explorer.take_scroll_to();
        // Shrinks to its rows rather than claiming the rest of the forced
        // panel height (STORY-089's own binary sizes this tall enough to
        // reach other panels — a session pane, say — beside the map; a
        // `ScrollArea` filling blank space below the rows would otherwise
        // sense drags there and swallow clicks meant for whatever sits
        // beneath the overlay at that point).
        egui::ScrollArea::vertical()
            .id_salt("explorer rows")
            .auto_shrink([false, true])
            .max_height(ui.available_height() - FOOTER_HEIGHT)
            .show(ui, |ui| {
                for row in explorer.rows(map, changed) {
                    let selected = navigation.selection() == Some(row.path.as_path());
                    let response = draw_row(ui, &row, selected);
                    if Some(&row.path) == scroll_to.as_ref() {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        if row.foldable {
                            explorer.fold(&row.path);
                        }
                        action.clicked = Some(row.path.clone());
                    }
                }
            });
        ui.add_space(6.0);
        footer(ui);
        // Reserves the rest of the forced panel height for the Frame's own
        // opaque background, with no sense attached, so it paints the full
        // column without intercepting anything below it.
        ui.allocate_space(vec2(ui.available_width(), ui.available_height()));
    });
    ui.ctx().accesskit_node_builder(outer.response.id, |node| {
        node.set_role(egui::accesskit::Role::Group);
        node.set_label("Explorer");
    });
    action
}

/// A lens chip: `candidate`'s lowercase name, marked when it is the active
/// lens, explaining itself on hover (STORY-055 finds it by `candidate.name()`
/// and reads [`lens_help`] from its hover text).
fn chip(ui: &mut egui::Ui, candidate: Lens, on: bool) -> egui::Response {
    let text = if on {
        RichText::new(candidate.name()).strong().color(INK)
    } else {
        RichText::new(candidate.name()).color(DIM)
    };
    let fill = if on {
        ui.visuals().selection.bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.add_sized(
        vec2(ui.available_width(), ui.spacing().interact_size.y),
        egui::Button::new(text).fill(fill),
    )
    .on_hover_text(lens_help(candidate))
}

/// The two-line key-hint footer, in terrain ink, key names spelled as text.
fn footer(ui: &mut egui::Ui) {
    ui.separator();
    ui.label(
        RichText::new("j/k move   l unfold   h fold/parent   Shift+L lens")
            .color(INK)
            .monospace()
            .size(11.0),
    );
    ui.label(
        RichText::new("/ filter   Enter open   Esc back   Cmd+B hide")
            .color(INK)
            .monospace()
            .size(11.0),
    );
}

/// Draws one row: its indent, a chevron when it is foldable, its badge, its
/// name, a change mark and its count. The whole row is a toggle button,
/// labelled with `row.name` and toggled when `selected`, so it reads the same
/// way the removed `tree()`'s rows did.
fn draw_row(ui: &mut egui::Ui, row: &Row, selected: bool) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        if selected {
            painter.rect_filled(
                rect,
                2.0,
                ui.visuals().selection.bg_fill.gamma_multiply(0.5),
            );
        } else if response.hovered() {
            painter.rect_filled(rect, 2.0, Color32::WHITE.gamma_multiply(0.05));
        }
        let mut x = rect.min.x + row.depth as f32 * INDENT;
        if row.foldable {
            painter.add(chevron(pos2(x + 5.0, rect.center().y), row.unfolded));
        }
        x += 14.0;
        x = draw_badge(&painter, x, rect.center().y, row);
        if row.change != Change::None {
            let colour = match row.change {
                Change::Modified => MODIFIED,
                Change::Holds => DIM,
                Change::None => Color32::TRANSPARENT,
            };
            painter.circle_filled(pos2(rect.max.x - 34.0, rect.center().y), 3.0, colour);
        }
        if let Some(count) = row.count {
            painter.text(
                pos2(rect.max.x - 10.0, rect.center().y),
                Align2::RIGHT_CENTER,
                count.to_string(),
                FontId::monospace(10.0),
                DIM,
            );
        }
        let ink = if selected { INK } else { DIM };
        painter.text(
            pos2(x, rect.center().y),
            Align2::LEFT_CENTER,
            &row.name,
            FontId::proportional(13.0),
            ink,
        );
    }
    let description = row_description(row);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Button);
        node.set_label(row.name.clone());
        node.set_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        });
        if let Some(description) = description {
            node.set_description(description);
        }
    });
    response
}

/// A chevron shape at `at`, pointing down while `unfolded`, right otherwise.
fn chevron(at: egui::Pos2, unfolded: bool) -> Shape {
    let size = 5.0;
    let points = if unfolded {
        vec![
            at + vec2(-size, -size * 0.6),
            at + vec2(size, -size * 0.6),
            at + vec2(0.0, size * 0.6),
        ]
    } else {
        vec![
            at + vec2(-size * 0.6, -size),
            at + vec2(-size * 0.6, size),
            at + vec2(size * 0.6, 0.0),
        ]
    };
    Shape::convex_polygon(points, DIM, Stroke::NONE)
}

/// Draws `row`'s badge at `x`, `y` and returns the x past it: a district
/// colour swatch for a folder, a short extension for a file, or a symbol
/// marker (fn / ty / use / §) for an item.
fn draw_badge(painter: &egui::Painter, x: f32, y: f32, row: &Row) -> f32 {
    match &row.badge {
        Badge::Folder => {
            if let Some(colour) = row.colour {
                let swatch = Rect::from_center_size(pos2(x + 5.0, y), vec2(10.0, 10.0));
                painter.rect_filled(swatch, 2.0, colour);
            }
            x + 18.0
        }
        Badge::Extension(ext) => {
            let text = if ext.is_empty() {
                "-".to_owned()
            } else {
                ext.to_uppercase()
            };
            painter.text(
                pos2(x, y),
                Align2::LEFT_CENTER,
                text,
                FontId::monospace(9.0),
                DIM,
            );
            x + 30.0
        }
        Badge::Symbol(kind) => {
            let text = match kind {
                SymbolBadge::Function => "fn",
                SymbolBadge::Type => "ty",
                SymbolBadge::Uses => "use",
                SymbolBadge::Section => "§",
            };
            painter.text(
                pos2(x, y),
                Align2::LEFT_CENTER,
                text,
                FontId::monospace(9.0),
                FLOW,
            );
            x + 30.0
        }
    }
}

/// `row`'s accessible description: its count and its change mark, when it
/// has either; `None` when it has neither, so the caller falls back to the
/// row's own value.
fn row_description(row: &Row) -> Option<String> {
    let count = row.count.map(|count| count.to_string());
    let change = match row.change {
        Change::Modified => Some("modified".to_owned()),
        Change::Holds => Some("holds changes".to_owned()),
        Change::None => None,
    };
    match (count, change) {
        (Some(count), Some(change)) => Some(format!("{count}, {change}")),
        (Some(count), None) => Some(count),
        (None, Some(change)) => Some(change),
        (None, None) => None,
    }
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
