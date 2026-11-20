//! The flat view: the same map seen from straight above, in the language of
//! a factory floor. Districts and folders are factories, files are machines
//! outlined by their kind, and functions are crates coloured by how
//! branching they are; the lens's lanes run between the machines as belts
//! with goods flowing from what is used to what uses it.
//!
//! It shares the city's ground: every rectangle is the layout's own, only
//! projected without height (see [`crate::fit_flat`]). Names are `Label`s, as
//! in the city, so the accessibility tree carries one entry per district,
//! folder and file.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{
    Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Vec2, pos2,
    vec2,
};

use super::lanes::{self, Focus};
use super::{
    ATTENTION, DIM, Drawn, DrawnTower, EDGE, GROUND, IMPACTED, INK, LINE_STRONG, NIGHT, OUTSIDE,
    Outline, PANEL, REVIEW_RECEDED, Span, ViewOptions, WRITTEN, district_colour, kind_colour,
    links, orbits, roads, tip,
};
use crate::graph::{Group, Item, ItemKind, Node, NodeKind, ProjectMap};
use crate::view::{View, fit_flat};
use crate::{LaneKind, TowerReview};

/// The body of a machine, the dark ink written on a coloured header, and
/// the amber of a file git says changed.
const BODY: Color32 = Color32::from_rgb(0x14, 0x1a, 0x22);
const ON_ACCENT: Color32 = Color32::from_rgb(0x07, 0x09, 0x0c);
const CHANGED: Color32 = Color32::from_rgb(0xff, 0xb3, 0x5c);

/// The colours of complexity: plain, branching, tangled, knotted.
const CX_PLAIN: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x84);
const CX_BRANCHING: Color32 = Color32::from_rgb(0xfc, 0xee, 0x0a);
const CX_TANGLED: Color32 = Color32::from_rgb(0xff, 0x9f, 0x1c);
const CX_KNOTTED: Color32 = Color32::from_rgb(0xff, 0x3b, 0x5c);

/// The share of the panel what is framed fills.
const MARGIN_FILL: f32 = 0.92;

/// The height of one crate row in a machine read in full.
const ROW: f32 = 22.0;

/// The colour of a crate of complexity `cx`; `None` for an item that is not
/// a function.
pub fn complexity_colour(cx: Option<u32>) -> Color32 {
    match cx {
        None => DIM,
        Some(0..=5) => CX_PLAIN,
        Some(6..=10) => CX_BRANCHING,
        Some(11..=20) => CX_TANGLED,
        Some(_) => CX_KNOTTED,
    }
}

/// The cyclomatic complexity of a function or test, from its lines: one, and
/// one more for every branch the lines read. `None` for anything else.
///
/// ponytail: counts branch keywords and boolean operators in the text, not
/// the syntax tree; swap for a tree-sitter count in code-analysis if the
/// numbers start to matter.
pub fn complexity(item: &Item) -> Option<u32> {
    if !matches!(item.kind, ItemKind::Function | ItemKind::Test) {
        return None;
    }
    const BRANCHES: [&str; 9] = [
        "if", "for", "while", "loop", "case", "catch", "elif", "except", "when",
    ];
    let mut cx = 1;
    for floor in &item.floors {
        let text = floor.text.trim_start();
        if text.starts_with("//") || text.starts_with('#') {
            continue;
        }
        cx += text
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|word| BRANCHES.contains(word))
            .count() as u32;
        cx += (text.matches("&&").count() + text.matches("||").count()) as u32;
    }
    Some(cx)
}

/// How a machine's outline is cut, by the kind of its file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileShape {
    /// Code: slightly rounded.
    Code,
    /// Tests and specifications: rounded.
    Test,
    /// Prose: a page with a folded corner.
    Doc,
    /// Settings, data and generated files: chamfered.
    Config,
    /// What builds or ships: an arrow.
    Script,
    /// Contracts, schemas and pictures: a pill.
    Pill,
}

/// The outline a node of `kind` named `name` is cut in.
pub fn shape_of(kind: NodeKind, name: &str) -> FileShape {
    let name = name.to_lowercase();
    match kind {
        NodeKind::Code
            if name.contains("test") || name.contains(".spec.") || name.starts_with("story_") =>
        {
            FileShape::Test
        }
        NodeKind::Code => FileShape::Code,
        NodeKind::Specification => FileShape::Test,
        NodeKind::Document => FileShape::Doc,
        NodeKind::Configuration
        | NodeKind::Generated
        | NodeKind::Secrets
        | NodeKind::SecretsExample => FileShape::Config,
        NodeKind::Build | NodeKind::Infrastructure => FileShape::Script,
        NodeKind::Contract | NodeKind::Schema | NodeKind::Image => FileShape::Pill,
    }
}

