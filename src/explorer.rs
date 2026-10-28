//! PROTOTYPE: the Explorer, a file tree over the left of the map, ported from
//! `proto/file-tree-navigation` (STORY-089). The lens tabs stay on the top
//! bar here, and L keeps switching the lens; the left and right arrows fold.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use cellworks_map::{Element, Framing, ItemKind, ProjectMap};
use cellworks_workspace::theme::{
    CYAN as FLOW, DIM, INK, LINE, PANEL, TERRAIN_INK as TERRAIN, WARN, WELL as DEEP,
    YELLOW as ATTENTION, YELLOW_SOFT as ATTENTION_SOFT,
};
use egui::{Color32, Rect, Stroke, vec2};

pub const WIDTH: f32 = 320.0;
const MARGIN: f32 = 10.0;
const ROW: f32 = 22.0;
const INDENT: f32 = 14.0;
pub const FLIGHT: f64 = 0.6;

/// Per project root: the files git sees as changed, and the file count under
/// each folder. Prototype: refreshed on the UI thread every few seconds.
struct Stats {
    at: Instant,
    modified: HashSet<PathBuf>,
    files: HashMap<PathBuf, usize>,
    total: usize,
}

static STATS: Mutex<Option<(PathBuf, Stats)>> = Mutex::new(None);

fn stats<T>(map: &ProjectMap, read: impl FnOnce(&Stats) -> T) -> T {
    let mut slot = STATS.lock().unwrap();
    let fresh = slot
        .as_ref()
        .is_some_and(|(root, s)| *root == map.root && s.at.elapsed() < Duration::from_secs(5));
    if !fresh {
        let mut files = HashMap::new();
        let total = count(map, None, &mut files);
        *slot = Some((
            map.root.clone(),
            Stats {
                at: Instant::now(),
                modified: modified(&map.root),
                files,
                total,
            },
        ));
    }
    read(&slot.as_ref().unwrap().1)
}

fn count(map: &ProjectMap, folder: Option<&Path>, files: &mut HashMap<PathBuf, usize>) -> usize {
    let mut n = 0;
    for (_, path) in map.held_by(folder).unwrap_or_default() {
        match map.find(path).map(|f| f.kind) {
            Some(Element::Folder) => n += count(map, Some(path), files),
            Some(Element::Node) => n += 1,
            _ => {}
        }
    }
    if let Some(folder) = folder {
        files.insert(folder.to_owned(), n);
    }
    n
}

fn modified(root: &Path) -> HashSet<PathBuf> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let (Some(top), Some(status)) = (
        git(&["rev-parse", "--show-toplevel"]),
        git(&["status", "--porcelain", "--untracked-files=all", "."]),
    ) else {
        return HashSet::new();
    };
    let top = PathBuf::from(top.trim());
    status
        .lines()
        .filter_map(|line| line.get(3..))
        .map(|path| top.join(path.rsplit(" -> ").next().unwrap_or(path).trim_matches('"')))
        .collect()
}

pub struct Row {
    pub depth: usize,
    pub name: String,
    pub path: PathBuf,
    pub kind: Element,
    pub opens: bool,
    /// Short type tag: the file extension, or fn / ty / use for a symbol.
    pub badge: String,
    /// `M` for a changed file, `•` for a folder holding one.
    pub git: &'static str,
    /// Files under a folder, lines of a file.
    pub meta: String,
    /// The district's colour, shown as a swatch on folder rows.
    pub swatch: Option<Color32>,
}

/// The visible rows: everything under an expanded row, or with a filter
/// every match and its ancestors.
pub fn rows(map: &ProjectMap, expanded: &HashSet<PathBuf>, filter: &str) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let mut out = Vec::new();
    walk(map, None, 0, expanded, !filter.is_empty(), &mut out);
    if filter.is_empty() {
        return out;
    }
    let mut keep = vec![false; out.len()];
    let mut pending: Option<usize> = None;
    for (i, row) in out.iter().enumerate().rev() {
        if row.name.to_lowercase().contains(&filter) {
            keep[i] = true;
            pending = Some(pending.map_or(row.depth, |d| d.min(row.depth)));
        } else if pending.is_some_and(|d| row.depth < d) {
            keep[i] = true;
            pending = Some(row.depth);
        }
    }
    out.into_iter()
        .zip(keep)
        .filter_map(|(row, keep)| keep.then_some(row))
        .collect()
}

