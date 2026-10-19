//! Painting what makes a building more than a box: the crowns on a
//! skyscraper, the row houses of a terrace, a monument's obelisk and an entry
//! point's spire; and how its code is used: doors, a courtyard, a forecourt,
//! masts and skybridges. The tiers of a skyscraper are towers of their own,
//! drawn with their floors like any other.
//!
//! The buildings of files that are not code: a library's hall and
//! colonnade, the wires between cabinets, a courthouse's plinth, a plant's
//! pipes, a depot's sawtooth roof and docks, a customs house's upper storey,
//! border and roads, a vault and its light, a ghost, a slab and a
//! billboard.

use std::path::PathBuf;

use egui::{Color32, Pos2, Rect, Shape, Stroke, pos2, vec2};

use super::{
    CABLE, CROWN_FILL, CROWN_HUES, DIM, DrawnTower, FLOW, GROUND, INK, LINE_STRONG, NIGHT, PANEL,
    PAVING, SCHEMA, SECRETS, SOLID, VAULT,
};
use crate::arrange::seed;
use crate::buildings::{self as building, CROWN, HALL, HOUSE};
use crate::city;
use crate::graph::Item;
use crate::{Form, View};

/// One building as drawn this frame: the path of its base, its form, its
/// tiers bottom to top, the interfaces its crowns stand for, for a terrace
/// whether each house has an annex, whether it carries a spire, how many
/// doors are on its front, whether a courtyard lies behind it and a
/// forecourt in front, and for each mast shown whether it hangs upside down.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnBuilding {
    pub path: PathBuf,
    pub form: Form,
    pub tiers: Vec<PathBuf>,
    pub crowns: Vec<String>,
    pub houses: Vec<bool>,
    pub spire: bool,
    pub doors: usize,
    pub courtyard: bool,
    pub forecourt: bool,
    pub masts: Vec<bool>,
}

/// One skybridge as drawn this frame: the bases of the buildings it joins,
/// how high it runs, and the pieces it was drawn in, each with how many
/// towers were drawn before it and its middle on screen.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnBridge {
    pub from: PathBuf,
    pub to: PathBuf,
    pub z: f32,
    pub pieces: Vec<(usize, Pos2)>,
}

impl DrawnBridge {
    /// How many of its pieces a tower of `towers`, as drawn back to front
    /// the same frame, drawn after the piece stands in front of. Worked out
    /// when asked rather than every frame.
    pub fn hidden_pieces(&self, towers: &[DrawnTower]) -> usize {
        self.pieces
            .iter()
            .filter(|(before, middle)| {
                towers[*before..]
                    .iter()
                    .any(|tower| tower.outline.contains(*middle))
            })
            .count()
    }
}

/// The most doors on a front.
pub(super) const DOORS: usize = 16;

/// How wide a door is at the widest and how tall.
const DOOR: (f32, f32) = (0.9, 1.6);

/// How wide each of `n` doors is on a front `length` long: at most
/// [`DOOR`] wide, and together no wider than eight tenths of the front, so
/// doors never overlap.
fn door_width(length: f32, n: usize) -> f32 {
    DOOR.0.min(0.8 * length / n.max(1) as f32)
}

/// How deep a courtyard is, how thick and high its walls.
const YARD: f32 = 3.0;
const YARD_WALL: (f32, f32) = (0.4, 1.2);

/// How far a forecourt reaches past its building on either side, and how
/// far in front of the building it starts.
const FORECOURT_SIDE: f32 = 1.0;
const FORECOURT_GAP: f32 = 0.5;

/// How half as wide and how thick a skybridge is.
const BRIDGE: (f32, f32) = (0.8, BRIDGE_DEPTH);

/// How deep a skybridge is, from its underside to its deck.
pub(super) const BRIDGE_DEPTH: f32 = 0.9;

/// How tall a crown's lower step is.
const CROWN_STEP: f32 = 0.8;

/// How tall a monument's plinth is, the side of its obelisk and of its cap,
/// and the least the obelisk rises.
const PLINTH: f32 = 0.8;
const OBELISK: f32 = 1.6;
const CAP: f32 = 1.0;
const OBELISK_LEAST: f32 = 5.0;

