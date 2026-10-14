//! Drawing the lanes: import lanes as faint pipes on the ground, call lanes
//! as arcs through the air, each in the colour of the building it starts
//! from, with traffic now and then. A selected tower shows only its own
//! lanes, a selected block, folder or district everything in or out of it;
//! a lane pointed at or picked stands out with what it carries.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Color32, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

use super::{DIM, INK, LINE_STRONG, PANEL};
use crate::Item;
use crate::graph::ProjectMap;
use crate::lanes::{self, LaneId, LaneKind, Lanes};
use crate::view::View;

/// One lane as drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnLane {
    pub id: LaneId,
    pub kind: LaneKind,
    /// Imports: the importing and the imported block. Calls: the calling and
    /// the called item.
    pub from: PathBuf,
    pub to: PathBuf,
    /// Calls: the number of the calling line.
    pub line: Option<usize>,
    /// Where it runs on screen, from start to end.
    pub screen: Vec<Pos2>,
    /// Imports: where it runs on the ground, and the height of each point;
    /// empty for a call through the air.
    pub ground: Vec<Pos2>,
    pub heights: Vec<f32>,
    /// Whether it runs in a highway between two districts.
    pub highway: bool,
    /// The colour of the building it starts from.
    pub colour: Color32,
    /// Whether it stands out, rather than lying quiet in the overview.
    pub lit: bool,
    /// Whether it was drawn at all: with something selected, a lane that
    /// has nothing to do with it is not.
    pub shown: bool,
    /// Whether traffic moved along it this frame.
    pub streak: bool,
    /// The towers a travel along it lands on: at its start and at its end.
    pub ends: [PathBuf; 2],
    /// What it reads when pointed at or picked.
    pub card: Vec<String>,
}

/// What the lanes are drawn for this frame.
pub(super) struct Focus<'a> {
    pub shown: Lanes,
    /// The element selected.
    pub selected: Option<&'a Path>,
    /// The lane picked, kept lit.
    pub picked: Option<&'a LaneId>,
    /// The lanes of the map, worked out once for it; none while the towers
    /// fly.
    pub network: Option<&'a lanes::Network>,
    pub pointer: Option<Pos2>,
}

/// The lanes of one frame, worked out before any is drawn: the import lanes
/// go under the towers, the call lanes over them.
pub(super) struct Frame<'a> {
    pub lanes: Vec<DrawnLane>,
    /// The street pieces lanes within districts share, and the imports along
    /// each, drawn only in the overview.
    streets: Option<&'a lanes::Streets>,
    /// The lane under the pointer, the arcs before the pipes.
    pub hovered: Option<usize>,
    /// Whether only what is lit is drawn: something is selected or picked.
    only_lit: bool,
}

/// The pipe colour of the quiet overview, its casing, and the arcs'.
const PIPE: Color32 = Color32::from_rgb(60, 72, 92);
const CASING: Color32 = Color32::from_rgb(10, 14, 22);
const AIR: Color32 = Color32::from_rgb(90, 200, 255);

/// Ground traffic: map units per second, and the rest between streaks.
const GROUND_PACE: f32 = 600.0;
const GROUND_REST: (f32, f32) = (0.6, 3.0);
/// Air traffic: seconds per crossing, and the rest between streaks.
const AIR_CROSSING: f32 = 2.0;
const AIR_REST: (f32, f32) = (2.5, 11.5);

/// The colour a building's lanes carry: always the same for its path.
pub fn colour_of(path: &Path) -> Color32 {
    let hash = seed(path.as_os_str().as_encoded_bytes());
    egui::ecolor::Hsva::new((hash % 997) as f32 / 997.0, 0.75, 1.0, 1.0).into()
}

fn seed(bytes: &[u8]) -> u32 {
    bytes.iter().fold(2_166_136_261_u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(16_777_619)
    })
}

/// The width of a pipe carrying `n` imports.
fn width(n: usize) -> f32 {
    (0.8 + (n as f32).sqrt() * 0.5).min(4.0)
}

