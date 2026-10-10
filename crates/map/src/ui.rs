//! Painting one project map: the ground of its districts and blocks, seen
//! isometrically, and the city of towers standing on it.
//!
//! The only module of this crate that touches an egui widget. Every name is a
//! `Label` placed at the front of the element it names rather than painted
//! text, so the frame's accessibility tree carries one entry per district,
//! sub-district, group and node. A click on a name is reported back; the
//! selected element is outlined and its label marked selected, so the
//! selection is visible both on screen and in the accessibility tree.

use std::path::{Path, PathBuf};

use egui::{Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

mod explain;
mod lanes;

pub use explain::{banner, caption, caption_text, lane_toggles, legend, legend_text, lens_bar};
pub use lanes::{DrawnLane, colour_of};

use crate::Framing;
use crate::city::{self, iso, iso_bounds};
use crate::graph::{DistrictKind, Group, Item, ItemKind, Node, ProjectMap};
use crate::layout::TITLE;
use crate::view::{Pivot, View, fit, turn_at};
use crate::{LaneId, LaneKind, Lanes, Lens};

/// One frame of the map as drawn.
pub struct Drawn {
    /// The path of the element clicked, if any: by its name, or a tower by
    /// any of its lines, or a block by its ground.
    pub clicked: Option<PathBuf>,
    /// Whether empty ground was clicked: between blocks, or off the city.
    pub ground_clicked: bool,
    /// The tower double-clicked, to step inside.
    pub entered: Option<PathBuf>,
    /// The tower one of whose lines is under the pointer.
    pub hovered: Option<PathBuf>,
    /// Where the map was seen from.
    pub view: View,
    /// The towers drawn, back to front.
    pub towers: Vec<DrawnTower>,
    /// How far the ground was dragged this frame, at the layout's scale.
    pub dragged: Vec2,
    /// The space the map was drawn in. Names placed past its edge make the
    /// `Ui` around it grow, so what comes after asks this instead.
    pub panel: Rect,
    /// Every district drawn, with the outline it was drawn in.
    pub districts: Vec<(PathBuf, Outline)>,
    /// Every lane drawn.
    pub lanes: Vec<DrawnLane>,
    /// The lane clicked, if any.
    pub lane_clicked: Option<LaneClick>,
    /// The lane under the pointer, if any.
    pub lane_hovered: Option<LaneId>,
    /// In the custom lens: the district or block dragged, and how far this
    /// frame, at the layout's scale; and whether the drag ended.
    pub moved: Option<(PathBuf, Vec2)>,
    pub move_ended: bool,
}

/// The outline a district is drawn in: each lens has its own, so the view
/// says which lens is on before a name is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    /// A round plaza with a dashed edge: the folders lens, the tests and the
    /// outskirts.
    Plaza,
    /// A square step: the imports lens.
    Step,
    /// A hexagon: the calls lens.
    Hexagon,
    /// A double ring: the co-change lens.
    Orbit,
    /// A cell's membrane, a thick ring with a thin one inside: the cells
    /// lens.
    Membrane,
    /// A round plaza with a solid edge: the engineer's own, the custom lens.
    Own,
}

impl Outline {
    /// The outline of a district of `kind` in `lens`.
    pub fn of(lens: Lens, kind: DistrictKind) -> Outline {
        match (lens, kind) {
            (Lens::Custom, _) => Outline::Own,
            (_, DistrictKind::Context | DistrictKind::NoContext) => Outline::Membrane,
            (_, DistrictKind::Folder | DistrictKind::Tests | DistrictKind::Outskirts) => {
                Outline::Plaza
            }
            (Lens::Imports, _) => Outline::Step,
            (Lens::Calls, _) => Outline::Hexagon,
            (Lens::CoChange, _) => Outline::Orbit,
            (Lens::Folders | Lens::Cells, _) => Outline::Plaza,
        }
    }
}

/// One tower as drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnTower {
    pub path: PathBuf,
    pub colour: Color32,
    /// Where each floor went on screen, the ground floor first.
    pub floors: Vec<Span>,
    /// The screen rectangle its building covers.
    pub outline: Rect,
    /// Whether it was drawn as glass, standing in front of the tower pointed
    /// at or selected.
    pub glass: bool,
    /// How brightly it was drawn: 1 in full, less behind glass or fog.
    pub light: f32,
    /// Whether it hangs in the cellar.
    pub cellar: bool,
}

/// One line on screen: from the point of its first character to the point of
/// its last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: Pos2,
    pub end: Pos2,
}

