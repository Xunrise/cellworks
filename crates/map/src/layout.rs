//! The city layout: every district a round plaza, its blocks packed around
//! the plaza's centre, the districts packed around each other.
//!
//! A block is a node: its towers stand in rows on it ([`city::place`]); a node
//! without code is a low, narrow block. The blocks of a district follow each
//! other along a spiral out from its centre, in folder order, so the blocks
//! of one folder lie together. Sub-districts and groups are not drawn as
//! ground of their own: each covers the blocks it holds, a little more for
//! every level, so a folder can still be framed, while its rectangle may
//! overlap a sibling's.
//!
//! Invariants: no two blocks overlap; no two district circles overlap (their
//! squares, `District::area`, may at the corners); every block
//! lies inside its district, and every folder inside the folder holding it.
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

/// The plaza left free at a district's centre, the space between blocks, and
/// the margin from the outermost block to the district's edge.
const PLAZA: f32 = 14.0;
const BLOCK_GAP: f32 = 14.0;
const DISTRICT_MARGIN: f32 = 10.0;

/// The space between districts.
const DISTRICT_GAP: f32 = 60.0;

/// How far a folder's rectangle reaches past what it holds.
const FOLDER_REACH: f32 = 1.0;

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
    // Blocks holding only tests lie in the district's cellar, under its
    // plaza, and take no ground among the other blocks.
    let (mut cellar, mut nodes): (Vec<&mut Node>, Vec<&mut Node>) = in_folder_order(district)
        .into_iter()
        .partition(|node| !node.items.is_empty() && node.items.iter().all(city::in_cellar));
    let cellar_radii: Vec<f32> = cellar
        .iter_mut()
        .map(|node| block(node).length() / 2.0)
        .collect();
    for (node, centre) in cellar.iter_mut().zip(spiral(&cellar_radii, 0.0, BLOCK_GAP)) {
        let offset = centre - node.area.center();
        node.area = node.area.translate(offset);
        for item in &mut node.items {
            item.area = item.area.translate(offset);
        }
    }
    let radii: Vec<f32> = nodes
        .iter_mut()
        .map(|node| block(node).length() / 2.0)
        .collect();
    let centres = spiral(&radii, PLAZA, BLOCK_GAP);
    let mut radius = PLAZA;
    for ((node, centre), r) in nodes.iter_mut().zip(&centres).zip(&radii) {
        let offset = *centre - node.area.center();
        node.area = node.area.translate(offset);
        for item in &mut node.items {
            item.area = item.area.translate(offset);
        }
        radius = radius.max(centre.to_vec2().length() + r);
    }
    radius += DISTRICT_MARGIN;
    for sub in &mut district.sub_districts {
        sub.area = cover(&mut sub.groups, &sub.nodes);
    }
    district.area = Rect::from_center_size(Pos2::ZERO, Vec2::splat(2.0 * radius));
    radius
}

/// Every node of `district`, folder by folder: each sub-district's groups
/// before its own nodes, the district's own nodes last.
fn in_folder_order(district: &mut District) -> Vec<&mut Node> {
    fn groups<'a>(inside: &'a mut [Group], into: &mut Vec<&'a mut Node>) {
        for group in inside {
            groups(&mut group.groups, into);
            into.extend(group.nodes.iter_mut());
        }
    }
    let mut nodes = Vec::new();
    for sub in &mut district.sub_districts {
        groups(&mut sub.groups, &mut nodes);
        nodes.extend(sub.nodes.iter_mut());
    }
    nodes.extend(district.nodes.iter_mut());
    nodes
}

/// Places the towers of `node` on its block at the origin, and returns the
/// block's size, left in `node.area` until the district places it.
fn block(node: &mut Node) -> Vec2 {
    let size = if node.items.is_empty() {
        BARE_BLOCK
    } else {
        city::place(&mut node.items, NARROWEST_BLOCK)
    };
    node.area = Rect::from_min_size(Pos2::ZERO, size);
    size
}

/// The rectangle of a folder: what it holds, and a little more. Sets the
/// rectangle of every group below it on the way.
fn cover(groups: &mut [Group], nodes: &[Node]) -> Rect {
    let mut area = nodes
        .iter()
        .fold(Rect::NOTHING, |area, node| area.union(node.area));
    for group in groups {
        group.area = cover(&mut group.groups, &group.nodes);
        area = area.union(group.area);
    }
    area.expand(FOLDER_REACH)
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
    let mut placed: Vec<(Pos2, f32)> = Vec::with_capacity(radii.len());
    let mut step = 0usize;
    for (index, &r) in radii.iter().enumerate() {
        if index == 0 && hole == 0.0 {
            placed.push((Pos2::ZERO, r));
            continue;
        }
        // ponytail: every candidate is checked against every circle placed;
        // a grid of buckets if districts reach thousands of blocks.
        step = step.saturating_sub(BACK);
        loop {
            let angle = step as f32 * TURN;
            let distance = hole + r + step as f32 * GROWTH;
            let at = pos2(angle.cos() * distance, angle.sin() * distance);
            let free = placed
                .iter()
                .all(|(other, other_r)| (*other - at).length() > other_r + r + gap);
            if free {
                placed.push((at, r));
                break;
            }
            step += 1;
        }
    }
    placed.into_iter().map(|(at, _)| at).collect()
}

/// Moves `district` and everything on it by `offset`.
pub(crate) fn translate(district: &mut District, offset: Vec2) {
    fn nodes(inside: &mut [Node], offset: Vec2) {
        for node in inside {
            node.area = node.area.translate(offset);
            for item in &mut node.items {
                item.area = item.area.translate(offset);
            }
        }
    }
    fn groups(inside: &mut [Group], offset: Vec2) {
        for group in inside {
            group.area = group.area.translate(offset);
            groups(&mut group.groups, offset);
            nodes(&mut group.nodes, offset);
        }
    }
    district.area = district.area.translate(offset);
    nodes(&mut district.nodes, offset);
    for sub in &mut district.sub_districts {
        sub.area = sub.area.translate(offset);
        groups(&mut sub.groups, offset);
        nodes(&mut sub.nodes, offset);
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