/// The side and height of a spire's plinth, the least its shaft rises, and
/// how large its beacon is at the least and the most.
const SPIRE_PLINTH: f32 = 2.4;
const SPIRE_PLINTH_HEIGHT: f32 = 2.5;
const SHAFT_LEAST: f32 = 14.0;
const BEACON: (f32, f32) = (5.0, 16.0);

/// A box on the ground `area` from height `z0` up to `z1`: its two faces
/// towards the viewer and its top, filled with `fill` and edged in `edge`.
pub(super) fn cube(
    view: &View,
    shapes: &mut Vec<Shape>,
    area: Rect,
    z: (f32, f32),
    edge: Color32,
    fill: Color32,
) {
    for (face, shade) in faces(view, area, z) {
        shapes.push(Shape::convex_polygon(
            face.clone(),
            fill.gamma_multiply(shade),
            Stroke::NONE,
        ));
        let mut closed = face;
        closed.push(closed[0]);
        shapes.push(Shape::line(closed, Stroke::new(1.0, edge)));
    }
}

/// How long the dashes of a dashed box are on screen, and the gaps.
const DASH: (f32, f32) = (3.0, 3.0);

/// [`cube`] edged in dashes.
fn dashed_cube(
    view: &View,
    shapes: &mut Vec<Shape>,
    area: Rect,
    z: (f32, f32),
    edge: Color32,
    fill: Color32,
) {
    for (face, shade) in faces(view, area, z) {
        shapes.push(Shape::convex_polygon(
            face.clone(),
            fill.gamma_multiply(shade),
            Stroke::NONE,
        ));
        let mut closed = face;
        closed.push(closed[0]);
        shapes.extend(Shape::dashed_line(
            &closed,
            Stroke::new(1.0, edge),
            DASH.0,
            DASH.1,
        ));
    }
}

/// The faces of a box on the ground `area` from height `z0` up to `z1`
/// that the viewer sees, its side, its front and its top, each with how
/// much its fill is shaded.
fn faces(view: &View, area: Rect, (z0, z1): (f32, f32)) -> [(Vec<Pos2>, f32); 3] {
    let at = |x: f32, y: f32, z: f32| view.to_screen(pos2(x, y), z);
    let (a, b) = (area.min, area.max);
    let side = vec![
        at(b.x, a.y, z0),
        at(b.x, b.y, z0),
        at(b.x, b.y, z1),
        at(b.x, a.y, z1),
    ];
    let front = vec![
        at(a.x, b.y, z0),
        at(b.x, b.y, z0),
        at(b.x, b.y, z1),
        at(a.x, b.y, z1),
    ];
    let top = vec![
        at(a.x, a.y, z1),
        at(b.x, a.y, z1),
        at(b.x, b.y, z1),
        at(a.x, b.y, z1),
    ];
    [(side, 0.7), (front, 0.85), (top, 1.0)]
}

/// The colour of the crown of the interface `name`: the same for every type
/// implementing it.
pub(super) fn crown_hue(name: &str) -> Color32 {
    CROWN_HUES[seed(name.as_bytes()) as usize % CROWN_HUES.len()]
}

/// The crowns `names` stacked on the roof `top` at height `z`, each two
/// steps in its interface's colour.
pub(super) fn crowns(
    view: &View,
    shapes: &mut Vec<Shape>,
    top: Rect,
    z: f32,
    names: &[String],
    lit: f32,
) {
    let fill = CROWN_FILL.gamma_multiply(lit.min(1.0));
    let lower = top.shrink2(top.size() * 0.15);
    let upper = lower.shrink2(lower.size() * 0.25);
    for (k, name) in names.iter().enumerate() {
        let edge = crown_hue(name).gamma_multiply(lit.min(1.0));
        let z = z + k as f32 * CROWN;
        cube(view, shapes, lower, (z, z + CROWN_STEP), edge, fill);
        cube(view, shapes, upper, (z + CROWN_STEP, z + CROWN), edge, fill);
    }
}

/// The ground `size` big that lies `along` the front of `item` from its start
/// and `back` from its back.
fn part_of_ground(item: &Item, along: f32, back: f32, size: (f32, f32)) -> Rect {
    let corner = item.area.min;
    if city::along_x(item) {
        Rect::from_min_size(corner + vec2(along, back), vec2(size.0, size.1))
    } else {
        Rect::from_min_size(corner + vec2(back, along), vec2(size.1, size.0))
    }
}

