//! The city layout: every district a round plaza, every folder a ring
//! inside the ring of the folder holding it, the districts packed around
//! each other.
//!
//! A block is a node: its towers stand in rows on it ([`city::place`]); a node
//! without code is a low, narrow block. Inside a folder its rings and blocks
//! are packed largest first, along a spiral out from the middle, and the
//! ring is centred on whichever point needs the smaller ring: the middle of
//! what it holds, or the largest of them. A district keeps a free plaza at
//! its centre; a district's own blocks of only tests lie in its cellar under
//! the plaza and take no ground.
//!
//! Invariants: no two blocks overlap; no two rings of one folder overlap; no
//! two district circles overlap (their squares, `District::area`, may at the
//! corners); every block lies inside its folder's ring, and every ring
//! inside the ring of the folder holding it. A folder's `area` is the square
//! around its ring.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::city;
use crate::graph::{District, Group, Node};

/// The strip a name takes on screen at most.
pub(crate) const TITLE: f32 = 22.0;

/// The block of a node without code.
const BARE_BLOCK: Vec2 = vec2(24.0, 6.0);

/// The narrowest block of a node with code.
const NARROWEST_BLOCK: f32 = 48.0;

/// The plaza left free at a district's centre, the space between blocks and
/// rings, and the margin from the outermost of them to the district's edge.
const PLAZA: f32 = 14.0;
const BLOCK_GAP: f32 = 14.0;
const DISTRICT_MARGIN: f32 = 10.0;

/// The margin from the outermost block or ring of a folder to its ring.
const RING_MARGIN: f32 = 8.0;

/// The space between districts.
const DISTRICT_GAP: f32 = 60.0;

/// Lays out every district and returns the project's extent.
pub(crate) fn lay_out(districts: &mut [District]) -> Rect {
    let radii: Vec<f32> = districts.iter_mut().map(lay_out_district).collect();
    let centres = spiral(&radii, 0.0, DISTRICT_GAP);
    let mut extent = Rect::NOTHING;
    for (district, centre) in districts.iter_mut().zip(centres) {
        translate(district, centre.to_vec2());
        extent = extent.union(district.area);
    }
    extent
}

/// Lays `district` out around the origin and returns its radius.
fn lay_out_district(district: &mut District) -> f32 {
    let (mut cellar, mut ground): (Vec<&mut Node>, Vec<&mut Node>) =
        district.nodes.iter_mut().partition(|node| tests_only(node));
    let cellar_radii: Vec<f32> = cellar.iter_mut().map(|node| block(node)).collect();
    for (node, centre) in cellar.iter_mut().zip(spiral(&cellar_radii, 0.0, BLOCK_GAP)) {
        move_node(node, centre);
    }

    let mut radii: Vec<f32> = district
        .sub_districts
        .iter_mut()
        .map(|sub| {
            let r = ring(&mut sub.groups, &mut sub.nodes);
            sub.area = square(Pos2::ZERO, r);
            r
        })
        .collect();
    radii.extend(ground.iter_mut().map(|node| block(node)));
    let centres = largest_first(&radii, PLAZA);
    let (at_subs, at_blocks) = centres.split_at(district.sub_districts.len());
    for (sub, centre) in district.sub_districts.iter_mut().zip(at_subs) {
        let offset = centre.to_vec2();
        sub.area = sub.area.translate(offset);
        shift_groups(&mut sub.groups, offset);
        shift_nodes(&mut sub.nodes, offset);
    }
    for (node, centre) in ground.iter_mut().zip(at_blocks) {
        move_node(node, *centre);
    }
    let radius = reach(&centres, &radii, Pos2::ZERO).max(PLAZA) + DISTRICT_MARGIN;
    district.area = square(Pos2::ZERO, radius);
    radius
}

/// Lays out a folder's own `groups` and `nodes` around the origin, each
/// group as a ring of its own, and returns the radius of the folder's ring.
fn ring(groups: &mut [Group], nodes: &mut [Node]) -> f32 {
    let mut radii: Vec<f32> = groups
        .iter_mut()
        .map(|group| {
            let r = ring(&mut group.groups, &mut group.nodes);
            group.area = square(Pos2::ZERO, r);
            r
        })
        .collect();
    radii.extend(nodes.iter_mut().map(block));
    let centres = centred(largest_first(&radii, 0.0), &radii);
    let (at_groups, at_nodes) = centres.split_at(groups.len());
    for (group, centre) in groups.iter_mut().zip(at_groups) {
        let offset = centre.to_vec2();
        group.area = group.area.translate(offset);
        shift_groups(&mut group.groups, offset);
        shift_nodes(&mut group.nodes, offset);
    }
    for (node, centre) in nodes.iter_mut().zip(at_nodes) {
        move_node(node, *centre);
    }
    reach(&centres, &radii, Pos2::ZERO) + RING_MARGIN
}