/// The name an item or block is read by in a card: its file and name.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Works out the lanes `focus` shows on `map` this frame.
pub(super) fn frame<'a>(map: &ProjectMap, view: &View, focus: &Focus<'a>) -> Frame<'a> {
    let mut lanes = Vec::new();
    let mut streets = None;
    let Some(network) = focus
        .network
        .filter(|_| focus.shown.imports || focus.shown.calls)
    else {
        return Frame {
            lanes,
            streets,
            hovered: None,
            only_lit: false,
        };
    };
    let flows: &[lanes::Flow] = if focus.shown.imports {
        &network.flows
    } else {
        &[]
    };
    if focus.shown.imports {
        streets = Some(&network.streets);
        for (flow, route) in flows.iter().zip(&network.routes) {
            lanes.push(DrawnLane {
                id: LaneId {
                    kind: LaneKind::Imports,
                    from: flow.from.clone(),
                    to: flow.to.clone(),
                    floor: None,
                },
                kind: LaneKind::Imports,
                from: flow.from.clone(),
                to: flow.to.clone(),
                line: None,
                screen: route
                    .points
                    .iter()
                    .zip(&route.heights)
                    .map(|(p, z)| view.to_screen(*p, *z))
                    .collect(),
                ground: route.points.clone(),
                heights: route.heights.clone(),
                highway: route.highway,
                colour: colour_of(&flow.from),
                lit: false,
                shown: true,
                streak: false,
                ends: [flow.imports[0].0.clone(), flow.imports[0].1.clone()],
                card: Vec::new(),
            });
        }
    }
    if focus.shown.calls {
        for call in &network.calls {
            lanes.push(DrawnLane {
                id: LaneId {
                    kind: LaneKind::Calls,
                    from: call.caller.clone(),
                    to: call.callee.clone(),
                    floor: Some(call.floor),
                },
                kind: LaneKind::Calls,
                from: call.caller.clone(),
                to: call.callee.clone(),
                line: Some(call.line),
                screen: arc(view, call.start, call.end),
                ground: Vec::new(),
                heights: Vec::new(),
                highway: false,
                colour: colour_of(&call.block),
                lit: false,
                shown: true,
                streak: false,
                ends: [call.caller.clone(), call.callee.clone()],
                card: Vec::new(),
            });
        }
    }

    // A selected tower: only its own lanes. A selected block, folder or
    // district: everything in or out of it.
    let tower = focus
        .selected
        .and_then(|path| map.item(path))
        .map(|item| item.path.as_path());
    let held: HashSet<&Path> = focus
        .selected
        .map(|path| {
            map.items_under(path)
                .into_iter()
                .map(|item| item.path.as_path())
                .collect()
        })
        .unwrap_or_default();
    for (i, lane) in lanes.iter_mut().enumerate() {
        // The import lanes come first, one per flow, in the same order.
        let flow = flows.get(i).filter(|_| lane.kind == LaneKind::Imports);
        let own = match (lane.kind, tower) {
            (LaneKind::Imports, Some(tower)) => {
                flow.is_some_and(|f| f.imports.iter().any(|(by, of)| by == tower || of == tower))
            }
            (LaneKind::Imports, None) => flow.is_some_and(|f| {
                f.imports
                    .iter()
                    .any(|(by, of)| held.contains(by.as_path()) || held.contains(of.as_path()))
            }),
            (LaneKind::Calls, _) => {
                held.contains(lane.from.as_path()) || held.contains(lane.to.as_path())
            }
        };
        lane.lit = own || focus.picked == Some(&lane.id);
    }
    // A selection that holds no item (the project itself) has nothing to
    // filter by.
    let only_lit = !held.is_empty() || focus.picked.is_some();
    for lane in &mut lanes {
        lane.shown = lane.lit || !only_lit;
    }
    // Under the pointer: an arc wins over the pipe beneath it.
    let hovered = focus.pointer.and_then(|at| {
        let near = |lane: &DrawnLane| {
            let reach = if lane.kind == LaneKind::Calls {
                4.0
            } else {
                4.0 + width(1) / 2.0
            };
            distance_to(&lane.screen, at) < reach
        };
        // Only a lane drawn can be pointed at.
        let calls = lanes
            .iter()
            .position(|l| l.shown && l.kind == LaneKind::Calls && near(l));
        calls.or_else(|| {
            lanes
                .iter()
                .position(|l| l.shown && l.kind == LaneKind::Imports && near(l))
        })
    });

    if let Some(i) = hovered {
        lanes[i].lit = true;
    }
    // Only a lane pointed at or picked is read, so only its card is written.
    for (i, lane) in lanes.iter_mut().enumerate() {
        if hovered == Some(i) || focus.picked == Some(&lane.id) {
            lane.card = card_text(lane, flows.get(i), map);
        }
    }
    Frame {
        lanes,
        streets,
        hovered,
        only_lit,
    }
}