impl Span {
    /// How far `at` is from the line.
    pub fn distance(&self, at: Pos2) -> f32 {
        let along = self.end - self.start;
        let t =
            ((at - self.start).dot(along) / along.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
        (self.start + along * t - at).length()
    }
}

/// The night the city stands in, the ground of its blocks, the edges of
/// folders and the colour of names.
const NIGHT: Color32 = Color32::from_rgb(0x04, 0x06, 0x0a);
const GROUND: Color32 = Color32::from_rgb(0x0b, 0x10, 0x16);
const EDGE: Color32 = Color32::from_rgb(0x1e, 0x27, 0x31);
const DIM: Color32 = Color32::from_rgb(0x7d, 0x8f, 0xa0);
const INK: Color32 = Color32::from_rgb(0xd7, 0xe2, 0xea);
const PANEL: Color32 = Color32::from_rgb(0x0d, 0x11, 0x17);
const LINE_STRONG: Color32 = Color32::from_rgb(0x34, 0x50, 0x6a);
const FLOW: Color32 = Color32::from_rgb(0x5e, 0xf6, 0xff);
const ATTENTION_SOFT: Color32 = Color32::from_rgb(0x3a, 0x36, 0x08);

/// The colours of the districts, in the order the map lists them; an eighth
/// district takes the first colour again.
const DISTRICT_COLOURS: [Color32; 7] = [
    Color32::from_rgb(0x5e, 0xf6, 0xff),
    Color32::from_rgb(0xff, 0x5c, 0xa8),
    Color32::from_rgb(0x9c, 0xff, 0x57),
    Color32::from_rgb(0xff, 0x8a, 0x3d),
    Color32::from_rgb(0xb2, 0x6b, 0xff),
    Color32::from_rgb(0xff, 0xd1, 0x66),
    Color32::from_rgb(0x3d, 0xff, 0xc8),
];

/// Tests are grey towers, whatever their district.
const TEST_COLOUR: Color32 = Color32::from_rgb(0x56, 0x67, 0x7a);

/// How much of the city outside the folder view still shows.
const OUTSIDE: f32 = 0.35;

/// How opaque a building's faces are, its side's colour, and how bright its
/// edges glow.
const SOLID: f32 = 0.92;
const SIDE: Color32 = Color32::from_rgb(0x07, 0x0a, 0x0e);
const EDGE_LIGHT: f32 = 0.35;

/// How wide a floor's stroke is far out.
const FAR_STROKE: f32 = 1.4;

/// How much of the cellar shows outside the underground view, and how much of
/// the city above shows inside it.
const CELLAR: f32 = 0.2;
const ABOVE_UNDERGROUND: f32 = 0.15;

/// How much of a tower shows while a turned camera faces another facade:
/// next to nothing in front of the facade, less the further behind it.
fn fog(item: &Item, pivot: Pivot) -> f32 {
    let here = item.area.center();
    let in_front = if pivot.along_x {
        here.y - pivot.ground.y
    } else {
        here.x - pivot.ground.x
    };
    if in_front > 0.5 {
        FOGGED_FRONT
    } else if in_front < -0.5 {
        (1.0 + in_front / FOG_DEPTH).max(FOGGED_BEHIND)
    } else {
        1.0
    }
}

/// Fog: what stands in front of a faced facade keeps this much, what stands
/// behind fades over this depth down to the least it keeps.
const FOGGED_FRONT: f32 = 0.06;
const FOG_DEPTH: f32 = 25.0;
const FOGGED_BEHIND: f32 = 0.12;

/// How bright a line glows, a line that calls something, and a tower pointed
/// at or selected; glass lets a tenth through.
const LINE: f32 = 0.45;
const CALLING_LINE: f32 = 0.85;
const FOCUSED: f32 = 1.3;
const GLASS: f32 = 0.1;

/// From this scale on a token is drawn as long as it is, so a tower reads as
/// a wall of code.
const DASHES_FROM: f32 = 1.2;

/// From this font size on, the floors close up are written as their lines.
const LETTERS_FROM: f32 = 7.0;

/// The colour of a road between cells.
const ROAD: Color32 = Color32::from_rgb(38, 46, 60);

/// The colour of a warning: an import cycle.
const WARNING: Color32 = Color32::from_rgb(255, 150, 60);

/// The colour of whatever needs the engineer: selection, the pointer.
const ATTENTION: Color32 = Color32::from_rgb(0xfc, 0xee, 0x0a);

/// `colour` at `bright`ness, added to what lies beneath rather than painted
/// over it, so points glow and crowded points glow brighter.
fn glow(colour: Color32, bright: f32) -> Color32 {
    let [r, g, b, _] = colour.to_array();
    let scaled = |channel: u8| (f32::from(channel) * bright).min(255.0) as u8;
    Color32::from_rgba_premultiplied(scaled(r), scaled(g), scaled(b), 0)
}

/// A small card beside the pointer naming the tower under it: its name, its
/// file and how many lines it has.
fn tip(ui: &egui::Ui, at: Pos2, item: &Item) {
    let painter = ui.painter();
    let file = item
        .path
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let lines = item.floors.len();
    let unit = if lines == 1 { "line" } else { "lines" };
    let name = painter.layout_no_wrap(item.name.clone(), egui::FontId::proportional(13.0), INK);
    let meta = painter.layout_no_wrap(
        format!("{file} · {lines} {unit}"),
        egui::FontId::monospace(11.0),
        DIM,
    );
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

/// Four corner brackets around `rect`.
fn brackets(shapes: &mut Vec<Shape>, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(2.0, colour);
    let arm = 10.0_f32.min(rect.width() / 3.0).min(rect.height() / 3.0);
    for (corner, dx, dy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0, -1.0),
    ] {
        let line = vec![
            corner + vec2(dx * arm, 0.0),
            corner,
            corner + vec2(0.0, dy * arm),
        ];
        shapes.push(Shape::line(line, stroke));
    }
}

/// How far from a line's points, on screen, a pointer still hits the line.
const PICK_REACH: f32 = 3.0;

/// Below this scale a floor is one point at its indentation; above it, one
/// point per token.
const TOKENS_FROM: f32 = 0.8;

/// The colour of the district at `index` in the map's list.
fn district_colour(index: usize) -> Color32 {
    DISTRICT_COLOURS[index % DISTRICT_COLOURS.len()]
}

/// What one frame of the map is drawn with: where it goes and what is
/// selected. The clicked element is written back to `clicked`; names and
/// towers are collected while the ground is drawn, and drawn over it.
struct Painter<'a> {
    view: View,
    selected: Option<&'a Path>,
    /// The folder whose folder view is drawn; everything outside it is not.
    folder_view: Option<&'a Path>,
    clicked: Option<PathBuf>,
    /// The colour of the district being drawn, and the centre of its plaza
    /// when the district itself is shown.
    colour: Color32,
    plaza: Option<Pos2>,
    labels: Vec<Labelled<'a>>,
    towers: Vec<(&'a Item, Color32)>,
    /// The ground of every block drawn, the node it stands for, and how high
    /// it is raised.
    blocks: Vec<(Rect, &'a Path, f32)>,
    /// The facade a turned camera faces, and how far it has turned.
    facing: Option<(Pivot, f32)>,
    /// Whether the view is underground: the cellar within reach.
    underground: bool,
    /// The outline of the district being drawn.
    outline: Outline,
    /// For every item, the district whose colour it keeps (its crate).
    homes: &'a std::collections::BTreeMap<PathBuf, usize>,
    /// How high the district being drawn is raised.
    lift: f32,
    /// The blocks holding the files of an import cycle.
    cycles: &'a std::collections::BTreeSet<PathBuf>,
    /// Whether the city is being arranged (the custom lens): a district's
    /// name then moves the district when dragged.
    arranging: bool,
    /// The district whose name was dragged this frame, how far on screen,
    /// and whether the drag ended.
    name_dragged: Option<(PathBuf, Vec2, bool)>,
}

/// A name to put on the map: the ground it names, how it is written, the
/// name, the path it stands for, and how high that ground is raised.
type Labelled<'a> = (Rect, Naming, &'a str, &'a Path, f32);

/// How a name is written: a district's in its colour along its front,
/// always; a block with code on it just below its front edge, quietly and
/// only close enough to read. Folders below a district and blocks without
/// code are named for the accessibility tree only.
#[derive(Clone, Copy)]
enum Naming {
    District(Color32),
    Folder,
    /// A block, and whether it has code on it.
    Block(bool),
}

/// Below this scale only district names are written; the others are still
/// there for the accessibility tree, in no colour.
const NAMES_FROM: f32 = 0.9;

/// What is drawn of a map and how: the view options of one frame.
#[derive(Clone, Copy, Debug)]
pub struct ViewOptions<'a> {
    /// The part of the map to fit into the space left.
    pub framing: Framing,
    /// The element outlined as selected.
    pub selected: Option<&'a Path>,
    /// The folder whose folder view is drawn; everything outside it faded.
    pub folder_view: Option<&'a Path>,
    /// The facade the camera turns towards, close enough (see [`turn_at`]).
    pub pivot: Option<Pivot>,
    /// Whether the cellar is in reach rather than faint.
    pub underground: bool,
    /// The lanes shown.
    pub lanes: Lanes,
    /// The lane picked, kept lit with what it carries.
    pub picked: Option<&'a LaneId>,
}

/// A click on a lane: the lane, and with Cmd held the tower at its far end
/// from the click, to travel to.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneClick {
    pub lane: LaneId,
    pub travel: Option<PathBuf>,
}