/// An enum as a terrace: a house per variant along its front, each with a
/// roof, and an annex behind each one whose variant carries data.
pub(super) fn terrace(
    view: &View,
    shapes: &mut Vec<Shape>,
    item: &Item,
    houses: &[bool],
    colour: Color32,
    lit: f32,
) {
    let edge = colour.gamma_multiply(lit.min(1.0));
    let fill = GROUND.gamma_multiply((lit * SOLID).clamp(0.0, SOLID));
    let z = item.base;
    for (i, annex) in houses.iter().enumerate() {
        let along = 0.3 + i as f32 * HOUSE;
        if *annex {
            let ground = part_of_ground(item, along + 0.4, 0.3, (2.2, 3.2));
            cube(view, shapes, ground, (z, z + 1.6), edge, fill);
        }
        let house = part_of_ground(item, along, 3.7, (3.0, 4.0));
        cube(view, shapes, house, (z, z + 2.2), edge, fill);
        let roof = part_of_ground(item, along + 0.5, 4.7, (2.0, 2.0));
        cube(view, shapes, roof, (z + 2.2, z + 3.1), edge, fill);
    }
}

/// A constant as a monument: a plinth, an obelisk as tall as its lines, at
/// least [`OBELISK_LEAST`], and its cap.
pub(super) fn monument(
    view: &View,
    shapes: &mut Vec<Shape>,
    item: &Item,
    colour: Color32,
    lit: f32,
) {
    let edge = colour.gamma_multiply(lit.min(1.0));
    let fill = GROUND.gamma_multiply((lit * SOLID).clamp(0.0, SOLID));
    let z = item.base;
    let centre = item.area.center();
    let tall = city::reach(item).max(OBELISK_LEAST);
    let plinth = DIM.gamma_multiply(lit.min(1.0));
    cube(view, shapes, item.area, (z, z + PLINTH), plinth, fill);
    let obelisk = Rect::from_center_size(centre, vec2(OBELISK, OBELISK));
    let top = z + PLINTH + tall;
    cube(view, shapes, obelisk, (z + PLINTH, top), edge, fill);
    let cap = Rect::from_center_size(centre, vec2(CAP, CAP));
    cube(view, shapes, cap, (top, top + PLINTH), edge, fill);
}

/// An entry point's spire on the roof of `item`: a plinth, a shaft a third
/// as tall as the building and at least [`SHAFT_LEAST`], and a haloed beacon
/// on top that grows with the zoom, so it stays in sight from far out.
pub(super) fn spire(view: &View, shapes: &mut Vec<Shape>, item: &Item, lit: f32) {
    let beacon = FLOW.gamma_multiply(lit.min(1.0));
    let top = city::height(item);
    let centre = item.area.center();
    let plinth = Rect::from_center_size(centre, vec2(SPIRE_PLINTH, SPIRE_PLINTH));
    let shaft_foot = top + SPIRE_PLINTH_HEIGHT;
    cube(view, shapes, plinth, (top, shaft_foot), beacon, GROUND);
    let tall = (city::reach(item) / 3.0).max(SHAFT_LEAST);
    let foot: Pos2 = view.to_screen(centre, shaft_foot);
    let head = view.to_screen(centre, shaft_foot + tall);
    shapes.push(Shape::line_segment([foot, head], Stroke::new(3.0, beacon)));
    let radius = (6.0 * view.scale()).clamp(BEACON.0, BEACON.1);
    shapes.push(Shape::circle_filled(
        head,
        radius * 2.2,
        beacon.gamma_multiply(0.15),
    ));
    shapes.push(Shape::circle_filled(head, radius, beacon));
}

/// How long `item`'s front is, and how deep its ground.
fn front_and_depth(item: &Item) -> (f32, f32) {
    let (w, h) = (item.area.width(), item.area.height());
    if city::along_x(item) { (w, h) } else { (h, w) }
}