/// What `lane` reads when pointed at or picked: an import lane the files
/// and the names imported through it (`flow`), a call lane the calling
/// line.
fn card_text(lane: &DrawnLane, flow: Option<&lanes::Flow>, map: &ProjectMap) -> Vec<String> {
    match (lane.kind, flow) {
        (LaneKind::Imports, Some(flow)) => {
            let names: Vec<String> = flow
                .imports
                .iter()
                .map(|(_, target)| {
                    map.item(target)
                        .map_or_else(|| file_name(target), |item| item.name.clone())
                })
                .collect();
            let mut shown: Vec<&str> = names.iter().map(String::as_str).take(4).collect();
            shown.dedup();
            let more = names.len().saturating_sub(shown.len());
            let more = if more > 0 {
                format!(", +{more}")
            } else {
                String::new()
            };
            vec![format!(
                "{} imports from {}  {}: {}{more}",
                file_name(&flow.from),
                file_name(&flow.to),
                names.len(),
                shown.join(", ")
            )]
        }
        (LaneKind::Calls, _) => {
            let (Some(caller), Some(callee)) = (map.item(&lane.from), map.item(&lane.to)) else {
                return Vec::new();
            };
            let file = |item: &Item| file_name(item.path.parent().unwrap_or(&item.path));
            let Some(floor) = lane.id.floor.and_then(|f| caller.floors.get(f)) else {
                return Vec::new();
            };
            vec![
                format!(
                    "{}:{} {} calls {} ({})",
                    file(caller),
                    floor.number,
                    caller.name,
                    callee.name,
                    file(callee)
                ),
                format!("    {}", floor.text.trim()),
            ]
        }
        (LaneKind::Imports, None) => Vec::new(),
    }
}

/// Draws the lanes of `kind` of `frame` into `shapes`, at time `time`.
pub(super) fn draw(
    frame: &mut Frame,
    kind: LaneKind,
    view: &View,
    time: f32,
    shapes: &mut Vec<Shape>,
) {
    let quiet_pipe = PIPE.gamma_multiply(if frame.only_lit { 0.0 } else { 0.3 });
    if kind == LaneKind::Imports && !frame.only_lit {
        for (&(i, j, along_x), &(carried, z)) in frame.streets.into_iter().flatten() {
            let start = pos2(i as f32 * lanes::GRID, j as f32 * lanes::GRID);
            let end = if along_x {
                start + vec2(lanes::GRID, 0.0)
            } else {
                start + vec2(0.0, lanes::GRID)
            };
            let line = [view.to_screen(start, z), view.to_screen(end, z)];
            shapes.push(Shape::line_segment(
                line,
                Stroke::new(width(carried) + 1.0, CASING.gamma_multiply(0.6)),
            ));
            shapes.push(Shape::line_segment(
                line,
                Stroke::new(width(carried), quiet_pipe),
            ));
        }
    }
    for lane in frame
        .lanes
        .iter_mut()
        .filter(|lane| lane.kind == kind && lane.shown)
    {
        if !lane.lit {
            match kind {
                LaneKind::Imports if lane.highway => {
                    shapes.push(Shape::line(
                        lane.screen.clone(),
                        Stroke::new(1.8, CASING.gamma_multiply(0.6)),
                    ));
                    shapes.push(Shape::line(
                        lane.screen.clone(),
                        Stroke::new(0.9, quiet_pipe),
                    ));
                }
                LaneKind::Imports => {
                    let s = &lane.screen;
                    for stub in [[s[0], s[1]], [s[3], s[4]]] {
                        shapes.push(Shape::line_segment(stub, Stroke::new(0.9, quiet_pipe)));
                    }
                }
                LaneKind::Calls => {
                    shapes.push(Shape::line(
                        lane.screen.clone(),
                        Stroke::new(0.7, AIR.gamma_multiply(0.14)),
                    ));
                }
            }
            let (duration, rest, strength) = match kind {
                LaneKind::Imports => (
                    (lanes::length(&lane.ground) / GROUND_PACE).max(0.08),
                    GROUND_REST,
                    0.8,
                ),
                LaneKind::Calls => (AIR_CROSSING, AIR_REST, 0.7),
            };
            let key = [
                lane.from.as_os_str().as_encoded_bytes(),
                &lane.id.floor.unwrap_or(0).to_le_bytes(),
                lane.to.as_os_str().as_encoded_bytes(),
            ]
            .concat();
            lane.streak = traffic(
                shapes,
                &lane.screen,
                lane.colour.gamma_multiply(strength),
                seed(&key),
                time,
                duration,
                rest,
            );
            continue;
        }
        // Lit: traced in its colour, with pulses running downstream.
        let w = if kind == LaneKind::Calls {
            1.2
        } else {
            width(1).max(1.5)
        };
        shapes.push(Shape::line(
            lane.screen.clone(),
            Stroke::new(w + 2.0, CASING),
        ));
        shapes.push(Shape::line(
            lane.screen.clone(),
            Stroke::new(w, lane.colour.gamma_multiply(0.4)),
        ));
        pulses(shapes, &lane.screen, lane.colour, time);
    }
}