/// The convex outline of `r` on screen cut as `shape`, clockwise from the
/// top left. Corner features are capped in screen points, so they stay
/// subtle close up.
pub fn outline(shape: FileShape, r: Rect) -> Vec<Pos2> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let m = r.width().min(r.height());
    let rounded = |radius: f32| {
        let mut points = Vec::with_capacity(28);
        for (centre, from) in [
            (pos2(r.min.x + radius, r.min.y + radius), PI),
            (pos2(r.max.x - radius, r.min.y + radius), -FRAC_PI_2),
            (pos2(r.max.x - radius, r.max.y - radius), 0.0),
            (pos2(r.min.x + radius, r.max.y - radius), FRAC_PI_2),
        ] {
            for k in 0..=6 {
                let a = from + k as f32 / 6.0 * FRAC_PI_2;
                points.push(centre + radius * vec2(a.cos(), a.sin()));
            }
        }
        points
    };
    match shape {
        FileShape::Code => rounded((m * 0.08).min(7.0)),
        FileShape::Test => rounded((m * 0.25).min(18.0)),
        FileShape::Pill => rounded((m * 0.5).min(22.0)),
        FileShape::Doc => {
            let c = (m * 0.18).min(18.0);
            vec![
                r.min,
                pos2(r.max.x - c, r.min.y),
                pos2(r.max.x, r.min.y + c),
                r.max,
                pos2(r.min.x, r.max.y),
            ]
        }
        FileShape::Config => {
            let c = (m * 0.12).min(12.0);
            vec![
                pos2(r.min.x + c, r.min.y),
                pos2(r.max.x - c, r.min.y),
                pos2(r.max.x, r.min.y + c),
                pos2(r.max.x, r.max.y - c),
                pos2(r.max.x - c, r.max.y),
                pos2(r.min.x + c, r.max.y),
                pos2(r.min.x, r.max.y - c),
                pos2(r.min.x, r.min.y + c),
            ]
        }
        FileShape::Script => {
            let c = (r.width() * 0.08).min(m * 0.5).min(16.0);
            vec![
                r.min,
                pos2(r.max.x - c, r.min.y),
                pos2(r.max.x, r.center().y),
                pos2(r.max.x - c, r.max.y),
                pos2(r.min.x, r.max.y),
                pos2(r.min.x + c, r.center().y),
            ]
        }
    }
}

/// The part of the convex polygon `poly` above `y`.
pub fn clip_above(poly: &[Pos2], y: f32) -> Vec<Pos2> {
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (a_in, b_in) = (a.y <= y, b.y <= y);
        if a_in {
            out.push(a);
        }
        if a_in != b_in {
            let t = (y - a.y) / (b.y - a.y);
            out.push(pos2(a.x + (b.x - a.x) * t, y));
        }
    }
    out
}

/// How much of a machine is read at its size on screen: a coloured chip, a
/// summary with its name and crates, or the whole of it with a crate per row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    Chip,
    Summary,
    Full,
}

/// The detail a machine `rect` wide and high on screen is read in.
pub fn detail(rect: Rect) -> Detail {
    if rect.width() < 48.0 || rect.height() < 24.0 {
        Detail::Chip
    } else if rect.width() < 190.0 || rect.height() < 120.0 {
        Detail::Summary
    } else {
        Detail::Full
    }
}

/// The height of a machine's header on screen.
fn header(rect: Rect) -> f32 {
    (rect.height() * 0.16).clamp(16.0, 26.0)
}

/// `text` cut to `width` points with an ellipsis, or `None` when fewer than
/// four characters would be left.
fn fitted(text: &str, size: f32, width: f32) -> Option<String> {
    // The proportional font runs about this wide per character.
    let per = size * 0.56;
    let count = text.chars().count();
    if count as f32 * per <= width {
        return Some(text.to_owned());
    }
    let keep = (width / per) as usize;
    (keep >= 5).then(|| text.chars().take(keep - 1).chain(['…']).collect())
}

/// The tab text of a folder named `name` and its width on screen, cut with an
/// ellipsis to `room` points; a width of zero when not even a few characters fit.
fn hall_tab(name: &str, size: f32, room: f32) -> (String, f32) {
    // The monospace font runs this wide per character, plus the tab's margins.
    let per = size * 0.62;
    let text = format!("{name}/");
    let count = text.chars().count();
    let keep = (((room - 12.0) / per).floor().max(0.0) as usize).min(count);
    if keep == count {
        return (text, count as f32 * per + 12.0);
    }
    if keep < 5 {
        return (text, 0.0);
    }
    let cut: String = text.chars().take(keep - 1).chain(['…']).collect();
    (cut, keep as f32 * per + 12.0)
}

/// A machine drawn this frame: its file, where, and its crates.
struct Machine<'a> {
    node: &'a Node,
    rect: Rect,
    colour: Color32,
    light: f32,
}

/// What one frame of the flat view collects while it draws.
struct Floor<'a> {
    painter: egui::Painter,
    view: View,
    panel: Rect,
    selected: Option<&'a Path>,
    folder_view: Option<&'a Path>,
    /// Names already written, so no two overlap: the first written wins.
    taken: Vec<Rect>,
    /// Every folder drawn, the outer first, with its path.
    folders: Vec<(Rect, &'a Path)>,
    /// Every folder's hall, named once the machines are.
    halls: Vec<(Rect, &'a str, &'a Path, Color32, f32)>,
    machines: Vec<Machine<'a>>,
    clicked: Option<PathBuf>,
    /// The selection's outline on screen, ringed once everything is drawn.
    ring: Option<Rect>,
}

impl<'a> Floor<'a> {
    fn screen(&self, area: Rect) -> Rect {
        Rect::from_two_pos(
            self.view.to_screen(area.min, 0.0),
            self.view.to_screen(area.max, 0.0),
        )
    }

    /// How brightly what lies at `path` is drawn: faded outside the folder
    /// view.
    fn light(&self, path: &Path) -> f32 {
        match self.folder_view {
            Some(folder) if !path.starts_with(folder) && !folder.starts_with(path) => OUTSIDE,
            _ => 1.0,
        }
    }

