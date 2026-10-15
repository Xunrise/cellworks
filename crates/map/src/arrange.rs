//! Arranging pieces like a town: the one way the map places the buildings
//! of a block, the blocks and folders of a folder, and the districts of a
//! project.
//!
//! The piece with the most weight stands first, then always the piece most
//! bound to those already standing, beside the one it is most bound to, or
//! beside the first when it is bound to none, so a town grows round its
//! centre rather than strung out along a lane. Where that piece has no room
//! left, beside one of the pieces standing nearest it, or one of the latest
//! placed, at the town's edge. A street runs
//! between two pieces, its width varying a little with each piece. Pieces
//! form a skyline: one shorter than the piece it stands beside stands on
//! that piece's front (towards ground `+x` or `+y`), a taller one behind it.
//! A piece with room keeps that much open ground around it.
//!
//! The same pieces always stand in the same places: every choice is made in
//! a fixed order, and the variety comes from each piece's seed.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use std::collections::HashMap;

use egui::{Pos2, Rect, Vec2, pos2};

use crate::lens::cluster::Weights;

/// One thing to stand: the ground it takes, how much it matters (the most
/// stands first), the open ground it keeps around it, how high it rises, and
/// the seed its street width and offset vary by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Piece {
    pub size: Vec2,
    pub weight: f32,
    pub room: f32,
    pub height: f32,
    pub seed: u32,
}

/// How many of the pieces standing nearest the one a piece is bound to it
/// tries to stand beside, and how many of the latest placed after them.
const NEAREST: usize = 12;

/// How strongly a place is drawn towards the first piece, against how near
/// it is to the piece it stands beside.
const INWARD: f32 = 0.35;

/// Where each of `pieces` stands, by the corner of its ground nearest the
/// origin; the first stands at the origin. `affinity` says how strongly two
/// pieces are bound, by their indices; streets are `street.0` wide and up to
/// `street.1` wider.
pub(crate) fn arrange(pieces: &[Piece], affinity: &Weights, street: (f32, f32)) -> Vec<Pos2> {
    let n = pieces.len();
    let Some(first) = first(pieces) else {
        return Vec::new();
    };
    let mut partners: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    for (&(a, b), &w) in affinity {
        if a < n && b < n && w > 0.0 {
            partners[a].push((b, w));
            partners[b].push((a, w));
        }
    }
    let area = |i: usize| pieces[i].size.x * pieces[i].size.y;
    let mut rects = vec![Rect::NOTHING; n];
    let mut placed: Vec<usize> = Vec::with_capacity(n);
    // How strongly each piece is bound to those standing.
    let mut pull = vec![0.0_f32; n];
    stand(
        first,
        Rect::from_min_size(Pos2::ZERO, pieces[first].size),
        &partners,
        &mut placed,
        &mut rects,
        &mut pull,
    );
    let centre = rects[first].center();
    let most_room = pieces.iter().map(|p| p.room).fold(0.0, f32::max);
    // Cells as wide as a piece is on average: a large piece covers several.
    let typical = pieces.iter().map(|p| p.size.max_elem()).sum::<f32>() / n as f32;
    let mut grid = Grid::new(typical.max(street.0 + street.1));
    grid.insert(first, rects[first]);
    // ponytail: choosing the next piece scans every piece left, O(n²) in
    // all; a heap of pulls if a district of tens of thousands of blocks lays
    // out too slowly (3,500 blocks take about 0.1 s).
    while placed.len() < n {
        let next = (0..n)
            .filter(|i| pull[*i] > f32::NEG_INFINITY)
            .max_by(|a, b| {
                pull[*a]
                    .total_cmp(&pull[*b])
                    .then(pieces[*a].height.total_cmp(&pieces[*b].height))
                    .then(area(*a).total_cmp(&area(*b)))
                    .then(b.cmp(a))
            })
            .expect("a piece is left");
        let piece = pieces[next];
        let gap = street.0 + jitter(piece.seed, 0) * street.1;
        let anchor = partners[next]
            .iter()
            .filter(|(j, _)| pull[*j] == f32::NEG_INFINITY)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .map_or(first, |(j, _)| *j);
        let at = rects[anchor].center();
        // Beside the pieces nearest the anchor, else beside the latest,
        // which stand at the town's edge.
        let distance = |j: usize| (rects[j].center() - at).length();
        let mut near = placed.clone();
        let by_distance =
            |a: &usize, b: &usize| distance(*a).total_cmp(&distance(*b)).then(a.cmp(b));
        if near.len() > NEAREST {
            near.select_nth_unstable_by(NEAREST, by_distance);
            near.truncate(NEAREST);
        }
        near.sort_by(by_distance);
        near.extend(placed.iter().rev().take(NEAREST));
        let clear = |j: usize| street.0 + pieces[j].room + piece.room;
        let reach = street.0 + most_room + piece.room;
        let free = |r: Rect| {
            grid.near(r.expand(reach))
                .all(|j| !rects[j].expand(clear(j) - 1e-3).intersects(r))
        };
        let cost = |r: &Rect| (r.center() - at).length() + INWARD * (r.center() - centre).length();
        let shift = (jitter(piece.seed, 1) - 0.5) * 2.0;
        let size = piece.size;
        let chosen = near.iter().find_map(|&beside| {
            let a = rects[beside];
            let off = gap + pieces[beside].room + piece.room;
            let (front, back) = sides(piece.height, pieces[beside].height);
            let mut candidates = Vec::with_capacity(12);
            for along in [0.5_f32, 0.0, 1.0] {
                let x = a.min.x + (a.width() - size.x) * along + shift;
                let y = a.min.y + (a.height() - size.y) * along + shift;
                if front {
                    candidates.push(pos2(x, a.max.y + off));
                    candidates.push(pos2(a.max.x + off, y));
                }
                if back {
                    candidates.push(pos2(x, a.min.y - off - size.y));
                    candidates.push(pos2(a.min.x - off - size.x, y));
                }
            }
            candidates
                .into_iter()
                .map(|corner| Rect::from_min_size(corner, size))
                .filter(|r| free(*r))
                .min_by(|p, q| cost(p).total_cmp(&cost(q)))
        });
        let rect = chosen.unwrap_or_else(|| {
            // Beside everything standing: in front of it, or behind it when
            // taller than the piece it is bound to.
            let extent = placed.iter().fold(Rect::NOTHING, |e, j| e.union(rects[*j]));
            let most = placed.iter().map(|j| clear(*j)).fold(0.0, f32::max) + gap;
            let (front, _) = sides(piece.height, pieces[anchor].height);
            let x = if front {
                extent.max.x + most
            } else {
                extent.min.x - most - size.x
            };
            Rect::from_min_size(pos2(x, extent.min.y), size)
        });
        stand(next, rect, &partners, &mut placed, &mut rects, &mut pull);
        grid.insert(next, rect);
    }
    rects.into_iter().map(|r| r.min).collect()
}