/// `n` lit doors spread across the front of `item`.
pub(super) fn doors(view: &View, shapes: &mut Vec<Shape>, item: &Item, n: usize, lit: f32) {
    let door = FLOW.gamma_multiply(lit.min(1.0));
    let (length, _) = front_and_depth(item);
    let z = item.base;
    let width = door_width(length, n);
    for i in 0..n {
        let middle = (i as f32 + 0.5) * length / n as f32;
        let (a, b) = (
            city::front(item, middle - width / 2.0),
            city::front(item, middle + width / 2.0),
        );
        let quad = vec![
            view.to_screen(a, z),
            view.to_screen(b, z),
            view.to_screen(b, z + DOOR.1),
            view.to_screen(a, z + DOOR.1),
        ];
        shapes.push(Shape::convex_polygon(
            quad,
            door.gamma_multiply(0.8),
            Stroke::new(1.0, door),
        ));
    }
}

/// A low walled courtyard behind `item`, as wide as its back.
pub(super) fn courtyard(view: &View, shapes: &mut Vec<Shape>, item: &Item, lit: f32) {
    let wall = DIM.gamma_multiply(lit.min(1.0));
    let fill = GROUND.gamma_multiply(0.6 * lit.min(1.0));
    let (length, _) = front_and_depth(item);
    let (thick, high) = YARD_WALL;
    let z = item.base;
    for ground in [
        part_of_ground(item, 0.0, -YARD, (length, thick)),
        part_of_ground(item, -thick, -YARD, (thick, YARD)),
        part_of_ground(item, length, -YARD, (thick, YARD)),
    ] {
        cube(view, shapes, ground, (z, z + high), wall, fill);
    }
}

/// A paved forecourt `deep` in front of `item`, a little wider than it.
pub(super) fn forecourt(
    view: &View,
    shapes: &mut Vec<Shape>,
    item: &Item,
    deep: f32,
    colour: Color32,
    lit: f32,
) {
    let (length, depth) = front_and_depth(item);
    let ground = part_of_ground(
        item,
        -FORECOURT_SIDE,
        depth + FORECOURT_GAP,
        (length + 2.0 * FORECOURT_SIDE, deep),
    );
    let edge = Stroke::new(1.0, colour.gamma_multiply(0.35 * lit.min(1.0)));
    let paving = PAVING.gamma_multiply(lit.min(1.0));
    shapes.push(Shape::convex_polygon(
        view.corners(ground, item.base),
        paving,
        edge,
    ));
}

/// `n` masts on the roof `roof` at height `z`, or hanging from it upside
/// down, each with a lit head.
#[allow(clippy::too_many_arguments)]
pub(super) fn masts(
    view: &View,
    shapes: &mut Vec<Shape>,
    roof: Rect,
    z: f32,
    n: usize,
    hangs: bool,
    colour: Color32,
    lit: f32,
) {
    let stroke = Stroke::new(2.0, colour.gamma_multiply(lit.min(1.0)));
    let head = CABLE.gamma_multiply(lit.min(1.0));
    for i in 0..n {
        let (at, tip) = building::mast(roof, z, i, n, hangs);
        let (foot, top) = (view.to_screen(at, z), view.to_screen(at, tip));
        shapes.push(Shape::line_segment([foot, top], stroke));
        shapes.push(Shape::circle_filled(top, 3.0, head));
    }
}

/// How long the dashes of a cable are on screen, and the gaps between them.
const CABLE_DASH: (f32, f32) = (6.0, 4.0);

/// One piece of a cable, from one end to the other, each a ground point and
/// its height: a dashed line.
pub(super) fn cable(view: &View, shapes: &mut Vec<Shape>, [(a, za), (b, zb)]: [(Pos2, f32); 2]) {
    let line = [view.to_screen(a, za), view.to_screen(b, zb)];
    let (dash, gap) = CABLE_DASH;
    shapes.extend(Shape::dashed_line(
        &line,
        Stroke::new(1.5, CABLE),
        dash,
        gap,
    ));
}

/// One piece of a skybridge, from `a` to `b` on the ground, `z` high: its
/// side and its deck.
pub(super) fn bridge(view: &View, shapes: &mut Vec<Shape>, (a, b): (Pos2, Pos2), z: f32, lit: f32) {
    let along = (b - a).normalized();
    let across = vec2(-along.y, along.x) * BRIDGE.0;
    let edge = Stroke::new(1.0, FLOW.gamma_multiply(lit.min(1.0)));
    let fill = CROWN_FILL.gamma_multiply(lit.min(1.0));
    let top = z + BRIDGE.1;
    let side = vec![
        view.to_screen(a - across, z),
        view.to_screen(b - across, z),
        view.to_screen(b - across, top),
        view.to_screen(a - across, top),
    ];
    let deck = [a + across, b + across, b - across, a - across]
        .map(|p| view.to_screen(p, top))
        .to_vec();
    shapes.push(Shape::convex_polygon(side, fill, edge));
    shapes.push(Shape::convex_polygon(deck, fill, edge));
}