    /// Writes `name` in `rect`, as a label the accessibility tree reads and a
    /// click selects, visibly only when `shown` and nothing written before
    /// lies there.
    fn name(&mut self, ui: &mut egui::Ui, rect: Rect, text: RichText, path: &Path, shown: bool) {
        let rect = rect.intersect(self.panel);
        if !rect.is_positive() {
            return;
        }
        // A name reserved beforehand finds its own place taken.
        let reserved = self.taken.contains(&rect);
        let free = shown && (reserved || !self.taken.iter().any(|taken| taken.intersects(rect)));
        let text = if free {
            if !reserved {
                self.taken.push(rect);
            }
            text
        } else {
            text.color(Color32::TRANSPARENT)
        };
        let response = ui
            .put(
                rect,
                egui::Label::new(text)
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .interact(Sense::click());
        if self.selected == Some(path) {
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_selected(true));
        }
        if response.clicked() {
            self.clicked = Some(path.to_path_buf());
        }
    }

    /// A folder below a district: a factory hall, its name over its top
    /// edge once there is room.
    fn folder(&mut self, area: Rect, name: &'a str, path: &'a Path, colour: Color32) {
        let rect = self.screen(area);
        if !rect.intersects(self.panel) {
            return;
        }
        let light = self.light(path);
        let radius = (rect.width().min(rect.height()) * 0.06).clamp(2.0, 10.0);
        self.painter
            .rect_filled(rect, radius, GROUND.gamma_multiply(0.85 * light));
        self.painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, colour.gamma_multiply(0.22 * light)),
            StrokeKind::Inside,
        );
        if self.selected == Some(path) {
            self.ring = Some(rect);
        }
        self.folders.push((rect, path));
        self.halls.push((rect, name, path, colour, light));
    }

    /// The name of a folder's hall: a tab over the middle of its top edge,
    /// written once the hall has room for it and nothing written before
    /// lies there; until then named for the accessibility tree only.
    fn hall_name(
        &mut self,
        ui: &mut egui::Ui,
        (rect, name, path, colour, light): (Rect, &str, &Path, Color32, f32),
    ) {
        const SIZE: f32 = 11.0;
        let (text, width) = hall_tab(name, SIZE, rect.width() - 20.0);
        let tab = Rect::from_min_size(
            pos2(rect.min.x + 10.0, rect.min.y - SIZE * 0.5 - 3.0),
            vec2(width, SIZE + 6.0),
        );
        let room = width > 0.0 && rect.height() > 40.0;
        let free = room && !self.taken.iter().any(|taken| taken.intersects(tab));
        if free {
            self.painter.rect(
                tab,
                2.0,
                PANEL.gamma_multiply(light),
                Stroke::new(1.0, colour.gamma_multiply(0.4 * light)),
                StrokeKind::Inside,
            );
        }
        self.name(
            ui,
            tab.shrink2(vec2(5.0, 0.0)),
            RichText::new(text)
                .monospace()
                .size(SIZE)
                .color(colour.gamma_multiply(0.85 * light)),
            path,
            free,
        );
    }

    fn group(&mut self, group: &'a Group, colour: Color32) {
        self.folder(group.area, &group.name, &group.path, colour);
        for inner in &group.groups {
            self.group(inner, colour);
        }
        self.nodes(&group.nodes, colour);
    }

    fn nodes(&mut self, nodes: &'a [Node], colour: Color32) {
        for node in nodes {
            let rect = self.screen(node.area);
            if !rect.intersects(self.panel.expand(40.0)) {
                continue;
            }
            let light = self.light(&node.path);
            let colour = kind_colour(node.kind).unwrap_or(colour);
            self.machines.push(Machine {
                node,
                rect,
                colour,
                light,
            });
        }
    }
}

/// How many crates did not fit a machine, and where that is written.
type More = (usize, Option<Rect>);

/// A crate as drawn: its item, its rectangle and its complexity.
struct Crate<'a> {
    item: &'a Item,
    rect: Rect,
    cx: Option<u32>,
}

/// The crates of `machine` at its `detail`: a row per item read in full, a
/// line of small crates along the bottom in summary, none as a chip. The
/// constants go last, and a `selected` item that would not fit takes the
/// last place, so it is always in view.
fn crates<'a>(
    machine: &Machine<'a>,
    detail: Detail,
    selected: Option<&Path>,
) -> (Vec<Crate<'a>>, usize, Option<Rect>) {
    let r = machine.rect;
    let mut items: Vec<&Item> = machine
        .node
        .items
        .iter()
        .filter(|item| item.kind != ItemKind::Uses)
        .collect();
    items.sort_by_key(|item| item.declaration.declares == crate::Declares::Constant);
    let slots: Vec<Rect> = match detail {
        Detail::Chip => Vec::new(),
        Detail::Summary => {
            let side = (r.height() * 0.18).clamp(5.0, 11.0);
            // Too low for a line of crates under its name.
            let fit = if r.height() < 42.0 {
                0
            } else {
                ((r.width() - 12.0) / (side + 3.0)).max(0.0) as usize
            };
            (0..items.len().min(fit))
                .map(|k| {
                    Rect::from_min_size(
                        pos2(
                            r.min.x + 6.0 + k as f32 * (side + 3.0),
                            r.max.y - side - 6.0,
                        ),
                        Vec2::splat(side),
                    )
                })
                .collect()
        }
        Detail::Full => {
            let top = r.min.y + header(r) + 22.0;
            let inner = r.width() - 20.0;
            let columns = if inner > 520.0 { 2 } else { 1 };
            let column = (inner - 8.0 * (columns as f32 - 1.0)) / columns as f32;
            let rows = ((r.max.y - 8.0 - top) / ROW).floor().max(0.0) as usize;
            let room = rows * columns;
            (0..items.len().min(room))
                .map(|k| {
                    Rect::from_min_size(
                        pos2(
                            r.min.x + 10.0 + (k / rows) as f32 * (column + 8.0),
                            top + (k % rows) as f32 * ROW,
                        ),
                        vec2(column, ROW - 4.0),
                    )
                })
                .collect()
        }
    };
    let mut slots = slots;
    // The last slot says how many more there are when they do not fit.
    let more_at = (detail == Detail::Full && items.len() > slots.len())
        .then(|| slots.pop())
        .flatten();
    let more = items.len() - slots.len();
    if let Some(last) = slots.len().checked_sub(1)
        && let Some(at) = items
            .iter()
            .position(|item| Some(item.path.as_path()) == selected)
        && at > last
    {
        items.swap(at, last);
    }
    let drawn = items
        .into_iter()
        .zip(slots)
        .map(|(item, rect)| Crate {
            item,
            rect,
            cx: complexity(item),
        })
        .collect();
    (drawn, more, more_at)
}