/// Draws `map` as `options` say, fitted to the space left in `ui`, and
/// reports what was clicked this frame: a name, a tower by any line of it, a
/// block, or empty ground. Draws nothing for an empty map.
pub fn project(ui: &mut egui::Ui, map: &ProjectMap, options: ViewOptions<'_>) -> Drawn {
    let ViewOptions {
        framing,
        selected,
        folder_view,
        pivot,
        underground,
        lanes: shown,
        picked,
    } = options;
    let panel = ui.available_rect_before_wrap();
    let view = fit(framing, panel);
    let turn = turn_at(view.scale());
    let view = match pivot {
        Some(pivot) if turn > 0.0 => view.turned(turn, pivot, panel.center()),
        _ => view,
    };
    ui.painter().rect_filled(panel, 0.0, NIGHT);
    let mut painter = Painter {
        view,
        selected,
        folder_view,
        clicked: None,
        colour: DISTRICT_COLOURS[0],
        plaza: None,
        labels: Vec::new(),
        towers: Vec::new(),
        blocks: Vec::new(),
        facing: pivot.filter(|_| turn > 0.0).map(|pivot| (pivot, turn)),
        underground,
        outline: Outline::Plaza,
        homes: &map.homes,
        lift: 0.0,
        cycles: &map.cycles,
        arranging: map.lens == Lens::Custom,
        name_dragged: None,
    };
    orbits(ui, map, &view);
    let mut outlines = Vec::new();
    // Back to front, so a raised district hides what stands behind it.
    let mut order: Vec<usize> = (0..map.districts.len()).collect();
    order.sort_by(|a, b| {
        let depth = |d: usize| view.to_screen(map.districts[d].area.center(), 0.0).y;
        depth(*a).total_cmp(&depth(*b)).then(a.cmp(b))
    });
    for index in order {
        let district = &map.districts[index];
        if district.elevation > 0.0 {
            platform(
                ui,
                &view,
                district.area,
                district.elevation,
                district_colour(index),
            );
        }
        painter.lift = district.elevation;
        painter.colour = district_colour(index);
        painter.plaza = None;
        painter.outline = Outline::of(map.lens, district.kind);
        outlines.push((district.path.clone(), painter.outline));
        let (area, name, path) = (district.area, &district.name, &district.path);
        let shown = painter.ground(ui, area, name, path, false, true);
        for sub in &district.sub_districts {
            let shown = painter.ground(ui, sub.area, &sub.name, &sub.path, shown, false);
            for group in &sub.groups {
                painter.group_with_children(ui, group, shown);
            }
            painter.nodes(ui, &sub.nodes, shown);
        }
        painter.nodes(ui, &district.nodes, shown);
    }
    roads(ui, map, &view);
    // The quiet names of folders and blocks lie under the ground's sense, so
    // a click on a tower standing over a name reaches the tower; a district's
    // name lies over it.
    let (districts, quiet): (Vec<_>, Vec<_>) = std::mem::take(&mut painter.labels)
        .into_iter()
        .partition(|(_, naming, _, _, _)| matches!(naming, Naming::District(_)));
    for (area, naming, name, path, lift) in quiet {
        painter.label(ui, area, naming, name, path, lift);
    }
    let ground = ui.interact(panel, ui.id().with("ground"), Sense::click_and_drag());
    let mut dragged = view.ground_delta(ground.drag_delta());
    // In the custom lens a drag that starts on a block moves the block, one
    // that starts on a district's open ground the district; elsewhere it
    // pans. What it moves is kept from frame to frame.
    let mut moved = None;
    let mut move_ended = false;
    if map.lens == Lens::Custom {
        let held = ui.id().with("custom move");
        if ground.drag_started()
            && let Some(at) = ui
                .input(|input| input.pointer.press_origin())
                .or(ground.interact_pointer_pos())
        {
            // Where the button went down: the drag only starts once the
            // pointer has moved, by then perhaps off a small block.
            let block = painter
                .blocks
                .iter()
                .find(|(area, _, lift)| area.contains(view.to_ground_at(at, *lift)))
                .map(|(_, path, _)| path.to_path_buf());
            let district = || {
                map.at(view.to_ground(at))
                    .first()
                    .map(|path| path.to_path_buf())
            };
            let target: Option<PathBuf> = block.or_else(district);
            ui.data_mut(|data| data.insert_temp(held, target));
            // The drag starts once the pointer has moved a little: the move
            // counts from where the button went down.
            if let Some(now) = ground.interact_pointer_pos() {
                dragged = view.ground_delta(now - at);
            }
        }
        if let Some(path) = ui
            .data(|data| data.get_temp::<Option<PathBuf>>(held))
            .flatten()
        {
            if dragged != Vec2::ZERO {
                moved = Some((path, dragged));
                dragged = Vec2::ZERO;
            }
            if ground.drag_stopped() {
                ui.data_mut(|data| data.remove::<Option<PathBuf>>(held));
                move_ended = true;
            }
        }
    }
    let pointer = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|pointer| panel.contains(*pointer));
    let time = ui.input(|input| input.time) as f32;
    let focus = lanes::Focus {
        shown,
        selected,
        picked,
        pointer,
    };
    let mut frame = lanes::frame(map, &view, &focus);
    let mut shapes = Vec::new();
    lanes::draw(&mut frame, LaneKind::Imports, &view, time, &mut shapes);
    ui.painter().extend(shapes);
    let (towers, hovered) = painter.towers(ui, pointer);
    let mut shapes = Vec::new();
    lanes::draw(&mut frame, LaneKind::Calls, &view, time, &mut shapes);
    ui.painter().extend(shapes);
    lanes::card(ui, &frame, picked, pointer);
    if shown.imports || shown.calls {
        // Traffic keeps moving.
        ui.ctx().request_repaint();
    }
    let mut entered = None;
    let mut ground_clicked = false;
    let mut lane_clicked = None;
    // A click on a lane picks it; with Cmd it travels to the far end.
    if ground.clicked()
        && let (Some(at), Some(lane)) = (
            ground.interact_pointer_pos(),
            frame.hovered.map(|i| &frame.lanes[i]),
        )
    {
        let command = ui.input(|input| {
            input.modifiers.command
                || input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::PointerButton { modifiers, .. } if modifiers.command))
        });
        let (start, end) = (
            lane.screen[0],
            *lane.screen.last().unwrap_or(&lane.screen[0]),
        );
        let far = if (start - at).length() > (end - at).length() {
            &lane.ends[0]
        } else {
            &lane.ends[1]
        };
        lane_clicked = Some(LaneClick {
            lane: lane.id.clone(),
            travel: command.then(|| far.clone()),
        });
    } else if (ground.clicked() || ground.double_clicked())
        && let Some(at) = ground.interact_pointer_pos()
    {
        let tower = pick(&towers, at, painter.underground);
        if ground.double_clicked() {
            entered = tower.clone();
        }
        let block = painter
            .blocks
            .iter()
            .find(|(area, _, lift)| area.contains(view.to_ground_at(at, *lift)))
            .map(|(_, path, _)| path.to_path_buf());
        match tower.or(block) {
            Some(path) => painter.clicked = Some(path),
            None => ground_clicked = true,
        }
    }
    for (area, naming, name, path, lift) in districts {
        painter.label(ui, area, naming, name, path, lift);
    }
    if let Some((path, by, ended)) = painter.name_dragged.take() {
        if by != Vec2::ZERO {
            moved = Some((path, view.ground_delta(by)));
        }
        move_ended |= ended;
    }
    Drawn {
        clicked: painter.clicked,
        ground_clicked,
        entered,
        hovered,
        view,
        towers,
        dragged,
        panel,
        districts: outlines,
        lane_hovered: frame.hovered.map(|i| frame.lanes[i].id.clone()),
        lanes: frame.lanes,
        lane_clicked,
        moved,
        move_ended,
    }
}