/// Lights the skybridge `bridge`, pointed at: its deck edged bright.
pub(super) fn light_bridge(view: &View, shapes: &mut Vec<Shape>, bridge: &building::Bridge) {
    let (a, b) = (bridge.start, bridge.end);
    let along = (b - a).normalized();
    let across = vec2(-along.y, along.x) * BRIDGE.0;
    let top = bridge.z + BRIDGE.1;
    let mut deck = [a + across, b + across, b - across, a - across]
        .map(|p| view.to_screen(p, top))
        .to_vec();
    deck.push(deck[0]);
    shapes.push(Shape::line(deck, Stroke::new(2.0, FLOW)));
}

/// What the skybridge `bridge` stands for, line by line: which type holds
/// which, the field that holds it with its line, and what that means.
pub fn bridge_card(bridge: &building::Bridge) -> [String; 3] {
    let file = bridge
        .from
        .parent()
        .and_then(|file| file.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    [
        format!("Skybridge: {} holds {}", bridge.holder, bridge.held),
        format!("{}    {file}:{}", bridge.field, bridge.line),
        format!(
            "{} is built from {}: one of its fields holds a {}",
            bridge.holder, bridge.held, bridge.held
        ),
    ]
}

/// A card beside the pointer saying what the skybridge `bridge` joins.
pub(super) fn bridge_tip(ui: &egui::Ui, at: Pos2, bridge: &building::Bridge) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        ui.id().with("bridge card"),
    ));
    let [head, field, meaning] = bridge_card(bridge);
    let rows = [
        painter.layout_no_wrap(head, egui::FontId::proportional(13.0), INK),
        painter.layout_no_wrap(field, egui::FontId::monospace(11.0), FLOW),
        painter.layout_no_wrap(meaning, egui::FontId::monospace(11.0), DIM),
    ];
    let width = rows.iter().map(|r| r.size().x).fold(0.0, f32::max);
    let height: f32 = rows.iter().map(|r| r.size().y + 2.0).sum();
    let corner = at + vec2(14.0, 14.0);
    let card = Rect::from_min_size(corner, vec2(width, height)).expand2(vec2(10.0, 6.0));
    painter.rect(
        card,
        0.0,
        PANEL,
        Stroke::new(1.0, LINE_STRONG),
        egui::StrokeKind::Inside,
    );
    let mut y = corner.y;
    for row in rows {
        let size = row.size().y;
        painter.galley(pos2(corner.x, y), row, INK);
        y += size + 2.0;
    }
}

/// How far apart the columns of a colonnade stand at the least, their
/// side, and how far in front of the hall they stand.
const COLONNADE_SPACING: f32 = 2.2;
const COLONNADE_COLUMN: f32 = 0.45;
const COLONNADE_GAP: f32 = 0.3;

/// A library's hall on the ground `hall`, its roof at `roof`, [`HALL`]
/// high, with a colonnade along its front.
pub(super) fn library_hall(
    view: &View,
    shapes: &mut Vec<Shape>,
    hall: Rect,
    roof: f32,
    colour: Color32,
    lit: f32,
) {
    let (edge, fill) = shades(colour, lit);
    let z = (roof - HALL, roof);
    cube(view, shapes, hall, z, edge, fill);
    let n = ((hall.width() / COLONNADE_SPACING) as usize).max(2);
    let spacing = (hall.width() - 2.0 * COLONNADE_GAP - COLONNADE_COLUMN) / (n - 1) as f32;
    for i in 0..n {
        let x = hall.min.x + COLONNADE_GAP + i as f32 * spacing;
        let column = Rect::from_min_size(
            pos2(x, hall.max.y + COLONNADE_GAP),
            vec2(COLONNADE_COLUMN, COLONNADE_COLUMN),
        );
        cube(view, shapes, column, z, edge, fill);
    }
}

/// The edge and fill of a building drawn in `colour`, `lit` as bright.
fn shades(colour: Color32, lit: f32) -> (Color32, Color32) {
    (
        colour.gamma_multiply(lit.min(1.0)),
        GROUND.gamma_multiply((lit * SOLID).clamp(0.0, SOLID)),
    )
}

