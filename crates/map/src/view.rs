//! Where the city is seen from: the projection of the ground and its heights
//! onto the screen, from the corner or turned to face a facade.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::Framing;
use crate::city::iso_bounds;

/// Where the map is seen from: the ground rectangle in view, the direction
/// the camera looks from, and how that picture is scaled and moved onto the
/// screen.
///
/// The camera looks from the corner (isometric) until it turns to face a
/// facade: [`View::turned`] blends the two ground axes from their isometric
/// directions to a straight front view of the facade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The part of the ground in view: the square, centred on the screen's
    /// centre, that gives this same view when it is framed alone.
    pub visible: Rect,
    scale: f32,
    offset: Vec2,
    /// Where one layout point along ground x and along ground y goes on
    /// screen, before scaling; height always goes straight up.
    across: Vec2,
    down: Vec2,
}

/// The ground axes seen from the corner.
const ISO_ACROSS: Vec2 = vec2(0.866, 0.5);
const ISO_DOWN: Vec2 = vec2(-0.866, 0.5);

/// How much of the ground's depth a camera facing a facade still shows, so
/// what stands behind sits a little higher than what stands in front.
const FRONT_DEPTH: f32 = 0.25;

/// The scales between which the camera turns from the corner to face a
/// facade, and how long the turn eases in and out.
const TURN_FROM: f32 = 4.0;
const TURN_TO: f32 = 16.0;

/// How far the camera has turned towards a facade at `scale`, from 0 (from
/// the corner) to 1 (facing it), eased in and out across the zoom steps.
pub fn turn_at(scale: f32) -> f32 {
    let t = ((scale.ln() - TURN_FROM.ln()) / (TURN_TO.ln() - TURN_FROM.ln())).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The facade a turning camera pivots on: a point of it on the ground, its
/// height there, and whether it runs along ground x.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pivot {
    pub ground: Pos2,
    pub z: f32,
    pub along_x: bool,
}

impl View {
    fn project(&self, ground: Pos2, z: f32) -> Vec2 {
        self.across * ground.x + self.down * ground.y - vec2(0.0, z)
    }

    fn unproject(&self, seen: Vec2) -> Pos2 {
        let (a, b, c, d) = (self.across.x, self.down.x, self.across.y, self.down.y);
        let det = a * d - b * c;
        pos2(
            (d * seen.x - b * seen.y) / det,
            (a * seen.y - c * seen.x) / det,
        )
    }

    /// Where the ground point `ground` at height `z` is on screen.
    pub fn to_screen(&self, ground: Pos2, z: f32) -> Pos2 {
        (self.project(ground, z) * self.scale + self.offset).to_pos2()
    }

    /// The ground point under the screen point `screen`.
    pub fn to_ground(&self, screen: Pos2) -> Pos2 {
        self.unproject((screen.to_vec2() - self.offset) / self.scale)
    }

    /// How far on the ground a move of `screen` on screen goes.
    pub fn ground_delta(&self, screen: Vec2) -> Vec2 {
        self.unproject(screen / self.scale).to_vec2()
    }

    /// The same view with the camera turned `t` of the way from the corner
    /// to facing `pivot`'s facade. The pivot is drawn `t` of the way from
    /// where it stood to `centre`, so a facade faced settles in the middle.
    pub fn turned(mut self, t: f32, pivot: Pivot, centre: Pos2) -> View {
        let before = self.to_screen(pivot.ground, pivot.z);
        let anchor = before + (centre - before) * t;
        let (front_across, front_down) = if pivot.along_x {
            (vec2(1.0, 0.0), vec2(0.0, FRONT_DEPTH))
        } else {
            (vec2(0.0, FRONT_DEPTH), vec2(-1.0, 0.0))
        };
        self.across = ISO_ACROSS + (front_across - ISO_ACROSS) * t;
        self.down = ISO_DOWN + (front_down - ISO_DOWN) * t;
        self.offset += anchor - self.to_screen(pivot.ground, pivot.z);
        self
    }

    /// Screen points per layout point.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// The corners of the ground rectangle `rect` on screen.
    pub(crate) fn corners(&self, rect: Rect) -> Vec<Pos2> {
        [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ]
        .into_iter()
        .map(|corner| self.to_screen(corner, 0.0))
        .collect()
    }

    /// The circle inscribed in the ground square `square`, on screen.
    pub(crate) fn ring(&self, square: Rect) -> Vec<Pos2> {
        let (centre, radius) = (square.center(), square.width() / 2.0);
        (0..48)
            .map(|step| {
                let angle = step as f32 / 48.0 * std::f32::consts::TAU;
                let ground = centre + radius * vec2(angle.cos(), angle.sin());
                self.to_screen(ground, 0.0)
            })
            .collect()
    }

    /// The screen rectangle the ground rectangle `rect` is seen in.
    pub(crate) fn bounds(&self, rect: Rect) -> Rect {
        Rect::from_points(&self.corners(rect))
    }
}