/// A district raised by `e`: a solid box from its ground down to its
/// footprint, the two sides facing the viewer dark and edged in `colour`,
/// with an opaque top for what stands on it.
fn platform(ui: &egui::Ui, view: &View, area: Rect, e: f32, colour: Color32) {
    let corners = [
        area.left_top(),
        area.right_top(),
        area.right_bottom(),
        area.left_bottom(),
    ];
    let top = |p: Pos2| view.to_screen(p, e);
    let foot = |p: Pos2| view.to_screen(p, 0.0);
    let edge = Stroke::new(1.0, colour.gamma_multiply(0.45));
    // The sides facing the viewer are the ones whose ground edge is nearest
    // the bottom of the screen; drawn back to front.
    let mut sides: Vec<(Pos2, Pos2)> = (0..4).map(|k| (corners[k], corners[(k + 1) % 4])).collect();
    sides.sort_by(|a, b| {
        let near = |(p, q): &(Pos2, Pos2)| foot(*p).y + foot(*q).y;
        near(a).total_cmp(&near(b))
    });
    for (k, (a, b)) in sides.iter().enumerate().skip(2) {
        let fill = if k == 2 { SIDE } else { GROUND };
        let face = vec![top(*a), top(*b), foot(*b), foot(*a)];
        ui.painter().add(Shape::convex_polygon(face, fill, edge));
        for step in 1..6 {
            let p = *a + (*b - *a) * (step as f32 / 6.0);
            let pillar = Stroke::new(1.0, colour.gamma_multiply(0.12));
            ui.painter().line_segment([top(p), foot(p)], pillar);
        }
    }
    let roof: Vec<Pos2> = corners.iter().map(|p| top(*p)).collect();
    ui.painter()
        .add(Shape::convex_polygon(roof, NIGHT, Stroke::NONE));
}

/// The cells lens's roads: from the upstream cell's membrane to the
/// downstream one's, named by the relationship's type, an arrow pointing
/// downstream.
fn roads(ui: &egui::Ui, map: &ProjectMap, view: &View) {
    for (up, down, kind) in &map.roads {
        let (a, b) = (map.districts[*up].area, map.districts[*down].area);
        let dir = (b.center() - a.center()).normalized();
        let start = a.center() + dir * a.width() / 2.0;
        let end = b.center() - dir * b.width() / 2.0;
        let line = [view.to_screen(start, 0.0), view.to_screen(end, 0.0)];
        ui.painter().line_segment(line, Stroke::new(9.0, NIGHT));
        ui.painter().line_segment(line, Stroke::new(6.0, ROAD));
        ui.painter()
            .extend(Shape::dashed_line(&line, Stroke::new(1.0, DIM), 6.0, 6.0));
        let back = (line[0] - line[1]).normalized() * 12.0;
        let side = vec2(-back.y, back.x) * 0.5;
        let tip = line[1];
        let arrow = vec![tip, tip + back + side, tip + back - side];
        ui.painter()
            .add(Shape::convex_polygon(arrow, DIM, Stroke::NONE));
        let middle = line[0] + (line[1] - line[0]) * 0.5;
        let galley =
            ui.painter()
                .layout_no_wrap(kind.clone(), egui::FontId::proportional(11.0), INK);
        let label = Rect::from_center_size(middle, galley.size() + vec2(10.0, 4.0));
        ui.painter().rect_filled(label, 3.0, PANEL);
        ui.painter().galley(label.min + vec2(5.0, 2.0), galley, INK);
    }
}

/// The co-change lens's orbits: a faint dashed circle round its hub through
/// every other district it arranged.
fn orbits(ui: &egui::Ui, map: &ProjectMap, view: &View) {
    let Some(hub) = map.hub.and_then(|hub| map.districts.get(hub)) else {
        return;
    };
    let centre = hub.area.center();
    for district in &map.districts {
        if district.path == hub.path || district.kind != DistrictKind::Cluster {
            continue;
        }
        let distance = (district.area.center() - centre).length();
        let mut orbit = view.ring(
            Rect::from_center_size(centre, Vec2::splat(2.0 * distance)),
            0.0,
        );
        orbit.push(orbit[0]);
        ui.painter().extend(Shape::dashed_line(
            &orbit,
            Stroke::new(1.0, DIM.gamma_multiply(0.25)),
            2.0,
            8.0,
        ));
    }
}

/// The tower one of whose lines is under the screen point `at`, the nearest
/// first, the cellar left out unless `underground`: a line is hit only where it has characters, so a tower is reached
/// through the gaps of the towers in front of it.
fn pick(towers: &[DrawnTower], at: Pos2, underground: bool) -> Option<PathBuf> {
    towers
        .iter()
        .rev()
        // The cellar is out of reach unless the view is underground.
        .filter(|tower| underground || !tower.cellar)
        .find(|tower| {
            tower
                .floors
                .iter()
                .any(|floor| floor.distance(at) <= PICK_REACH)
        })
        .map(|tower| tower.path.clone())
}

impl<'a> Painter<'a> {
    fn group_with_children(&mut self, ui: &mut egui::Ui, group: &'a Group, inside: bool) {
        let shown = self.ground(ui, group.area, &group.name, &group.path, inside, false);
        for inner in &group.groups {
            self.group_with_children(ui, inner, shown);
        }
        self.nodes(ui, &group.nodes, shown);
    }