/// Draws `map` flat, from straight above, as `options` say, fitted to the
/// space left in `ui`, and reports what was clicked as the city does. Files
/// in `changed` glow amber. Draws nothing for an empty map.
pub fn flat(
    ui: &mut egui::Ui,
    map: &ProjectMap,
    options: ViewOptions<'_>,
    changed: &HashSet<PathBuf>,
) -> Drawn {
    let ViewOptions {
        framing,
        selected,
        folder_view,
        lanes: shown_lanes,
        picked,
        network,
        review,
        within,
        ..
    } = options;
    let panel = ui.available_rect_before_wrap();
    // What is framed, rather than a free camera, keeps a margin: nothing
    // touches the panel's edge.
    let mut framing = framing;
    if framing.closest.is_finite() {
        framing.fill *= MARGIN_FILL;
    }
    let view = fit_flat(framing, within.unwrap_or(panel));
    let painter = ui.painter_at(panel);
    painter.rect_filled(panel, 0.0, NIGHT);
    grid(&painter, &view, panel);
    let time = ui.input(|input| input.time) as f32;
    let pointer = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|pointer| panel.contains(*pointer));
    let command = ui.input(|input| input.modifiers.command);
    let mut floor = Floor {
        painter: painter.clone(),
        view,
        panel,
        selected,
        folder_view,
        taken: Vec::new(),
        folders: Vec::new(),
        halls: Vec::new(),
        machines: Vec::new(),
        clicked: None,
        ring: None,
    };

    // The districts first, their names over everything below them.
    let mut districts = Vec::new();
    let mut named = Vec::new();
    for (index, district) in map.districts.iter().enumerate() {
        let colour = district_colour(index);
        let outline = Outline::of(map.lens, district.kind);
        districts.push((district.path.clone(), outline));
        let rect = floor.screen(district.area);
        if !rect.intersects(panel) {
            continue;
        }
        let light = floor.light(&district.path);
        hall(&painter, rect, outline, colour, light);
        if selected == Some(district.path.as_path()) {
            floor.ring = Some(rect);
        }
        named.push((index, rect, light));
    }
    // The districts' names are reserved first, so nothing is written over
    // them, and written last, over everything on the floor.
    let mut signs = Vec::new();
    for (index, rect, light) in named {
        let name = &map.districts[index].name;
        let size = if view.scale() > 1.0 { 15.0 } else { 13.0 };
        let width = name.chars().count() as f32 * size * 0.62 + 18.0;
        let chip = Rect::from_min_size(rect.min + vec2(6.0, 6.0), vec2(width, size + 8.0));
        let label = chip.translate(vec2(6.0, 0.0)).intersect(panel);
        let free = !floor.taken.iter().any(|taken| taken.intersects(label));
        if free {
            floor.taken.push(label);
        }
        signs.push((index, chip, label, size, light, free));
    }
    for (index, district) in map.districts.iter().enumerate() {
        let colour = district_colour(index);
        for sub in &district.sub_districts {
            floor.folder(sub.area, &sub.name, &sub.path, colour);
            for group in &sub.groups {
                floor.group(group, colour);
            }
            floor.nodes(&sub.nodes, colour);
        }
        floor.nodes(&district.nodes, colour);
    }

    // What the pointer is over, before the belts, so Cmd lights its lanes.
    let mut crates_of: Vec<(Detail, Vec<Crate>, More)> = floor
        .machines
        .iter()
        .map(|machine| {
            let detail = detail(machine.rect);
            let (crates, more, more_at) = crates(machine, detail, selected);
            (detail, crates, (more, more_at))
        })
        .collect();
    let hovered_crate = pointer.and_then(|at| {
        crates_of
            .iter()
            .flat_map(|(_, crates, _)| crates)
            .find(|c| c.rect.contains(at))
            .map(|c| c.item)
    });
    let hovered_machine = pointer.and_then(|at| {
        floor
            .machines
            .iter()
            .rposition(|machine| machine.rect.contains(at))
    });

    orbits(ui, map, &view);
    roads(ui, map, &view);
    links(ui, map, &view);
    let lit_by = match (command, hovered_crate, hovered_machine) {
        (true, Some(item), _) => Some(item.path.as_path()),
        (true, None, Some(m)) => Some(floor.machines[m].node.path.as_path()),
        _ => selected,
    };
    let focus = Focus {
        shown: shown_lanes,
        selected: lit_by,
        picked,
        pointer,
        network,
        flat: true,
    };
    let frame = lanes::frame(map, &view, &focus);
    let mut shapes = Vec::new();
    for lane in frame.lanes.iter().filter(|lane| lane.shown) {
        belt(
            &mut shapes,
            lane,
            time,
            frame.hovered.is_some_and(|i| frame.lanes[i].id == lane.id),
        );
    }
    painter.extend(shapes);

    // The machines, then their names: the first written wins its place.
    let mut towers = Vec::new();
    let pulse = 0.5 + 0.5 * (time * 4.0).sin();
    for (machine, (detail, crates, more)) in floor.machines.iter().zip(&mut crates_of) {
        let changed = changed.contains(&machine.node.path);
        draw_machine(
            &painter, machine, *detail, crates, *more, changed, review, time,
        );
        if selected == Some(machine.node.path.as_path()) {
            floor.ring = Some(machine.rect);
        }
        for c in crates.iter() {
            if selected == Some(c.item.path.as_path()) {
                floor.ring = Some(c.rect);
            }
            towers.push(DrawnTower {
                path: c.item.path.clone(),
                colour: complexity_colour(c.cx),
                floors: vec![Span {
                    start: c.rect.left_center(),
                    end: c.rect.right_center(),
                }],
                outline: c.rect,
                glass: false,
                light: machine.light,
                cellar: c.item.tests,
                review: review.and_then(|review| review.towers.get(&c.item.path).cloned()),
            });
        }
    }
    let machines = std::mem::take(&mut floor.machines);
    for machine in &machines {
        let r = machine.rect;
        let (rect, size, colour, shown) = match detail(r) {
            Detail::Chip => (r, 10.0, Color32::TRANSPARENT, false),
            Detail::Summary => (
                Rect::from_center_size(
                    pos2(
                        r.center().x,
                        r.min.y + 4.0 + (r.height() - 22.0).clamp(0.0, 24.0) * 0.5 + 8.0,
                    ),
                    vec2(r.width() - 12.0, 16.0),
                ),
                (r.width() * 0.085).clamp(11.0, 13.0),
                INK.gamma_multiply(machine.light),
                true,
            ),
            Detail::Full => (
                Rect::from_min_size(r.min + vec2(8.0, 0.0), vec2(r.width() - 16.0, header(r))),
                (header(r) * 0.55).clamp(10.0, 14.0),
                ON_ACCENT,
                true,
            ),
        };
        floor.name(
            ui,
            rect,
            RichText::new(&machine.node.name).size(size).color(colour),
            &machine.node.path,
            shown,
        );
    }

    for hall in std::mem::take(&mut floor.halls) {
        floor.hall_name(ui, hall);
    }
    for (index, chip, label, size, light, free) in signs {
        let (district, colour) = (&map.districts[index], district_colour(index));
        if free {
            painter.rect_filled(chip, 2.0, PANEL.gamma_multiply(0.92 * light));
            painter.rect_filled(
                Rect::from_min_size(chip.min, vec2(3.0, chip.height())),
                0.0,
                colour.gamma_multiply(light),
            );
        }
        floor.name(
            ui,
            label,
            RichText::new(&district.name)
                .strong()
                .size(size)
                .color(colour.gamma_multiply(light)),
            &district.path,
            free,
        );
    }

    if let Some(ring) = floor.ring {
        let outer = ring.expand(3.0 + 3.0 * pulse);
        painter.rect_stroke(ring, 3.0, Stroke::new(2.0, ATTENTION), StrokeKind::Outside);
        painter.rect_stroke(
            outer,
            5.0,
            Stroke::new(1.5, ATTENTION.gamma_multiply(0.5 * (1.0 - pulse))),
            StrokeKind::Outside,
        );
    }

    // Hover cards: a crate's, else its machine's; a lane's over both.
    if frame.hovered.is_none()
        && let Some(at) = pointer
    {
        if let Some(item) = hovered_crate {
            tip(ui, at, item);
        } else if let Some(m) = hovered_machine {
            machine_card(ui, at, machines[m].node);
        }
    }
    lanes::card(ui, &frame, picked, pointer);
    if floor.ring.is_some() || frame.lanes.iter().any(|lane| lane.shown) {
        // The ring pulses and the goods keep moving.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }

    // Clicks and drags on the ground.
    let ground = ui.interact(panel, ui.id().with("flat ground"), Sense::click_and_drag());
    let dragged = view.ground_delta(ground.drag_delta());
    let (mut entered, mut ground_clicked, mut lane_clicked) = (None, false, None);
    if ground.clicked()
        && let (Some(at), Some(lane)) = (
            ground.interact_pointer_pos(),
            frame.hovered.map(|i| &frame.lanes[i]),
        )
    {
        let start = lane.screen[0];
        let end = *lane.screen.last().unwrap_or(&start);
        let far = if (start - at).length() > (end - at).length() {
            &lane.ends[0]
        } else {
            &lane.ends[1]
        };
        lane_clicked = Some(super::LaneClick {
            lane: lane.id.clone(),
            travel: command.then(|| far.clone()),
        });
    } else if (ground.clicked() || ground.double_clicked())
        && let Some(at) = ground.interact_pointer_pos()
    {
        let crate_hit = towers
            .iter()
            .rev()
            .find(|tower| tower.outline.contains(at))
            .map(|tower| tower.path.clone());
        let machine_hit = machines
            .iter()
            .rev()
            .find(|machine| machine.rect.contains(at))
            .map(|machine| machine.node.path.clone());
        let folder_hit = floor
            .folders
            .iter()
            .rev()
            .find(|(rect, _)| rect.contains(at))
            .map(|(_, path)| path.to_path_buf());
        let hit = crate_hit.or(machine_hit).or(folder_hit);
        if ground.double_clicked() {
            entered = hit.clone();
        }
        match hit {
            Some(path) => floor.clicked = Some(path),
            None => ground_clicked = true,
        }
    }
    Drawn {
        clicked: floor.clicked,
        ground_clicked,
        entered,
        hovered: hovered_crate.map(|item| item.path.clone()),
        view,
        towers,
        buildings: Vec::new(),
        bridges: Vec::new(),
        cables: Vec::new(),
        bridge_hovered: None,
        dragged,
        panel,
        districts,
        folders: Vec::new(),
        outlines: Vec::new(),
        lane_hovered: frame.hovered.map(|i| frame.lanes[i].id.clone()),
        lanes: frame.lanes,
        lane_clicked,
        moved: None,
        move_ended: false,
    }
}