/// Whether every tower on `node` belongs to the tests.
fn tests_only(node: &Node) -> bool {
    !node.items.is_empty() && node.items.iter().all(city::in_cellar)
}

/// Places the towers of `node` on its block at the origin, and returns the
/// radius of the circle around the block, the block left in `node.area`
/// until it is moved into place.
fn block(node: &mut Node) -> f32 {
    let size = if node.items.is_empty() {
        BARE_BLOCK
    } else {
        city::place(&mut node.items, NARROWEST_BLOCK)
    };
    node.area = Rect::from_min_size(Pos2::ZERO, size);
    size.length() / 2.0
}

/// Moves `node` and its towers so its block is centred on `centre`.
fn move_node(node: &mut Node, centre: Pos2) {
    let offset = centre - node.area.center();
    shift_nodes(std::slice::from_mut(node), offset);
}

/// The square around the circle of `radius` at `centre`.
fn square(centre: Pos2, radius: f32) -> Rect {
    Rect::from_center_size(centre, Vec2::splat(2.0 * radius))
}

/// How far from `at` the farthest of the circles of `radii` at `centres`
/// reaches.
fn reach(centres: &[Pos2], radii: &[f32], at: Pos2) -> f32 {
    centres
        .iter()
        .zip(radii)
        .map(|(centre, r)| (*centre - at).length() + r)
        .fold(0.0, f32::max)
}

/// Centres for circles of `radii`, in the given order, placed along the
/// spiral largest first: with no `hole` the largest stands in the middle,
/// the smaller ones around it.
fn largest_first(radii: &[f32], hole: f32) -> Vec<Pos2> {
    let mut order: Vec<usize> = (0..radii.len()).collect();
    order.sort_by(|a, b| radii[*b].total_cmp(&radii[*a]));
    let sorted: Vec<f32> = order.iter().map(|&i| radii[i]).collect();
    let mut centres = vec![Pos2::ZERO; radii.len()];
    for (i, centre) in order.into_iter().zip(spiral(&sorted, hole, BLOCK_GAP)) {
        centres[i] = centre;
    }
    centres
}

/// `centres` moved so that the circles of `radii` need the smallest ring of
/// the two around them: one centred on their middle, or on the largest.
fn centred(centres: Vec<Pos2>, radii: &[f32]) -> Vec<Pos2> {
    let middle = centres
        .iter()
        .zip(radii)
        .fold(Rect::NOTHING, |area, (centre, r)| {
            area.union(square(*centre, *r))
        })
        .center();
    let largest = centres
        .iter()
        .zip(radii)
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map_or(Pos2::ZERO, |(centre, _)| *centre);
    let at = if reach(&centres, radii, middle) < reach(&centres, radii, largest) {
        middle
    } else {
        largest
    };
    centres
        .into_iter()
        .map(|centre| centre - at.to_vec2())
        .collect()
}

/// Centres for circles of `radii`, in order, along a spiral out from the
/// origin: each takes the first place on the spiral where it keeps `gap` from
/// every circle placed before it. With no `hole`, the first sits at the
/// origin.
fn spiral(radii: &[f32], hole: f32, gap: f32) -> Vec<Pos2> {
    const TURN: f32 = 0.3;
    const GROWTH: f32 = 0.6;
    // Three turns back, to find room the spiral passed on its way out.
    const BACK: usize = 63;
    // Circles placed, bucketed on a grid as wide as the largest clearance
    // two circles need: a circle that could touch a candidate lies in the
    // candidate's bucket or one of the eight around it.
    let widest = radii.iter().copied().fold(0.0, f32::max);
    let bucket = (2.0 * widest + gap).max(1.0);
    let key = |at: Pos2| {
        (
            (at.x / bucket).floor() as i64,
            (at.y / bucket).floor() as i64,
        )
    };
    let mut buckets: std::collections::HashMap<(i64, i64), Vec<(Pos2, f32)>> =
        std::collections::HashMap::new();
    let mut placed: Vec<Pos2> = Vec::with_capacity(radii.len());
    let mut step = 0usize;
    for (index, &r) in radii.iter().enumerate() {
        let at = if index == 0 && hole == 0.0 {
            Pos2::ZERO
        } else {
            step = step.saturating_sub(BACK);
            loop {
                let angle = step as f32 * TURN;
                let distance = hole + r + step as f32 * GROWTH;
                let at = pos2(angle.cos() * distance, angle.sin() * distance);
                let (x, y) = key(at);
                let free = (x - 1..=x + 1)
                    .flat_map(|i| (y - 1..=y + 1).map(move |j| (i, j)))
                    .filter_map(|cell| buckets.get(&cell))
                    .flatten()
                    .all(|(other, other_r)| (*other - at).length() > other_r + r + gap);
                if free {
                    break at;
                }
                step += 1;
            }
        };
        buckets.entry(key(at)).or_default().push((at, r));
        placed.push(at);
    }
    placed
}