    /// Draws the folder at `path`, in full when it is shown: when its parent
    /// is shown (`inside`), it is the folder view, or there is no folder view.
    /// Outside the folder view the city stays, faded, and its names are not
    /// put on the map. A district is a plaza: a round ground faintly tinted in
    /// its colour with a dashed edge; a deeper folder has no ground of its
    /// own, only its name. Returns whether it is shown, which is whether what
    /// it holds is shown.
    fn ground(
        &mut self,
        ui: &egui::Ui,
        area: Rect,
        name: &'a str,
        path: &'a Path,
        inside: bool,
        district: bool,
    ) -> bool {
        let shown = inside || self.folder_view.is_none_or(|folder| folder == path);
        let fade = if shown { 1.0 } else { OUTSIDE };
        if district {
            let ring = match self.outline {
                Outline::Plaza | Outline::Orbit | Outline::Membrane | Outline::Own => {
                    self.view.ring(area, self.lift)
                }
                Outline::Step => self.view.corners(area, self.lift),
                Outline::Hexagon => self.view.polygon(area, 6, self.lift),
            };
            let fill = self.colour.gamma_multiply(0.03 * fade);
            ui.painter()
                .add(Shape::convex_polygon(ring.clone(), fill, Stroke::NONE));
            let mut closed = ring;
            closed.push(closed[0]);
            let edge = Stroke::new(1.0, self.colour.gamma_multiply(0.25 * fade));
            let solid = Stroke::new(1.5, self.colour.gamma_multiply(0.4 * fade));
            match self.outline {
                Outline::Plaza => ui
                    .painter()
                    .extend(Shape::dashed_line(&closed, edge, 6.0, 6.0)),
                Outline::Step | Outline::Hexagon | Outline::Own => {
                    ui.painter().add(Shape::line(closed, solid));
                }
                Outline::Membrane => {
                    let membrane = Stroke::new(6.0, self.colour.gamma_multiply(0.12 * fade));
                    ui.painter().add(Shape::line(closed.clone(), membrane));
                    let ring = Stroke::new(1.8, self.colour.gamma_multiply(0.5 * fade));
                    ui.painter().add(Shape::line(closed, ring));
                    let inner = Rect::from_center_size(area.center(), area.size() * 0.94);
                    let mut inner = self.view.ring(inner, self.lift);
                    inner.push(inner[0]);
                    let thin = Stroke::new(1.0, self.colour.gamma_multiply(0.25 * fade));
                    ui.painter().add(Shape::line(inner, thin));
                }
                Outline::Orbit => {
                    ui.painter().add(Shape::line(closed, solid));
                    let inner = Rect::from_center_size(area.center(), area.size() * 0.88);
                    let mut inner = self.view.ring(inner, self.lift);
                    inner.push(inner[0]);
                    ui.painter()
                        .extend(Shape::dashed_line(&inner, edge, 3.0, 5.0));
                }
            }
            self.plaza = Some(area.center());
            if !shown {
                let front = Rect::from_points(&self.view.ring(area, self.lift)).center_bottom();
                ui.painter().text(
                    front,
                    egui::Align2::CENTER_BOTTOM,
                    name,
                    egui::FontId::proportional(13.0),
                    self.colour.gamma_multiply(fade),
                );
            }
        }
        if !shown {
            return false;
        }
        let naming = if district {
            Naming::District(self.colour)
        } else {
            Naming::Folder
        };
        self.labels.push((area, naming, name, path, self.lift));
        true
    }

    /// Draws the blocks of `nodes` and queues their towers; faded, unnamed
    /// and not to be clicked when they lie outside the folder view.
    fn nodes(&mut self, ui: &egui::Ui, nodes: &'a [Node], shown: bool) {
        let fade = if shown { 1.0 } else { OUTSIDE };
        for node in nodes {
            if let Some(plaza) = self.plaza {
                let avenue = [
                    self.view.to_screen(plaza, self.lift),
                    self.view.to_screen(node.area.center(), self.lift),
                ];
                let stroke = Stroke::new(1.0, self.colour.gamma_multiply(0.2 * fade));
                ui.painter().line_segment(avenue, stroke);
            }
            // A block without code is ground, not a place to build on; a
            // block of only tests lies in the cellar, outlined.
            let cellar = !node.items.is_empty() && node.items.iter().all(city::in_cellar);
            let (fill, edge) = if cellar {
                (Color32::TRANSPARENT, EDGE.gamma_multiply(0.4))
            } else if node.items.is_empty() {
                (GROUND.gamma_multiply(0.4), EDGE.gamma_multiply(0.6))
            } else {
                (GROUND.gamma_multiply(0.85), self.colour.gamma_multiply(0.3))
            };
            ui.painter().add(Shape::convex_polygon(
                self.view.corners(node.area, self.lift),
                fill.gamma_multiply(fade),
                Stroke::new(1.0, edge.gamma_multiply(fade)),
            ));
            if self.cycles.contains(&node.path) {
                // An import cycle: a warning, dashed and glowing, and said.
                let mut edge = self.view.corners(node.area.expand(2.0), self.lift);
                edge.push(edge[0]);
                ui.painter().add(Shape::line(
                    edge.clone(),
                    Stroke::new(4.0, WARNING.gamma_multiply(0.15 * fade)),
                ));
                ui.painter().extend(Shape::dashed_line(
                    &edge,
                    Stroke::new(1.5, WARNING.gamma_multiply(0.9 * fade)),
                    5.0,
                    3.0,
                ));
                ui.painter().text(
                    self.view.to_screen(node.area.left_bottom(), self.lift),
                    egui::Align2::RIGHT_TOP,
                    "import cycle",
                    egui::FontId::proportional(11.0),
                    WARNING.gamma_multiply(fade),
                );
            }
            if shown {
                self.labels.push((
                    node.area,
                    Naming::Block(!node.items.is_empty()),
                    &node.name,
                    &node.path,
                    self.lift,
                ));
                if !cellar || self.underground {
                    self.blocks.push((node.area, &node.path, self.lift));
                }
            }
            for item in &node.items {
                let colour = match item.kind {
                    ItemKind::Test => TEST_COLOUR,
                    _ => self
                        .homes
                        .get(&item.path)
                        .map_or(self.colour, |home| district_colour(*home)),
                };
                self.towers.push((item, colour.gamma_multiply(fade)));
            }
        }
    }