/// A faint grid under the floor, a line every so many layout points, so a
/// pan reads even over empty ground.
fn grid(painter: &egui::Painter, view: &View, panel: Rect) {
    let step = [10.0_f32, 50.0, 250.0, 1250.0, 6250.0]
        .into_iter()
        .find(|step| step * view.scale() >= 28.0)
        .unwrap_or(6250.0);
    let (from, to) = (view.to_ground(panel.min), view.to_ground(panel.max));
    let stroke = Stroke::new(1.0, EDGE.gamma_multiply(0.35));
    let mut x = (from.x / step).floor() * step;
    while x <= to.x {
        let at = view.to_screen(pos2(x, 0.0), 0.0).x;
        painter.line_segment([pos2(at, panel.min.y), pos2(at, panel.max.y)], stroke);
        x += step;
    }
    let mut y = (from.y / step).floor() * step;
    while y <= to.y {
        let at = view.to_screen(pos2(0.0, y), 0.0).y;
        painter.line_segment([pos2(panel.min.x, at), pos2(panel.max.x, at)], stroke);
        y += step;
    }
}

/// A district's hall, its edge drawn as the lens draws its outline: so the
/// flat view, too, says which lens is on before a name is read.
fn hall(painter: &egui::Painter, rect: Rect, outline: Outline, colour: Color32, light: f32) {
    let radius = match outline {
        Outline::Step => 0.0,
        Outline::Hexagon => 4.0,
        _ => (rect.width().min(rect.height()) * 0.05).clamp(4.0, 24.0),
    };
    painter.rect_filled(rect, radius, colour.gamma_multiply(0.05 * light));
    let edge = Stroke::new(1.2, colour.gamma_multiply(0.45 * light));
    match outline {
        Outline::Plaza => {
            let corners = [
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
                rect.left_top(),
            ];
            painter.extend(Shape::dashed_line(&corners, edge, 8.0, 5.0));
        }
        Outline::Membrane => {
            painter.rect_stroke(
                rect,
                radius,
                Stroke::new(3.0, colour.gamma_multiply(0.35 * light)),
                StrokeKind::Inside,
            );
            painter.rect_stroke(rect.shrink(5.0), radius, edge, StrokeKind::Inside);
        }
        Outline::Orbit => {
            painter.rect_stroke(rect, radius, edge, StrokeKind::Inside);
            painter.rect_stroke(
                rect.expand(4.0),
                radius + 4.0,
                Stroke::new(0.8, colour.gamma_multiply(0.25 * light)),
                StrokeKind::Inside,
            );
        }
        Outline::Step | Outline::Hexagon | Outline::Own => {
            painter.rect_stroke(rect, radius, edge, StrokeKind::Inside);
        }
    }
}

