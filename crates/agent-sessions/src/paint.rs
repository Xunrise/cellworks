//! How a unit and its work look, in the city's projection or the flat
//! view's: every shape is laid out on the ground (and, in the city, at a
//! height) and projected through the same `View`, so the flat view, which
//! sees no heights, draws a construction's rising floors as lines filling
//! the machine from the bottom up, and a scan beam as a line sweeping it.

use egui::{Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

use crate::ActionKind;
use crate::units::{Pose, Target, Unit};

const READ: Color32 = Color32::from_rgb(90, 200, 255);
const WRITE: Color32 = Color32::from_rgb(255, 190, 60);
const RUN: Color32 = Color32::from_rgb(255, 140, 50);
const PASSED: Color32 = Color32::from_rgb(80, 225, 120);
const FAILED: Color32 = Color32::from_rgb(255, 80, 80);
const SEARCH: Color32 = Color32::from_rgb(190, 130, 255);
const DELEGATE: Color32 = Color32::from_rgb(255, 110, 200);
const FETCH: Color32 = Color32::from_rgb(80, 230, 200);
const WAIT: Color32 = Color32::YELLOW;
const QUIET: Color32 = Color32::from_rgb(140, 150, 170);

/// The height animations reach on a building or folder, which has none of
/// its own in the model.
const NOMINAL_HEIGHT: f32 = 8.0;

/// The least an effect rises, and reaches out around its tower, on screen.
const MIN_SCREEN_HEIGHT: f32 = 44.0;
const MIN_SCREEN_REACH: f32 = 9.0;

/// The unit's colour in `pose`, for its body, path and label line.
pub(crate) fn colour(pose: Pose, unit: &Unit) -> Color32 {
    match pose {
        Pose::Moving | Pose::Working(_) => kind_colour(unit.action.kind, unit.action.outcome),
        Pose::Waiting => WAIT,
        Pose::Thinking => READ.gamma_multiply(0.8),
        Pose::Idle => QUIET,
        Pose::Done => PASSED,
    }
}

fn kind_colour(kind: ActionKind, outcome: Option<bool>) -> Color32 {
    match kind {
        ActionKind::Read => READ,
        ActionKind::Write => WRITE,
        ActionKind::Run => match outcome {
            None => RUN,
            Some(true) => PASSED,
            Some(false) => FAILED,
        },
        ActionKind::Search => SEARCH,
        ActionKind::Delegate => DELEGATE,
        ActionKind::Fetch => FETCH,
    }
}

/// The rest of a walk, `ahead` on the ground, as marching dots toward a
/// ring at its end.
pub(crate) fn path(
    painter: &Painter,
    view: &cellworks_map::View,
    ahead: &[Pos2],
    colour: Color32,
    t: f64,
) {
    let screen: Vec<Pos2> = ahead.iter().map(|p| view.to_screen(*p, 0.0)).collect();
    const GAP: f32 = 9.0;
    let mut offset = GAP - (t as f32 * 30.0) % GAP;
    for pair in screen.windows(2) {
        let leg = pair[1] - pair[0];
        let length = leg.length();
        while offset < length {
            painter.circle_filled(
                pair[0] + leg * (offset / length),
                1.6,
                colour.gamma_multiply(0.75),
            );
            offset += GAP;
        }
        offset -= length;
    }
    if let Some(end) = screen.last() {
        let pulse = 0.5 + 0.5 * (t * 4.0).sin() as f32;
        painter.circle_stroke(*end, 5.0 + 2.0 * pulse, Stroke::new(1.5, colour));
    }
}

/// The animation of the work `unit` does on its target, standing at
/// `at` on screen.
pub(crate) fn work(
    painter: &Painter,
    view: &cellworks_map::View,
    unit: &Unit,
    pose: Pose,
    at: Pos2,
    t: f64,
    motion: bool,
) {
    let Pose::Working(kind) = pose else {
        return;
    };
    let flat_target;
    let target = if view.is_flat() {
        flat_target = Target {
            area: unit.target.block,
            floor: None,
            ..unit.target.clone()
        };
        &flat_target
    } else {
        &unit.target
    };
    // Small towers get effects of a readable size on screen all the same.
    let height = target
        .z
        .max(NOMINAL_HEIGHT)
        .max(MIN_SCREEN_HEIGHT / view.scale());
    match kind {
        ActionKind::Write => construct(painter, view, target, unit.action.added, height, t, motion),
        ActionKind::Read => {
            let s = if motion {
                0.5 + 0.5 * (t * 2.6).sin() as f32
            } else {
                0.6
            };
            let reach = (MIN_SCREEN_REACH * 0.7 / view.scale()).max(0.6);
            let level = level(view, target.area.expand(reach), height, s);
            line(painter, &level, Stroke::new(2.0, READ));
            let middle = level.iter().fold(Pos2::ZERO, |a, p| a + p.to_vec2()) / level.len() as f32;
            painter.line_segment([at, middle], Stroke::new(1.0, READ.gamma_multiply(0.4)));
        }
        ActionKind::Run => {
            let colour = kind_colour(kind, unit.action.outcome);
            for i in 0..2 {
                let phase = if motion {
                    ((t * 1.2 + i as f64 * 0.5) % 1.0) as f32
                } else {
                    0.5
                };
                let ring = ground_ring(view, unit.ground, (6.0 + 20.0 * phase) / view.scale());
                line(
                    painter,
                    &ring,
                    Stroke::new(2.0, colour.gamma_multiply(1.0 - phase)),
                );
            }
            if let Some(ok) = unit.action.outcome {
                outcome(painter, at + vec2(0.0, -17.0), ok);
            }
        }
        ActionKind::Search => {
            let centre = target.area.center();
            let radius = (target.area.size().max_elem() * 0.5)
                .min(90.0 / view.scale())
                .max(14.0 / view.scale());
            let angle = if motion { t as f32 * 3.0 } else { 0.8 };
            line(
                painter,
                &ground_ring(view, centre, radius),
                Stroke::new(1.0, SEARCH.gamma_multiply(0.5)),
            );
            for i in 0..10 {
                let a = angle - i as f32 * 0.09;
                let tip = centre + vec2(a.cos(), a.sin()) * radius;
                painter.line_segment(
                    [view.to_screen(centre, 0.0), view.to_screen(tip, 0.0)],
                    Stroke::new(2.0, SEARCH.gamma_multiply(1.0 - i as f32 / 10.0)),
                );
            }
        }
        ActionKind::Delegate => {
            for i in 0..3 {
                let a = (if motion { t as f32 * 2.2 } else { 0.0 })
                    + i as f32 * std::f32::consts::TAU / 3.0;
                painter.circle_filled(at + vec2(11.0 * a.cos(), 6.0 * a.sin()), 2.2, DELEGATE);
            }
        }
        ActionKind::Fetch => {
            let rise = if motion {
                (t as f32 * 40.0) % 10.0
            } else {
                0.0
            };
            for i in 0..6 {
                let y = -8.0 - i as f32 * 10.0 - rise;
                painter.line_segment(
                    [at + vec2(0.0, y), at + vec2(0.0, y - 5.0)],
                    Stroke::new(2.0, FETCH),
                );
            }
        }
    }
}

/// Scaffolding around the tower, its rungs rising to the top and starting
/// over, and the floors being written lit up.
fn construct(
    painter: &Painter,
    view: &cellworks_map::View,
    target: &Target,
    added: usize,
    height: f32,
    t: f64,
    motion: bool,
) {
    let top = height + added as f32 * cellworks_map::city::FLOOR;
    let frame = target
        .area
        .expand((MIN_SCREEN_REACH / view.scale()).max(0.9));
    let stroke = Stroke::new(1.6, WRITE.gamma_multiply(0.9));
    if view.is_flat() {
        // Corner brackets around the machine.
        let r = Rect::from_two_pos(
            view.to_screen(frame.min, 0.0),
            view.to_screen(frame.max, 0.0),
        );
        let arm = (r.width().min(r.height()) * 0.3).max(4.0);
        for (corner, dx, dy) in [
            (r.left_top(), 1.0, 1.0),
            (r.right_top(), -1.0, 1.0),
            (r.right_bottom(), -1.0, -1.0),
            (r.left_bottom(), 1.0, -1.0),
        ] {
            painter.line_segment(
                [corner, corner + vec2(arm * dx, 0.0)],
                Stroke::new(2.0, WRITE),
            );
            painter.line_segment(
                [corner, corner + vec2(0.0, arm * dy)],
                Stroke::new(2.0, WRITE),
            );
        }
    } else {
        for corner in corners(frame) {
            painter.line_segment(
                [view.to_screen(corner, 0.0), view.to_screen(corner, top)],
                stroke,
            );
        }
    }
    const RUNGS: usize = 6;
    let shown = if motion {
        (t * 2.5) as usize % (RUNGS + 2)
    } else {
        RUNGS
    };
    for k in 1..=shown.min(RUNGS) {
        line(
            painter,
            &level(view, frame, top, k as f32 / RUNGS as f32),
            stroke,
        );
    }
    if let Some((low, high)) = target.floor {
        let glow = if motion {
            0.6 + 0.4 * (t * 5.0).sin() as f32
        } else {
            1.0
        };
        let colour = WRITE.gamma_multiply(glow);
        let (a, b) = (
            level(view, target.area, top, low / top),
            level(view, target.area, top, high / top),
        );
        if view.is_flat() {
            let band = Rect::from_points(&[a[0], a[1], b[0], b[1]]).expand2(vec2(0.0, 1.5));
            painter.rect_filled(band, 0.0, colour.gamma_multiply(0.45));
        } else {
            line(painter, &a, Stroke::new(2.5, colour));
            line(painter, &b, Stroke::new(2.5, colour));
        }
    }
}

/// The unit itself: a small diamond, outlined, ringed white when selected.
pub(crate) fn body(painter: &Painter, at: Pos2, colour: Color32, selected: bool) {
    let d = 7.0;
    painter.circle_filled(at, 11.0, colour.gamma_multiply(0.18));
    let points = vec![
        at + vec2(0.0, -d),
        at + vec2(d, 0.0),
        at + vec2(0.0, d),
        at + vec2(-d, 0.0),
    ];
    painter.add(Shape::convex_polygon(
        points,
        colour,
        Stroke::new(1.2, Color32::from_black_alpha(220)),
    ));
    if selected {
        painter.circle_stroke(at, 10.0, Stroke::new(2.0, Color32::WHITE));
    }
}

/// The marker over a resting unit: "!" waiting for the engineer, three
/// dots thinking, "z" idle, a tick when done.
pub(crate) fn marker(painter: &Painter, at: Pos2, pose: Pose, t: f64, motion: bool) {
    let above = at + vec2(0.0, -17.0);
    match pose {
        Pose::Waiting => {
            let pulse = if motion {
                (t * 3.0).sin() as f32 * 0.5 + 0.5
            } else {
                0.5
            };
            painter.circle_stroke(at, 10.0 + 6.0 * pulse, Stroke::new(2.0, WAIT));
            painter.circle_filled(above, 8.0, WAIT);
            painter.text(
                above,
                egui::Align2::CENTER_CENTER,
                "!",
                FontId::proportional(13.0),
                Color32::BLACK,
            );
        }
        Pose::Thinking => {
            for i in 0..3 {
                let lit = if motion {
                    ((t * 3.0) as i64 % 3) == i
                } else {
                    i == 1
                };
                let alpha = if lit { 1.0 } else { 0.35 };
                painter.circle_filled(
                    above + vec2((i as f32 - 1.0) * 5.0, 0.0),
                    1.8,
                    READ.gamma_multiply(alpha),
                );
            }
        }
        Pose::Idle => {
            painter.text(
                above,
                egui::Align2::CENTER_CENTER,
                "z",
                FontId::proportional(11.0),
                QUIET,
            );
        }
        Pose::Done => outcome(painter, above, true),
        Pose::Moving | Pose::Working(_) => {}
    }
}

/// Bright sparks flying off a unit at work on a construction.
pub(crate) fn sparks(painter: &Painter, at: Pos2, t: f64, seed: u64) {
    let tick = (t * 14.0) as u64;
    for i in 0..3u64 {
        let h = tick
            .wrapping_mul(2654435761)
            .wrapping_add(i * 97 + seed)
            .wrapping_mul(40503);
        if !(h >> 7).is_multiple_of(3) {
            let d = vec2((h % 13) as f32 - 6.0, -(((h >> 4) % 10) as f32) - 5.0);
            painter.circle_filled(at + d, 1.6, Color32::from_rgb(255, 220, 60));
        }
    }
}

/// A tick in a green disc, or a cross in a red one: drawn, not a glyph,
/// so it needs no font that holds it.
fn outcome(painter: &Painter, at: Pos2, ok: bool) {
    let stroke = Stroke::new(2.0, Color32::BLACK);
    painter.circle_filled(at, 7.0, if ok { PASSED } else { FAILED });
    if ok {
        painter.line_segment([at + vec2(-3.5, 0.0), at + vec2(-1.0, 3.0)], stroke);
        painter.line_segment([at + vec2(-1.0, 3.0), at + vec2(3.5, -3.0)], stroke);
    } else {
        painter.line_segment([at + vec2(-3.0, -3.0), at + vec2(3.0, 3.0)], stroke);
        painter.line_segment([at + vec2(-3.0, 3.0), at + vec2(3.0, -3.0)], stroke);
    }
}

fn corners(area: Rect) -> [Pos2; 4] {
    [
        area.min,
        pos2(area.max.x, area.min.y),
        area.max,
        pos2(area.min.x, area.max.y),
    ]
}

/// A level `s` (0 ground, 1 top) of a tower `height` high on `area`: its
/// outline at that height in the city; in the flat view, which sees no
/// heights, a line across the machine `s` of the way up from its bottom.
fn level(view: &cellworks_map::View, area: Rect, height: f32, s: f32) -> Vec<Pos2> {
    let s = s.clamp(0.0, 1.0);
    if view.is_flat() {
        let y = area.max.y - s * area.height();
        vec![
            view.to_screen(pos2(area.min.x, y), 0.0),
            view.to_screen(pos2(area.max.x, y), 0.0),
        ]
    } else {
        corners(area)
            .iter()
            .map(|c| view.to_screen(*c, s * height))
            .collect()
    }
}

/// A ring of ground `radius` around `centre`, as the view sees it.
fn ground_ring(view: &cellworks_map::View, centre: Pos2, radius: f32) -> Vec<Pos2> {
    (0..24)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 24.0;
            view.to_screen(centre + vec2(a.cos(), a.sin()) * radius, 0.0)
        })
        .collect()
}

/// Two points as a segment, more as a closed outline.
fn line(painter: &Painter, points: &[Pos2], stroke: Stroke) {
    if points.len() == 2 {
        painter.line_segment([points[0], points[1]], stroke);
    } else {
        painter.add(Shape::closed_line(points.to_vec(), stroke));
    }
}