fn walk(
    map: &ProjectMap,
    folder: Option<&Path>,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    all: bool,
    out: &mut Vec<Row>,
) {
    let Some(entries) = map.held_by(folder) else {
        return;
    };
    for (name, path) in entries {
        let Some(found) = map.find(path) else {
            continue;
        };
        let open = all || expanded.contains(path);
        match found.kind {
            Element::Folder => {
                let mut r = row(depth, name, path, Element::Folder, true);
                r.swatch = Some(cellworks_map::ui::district_colour_at(map, path));
                stats(map, |s| {
                    r.meta = s.files.get(path).map(usize::to_string).unwrap_or_default();
                    r.git = if s.modified.iter().any(|m| m.starts_with(path)) {
                        "•"
                    } else {
                        ""
                    };
                });
                out.push(r);
                if open {
                    walk(map, Some(path), depth + 1, expanded, all, out);
                }
            }
            Element::Node => {
                let items = map.items_under(path);
                let mut r = row(depth, name, path, Element::Node, !items.is_empty());
                r.badge = path
                    .extension()
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let lines = items.iter().flat_map(|i| &i.floors).map(|f| f.number).max();
                r.meta = lines.map(|n| n.to_string()).unwrap_or_default();
                r.git = if stats(map, |s| s.modified.contains(path)) {
                    "M"
                } else {
                    ""
                };
                out.push(r);
                if open {
                    for item in items.iter().filter(|item| !item.tests) {
                        let mut r = row(depth + 1, &item.name, &item.path, Element::Item, false);
                        r.badge = match item.kind {
                            ItemKind::Function | ItemKind::Test => "fn",
                            ItemKind::Type => "ty",
                            ItemKind::Uses => "use",
                            ItemKind::Section => "§",
                            ItemKind::Text => "",
                        }
                        .to_owned();
                        out.push(r);
                    }
                }
            }
            Element::Item => {}
        }
    }
}

fn row(depth: usize, name: &str, path: &Path, kind: Element, opens: bool) -> Row {
    Row {
        depth,
        name: name.to_owned(),
        path: path.to_owned(),
        kind,
        opens,
        badge: String::new(),
        git: "",
        meta: String::new(),
        swatch: None,
    }
}

/// Expands every folder and node above `path`.
pub fn reveal(map: &ProjectMap, expanded: &mut HashSet<PathBuf>, path: &Path) {
    let mut at = map.find(path).and_then(|found| found.parent);
    while let Some(parent) = at {
        expanded.insert(parent.to_owned());
        at = map.find(parent).and_then(|found| found.parent);
    }
}

/// `target` with its area widened leftwards by the share of `panel` the
/// Explorer covers, so the framed element centres in the uncovered part.
pub fn beside(mut target: Framing, panel: Rect) -> Framing {
    // A user-zoomed camera (fill 1, no closest limit) stays where the user put it.
    let zoomed = target.fill >= 1.0 && target.closest.is_infinite();
    if zoomed || panel.width() <= WIDTH * 2.0 {
        return target;
    }
    let share = (WIDTH + MARGIN) / panel.width();
    target.area.min.x -= target.area.width() * share / (1.0 - share);
    target
}

/// `from` flown `t` (0..=1) of the way towards `to`, eased.
pub fn flown(from: Framing, to: Framing, t: f32) -> Framing {
    let e = 1.0 - (1.0 - t).powi(3);
    if e >= 1.0 {
        return to;
    }
    let lerp = |a: f32, b: f32| a + (b - a) * e;
    Framing {
        area: from.area.lerp_towards(&to.area, e),
        fill: lerp(from.fill, to.fill),
        closest: from.closest.max(to.closest),
        headroom: lerp(from.headroom, to.headroom),
    }
}

#[derive(Default)]
pub struct Picked {
    pub row: Option<PathBuf>,
    pub toggle: Option<PathBuf>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    ctx: &egui::Context,
    panel: Rect,
    map: &ProjectMap,
    selection: Option<&Path>,
    expanded: &HashSet<PathBuf>,
    filter: &mut String,
    scroll_to: bool,
) -> Picked {
    let mut picked = Picked::default();
    let height = (panel.height() - 2.0 * MARGIN).max(120.0);
    egui::Area::new(egui::Id::new("explorer"))
        .fixed_pos(panel.min + vec2(MARGIN, MARGIN))
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(PANEL)
                .stroke(Stroke::new(1.0, LINE))
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(WIDTH - 16.0);
                    ui.set_height(height - 16.0);
                    ui.horizontal(|ui| {
                        cellworks_workspace::theme::eyebrow_label(ui, "Explorer", ATTENTION);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let total = stats(map, |s| s.total);
                            ui.label(egui::RichText::new(format!("{total} files")).monospace().size(11.0).color(TERRAIN));
                        });
                    });
                    ui.add_space(8.0);
                    filter_field(ui, filter);
                    ui.add_space(4.0);
                    let list = rows(map, expanded, filter);
                    egui::Panel::bottom(egui::Id::new("explorer-keys"))
                        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(
                                    "j/k move · ←/→ fold · click toggle · / filter\nL next lens · Esc whole map · E hide",
                                )
                                .monospace()
                                .size(10.5)
                                .color(TERRAIN),
                            );
                        });
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for row in &list {
                                draw_row(ui, row, selection, expanded, scroll_to, &mut picked);
                            }
                        });
                });
        });
    picked
}