/// The pieces standing, by the cells of a square grid their ground covers,
/// so a place is checked only against the pieces near it.
struct Grid {
    cell: f32,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl Grid {
    fn new(cell: f32) -> Grid {
        Grid {
            cell: cell.max(1.0),
            cells: HashMap::new(),
        }
    }

    /// The cells `rect` covers.
    fn covered(&self, rect: Rect) -> impl Iterator<Item = (i32, i32)> + use<> {
        let at = |v: f32| (v / self.cell).floor() as i32;
        let (x0, x1, y0, y1) = (
            at(rect.min.x),
            at(rect.max.x),
            at(rect.min.y),
            at(rect.max.y),
        );
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    }

    fn insert(&mut self, i: usize, rect: Rect) {
        for cell in self.covered(rect).collect::<Vec<_>>() {
            self.cells.entry(cell).or_default().push(i);
        }
    }

    /// The pieces standing in the cells `rect` covers, some more than once.
    fn near(&self, rect: Rect) -> impl Iterator<Item = usize> + '_ {
        self.covered(rect)
            .filter_map(|cell| self.cells.get(&cell))
            .flatten()
            .copied()
    }
}

/// Stands piece `i` on `rect`: it pulls its partners towards those
/// standing, and is no longer pulled itself.
fn stand(
    i: usize,
    rect: Rect,
    partners: &[Vec<(usize, f32)>],
    placed: &mut Vec<usize>,
    rects: &mut [Rect],
    pull: &mut [f32],
) {
    rects[i] = rect;
    placed.push(i);
    for &(j, w) in &partners[i] {
        pull[j] += w;
    }
    pull[i] = f32::NEG_INFINITY;
}