/// A belt along `lane`, from what is used to what uses it: a dark band, the
/// lane's colour in it, and goods moving downstream.
fn belt(shapes: &mut Vec<Shape>, lane: &lanes::DrawnLane, time: f32, hovered: bool) {
    let points = &lane.screen;
    if points.len() < 2 {
        return;
    }
    let lit = lane.lit || hovered;
    let (width, strength) = match (lane.kind, lit) {
        (LaneKind::Imports, true) => (4.0, 0.55),
        (LaneKind::Imports, false) => (2.5, 0.18),
        (LaneKind::Calls, true) => (2.0, 0.5),
        (LaneKind::Calls, false) => (1.2, 0.14),
    };
    shapes.push(Shape::line(
        points.clone(),
        Stroke::new(width + 2.0, NIGHT.gamma_multiply(0.8)),
    ));
    let line = Stroke::new(width, lane.colour.gamma_multiply(strength));
    if lane.kind == LaneKind::Calls {
        shapes.extend(Shape::dashed_line(points, line, 6.0, 4.0));
    } else {
        shapes.push(Shape::line(points.clone(), line));
    }
    let spacing = if lit { 26.0 } else { 70.0 };
    let length: f32 = points.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let mut along = (time * 36.0) % spacing;
    while along < length {
        if let Some(at) = lanes::point_along(points, along) {
            let radius = if lit { 2.6 } else { 1.6 };
            shapes.push(Shape::circle_filled(
                at,
                radius,
                lane.colour.gamma_multiply(if lit { 0.95 } else { 0.4 }),
            ));
        }
        along += spacing;
    }
}