    /// Draws the queued towers, the farthest first so nearer towers stand in
    /// front of them, and reports each as drawn, with the tower one of whose
    /// lines is under `pointer`. Towers in front of the tower pointed at, or
    /// else the selected one, turn to glass where they would hide it. Far out
    /// a floor is one point at its indentation; close enough, one point per
    /// token.
    fn towers(
        &mut self,
        ui: &egui::Ui,
        pointer: Option<Pos2>,
    ) -> (Vec<DrawnTower>, Option<PathBuf>) {
        let mut towers = std::mem::take(&mut self.towers);
        let view = self.view;
        let depth = |item: &Item| depth(&view, item);
        towers.sort_by(|(one, _), (other, _)| depth(one).total_cmp(&depth(other)));
        let scale = self.view.scale();
        let tokens = scale >= TOKENS_FROM;
        let point = if tokens {
            (scale * 0.32).clamp(1.0, 3.0)
        } else {
            1.8
        };
        let mut drawn: Vec<DrawnTower> = towers
            .iter()
            .map(|(item, colour)| DrawnTower {
                path: item.path.clone(),
                colour: *colour,
                floors: (0..item.floors.len())
                    .map(|index| {
                        let z = city::floor_z(item, index);
                        let (start, end) = city::floor_span(item, index);
                        Span {
                            start: self.view.to_screen(start, z),
                            end: self.view.to_screen(end, z),
                        }
                    })
                    .collect(),
                outline: Rect::from_points(&self.solid(item).concat()),
                glass: false,
                light: 1.0,
                cellar: city::in_cellar(item),
            })
            .collect();
        let hovered = pointer.and_then(|at| pick(&drawn, at, self.underground));
        let focus = hovered.as_deref().or(self.selected);
        if let Some(index) = drawn
            .iter()
            .position(|tower| Some(tower.path.as_path()) == focus)
        {
            let hidden = drawn[index].outline;
            for tower in &mut drawn[index + 1..] {
                tower.glass = tower.outline.intersects(hidden);
            }
        }

        // Close enough, a floor is its line, written along the wall with one
        // character per column, so each letter stands where its point stood.
        let advance = ui.fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(1.0), '0'));
        let font = city::COLUMN * scale / advance.max(f32::EPSILON);
        let letters = (font >= LETTERS_FROM).then_some(font);
        let turn = self.facing.map_or(0.0, |(_, turn)| turn);
        let clip = ui.painter().clip_rect();
        let mut shapes = Vec::new();
        for ((item, _), tower) in towers.iter().zip(drawn.iter_mut()) {
            let focused = Some(item.path.as_path()) == focus;
            let mut lit = if focused { FOCUSED } else { 1.0 };
            // Written walls crowd each other: the one being read stands out.
            if letters.is_some() && focus.is_some() && !focused {
                lit *= 0.5;
            }
            if tower.glass {
                lit *= GLASS;
            }
            if let Some((pivot, turn)) = self.facing {
                lit *= 1.0 + (fog(item, pivot) - 1.0) * turn;
            }
            // The cellar shows faintly; the underground view brings it
            // forward and the city above steps back.
            lit *= match (city::in_cellar(item), self.underground) {
                (true, false) => CELLAR,
                (false, true) => ABOVE_UNDERGROUND,
                _ => 1.0,
            };
            tower.light = lit;
            if !clip.intersects(tower.outline) {
                continue;
            }
            self.body(&mut shapes, item, tower.colour, lit, turn);
            for (index, floor) in item.floors.iter().enumerate() {
                let z = city::floor_z(item, index);
                let bright = if floor.calls.is_empty() {
                    LINE
                } else {
                    CALLING_LINE
                };
                let fill = glow(tower.colour, bright * lit);
                if let Some(font) = letters {
                    shapes.push(self.written(ui, item, index, font, tower.colour, lit));
                    continue;
                }
                if scale >= DASHES_FROM {
                    for (start, end) in city::token_spans(item, index) {
                        let line = [self.view.to_screen(start, z), self.view.to_screen(end, z)];
                        shapes.push(Shape::line_segment(line, Stroke::new(point, fill)));
                    }
                    continue;
                }
                if tokens {
                    for ground in city::token_points(item, index) {
                        let at = self.view.to_screen(ground, z);
                        let square = Rect::from_center_size(at, vec2(point, point));
                        shapes.push(Shape::rect_filled(square, 0.0, fill));
                    }
                } else {
                    // Far out a floor is one stroke as wide as its line, so a
                    // tower keeps its width.
                    let (start, end) = city::floor_span(item, index);
                    let line = [self.view.to_screen(start, z), self.view.to_screen(end, z)];
                    shapes.push(Shape::line_segment(line, Stroke::new(FAR_STROKE, fill)));
                }
            }
        }
        // Brackets at the corners of the selected tower, and fainter ones
        // around the one pointed at.
        for (tower, colour) in [
            (self.selected, Color32::WHITE.gamma_multiply(0.85)),
            (hovered.as_deref(), ATTENTION.gamma_multiply(0.45)),
        ] {
            if let Some(tower) = tower.and_then(|path| drawn.iter().find(|t| t.path == path)) {
                brackets(&mut shapes, tower.outline.expand(4.0), colour);
            }
        }
        ui.painter().extend(shapes);
        if let (Some(at), Some((item, _))) = (
            pointer,
            towers
                .iter()
                .find(|(item, _)| Some(item.path.as_path()) == hovered.as_deref()),
        ) {
            tip(ui, at, item);
        }
        (drawn, hovered)
    }

    /// The corners of `item`'s box on screen: its front face, and the side
    /// at the end of its lines and its roof, each as four corners. A box in
    /// the cellar hangs from the ground, its "roof" at its bottom.
    fn solid(&self, item: &Item) -> [Vec<Pos2>; 3] {
        let length = item.area.width().max(item.area.height());
        let ground = item.base;
        let height = if city::in_cellar(item) {
            item.base - city::reach(item)
        } else {
            city::height(item)
        };
        let at = |point: Pos2, z: f32| self.view.to_screen(point, z);
        let (front_start, front_end) = (city::front(item, 0.0), city::front(item, length));
        let (back_start, back_end) = (city::back(item, 0.0), city::back(item, length));
        [
            vec![
                at(front_start, ground),
                at(front_end, ground),
                at(front_end, height),
                at(front_start, height),
            ],
            vec![
                at(front_end, ground),
                at(back_end, ground),
                at(back_end, height),
                at(front_end, height),
            ],
            vec![
                at(front_start, height),
                at(front_end, height),
                at(back_end, height),
                at(back_start, height),
            ],
        ]
    }

    /// Draws `item`'s box: a dark front face, a darker side and a roof, edged
    /// in `colour`, so a nearer building hides the ones behind it. `lit`
    /// thins it out with the tower; the side and roof fade as the camera
    /// turns `turn` of the way to face a facade.
    fn body(&self, shapes: &mut Vec<Shape>, item: &Item, colour: Color32, lit: f32, turn: f32) {
        let [front, side, roof] = self.solid(item);
        let opacity = (lit.min(1.0) * SOLID).clamp(0.0, SOLID);
        let edge = colour.gamma_multiply((lit * EDGE_LIGHT).min(1.0));
        let body = 1.0 - turn;
        if body > 0.0 {
            let side_fill = SIDE.gamma_multiply(opacity * body);
            shapes.push(Shape::convex_polygon(side.clone(), side_fill, Stroke::NONE));
            let roof_fill = GROUND.gamma_multiply(opacity * body);
            let roof_edge = Stroke::new(1.0, edge.gamma_multiply(body));
            shapes.push(Shape::convex_polygon(roof, roof_fill, roof_edge));
            shapes.push(Shape::line_segment([side[1], side[2]], roof_edge));
        }
        let face = GROUND.gamma_multiply(opacity);
        shapes.push(Shape::convex_polygon(front.clone(), face, Stroke::NONE));
        let edge = Stroke::new(1.0, edge);
        shapes.push(Shape::line_segment([front[0], front[3]], edge));
        shapes.push(Shape::line_segment([front[1], front[2]], edge));
    }

    /// Floor `index` of `item` as its line of code, written along the wall at
    /// `font` size from the wall's start, a character per column, cut off at
    /// the wall's end.
    fn written(
        &self,
        ui: &egui::Ui,
        item: &Item,
        index: usize,
        font: f32,
        colour: Color32,
        lit: f32,
    ) -> Shape {
        let floor = &item.floors[index];
        let z = city::floor_z(item, index);
        let start = self.view.to_screen(city::front(item, 0.0), z);
        let end = self.view.to_screen(city::front(item, city::WIDEST), z);
        let length = item.area.width().max(item.area.height());
        let columns = (length / city::COLUMN).round() as usize;
        let text: String = floor
            .text
            .replace('\t', "    ")
            .chars()
            .take(columns)
            .collect();
        let bright = if floor.calls.is_empty() { 0.75 } else { 1.0 };
        let galley = ui.painter().layout_no_wrap(
            text,
            egui::FontId::monospace(font),
            colour.gamma_multiply((bright * lit).min(1.0)),
        );
        let angle = (end - start).angle();
        // The galley hangs from its top left corner; lift it so the letters
        // stand on the floor's line.
        let lift = egui::emath::Rot2::from_angle(angle) * vec2(0.0, -galley.size().y * 0.8);
        egui::epaint::TextShape::new(start + lift, galley, colour)
            .with_angle(angle)
            .into()
    }

    /// Writes `name` on the ground `area` as `naming` says, outlines that
    /// ground when `path` is selected, and notes a click on the name.
    fn label(
        &mut self,
        ui: &mut egui::Ui,
        area: Rect,
        naming: Naming,
        name: &str,
        path: &Path,
        lift: f32,
    ) {
        let bounds = match naming {
            Naming::District(_) => Rect::from_points(&self.view.ring(area, lift)),
            Naming::Folder | Naming::Block(_) => Rect::from_points(&self.view.corners(area, lift)),
        };
        let strip = TITLE.min(bounds.height());
        let front = Rect::from_min_max(pos2(bounds.min.x, bounds.max.y - strip), bounds.max);
        let quiet = if self.view.scale() >= NAMES_FROM {
            DIM
        } else {
            Color32::TRANSPARENT
        };
        let (at, text) = match naming {
            Naming::District(colour) => {
                // Just as wide as the name, so it neither truncates under a
                // small plaza nor covers what stands beside it under a
                // large one.
                let size = if self.view.scale() > 1.0 { 15.0 } else { 12.0 };
                let width = name.chars().count() as f32 * size * 0.62 + 16.0;
                let at = Rect::from_center_size(front.center(), vec2(width, front.height()));
                (at, RichText::new(name).strong().size(size).color(colour))
            }
            // Folders below a district and blocks without code are ground,
            // not landmarks: named for the accessibility tree only.
            Naming::Folder | Naming::Block(false) => (
                front,
                RichText::new(name)
                    .monospace()
                    .size(11.0)
                    .color(Color32::TRANSPARENT),
            ),
            Naming::Block(true) => {
                let front = self.view.to_screen(pos2(area.center().x, area.max.y), lift);
                let below = Rect::from_min_size(
                    pos2(front.x - bounds.width() / 2.0, front.y + 4.0),
                    vec2(bounds.width(), 16.0),
                );
                (
                    below,
                    RichText::new(name).monospace().size(11.0).color(quiet),
                )
            }
        };
        // The sense goes on the response, not the widget, so the entry stays a
        // label in the accessibility tree.
        let response = ui
            .put(
                at,
                egui::Label::new(text)
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .interact(if self.arranging && matches!(naming, Naming::District(_)) {
                Sense::click_and_drag()
            } else {
                Sense::click()
            });
        if response.dragged() || response.drag_stopped() {
            let mut by = response.drag_delta();
            // The drag starts once the pointer has moved a little: the move
            // counts from where the button went down.
            if response.drag_started()
                && let (Some(down), Some(now)) = (
                    ui.input(|i| i.pointer.press_origin()),
                    response.interact_pointer_pos(),
                )
            {
                by = now - down;
            }
            self.name_dragged = Some((path.to_path_buf(), by, response.drag_stopped()));
        }
        if self.selected == Some(path) {
            let stroke = ui.visuals().selection.stroke;
            ui.painter()
                .add(Shape::closed_line(self.view.corners(area, lift), stroke));
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_selected(true));
        }
        if response.clicked() {
            self.clicked = Some(path.to_path_buf());
        }
    }
}