/// The piece that stands first: the most weight, then the largest, then the
/// first given.
pub(crate) fn first(pieces: &[Piece]) -> Option<usize> {
    let area = |p: &Piece| p.size.x * p.size.y;
    (0..pieces.len()).max_by(|a, b| {
        let (p, q) = (&pieces[*a], &pieces[*b]);
        p.weight
            .total_cmp(&q.weight)
            .then(area(p).total_cmp(&area(q)))
            .then(b.cmp(a))
    })
}

/// Whether a piece `height` high may stand on the front sides, and on the
/// back sides, of one `beside` high: a shorter one in front, a taller one
/// behind. Flat ground hides nothing and stands anywhere.
fn sides(height: f32, beside: f32) -> (bool, bool) {
    if height <= 0.0 || beside <= 0.0 {
        return (true, true);
    }
    (height <= beside, height >= beside)
}

/// A number from 0 to 1 that byte `k` of `seed` gives.
fn jitter(seed: u32, k: u32) -> f32 {
    ((seed >> (k * 8)) & 0xff) as f32 / 255.0
}

/// The smallest rectangle around `extent` whose centre is `centre`.
pub(crate) fn about(extent: Rect, centre: Pos2) -> Rect {
    let half = (centre - extent.min).max(extent.max - centre);
    Rect::from_center_size(centre, 2.0 * half)
}

/// How far in from a corner of an outline its rounding cuts, as a share of
/// each edge.
const ROUNDING: f32 = 0.25;