/// Moves `district` and everything on it by `offset`.
pub(crate) fn translate(district: &mut District, offset: Vec2) {
    district.area = district.area.translate(offset);
    shift_nodes(&mut district.nodes, offset);
    for sub in &mut district.sub_districts {
        sub.area = sub.area.translate(offset);
        shift_groups(&mut sub.groups, offset);
        shift_nodes(&mut sub.nodes, offset);
    }
}

fn shift_nodes(nodes: &mut [Node], offset: Vec2) {
    for node in nodes {
        node.area = node.area.translate(offset);
        for item in &mut node.items {
            item.area = item.area.translate(offset);
        }
    }
}

fn shift_groups(groups: &mut [Group], offset: Vec2) {
    for group in groups {
        group.area = group.area.translate(offset);
        shift_groups(&mut group.groups, offset);
        shift_nodes(&mut group.nodes, offset);
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{Directories, ProjectMap, default_prefix_rule};

    fn map_of(files: &[PathBuf]) -> ProjectMap {
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of("shop", Path::new("/p/shop"), files, &strategy)
    }

    /// A fixed-seed generator, so a failing tree is reproducible.
    fn generated_files(count: usize) -> Vec<PathBuf> {
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |bound: u64| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (state >> 33) % bound
        };
        (0..count)
            .map(|n| {
                let depth = next(6);
                let mut path = PathBuf::from("src");
                for _ in 0..depth {
                    path.push(format!("f{}", next(4)));
                }
                path.push(format!("file{n}.rs"));
                path
            })
            .collect()
    }

    fn disjoint(one: Rect, other: Rect) -> bool {
        one.max.x <= other.min.x
            || other.max.x <= one.min.x
            || one.max.y <= other.min.y
            || other.max.y <= one.min.y
    }

    /// Each folder's rectangle with the rectangles it holds directly.
    type Holding = (Rect, Vec<Rect>);

    fn group_holdings(group: &Group, into: &mut Vec<Holding>) {
        let held = group.groups.iter().map(|g| g.area);
        let nodes = group.nodes.iter().map(|n| n.area);
        into.push((group.area, held.chain(nodes).collect()));
        for inner in &group.groups {
            group_holdings(inner, into);
        }
    }

    fn holdings(map: &ProjectMap) -> Vec<Holding> {
        let mut all = Vec::new();
        for district in &map.districts {
            let subs = district.sub_districts.iter().map(|s| s.area);
            let nodes = district.nodes.iter().map(|n| n.area);
            all.push((district.area, subs.chain(nodes).collect()));
            for sub in &district.sub_districts {
                let groups = sub.groups.iter().map(|g| g.area);
                let nodes = sub.nodes.iter().map(|n| n.area);
                all.push((sub.area, groups.chain(nodes).collect()));
                for group in &sub.groups {
                    group_holdings(group, &mut all);
                }
            }
        }
        all
    }

    #[test]
    fn no_two_blocks_or_districts_overlap_and_every_folder_holds_what_it_holds() {
        let map = map_of(&generated_files(400));
        let holdings = holdings(&map);
        let mut pending: Vec<&crate::graph::Node> = Vec::new();
        for district in &map.districts {
            pending.extend(&district.nodes);
            for sub in &district.sub_districts {
                pending.extend(&sub.nodes);
                let mut groups: Vec<&Group> = sub.groups.iter().collect();
                while let Some(group) = groups.pop() {
                    pending.extend(&group.nodes);
                    groups.extend(&group.groups);
                }
            }
        }
        let blocks: Vec<Rect> = pending.iter().map(|node| node.area).collect();

        assert_eq!(blocks.len(), 400);
        for (i, one) in blocks.iter().enumerate() {
            for other in &blocks[i + 1..] {
                assert!(disjoint(*one, *other), "{one:?} overlaps {other:?}");
            }
        }
        for (i, one) in map.districts.iter().enumerate() {
            for other in &map.districts[i + 1..] {
                let apart = (one.area.center() - other.area.center()).length();
                assert!(
                    apart > (one.area.width() + other.area.width()) / 2.0,
                    "{:?} {:?}",
                    one.area,
                    other.area
                );
            }
        }
        assert!(holdings.len() > 10, "the generated tree was too small");
        for (folder, held) in holdings {
            for inner in held {
                assert!(
                    folder.contains_rect(inner),
                    "{inner:?} is outside {folder:?}"
                );
            }
        }
    }

    /// A circle: its centre and radius. A folder's is its ring; a block's
    /// the circle around it.
    type Circle = (Pos2, f32);

    fn ring_of(area: Rect) -> Circle {
        (area.center(), area.width() / 2.0)
    }

    fn around(block: Rect) -> Circle {
        (block.center(), block.size().length() / 2.0)
    }

    /// Each folder's ring with the rings and blocks it holds directly.
    fn rings(map: &ProjectMap) -> Vec<(Circle, Vec<Circle>)> {
        fn inner(groups: &[Group], nodes: &[crate::graph::Node]) -> Vec<Circle> {
            let rings = groups.iter().map(|g| ring_of(g.area));
            rings.chain(nodes.iter().map(|n| around(n.area))).collect()
        }
        fn walk(group: &Group, into: &mut Vec<(Circle, Vec<Circle>)>) {
            into.push((ring_of(group.area), inner(&group.groups, &group.nodes)));
            for g in &group.groups {
                walk(g, into);
            }
        }
        let mut all = Vec::new();
        for district in &map.districts {
            for sub in &district.sub_districts {
                all.push((ring_of(sub.area), inner(&sub.groups, &sub.nodes)));
                for group in &sub.groups {
                    walk(group, &mut all);
                }
            }
        }
        all
    }

    #[test]
    fn every_ring_lies_in_the_ring_holding_it_and_apart_from_its_siblings() {
        let map = map_of(&generated_files(400));
        let rings = rings(&map);

        assert!(rings.len() > 10, "the generated tree was too small");
        for ((centre, radius), held) in &rings {
            for (i, (inner, r)) in held.iter().enumerate() {
                let out = (*inner - *centre).length() + r;
                assert!(
                    out <= radius + 1e-3,
                    "{inner:?} reaches {out} past {radius}"
                );
                for (other, other_r) in &held[i + 1..] {
                    assert!(
                        (*inner - *other).length() >= r + other_r,
                        "{inner:?} and {other:?} overlap"
                    );
                }
            }
        }
    }

    #[test]
    fn the_largest_stands_in_the_middle() {
        let radii = [3.0, 20.0, 5.0];

        let centres = largest_first(&radii, 0.0);

        assert_eq!(centres[1], Pos2::ZERO);
        assert!(centres[0] != Pos2::ZERO && centres[2] != Pos2::ZERO);
    }

    #[test]
    fn a_ring_centres_on_whichever_point_needs_the_smaller_ring() {
        // One large circle and a small one beside it: centred on the large,
        // the ring reaches the small one's far side; centred on the middle of
        // both, it would reach farther past the large one.
        let radii = [50.0, 5.0];
        let centres = vec![Pos2::ZERO, pos2(70.0, 0.0)];

        let moved = centred(centres.clone(), &radii);
        let needed = reach(&moved, &radii, Pos2::ZERO);

        let middle = Rect::from_min_max(pos2(-50.0, -50.0), pos2(75.0, 50.0)).center();
        let on_middle = reach(&centres, &radii, middle);
        let on_largest = reach(&centres, &radii, Pos2::ZERO);
        assert!((needed - on_middle.min(on_largest)).abs() < 1e-3);

        // Two equal circles: their middle needs the smaller ring.
        let radii = [10.0, 10.0];
        let moved = centred(vec![Pos2::ZERO, pos2(30.0, 0.0)], &radii);
        assert!((reach(&moved, &radii, Pos2::ZERO) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn the_same_files_lay_out_to_identical_geometry() {
        let files = generated_files(200);

        assert_eq!(map_of(&files), map_of(&files));
    }

    #[test]
    fn circles_on_the_spiral_keep_their_gap() {
        let radii = [10.0, 3.0, 25.0, 7.0, 7.0, 40.0, 1.0];

        let centres = spiral(&radii, 14.0, 6.0);

        for i in 0..radii.len() {
            assert!(centres[i].to_vec2().length() >= 14.0 + radii[i] - 1e-3);
            for j in i + 1..radii.len() {
                let apart = (centres[i] - centres[j]).length();
                assert!(apart > radii[i] + radii[j] + 6.0, "{i} and {j}");
            }
        }
    }

    #[test]
    fn a_map_grows_with_its_content_and_does_not_string_out() {
        let small = map_of(&generated_files(10)).extent;
        let large = map_of(&generated_files(1000)).extent;

        let per_file = |extent: Rect, files: f32| extent.area() / files;
        assert!(
            per_file(large, 1000.0) < 4.0 * per_file(small, 10.0),
            "small {small:?}, large {large:?}"
        );
        let aspect = large.width() / large.height();
        assert!((0.25..4.0).contains(&aspect), "{large:?}");
    }
}