/// How far back `item` stands as `view` sees it: lower on screen is nearer,
/// and towers further back are drawn first.
fn depth(view: &View, item: &Item) -> f32 {
    view.to_screen(item.area.center(), 0.0).y
}

/// The minimap's size, and its distance from the panel's edge.
const MINIMAP: Vec2 = vec2(210.0, 140.0);
const MINIMAP_MARGIN: f32 = 12.0;

/// Draws the minimap of `map` in the bottom right corner of `panel`: the city
/// seen as the main view sees it, one point per tower in its district's
/// colour, and a frame around the part of the ground `view` shows. Returns
/// the path of the district clicked on it this frame. Each district is named
/// "<district> on the minimap" in the accessibility tree, and the frame "View
/// on the minimap".
pub fn minimap(ui: &mut egui::Ui, map: &ProjectMap, view: &View, panel: Rect) -> Option<PathBuf> {
    let seen = map.districts.iter().fold(Rect::NOTHING, |seen, district| {
        seen.union(iso_bounds(district.area))
    });
    if map.is_empty() || !seen.is_positive() {
        return None;
    }
    let at = Rect::from_min_size(panel.max - MINIMAP - Vec2::splat(MINIMAP_MARGIN), MINIMAP);
    let inner = at.shrink(6.0);
    let k = (inner.width() / seen.width()).min(inner.height() / seen.height());
    let offset = inner.center().to_vec2() - seen.center().to_vec2() * k;
    let to_minimap = |ground: Pos2| (iso(ground, 0.0).to_vec2() * k + offset).to_pos2();

    let painter = ui.painter_at(at);
    painter.rect(
        at,
        0.0,
        PANEL.gamma_multiply(0.92),
        Stroke::new(1.0, DIM.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    let mut clicked = None;
    for (index, district) in map.districts.iter().enumerate() {
        let colour = district_colour(index);
        for item in crate::graph::items(district) {
            let fill = match item.kind {
                ItemKind::Test => TEST_COLOUR,
                _ => map
                    .homes
                    .get(&item.path)
                    .map_or(colour, |home| district_colour(*home)),
            };
            let point = to_minimap(item.area.center());
            painter.rect_filled(
                Rect::from_center_size(point, vec2(1.6, 1.6)),
                0.0,
                fill.gamma_multiply(0.8),
            );
        }
        let area = district.area;
        let corners = [
            area.left_top(),
            area.right_top(),
            area.right_bottom(),
            area.left_bottom(),
        ];
        let ring = Rect::from_points(&corners.map(to_minimap));
        let id = ui.id().with(("minimap", &district.path));
        let response = ui.interact(ring.intersect(at), id, Sense::click());
        let label = format!("{} on the minimap", district.name);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Button);
            node.set_label(label);
        });
        if response.clicked() {
            clicked = Some(district.path.clone());
        }
    }
    let corners: Vec<Pos2> = [
        panel.left_top(),
        panel.right_top(),
        panel.right_bottom(),
        panel.left_bottom(),
    ]
    .into_iter()
    .map(|corner| to_minimap(view.to_ground(corner)))
    .collect();
    let frame = Rect::from_points(&corners).intersect(at);
    painter.add(Shape::closed_line(
        corners,
        Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.8)),
    ));
    painter.text(
        at.min + vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        "minimap",
        egui::FontId::monospace(10.0),
        DIM.gamma_multiply(0.9),
    );
    let response = ui.interact(frame, ui.id().with("minimap view"), Sense::hover());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Image);
        node.set_label("View on the minimap");
        // A widget that only senses hovering gets no bounds of its own.
        let bounds = egui::accesskit::Rect {
            x0: frame.min.x.into(),
            y0: frame.min.y.into(),
            x1: frame.max.x.into(),
            y1: frame.max.y.into(),
        };
        node.set_bounds(bounds);
    });
    clicked
}