/// The view that scales `framing.area`, with its headroom above it, to fill
/// `framing.fill` of `panel`, centred, and never closer than
/// `framing.closest`.
pub(crate) fn fit(framing: Framing, panel: Rect) -> View {
    let mut seen = iso_bounds(framing.area);
    seen.min.y -= framing.headroom;
    let scale = (panel.width() * framing.fill / seen.width().max(f32::EPSILON))
        .min(panel.height() * framing.fill / seen.height().max(f32::EPSILON))
        .min(framing.closest);
    let offset = panel.center().to_vec2() - seen.center().to_vec2() * scale;
    // A square of side L on the ground is seen 2 × 0.866 L wide and L high.
    let side = (panel.width() / (2.0 * 0.866)).min(panel.height()) / scale;
    let mut view = View {
        visible: Rect::NOTHING,
        scale,
        offset,
        across: ISO_ACROSS,
        down: ISO_DOWN,
    };
    view.visible = Rect::from_center_size(view.to_ground(panel.center()), vec2(side, side));
    view
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{Directories, ProjectMap, default_prefix_rule};

    #[test]
    fn every_district_lies_inside_the_panel_it_is_fitted_to() {
        let files: Vec<PathBuf> = (0..300)
            .map(|n| PathBuf::from(format!("src/d{}/s{}/f{n}.rs", n % 7, n % 3)))
            .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of("shop", Path::new("/p/shop"), &files, &strategy);
        let panel = Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0));

        let view = fit(
            Framing {
                area: map.extent,
                fill: 1.0,
                closest: 1.0,
                headroom: 0.0,
            },
            panel,
        );

        for district in &map.districts {
            let drawn = view.bounds(district.area);
            assert!(
                panel.expand(0.01).contains_rect(drawn),
                "{drawn:?} is outside {panel:?}"
            );
        }
    }

    #[test]
    fn a_framing_that_may_scale_up_fills_its_share_of_the_panel() {
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let area = Rect::from_min_size(pos2(40.0, 40.0), vec2(100.0, 20.0));
        let framing = |closest| Framing {
            area,
            fill: 0.5,
            closest,
            headroom: 0.0,
        };

        let close = fit(framing(f32::MAX), panel).bounds(area);
        let capped = fit(framing(1.0), panel).bounds(area);

        assert!((close.height() - 250.0).abs() < 0.01, "{close:?}");
        assert!((close.center() - panel.center()).length() < 0.01);
        assert!((capped.height() - 60.0).abs() < 0.01, "{capped:?}");
    }

    #[test]
    fn the_visible_square_framed_alone_gives_the_same_view() {
        let panel = Rect::from_min_size(pos2(0.0, 30.0), vec2(1200.0, 870.0));
        let view = fit(
            Framing {
                area: Rect::from_min_size(pos2(40.0, 40.0), vec2(300.0, 120.0)),
                fill: 0.7,
                closest: f32::MAX,
                headroom: 25.0,
            },
            panel,
        );

        let again = fit(
            Framing {
                area: view.visible,
                fill: 1.0,
                closest: f32::MAX,
                headroom: 0.0,
            },
            panel,
        );

        assert!(
            (again.scale - view.scale).abs() < 1e-3,
            "{again:?} {view:?}"
        );
        let moved = (again.offset - view.offset).length();
        assert!(moved < 0.05, "{again:?} {view:?}");
    }
    fn iso_view() -> View {
        let panel = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 800.0));
        let area = Rect::from_min_size(pos2(-50.0, -50.0), vec2(100.0, 100.0));
        fit(
            Framing {
                area,
                fill: 1.0,
                closest: f32::INFINITY,
                headroom: 0.0,
            },
            panel,
        )
    }

    #[test]
    fn the_projection_and_its_inverse_meet_however_far_the_camera_turned() {
        let pivot = Pivot {
            ground: pos2(3.0, -4.0),
            z: 10.0,
            along_x: true,
        };
        for t in [0.0, 0.3, 0.7, 1.0] {
            let view = iso_view().turned(t, pivot, pos2(500.0, 400.0));
            let ground = pos2(12.0, 7.5);

            let back = view.to_ground(view.to_screen(ground, 0.0));

            assert!((back - ground).length() < 1e-3, "{t}: {back:?}");
        }
    }

    #[test]
    fn the_pivot_moves_towards_the_centre_as_the_camera_turns() {
        let pivot = Pivot {
            ground: pos2(30.0, 10.0),
            z: 5.0,
            along_x: true,
        };
        let centre = pos2(500.0, 400.0);
        let start = iso_view().to_screen(pivot.ground, pivot.z);

        let half = iso_view()
            .turned(0.5, pivot, centre)
            .to_screen(pivot.ground, pivot.z);
        let full = iso_view()
            .turned(1.0, pivot, centre)
            .to_screen(pivot.ground, pivot.z);

        assert!((half - (start + (centre - start) * 0.5)).length() < 1e-3);
        assert!((full - centre).length() < 1e-3);
    }

    #[test]
    fn facing_a_facade_its_lines_run_level() {
        let pivot = Pivot {
            ground: pos2(0.0, 0.0),
            z: 0.0,
            along_x: true,
        };
        let view = iso_view().turned(1.0, pivot, pos2(500.0, 400.0));

        let start = view.to_screen(pos2(0.0, 0.0), 5.0);
        let end = view.to_screen(pos2(20.0, 0.0), 5.0);

        assert!((start.y - end.y).abs() < 1e-3);
        assert!(end.x > start.x);
    }

    #[test]
    fn the_camera_turns_only_between_its_two_scales() {
        assert_eq!(turn_at(2.0), 0.0);
        assert_eq!(turn_at(TURN_FROM), 0.0);
        assert!(turn_at(8.0) > 0.0 && turn_at(8.0) < 1.0);
        assert_eq!(turn_at(TURN_TO), 1.0);
        assert_eq!(turn_at(40.0), 1.0);
    }
}