/// How long the dashes of a wire, a pipe or a road are on screen, and the
/// gaps between them.
const WIRE_DASH: (f32, f32) = (6.0, 4.0);

/// A dashed wire from `a` to `b`, each a ground point and its height.
pub(super) fn wire(
    view: &View,
    shapes: &mut Vec<Shape>,
    (a, za): (Pos2, f32),
    (b, zb): (Pos2, f32),
    colour: Color32,
) {
    let line = [view.to_screen(a, za), view.to_screen(b, zb)];
    shapes.extend(Shape::dashed_line(
        &line,
        Stroke::new(1.2, colour),
        WIRE_DASH.0,
        WIRE_DASH.1,
    ));
}

/// How high a courthouse's plinth is, and how far it reaches past what
/// stands on it.
const PLINTH_REACH: f32 = 0.5;

/// The plinth a courthouse stands on: under `ground`, its top at `top`.
pub(super) fn plinth(
    view: &View,
    shapes: &mut Vec<Shape>,
    ground: Rect,
    top: f32,
    colour: Color32,
    lit: f32,
) {
    let (edge, fill) = shades(colour, lit);
    let z = (top - building::PLINTH, top);
    cube(view, shapes, ground.expand(PLINTH_REACH), z, edge, fill);
}

/// How far a plant's pipes run out from its side, and how many there are.
const PIPE_RUN: f32 = 14.0;
const PIPES: usize = 3;

/// Pipes from the side of the plant `item` out along the ground.
pub(super) fn pipes(view: &View, shapes: &mut Vec<Shape>, item: &Item, colour: Color32, lit: f32) {
    let (a, b) = (item.area.min, item.area.max);
    let depth = item.area.height();
    let colour = colour.gamma_multiply(lit.min(1.0));
    let z = item.base;
    for k in 0..PIPES {
        let y = a.y + depth * (0.3 + 0.2 * k as f32);
        let spread = (k as f32 - 1.0) * 6.0;
        let from = (pos2(b.x, y), z + 2.0_f32.min(city::reach(item)));
        let to = (pos2(b.x + PIPE_RUN, y + spread), z + 0.3);
        wire(view, shapes, from, to, colour);
    }
}

/// The colour of a depot's docks; its roof is paved.
const DOCK: Color32 = Color32::from_rgb(0x9c, 0xff, 0x57);

/// How many teeth a depot's sawtooth roof has, how high they are, and how
/// many loading docks stand along its front.
const TEETH: usize = 5;
const TOOTH: f32 = 1.2;
const DOCKS: usize = 4;

/// A depot's sawtooth roof on `item` and its loading docks in front.
pub(super) fn depot(view: &View, shapes: &mut Vec<Shape>, item: &Item, colour: Color32, lit: f32) {
    let (edge, fill) = shades(colour, lit);
    let (a, b) = (item.area.min, item.area.max);
    let (w, d) = (item.area.width(), item.area.height());
    let top = city::height(item);
    let roof = PAVING.gamma_multiply((lit * SOLID).clamp(0.0, SOLID));
    for i in 0..TEETH {
        let tooth = Rect::from_min_size(
            pos2(a.x + i as f32 * w / TEETH as f32, a.y),
            vec2(w / TEETH as f32, d),
        );
        cube(view, shapes, tooth, (top, top + TOOTH), edge, roof);
    }
    let dock = DOCK.gamma_multiply(lit.min(1.0));
    for i in 0..DOCKS {
        let x = a.x + (i as f32 + 0.5) * w / DOCKS as f32 - 1.0;
        let ground = Rect::from_min_size(pos2(x, b.y), vec2(2.0, 1.5));
        cube(
            view,
            shapes,
            ground,
            (item.base, item.base + 0.6),
            dock,
            fill,
        );
    }
}

/// A customs house's upper storey, half its ground and two high, on its
/// roof.
pub(super) fn upper_storey(
    view: &View,
    shapes: &mut Vec<Shape>,
    item: &Item,
    colour: Color32,
    lit: f32,
) {
    let (edge, fill) = shades(colour, lit);
    let top = city::height(item);
    let upper = item.area.shrink2(item.area.size() * 0.25);
    cube(view, shapes, upper, (top, top + 2.0), edge, fill);
}