/// One machine at its `detail`: its body cut by its file's kind, its header
/// in its district's or kind's colour, its crates, and the amber of a file
/// git says changed.
#[allow(clippy::too_many_arguments)]
fn draw_machine(
    painter: &egui::Painter,
    machine: &Machine,
    detail: Detail,
    crates: &[Crate],
    (more, more_at): More,
    changed: bool,
    review: Option<&crate::ReviewOverlay>,
    time: f32,
) {
    let r = machine.rect;
    let node = machine.node;
    let mut a = machine.light;
    let reviewed: Vec<&TowerReview> = review
        .map(|review| {
            node.items
                .iter()
                .filter_map(|item| review.towers.get(&item.path))
                .collect()
        })
        .unwrap_or_default();
    if review.is_some() && reviewed.is_empty() {
        a *= REVIEW_RECEDED;
    }
    let shape = shape_of(node.kind, &node.name);
    let cut = outline(shape, r);
    let accent = machine.colour;
    if detail == Detail::Chip {
        painter.add(Shape::convex_polygon(
            cut,
            accent.gamma_multiply(0.5 * a),
            Stroke::new(0.6, accent.gamma_multiply(0.7 * a)),
        ));
        if changed {
            let radius = (r.height() * 0.3).clamp(1.5, 4.0);
            painter.circle(r.right_top(), radius, CHANGED, Stroke::new(1.0, NIGHT));
        }
        return;
    }
    painter.add(Shape::convex_polygon(
        cut.iter().map(|p| *p + vec2(0.0, 3.0)).collect(),
        Color32::from_black_alpha((110.0 * a) as u8),
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        cut.clone(),
        BODY.gamma_multiply(a),
        Stroke::new(1.0, accent.gamma_multiply(0.45 * a)),
    ));
    if changed {
        // Heat: a file git says changed glows amber, breathing slowly, and
        // carries the Explorer's mark in its corner.
        let glow = 0.55 + 0.35 * (time * 2.0).sin();
        for (grow, width, strength) in [(3.0, 4.0, 0.25), (1.0, 1.5, 0.9)] {
            painter.add(Shape::closed_line(
                outline(shape, r.expand(grow)),
                Stroke::new(width, CHANGED.gamma_multiply(strength * glow * a)),
            ));
        }
        painter.circle(
            r.right_top() + vec2(-2.0, 2.0),
            4.5,
            CHANGED,
            Stroke::new(1.5, NIGHT),
        );
    }
    if reviewed
        .iter()
        .any(|tower| !tower.written.is_empty() || !tower.removed.is_empty())
    {
        painter.add(Shape::closed_line(
            cut.clone(),
            Stroke::new(2.0, WRITTEN.gamma_multiply(a)),
        ));
    } else if reviewed.iter().any(|tower| tower.impacted) {
        painter.add(Shape::closed_line(
            cut.clone(),
            Stroke::new(2.0, IMPACTED.gamma_multiply(a)),
        ));
    }
    match detail {
        Detail::Chip => {}
        Detail::Summary => {
            painter.add(Shape::convex_polygon(
                clip_above(&cut, r.min.y + 4.0),
                accent.gamma_multiply(a),
                Stroke::NONE,
            ));
            for c in crates {
                painter.rect_filled(
                    c.rect,
                    2.0,
                    complexity_colour(c.cx).gamma_multiply(0.85 * a),
                );
            }
        }
        Detail::Full => {
            let h = header(r);
            painter.add(Shape::convex_polygon(
                clip_above(&cut, r.min.y + h),
                accent.gamma_multiply(a),
                Stroke::NONE,
            ));
            let functions = node.items.iter().filter_map(complexity).count();
            let most = node.items.iter().filter_map(complexity).max();
            let lines: usize = node.items.iter().map(|item| item.floors.len()).sum();
            let meta = match most {
                Some(most) => format!(
                    "{lines} lines · {} items · {functions} fn · max cx {most}",
                    node.items.len()
                ),
                None if node.items.is_empty() => kind_name(node.kind).to_owned(),
                None => format!("{lines} lines · {} items", node.items.len()),
            };
            if let Some(meta) = fitted(&meta, 11.0, r.width() - 16.0) {
                painter.text(
                    pos2(r.min.x + 8.0, r.min.y + h + 4.0),
                    Align2::LEFT_TOP,
                    meta,
                    FontId::proportional(11.0),
                    DIM.gamma_multiply(a),
                );
            }
            for c in crates {
                let tint = complexity_colour(c.cx);
                let light = match review.and_then(|review| review.towers.get(&c.item.path)) {
                    Some(_) => a.max(machine.light),
                    None => a,
                };
                painter.rect_filled(c.rect, 3.0, PANEL.gamma_multiply(light));
                painter.rect_stroke(
                    c.rect,
                    3.0,
                    Stroke::new(1.0, LINE_STRONG.gamma_multiply(0.6 * light)),
                    StrokeKind::Inside,
                );
                painter.rect_filled(
                    Rect::from_min_size(c.rect.min, vec2(3.0, c.rect.height())),
                    1.5,
                    tint.gamma_multiply(light),
                );
                let number = c.cx.map(|cx| cx.to_string()).unwrap_or_default();
                if !number.is_empty() {
                    painter.text(
                        pos2(c.rect.max.x - 6.0, c.rect.center().y),
                        Align2::RIGHT_CENTER,
                        &number,
                        FontId::monospace(11.0),
                        tint.gamma_multiply(light),
                    );
                }
                let room = c.rect.width() - 16.0 - if number.is_empty() { 0.0 } else { 24.0 };
                let colour = if c.item.tests { DIM } else { INK };
                if let Some(name) = fitted(&c.item.name, 12.0, room) {
                    painter.text(
                        pos2(c.rect.min.x + 9.0, c.rect.center().y),
                        Align2::LEFT_CENTER,
                        name,
                        FontId::proportional(12.0),
                        colour.gamma_multiply(light),
                    );
                }
            }
            if let Some(at) = more_at {
                painter.text(
                    pos2(at.min.x + 2.0, at.center().y),
                    Align2::LEFT_CENTER,
                    format!("+{more} more"),
                    FontId::proportional(11.5),
                    DIM.gamma_multiply(a),
                );
            }
        }
    }
}