/// The filter: a deep field with a `/` prompt. `/` anywhere focuses it, as in vim.
fn filter_field(ui: &mut egui::Ui, filter: &mut String) {
    let id = egui::Id::new("explorer-filter");
    let focused = ui.memory(|m| m.has_focus(id));
    let slash = !focused
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "/"))
        });
    if slash {
        ui.input_mut(|i| {
            i.events
                .retain(|e| !matches!(e, egui::Event::Text(t) if t == "/"))
        });
        ui.memory_mut(|m| m.request_focus(id));
    }
    egui::Frame::new()
        .fill(DEEP)
        .stroke(Stroke::new(
            1.0,
            if focused {
                FLOW.gamma_multiply(0.6)
            } else {
                LINE
            },
        ))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("/").monospace().color(TERRAIN));
                ui.add(
                    egui::TextEdit::singleline(filter)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .hint_text(egui::RichText::new("filter files and symbols").color(TERRAIN))
                        .font(egui::FontId::monospace(12.0))
                        .text_color(INK)
                        .desired_width(f32::INFINITY),
                );
            });
        });
}

fn draw_row(
    ui: &mut egui::Ui,
    row: &Row,
    selection: Option<&Path>,
    expanded: &HashSet<PathBuf>,
    scroll_to: bool,
    picked: &mut Picked,
) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW), egui::Sense::click());
    let selected = selection == Some(row.path.as_path());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 0.0, ATTENTION_SOFT);
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(2.0, ROW)),
            0.0,
            ATTENTION,
        );
        if scroll_to {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
        }
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, LINE);
    }
    let x = rect.min.x + 6.0 + row.depth as f32 * INDENT;
    let y = rect.center().y;
    let font = egui::FontId::monospace(12.5);
    if row.opens {
        let open = expanded.contains(&row.path);
        // Drawn, not text: the UI font has no ▼.
        let c = egui::pos2(x + 3.0, y);
        let points = if open {
            vec![
                c + vec2(-3.5, -2.0),
                c + vec2(3.5, -2.0),
                c + vec2(0.0, 2.5),
            ]
        } else {
            vec![
                c + vec2(-2.0, -3.5),
                c + vec2(-2.0, 3.5),
                c + vec2(2.5, 0.0),
            ]
        };
        painter.add(egui::Shape::convex_polygon(points, DIM, Stroke::NONE));
    }
    let (mut text, colour) = match row.kind {
        Element::Folder => (
            format!("{}/", row.name),
            if selected { ATTENTION } else { INK },
        ),
        Element::Node => (row.name.clone(), if selected { ATTENTION } else { FLOW }),
        Element::Item => (
            format!("ƒ {}", row.name),
            if selected { ATTENTION } else { DIM },
        ),
    };
    let mut name_x = x + 14.0;
    if let Some(swatch) = row.swatch {
        let tag = Rect::from_min_size(egui::pos2(name_x, y - 6.0), vec2(12.0, 12.0));
        painter.rect_filled(tag, 0.0, swatch.gamma_multiply(0.25));
        painter.rect_stroke(tag, 0.0, Stroke::new(1.0, swatch), egui::StrokeKind::Inside);
        name_x = tag.max.x + 6.0;
    }
    if !row.badge.is_empty() {
        let badge_font = egui::FontId::monospace(9.0);
        let galley = painter.layout_no_wrap(row.badge.clone(), badge_font, DIM);
        let tag = Rect::from_min_size(
            egui::pos2(name_x, y - 7.0),
            vec2((galley.size().x + 6.0).max(16.0), 14.0),
        );
        painter.rect_stroke(tag, 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        let ink = match row.badge.as_str() {
            "fn" => FLOW,
            "ty" => Color32::from_rgb(0xc3, 0x7c, 0xff),
            _ => DIM,
        };
        painter.text(
            tag.center(),
            egui::Align2::CENTER_CENTER,
            &row.badge,
            egui::FontId::monospace(9.0),
            ink,
        );
        name_x = tag.max.x + 6.0;
    }
    if row.kind == Element::Item {
        text = row.name.clone();
    }
    painter.text(
        egui::pos2(name_x, y),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        colour,
    );
    let mut right = rect.max.x - 8.0;
    if !row.meta.is_empty() {
        let r = painter.text(
            egui::pos2(right, y),
            egui::Align2::RIGHT_CENTER,
            &row.meta,
            egui::FontId::monospace(11.0),
            TERRAIN,
        );
        right = r.min.x - 8.0;
    }
    if !row.git.is_empty() {
        painter.text(
            egui::pos2(right, y),
            egui::Align2::RIGHT_CENTER,
            row.git,
            egui::FontId::monospace(11.0),
            WARN,
        );
    }
    if response.clicked() {
        let on_chevron = response
            .interact_pointer_pos()
            .is_some_and(|p| p.x < x + 12.0);
        if row.opens && (on_chevron || selected) {
            picked.toggle = Some(row.path.clone());
        }
        if !on_chevron {
            picked.row = Some(row.path.clone());
        }
    }
}