/// How far past a customs house its border and roads run.
const BORDER_RUN: f32 = 6.0;

/// The border a customs house on `area` straddles, across its ground, and a
/// road out through each gate, at height `z`.
pub(super) fn border(view: &View, shapes: &mut Vec<Shape>, area: Rect, z: f32, colour: Color32) {
    let (a, b) = (area.min, area.max);
    let run = vec2(BORDER_RUN, BORDER_RUN);
    wire(view, shapes, (a - run, z), (b + run, z), SCHEMA);
    let y = area.center().y;
    wire(
        view,
        shapes,
        (pos2(a.x - BORDER_RUN, y), z),
        (pos2(a.x, y), z),
        colour,
    );
    wire(
        view,
        shapes,
        (pos2(b.x, y), z),
        (pos2(b.x + BORDER_RUN, y), z),
        colour,
    );
}

/// A vault: sealed, dark, edged in the colour of secrets, with a light on
/// its front; in glass, dashed and cyan, for an example of the secrets.
pub(super) fn vault(view: &View, shapes: &mut Vec<Shape>, item: &Item, glass: bool, lit: f32) {
    let z = (item.base, city::height(item));
    let lit = lit.min(1.0);
    if glass {
        let fill = FLOW.gamma_multiply(0.08 * lit);
        dashed_cube(view, shapes, item.area, z, FLOW.gamma_multiply(lit), fill);
        return;
    }
    let fill = VAULT.gamma_multiply((lit * SOLID).clamp(0.0, SOLID));
    let edge = SECRETS.gamma_multiply(lit);
    cube(view, shapes, item.area, z, edge, fill);
    let light = view.to_screen(
        city::front(item, item.area.width().max(item.area.height()) / 2.0),
        (z.0 + z.1) / 2.0,
    );
    let radius = 3.0 * view.scale().clamp(0.5, 2.0);
    shapes.push(Shape::circle_filled(light, radius, edge));
}

/// A ghost, dashed and dim with nothing inside, or a slab sunk below the
/// ground, dashed in `colour` over the night.
pub(super) fn outline(
    view: &View,
    shapes: &mut Vec<Shape>,
    item: &Item,
    form: Form,
    colour: Color32,
    lit: f32,
) {
    let z = (item.base, city::height(item));
    let (edge, fill) = match form {
        Form::Slab => (
            colour,
            NIGHT.gamma_multiply((lit * SOLID).clamp(0.0, SOLID)),
        ),
        _ => (DIM.gamma_multiply(0.6), Color32::TRANSPARENT),
    };
    let edge = edge.gamma_multiply(lit.min(1.0));
    dashed_cube(view, shapes, item.area, z, edge, fill);
}

/// The fill of a billboard's panel, how high its post is and its side.
const BILLBOARD_FILL: Color32 = Color32::from_rgb(0x0b, 0x2a, 0x2e);
const POST: f32 = 3.0;
const POST_SIDE: f32 = 0.5;

/// An image's billboard on the block `ground` at height `z`: a post in its
/// middle and a panel on top, as wide as the block and half as tall.
pub(super) fn billboard(view: &View, shapes: &mut Vec<Shape>, ground: Rect, z: f32, lit: f32) {
    let lit = lit.min(1.0);
    let centre = ground.center();
    let post = Rect::from_center_size(centre, vec2(POST_SIDE, POST_SIDE));
    cube(
        view,
        shapes,
        post,
        (z, z + POST),
        DIM.gamma_multiply(lit),
        GROUND,
    );
    let width = ground.width();
    let panel = Rect::from_min_size(pos2(ground.min.x, centre.y - 0.2), vec2(width, 0.4));
    let fill = BILLBOARD_FILL.gamma_multiply(lit);
    let top = (z + POST, z + POST + width * 0.5);
    cube(view, shapes, panel, top, FLOW.gamma_multiply(lit), fill);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doors_fit_the_width_of_their_building() {
        assert_eq!(door_width(40.0, 3), DOOR.0);

        let (length, n) = (6.0, 16);
        let width = door_width(length, n);

        assert!(
            width < length / n as f32,
            "the doors leave gaps between them"
        );
        assert!(width * n as f32 <= 0.8 * length + 1e-4);
    }
}