/// The kind of a file as a card reads it.
fn kind_name(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Code => "code",
        NodeKind::Document => "document",
        NodeKind::Configuration => "configuration",
        NodeKind::Specification => "specification",
        NodeKind::Infrastructure => "infrastructure",
        NodeKind::Build => "build",
        NodeKind::Contract => "contract",
        NodeKind::Secrets => "secrets",
        NodeKind::SecretsExample => "secrets example",
        NodeKind::Generated => "generated",
        NodeKind::Schema => "schema",
        NodeKind::Image => "image",
    }
}

/// A small card beside the pointer naming the machine under it: its file,
/// its kind and what stands in it.
fn machine_card(ui: &egui::Ui, at: Pos2, node: &Node) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        ui.id().with("machine card"),
    ));
    let lines: usize = node.items.iter().map(|item| item.floors.len()).sum();
    let most = node.items.iter().filter_map(complexity).max();
    let mut meta = format!(
        "{} · {} items · {lines} lines",
        kind_name(node.kind),
        node.items.len()
    );
    if let Some(most) = most {
        meta += &format!(" · max cx {most}");
    }
    let name = painter.layout_no_wrap(node.name.clone(), FontId::proportional(13.0), INK);
    let meta = painter.layout_no_wrap(meta, FontId::monospace(11.0), DIM);
    let corner = at + vec2(14.0, 14.0);
    let size = vec2(
        name.size().x.max(meta.size().x),
        name.size().y + 2.0 + meta.size().y,
    );
    let card = Rect::from_min_size(corner, size).expand2(vec2(10.0, 6.0));
    painter.rect(
        card,
        0.0,
        PANEL,
        Stroke::new(1.0, LINE_STRONG),
        StrokeKind::Inside,
    );
    painter.galley(corner, name.clone(), INK);
    painter.galley(corner + vec2(0.0, name.size().y + 2.0), meta, DIM);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outlines_stay_inside_their_rectangle_and_clip_to_the_header() {
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 80.0));
        for shape in [
            FileShape::Code,
            FileShape::Test,
            FileShape::Doc,
            FileShape::Config,
            FileShape::Script,
            FileShape::Pill,
        ] {
            assert!(
                outline(shape, r)
                    .iter()
                    .all(|p| r.expand(0.01).contains(*p))
            );
        }
        let head = clip_above(&outline(FileShape::Doc, r), 20.0);
        assert!(head.iter().all(|p| p.y <= 20.0) && head.len() == 5);
    }

    #[test]
    fn detail_grows_with_the_machine_on_screen() {
        let sized = |w, h| detail(Rect::from_min_size(Pos2::ZERO, vec2(w, h)));
        assert_eq!(sized(40.0, 30.0), Detail::Chip);
        assert_eq!(sized(90.0, 20.0), Detail::Chip);
        assert_eq!(sized(120.0, 80.0), Detail::Summary);
        assert_eq!(sized(300.0, 200.0), Detail::Full);
    }

    #[test]
    fn complexity_counts_the_branches_of_a_function() {
        let floor = |text: &str| crate::graph::Floor {
            number: 1,
            text: text.to_owned(),
            indent: 0,
            width: text.len(),
            tokens: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
        };
        let item = |kind, lines: &[&str]| Item {
            name: "f".to_owned(),
            path: PathBuf::from("a.rs/f"),
            kind,
            tests: false,
            floors: lines.iter().map(|line| floor(line)).collect(),
            area: Rect::NOTHING,
            base: 0.0,
            declaration: Default::default(),
        };
        let branching = item(
            ItemKind::Function,
            &[
                "fn f(a: bool) {",
                "    if a && ready() {",
                "    // if not",
                "    for x in y {}",
                "}",
            ],
        );
        assert_eq!(complexity(&branching), Some(4));
        assert_eq!(complexity(&item(ItemKind::Type, &["struct S;"])), None);
        assert_eq!(complexity_colour(Some(2)), CX_PLAIN);
        assert_eq!(complexity_colour(Some(30)), CX_KNOTTED);
    }

    #[test]
    fn a_hall_tab_is_cut_to_its_hall_never_hidden_while_a_few_characters_fit() {
        let (whole, width) = hall_tab("src", 11.0, 400.0);
        assert_eq!(whole, "src/");
        assert!(width < 400.0);
        let long = "0005-story-003-a-very-long-folder-name-that-runs-past-its-hall";
        let (cut, width) = hall_tab(long, 11.0, 200.0);
        assert!(cut.ends_with('…'), "{cut}");
        assert!(width <= 200.0, "{width}");
        assert_eq!(hall_tab(long, 11.0, 30.0).1, 0.0);
    }

    #[test]
    fn a_name_is_cut_to_its_room_or_left_out() {
        assert_eq!(
            fitted("pricing.rs", 12.0, 200.0).as_deref(),
            Some("pricing.rs")
        );
        assert!(
            fitted("a_very_long_file_name.rs", 12.0, 70.0)
                .unwrap()
                .ends_with('…')
        );
        assert_eq!(fitted("a_very_long_file_name.rs", 12.0, 20.0), None);
    }
}