/// Draws the card of the lane hovered, or else of the lane picked at its
/// middle, on top of everything.
pub(super) fn card(
    ui: &egui::Ui,
    frame: &Frame<'_>,
    picked: Option<&LaneId>,
    pointer: Option<Pos2>,
) {
    let (lane, at) = match (frame.hovered, pointer) {
        (Some(i), Some(at)) => (&frame.lanes[i], at),
        _ => {
            let Some(lane) = frame.lanes.iter().find(|lane| Some(&lane.id) == picked) else {
                return;
            };
            (lane, lane.screen[lane.screen.len() / 2])
        }
    };
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        ui.id().with("lane card"),
    ));
    let title = match lane.kind {
        LaneKind::Imports => "Import lane",
        LaneKind::Calls => "Call lane",
    };
    let head = painter.layout_no_wrap(title.to_owned(), egui::FontId::proportional(13.0), INK);
    let mut rows: Vec<_> = lane
        .card
        .iter()
        .map(|text| {
            painter.layout_no_wrap(text.clone(), egui::FontId::monospace(11.0), lane.colour)
        })
        .collect();
    rows.push(painter.layout_no_wrap(
        "click: keep lit   cmd+click: go to the far end".to_owned(),
        egui::FontId::monospace(11.0),
        DIM,
    ));
    let width = rows
        .iter()
        .map(|r| r.size().x)
        .fold(head.size().x, f32::max);
    let height = head.size().y + rows.iter().map(|r| r.size().y + 2.0).sum::<f32>();
    let screen = ui.ctx().content_rect();
    let mut corner = at + vec2(16.0, 16.0);
    corner.x = corner
        .x
        .min(screen.max.x - width - 14.0)
        .max(screen.min.x + 10.0);
    if corner.y + height > screen.max.y - 10.0 {
        corner.y = at.y - height - 16.0;
    }
    let card = Rect::from_min_size(corner, vec2(width, height)).expand2(vec2(10.0, 6.0));
    painter.rect(
        card,
        0.0,
        PANEL,
        Stroke::new(1.0, LINE_STRONG),
        StrokeKind::Inside,
    );
    painter.galley(corner, head.clone(), INK);
    let mut y = corner.y + head.size().y + 2.0;
    for row in rows {
        let h = row.size().y;
        painter.galley(pos2(corner.x, y), row, INK);
        y += h + 2.0;
    }
}

/// The arc from `start` up and over to `end`, each a ground point and its
/// height, on screen.
fn arc(view: &View, start: (Pos2, f32), end: (Pos2, f32)) -> Vec<Pos2> {
    let a = view.to_screen(start.0, start.1);
    let b = view.to_screen(end.0, end.1);
    let rise = ((b - a).length() * 0.35).max(12.0);
    let top = pos2((a.x + b.x) / 2.0, a.y.min(b.y) - rise);
    (0..=16)
        .map(|k| {
            let t = k as f32 / 16.0;
            let u = 1.0 - t;
            (a.to_vec2() * u * u + top.to_vec2() * 2.0 * u * t + b.to_vec2() * t * t).to_pos2()
        })
        .collect()
}