/// Rows inside a tower: their height, and where the line number ends, the
/// line starts and the calls start, from the panel's left edge.
const ROW: f32 = 17.0;
const NUMBER_END: f32 = 70.0;
const TEXT_START: f32 = 86.0;
const CALLS_START: f32 = 760.0;

/// Draws the tower at `item` from inside, face on, over the city dimmed
/// behind it: one row per floor in source order, each with its line number,
/// its line, a tick under each token in the tower's colour, and the items it
/// calls on the right; floor `floor`, from 0, selected. Returns the floor
/// whose row was clicked this frame. Each row is named by its text in the
/// accessibility tree and marked when selected.
pub fn inside(
    ui: &mut egui::Ui,
    map: &ProjectMap,
    item: &Path,
    floor: usize,
    panel: Rect,
) -> Option<usize> {
    let item = map.item(item)?;
    // Over the city: the city takes no click while a tower is open.
    ui.interact(panel, ui.id().with("inside"), Sense::click());
    let painter = ui.painter_at(panel);
    painter.rect_filled(panel, 0.0, NIGHT.gamma_multiply(0.86));
    let colour = match item.kind {
        ItemKind::Test => TEST_COLOUR,
        _ => district_colour_of(map, &item.path),
    };
    let left = panel.min.x + 40.0;
    let file = map.find(item.path.parent()?).map_or("", |found| found.name);
    painter.text(
        pos2(left, panel.min.y + 24.0),
        egui::Align2::LEFT_TOP,
        format!("Inside {}", item.name),
        egui::FontId::proportional(18.0),
        INK,
    );
    let selected_line = item.floors.get(floor).map_or(0, |f| f.number);
    painter.text(
        pos2(left, panel.min.y + 50.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{file} · floor {} of {} · line {selected_line}   j k move · Esc leave",
            floor + 1,
            item.floors.len()
        ),
        egui::FontId::monospace(12.0),
        DIM,
    );

    let top = panel.min.y + 84.0;
    let rows = ((panel.max.y - top) / ROW).floor().max(1.0) as usize;
    let first = floor
        .saturating_sub(rows / 2)
        .min(item.floors.len().saturating_sub(rows));
    let mut clicked = None;
    for (index, line) in item.floors.iter().enumerate().skip(first).take(rows) {
        let y = top + (index - first) as f32 * ROW;
        let row = Rect::from_min_size(pos2(left - 6.0, y), vec2(panel.max.x - left - 20.0, ROW));
        let selected = index == floor;
        if selected {
            painter.rect_filled(row, 0.0, ATTENTION_SOFT.gamma_multiply(0.9));
            let bar = Rect::from_min_size(row.min, vec2(2.0, ROW));
            painter.rect_filled(bar, 0.0, ATTENTION);
        }
        let middle = y + ROW / 2.0;
        painter.text(
            pos2(left + NUMBER_END, middle),
            egui::Align2::RIGHT_CENTER,
            line.number.to_string(),
            egui::FontId::monospace(11.0),
            DIM.gamma_multiply(0.9),
        );
        let text = painter.layout_no_wrap(
            line.text.replace('\t', "    "),
            egui::FontId::monospace(12.0),
            INK.gamma_multiply(0.92),
        );
        let column = text.size().x / line.text.chars().count().max(1) as f32;
        let text_at = pos2(left + TEXT_START, middle - text.size().y / 2.0);
        for token in &line.tokens {
            let tick = Rect::from_min_size(
                pos2(text_at.x + token.start as f32 * column, middle + 6.0),
                vec2(3.0, 1.5),
            );
            painter.rect_filled(tick, 0.0, colour.gamma_multiply(0.35));
        }
        painter.galley(text_at, text, INK);
        let calls: Vec<&str> = line
            .calls
            .iter()
            .filter_map(|call| map.find(call).map(|found| found.name))
            .collect();
        if !calls.is_empty() {
            painter.text(
                pos2(left + CALLS_START, middle),
                egui::Align2::LEFT_CENTER,
                format!("→ {}", calls.join(", ")),
                egui::FontId::monospace(12.0),
                FLOW,
            );
        }

        let response = ui.interact(row, ui.id().with(("floor", index)), Sense::click());
        let mut name = format!("{:>5}  {}", line.number, line.text);
        if !calls.is_empty() {
            name.push_str(&format!("    calls {}", calls.join(", ")));
        }
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::ListBoxOption);
            node.set_label(name);
            node.set_toggled(if selected {
                egui::accesskit::Toggled::True
            } else {
                egui::accesskit::Toggled::False
            });
        });
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// The colour of the district the element at `path` lies in: the innermost
/// district whose path holds it.
fn district_colour_of(map: &ProjectMap, path: &Path) -> Color32 {
    if let Some(home) = map.homes.get(path) {
        return district_colour(*home);
    }
    map.districts
        .iter()
        .enumerate()
        .filter(|(_, district)| path.starts_with(&district.path))
        .max_by_key(|(_, district)| district.path.as_os_str().len())
        .map_or(DISTRICT_COLOURS[0], |(index, _)| district_colour(index))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{Directories, default_prefix_rule};

    #[test]
    fn a_folder_view_still_draws_the_city_around_it_faded() {
        use std::collections::BTreeMap;

        use crate::graph::{Floor, Item, ItemKind};

        let tower = |file: &str| Item {
            name: "f".to_owned(),
            path: Path::new("/p/shop").join(file).join("f"),
            kind: ItemKind::Function,
            tests: false,
            floors: vec![Floor {
                number: 1,
                text: "fn f() {}".to_owned(),
                indent: 0,
                width: 9,
                tokens: Vec::new(),
                calls: Vec::new(),
                imports: Vec::new(),
            }],
            area: Rect::NOTHING,
            base: 0.0,
        };
        let files = ["catalog/a.rs", "orders/b.rs"].map(PathBuf::from);
        let items = BTreeMap::from(files.clone().map(|file| {
            let item = tower(file.to_str().unwrap());
            (file, vec![item])
        }));
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of_code("shop", Path::new("/p/shop"), &files, &strategy, items);
        let framing = Framing {
            area: map.extent,
            fill: 1.0,
            closest: 1.0,
            headroom: 0.0,
        };
        let catalog = Path::new("/p/shop/catalog");

        let ctx = egui::Context::default();
        let mut towers = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            towers = project(
                ui,
                &map,
                ViewOptions {
                    framing,
                    selected: None,
                    folder_view: Some(catalog),
                    pivot: None,
                    underground: false,
                    lanes: Lanes::default(),
                    picked: None,
                },
            )
            .towers;
        });
        output.textures_delta.clear();

        assert_eq!(
            towers.len(),
            2,
            "the tower outside the folder view is drawn"
        );
        let colour = |file: &str| {
            let path = Path::new("/p/shop").join(file).join("f");
            towers.iter().find(|t| t.path == path).map(|t| t.colour)
        };
        let (inside, outside) = (
            colour("catalog/a.rs").unwrap(),
            colour("orders/b.rs").unwrap(),
        );
        assert!(
            outside.a() < inside.a(),
            "{outside:?} is not fainter than {inside:?}"
        );
    }
}