/// The ground round `points`, each `margin` out on every side: their convex
/// hull with every corner cut a quarter of each edge in, but no farther than
/// `margin`, so the cut stays clear of what it goes round; in order round
/// it. Empty for no point.
pub(crate) fn outline(points: impl Iterator<Item = Pos2>, margin: f32) -> Vec<Pos2> {
    let mut points: Vec<Pos2> = points
        .filter(|p| p.is_finite())
        .flat_map(|p| {
            let r = Rect::from_center_size(p, Vec2::splat(2.0 * margin));
            [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ]
        })
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    // The hull, by Andrew's monotone chain: the lower half, then the upper.
    let turn = |o: Pos2, a: Pos2, b: Pos2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
    let mut hull: Vec<Pos2> = Vec::with_capacity(points.len() + 1);
    for half in [points.clone(), points.into_iter().rev().collect()] {
        let start = hull.len();
        for p in half {
            while hull.len() >= start + 2
                && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    let n = hull.len();
    (0..n)
        .flat_map(|i| {
            let (before, here, after) = (hull[(i + n - 1) % n], hull[i], hull[(i + 1) % n]);
            let cut = |to: Pos2| {
                let edge = to - here;
                here + edge * (ROUNDING).min(margin / edge.length().max(f32::EPSILON))
            };
            [cut(before), cut(after)]
        })
        .collect()
}

/// A seed that is always the same for `bytes`: a path, a name, a key.
pub(crate) fn seed(bytes: &[u8]) -> u32 {
    bytes.iter().fold(2_166_136_261_u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(16_777_619)
    })
}

#[cfg(test)]
mod tests {
    use egui::vec2;

    use super::*;
    use crate::lens::cluster::link;

    fn piece(w: f32, d: f32, height: f32, seed: u32) -> Piece {
        Piece {
            size: vec2(w, d),
            weight: 0.0,
            room: 0.0,
            height,
            seed,
        }
    }

    fn rects(pieces: &[Piece], corners: &[Pos2]) -> Vec<Rect> {
        pieces
            .iter()
            .zip(corners)
            .map(|(p, c)| Rect::from_min_size(*c, p.size))
            .collect()
    }

    /// Thirty pieces of varied sizes and heights, some bound to others.
    fn town() -> (Vec<Piece>, Weights) {
        let pieces: Vec<Piece> = (0..30)
            .map(|i| {
                let f = i as f32;
                piece(
                    4.0 + (f * 7.0) % 20.0,
                    3.0 + (f * 5.0) % 11.0,
                    (f * 13.0) % 40.0,
                    seed(&[i]),
                )
            })
            .collect();
        let mut affinity = Weights::new();
        for i in 0..29 {
            if i % 3 != 0 {
                link(&mut affinity, i, (i * 7 + 3) % 30, 1.0 + (i % 4) as f32);
            }
        }
        (pieces, affinity)
    }

    #[test]
    fn the_same_pieces_stand_in_the_same_places() {
        let (pieces, affinity) = town();

        assert_eq!(
            arrange(&pieces, &affinity, (2.0, 3.0)),
            arrange(&pieces, &affinity, (2.0, 3.0))
        );
    }

    #[test]
    fn no_two_pieces_overlap_and_a_street_runs_between_them() {
        let (mut pieces, affinity) = town();
        pieces[4].room = 5.0;
        let corners = arrange(&pieces, &affinity, (2.0, 3.0));
        let rects = rects(&pieces, &corners);

        for (i, one) in rects.iter().enumerate() {
            for (j, other) in rects.iter().enumerate().skip(i + 1) {
                let clear = 2.0 + pieces[i].room + pieces[j].room - 1e-2;
                assert!(
                    !one.expand(clear).intersects(*other),
                    "{i} {one:?} and {j} {other:?}"
                );
            }
        }
    }

    #[test]
    fn a_shorter_piece_stands_in_front_of_a_taller_one_and_a_taller_behind() {
        let mut affinity = Weights::new();
        link(&mut affinity, 0, 1, 1.0);
        let in_front = |a: Rect, b: Rect| b.min.x >= a.max.x || b.min.y >= a.max.y;

        let mut tall_first = [piece(10.0, 10.0, 30.0, 1), piece(6.0, 6.0, 5.0, 2)];
        tall_first[0].weight = 1.0;
        let r = rects(&tall_first, &arrange(&tall_first, &affinity, (2.0, 3.0)));
        assert!(in_front(r[0], r[1]), "{r:?}");

        let mut short_first = [piece(10.0, 10.0, 5.0, 1), piece(6.0, 6.0, 30.0, 2)];
        short_first[0].weight = 1.0;
        let r = rects(&short_first, &arrange(&short_first, &affinity, (2.0, 3.0)));
        assert!(
            r[1].max.x <= r[0].min.x || r[1].max.y <= r[0].min.y,
            "{r:?}"
        );
    }

    #[test]
    fn a_piece_stands_beside_its_strongest_partner() {
        // 0 stands first; 1 and 2 are bound to it loosely; 3 is bound to 2
        // strongly and to 0 a little.
        let pieces: Vec<Piece> = (0..4).map(|i| piece(8.0, 8.0, 0.0, seed(&[i]))).collect();
        let mut affinity = Weights::new();
        link(&mut affinity, 0, 1, 1.0);
        link(&mut affinity, 0, 2, 1.0);
        link(&mut affinity, 2, 3, 9.0);
        link(&mut affinity, 0, 3, 0.5);

        let r = rects(&pieces, &arrange(&pieces, &affinity, (2.0, 3.0)));

        assert!(r[3].expand(2.0 + 3.0 + 1e-3).intersects(r[2]), "{r:?}");
    }

    #[test]
    fn an_outline_goes_round_what_it_holds_with_its_corners_cut() {
        let held = [
            Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 4.0)),
            Rect::from_min_size(pos2(14.0, 6.0), vec2(4.0, 8.0)),
        ];
        let corners = held.iter().flat_map(|r| {
            [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
            ]
        });

        let outline = outline(corners, 2.0);

        let around = Rect::from_points(&outline);
        assert!(around.contains_rect(held[0].union(held[1])), "{around:?}");
        assert!(
            around.max.x <= 20.0 + 1e-3 && around.min.x >= -2.0 - 1e-3,
            "{around:?}"
        );
        // Cut corners: no corner of the box around the outline lies on it.
        for corner in [around.left_top(), around.right_bottom()] {
            assert!(
                outline.iter().all(|p| (*p - corner).length() > 0.5),
                "{outline:?}"
            );
        }
        // Convex, turning the same way all round.
        let n = outline.len();
        let turns: Vec<f32> = (0..n)
            .map(|i| {
                let (a, b, c) = (outline[i], outline[(i + 1) % n], outline[(i + 2) % n]);
                (b - a).x * (c - b).y - (b - a).y * (c - b).x
            })
            .collect();
        assert!(turns.iter().all(|t| *t >= -1e-3) || turns.iter().all(|t| *t <= 1e-3));
    }

    #[test]
    fn about_a_point_reaches_as_far_on_each_side() {
        let extent = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 4.0));

        let around = about(extent, pos2(2.0, 1.0));

        assert_eq!(around.center(), pos2(2.0, 1.0));
        assert!(around.contains_rect(extent));
        assert_eq!(around.size(), vec2(16.0, 6.0));
    }
}