/// How far `at` lies from the polyline `points`.
fn distance_to(points: &[Pos2], at: Pos2) -> f32 {
    points
        .windows(2)
        .map(|leg| {
            let (a, b) = (leg[0], leg[1]);
            let ab = b - a;
            let t = ((at - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
            (a + ab * t - at).length()
        })
        .fold(f32::INFINITY, f32::min)
}

/// The point `along` screen points down `points`.
fn point_along(points: &[Pos2], along: f32) -> Option<Pos2> {
    let mut left = along;
    for leg in points.windows(2) {
        let piece = (leg[1] - leg[0]).length();
        if left <= piece {
            return Some(leg[0] + (leg[1] - leg[0]) * (left / piece.max(1e-3)));
        }
        left -= piece;
    }
    None
}

/// Small dots running down `points`, subtle, in `colour`.
fn pulses(shapes: &mut Vec<Shape>, points: &[Pos2], colour: Color32, time: f32) {
    const SPACING: f32 = 28.0;
    let length = points
        .windows(2)
        .map(|w| (w[1] - w[0]).length())
        .sum::<f32>();
    let mut along = (time * 40.0) % SPACING;
    while along < length {
        if let Some(at) = point_along(points, along) {
            shapes.push(Shape::circle_filled(at, 2.7, colour.gamma_multiply(0.1)));
            shapes.push(Shape::circle_filled(at, 1.5, colour.gamma_multiply(0.65)));
        }
        along += SPACING;
    }
}

/// Now and then a short streak runs down `points`, taking `duration`
/// seconds, then rests between `rest.0` and `rest.1` seconds, the same for
/// the same `seed` every frame. Returns whether one ran at `time`.
fn traffic(
    shapes: &mut Vec<Shape>,
    points: &[Pos2],
    colour: Color32,
    seed: u32,
    time: f32,
    duration: f32,
    rest: (f32, f32),
) -> bool {
    let length = points
        .windows(2)
        .map(|w| (w[1] - w[0]).length())
        .sum::<f32>();
    let mix = |x: u32| {
        x.wrapping_mul(2_654_435_761)
            .rotate_left(15)
            .wrapping_mul(2_246_822_519)
    };
    let period = duration + rest.0 + (mix(seed) % 1000) as f32 / 1000.0 * (rest.1 - rest.0);
    let phase = (mix(seed ^ 0x9e37) % 1000) as f32 / 1000.0 * period;
    let head = (time + phase) % period / duration.max(1e-3) * length;
    let tail = (length * 0.25).clamp(8.0, 40.0);
    if head > length + tail || length <= 0.0 {
        return false;
    }
    const STEPS: usize = 8;
    for k in 0..STEPS {
        let a = (head - tail * k as f32 / STEPS as f32).clamp(0.0, length);
        let b = (head - tail * (k + 1) as f32 / STEPS as f32).clamp(0.0, length);
        if let (Some(p), Some(q)) = (point_along(points, a), point_along(points, b)) {
            let fade = 1.0 - k as f32 / STEPS as f32;
            shapes.push(Shape::line_segment(
                [p, q],
                Stroke::new(1.6 * fade + 0.4, colour.gamma_multiply(fade)),
            ));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_runs_now_and_then_and_the_same_for_the_same_lane() {
        let points = [Pos2::ZERO, pos2(200.0, 0.0)];
        let moving = |seed: u32| {
            (0..1000)
                .filter(|k| {
                    traffic(
                        &mut Vec::new(),
                        &points,
                        Color32::WHITE,
                        seed,
                        *k as f32 * 0.02,
                        0.4,
                        GROUND_REST,
                    )
                })
                .count()
        };

        let share = moving(7) as f32 / 1000.0;
        assert!(share > 0.0 && share < 0.5, "{share}");
        assert_eq!(moving(7), moving(7));
    }

    #[test]
    fn a_building_s_lanes_always_carry_its_colour() {
        assert_eq!(
            colour_of(Path::new("/p/a.rs")),
            colour_of(Path::new("/p/a.rs"))
        );
        assert_ne!(
            colour_of(Path::new("/p/a.rs")),
            colour_of(Path::new("/p/b.rs"))
        );
    }
}
