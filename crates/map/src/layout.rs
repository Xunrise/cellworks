//! The deterministic layout: a nested packing computed bottom-up in one pass.
//!
//! Each box is laid out at the origin from its already laid-out children, then
//! moved into place by its parent. A box is exactly as large as what it holds,
//! and nothing moves once placed.
//!
//! Non-overlap holds by construction: children are packed into rows that are
//! disjoint horizontal bands, children within a row occupy disjoint
//! x-intervals, and every child sits strictly inside its parent's padding.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::graph::{District, Group, Node, SubDistrict};

/// The footprint of a node, large enough for a short label.
pub(crate) const NODE: Vec2 = vec2(120.0, 24.0);

/// The strip at the top of a box that carries its label.
pub(crate) const TITLE: f32 = 22.0;

/// Space between a box's edge and its children, and between siblings. Larger
/// at higher levels so the folder hierarchy reads at a glance.
const PROJECT_GAP: f32 = 48.0;
const DISTRICT_PADDING: f32 = 24.0;
const SUB_DISTRICT_PADDING: f32 = 12.0;
const GROUP_PADDING: f32 = 6.0;

/// Lays out every district and returns the project's extent.
pub(crate) fn lay_out(districts: &mut [District]) -> Rect {
    let mut boxes: Vec<&mut dyn Container> = Vec::new();
    for district in districts.iter_mut() {
        district.lay_out();
        boxes.push(district);
    }
    let size = pack(boxes, &mut [], PROJECT_GAP, Pos2::ZERO);
    Rect::from_min_size(Pos2::ZERO, size)
}

/// A box that holds boxes and nodes.
trait Container {
    fn area(&mut self) -> &mut Rect;
    fn padding(&self) -> f32;
    fn parts(&mut self) -> (Vec<&mut dyn Container>, &mut [Node]);

    /// Lays this box and everything below it out with its corner at the
    /// origin.
    fn lay_out(&mut self) {
        let padding = self.padding();
        let (mut boxes, nodes) = self.parts();
        for child in boxes.iter_mut() {
            child.lay_out();
        }
        let inner = pack(boxes, nodes, padding, pos2(padding, TITLE + padding));
        *self.area() = Rect::from_min_size(
            Pos2::ZERO,
            inner + vec2(2.0 * padding, TITLE + 2.0 * padding),
        );
    }

    /// Moves this box and everything below it by `offset`.
    fn translate(&mut self, offset: Vec2) {
        let area = self.area();
        *area = area.translate(offset);
        let (boxes, nodes) = self.parts();
        for child in boxes {
            child.translate(offset);
        }
        for node in nodes {
            node.area = node.area.translate(offset);
        }
    }
}

/// Places laid-out boxes, then nodes, in rows starting at `origin`, and
/// returns the size they take. A row ends when it would pass a width derived
/// from the children's total area, so the result is roughly square.
fn pack(mut boxes: Vec<&mut dyn Container>, nodes: &mut [Node], gap: f32, origin: Pos2) -> Vec2 {
    let mut sizes: Vec<Vec2> = boxes.iter_mut().map(|b| b.area().size()).collect();
    sizes.extend(std::iter::repeat_n(NODE, nodes.len()));

    let total_area: f32 = sizes.iter().map(|s| (s.x + gap) * (s.y + gap)).sum();
    let widest = sizes.iter().map(|s| s.x).fold(0.0, f32::max);
    let target = total_area.sqrt().max(widest);

    let mut offsets = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut row_height, mut width) = (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32);
    for size in &sizes {
        if x > 0.0 && x + size.x > target {
            y += row_height + gap;
            x = 0.0;
            row_height = 0.0;
        }
        offsets.push(vec2(x, y));
        width = width.max(x + size.x);
        row_height = row_height.max(size.y);
        x += size.x + gap;
    }

    let mut offsets = offsets.into_iter();
    for (child, offset) in boxes.iter_mut().zip(offsets.by_ref()) {
        child.translate(origin.to_vec2() + offset);
    }
    for (node, offset) in nodes.iter_mut().zip(offsets) {
        node.area = Rect::from_min_size(origin + offset, NODE);
    }

    vec2(width, y + row_height)
}

impl Container for District {
    fn area(&mut self) -> &mut Rect {
        &mut self.area
    }
    fn padding(&self) -> f32 {
        DISTRICT_PADDING
    }
    fn parts(&mut self) -> (Vec<&mut dyn Container>, &mut [Node]) {
        let boxes = self
            .sub_districts
            .iter_mut()
            .map(|b| b as &mut dyn Container)
            .collect();
        (boxes, &mut self.nodes)
    }
}

impl Container for SubDistrict {
    fn area(&mut self) -> &mut Rect {
        &mut self.area
    }
    fn padding(&self) -> f32 {
        SUB_DISTRICT_PADDING
    }
    fn parts(&mut self) -> (Vec<&mut dyn Container>, &mut [Node]) {
        let boxes = self
            .groups
            .iter_mut()
            .map(|b| b as &mut dyn Container)
            .collect();
        (boxes, &mut self.nodes)
    }
}

impl Container for Group {
    fn area(&mut self) -> &mut Rect {
        &mut self.area
    }
    fn padding(&self) -> f32 {
        GROUP_PADDING
    }
    fn parts(&mut self) -> (Vec<&mut dyn Container>, &mut [Node]) {
        let boxes = self
            .groups
            .iter_mut()
            .map(|b| b as &mut dyn Container)
            .collect();
        (boxes, &mut self.nodes)
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

    /// Every element's rectangle with its children's, flattened.
    struct Boxed {
        area: Rect,
        children: Vec<Boxed>,
    }

    fn of_group(group: &Group) -> Boxed {
        Boxed {
            area: group.area,
            children: group
                .groups
                .iter()
                .map(of_group)
                .chain(group.nodes.iter().map(of_node))
                .collect(),
        }
    }

    fn of_node(node: &Node) -> Boxed {
        Boxed {
            area: node.area,
            children: Vec::new(),
        }
    }

    fn tree(map: &ProjectMap) -> Vec<Boxed> {
        map.districts
            .iter()
            .map(|d| Boxed {
                area: d.area,
                children: d
                    .sub_districts
                    .iter()
                    .map(|s| Boxed {
                        area: s.area,
                        children: s
                            .groups
                            .iter()
                            .map(of_group)
                            .chain(s.nodes.iter().map(of_node))
                            .collect(),
                    })
                    .chain(d.nodes.iter().map(of_node))
                    .collect(),
            })
            .collect()
    }

    fn strictly_inside(inner: Rect, outer: Rect) -> bool {
        inner.min.x > outer.min.x
            && inner.min.y > outer.min.y
            && inner.max.x < outer.max.x
            && inner.max.y < outer.max.y
    }

    fn disjoint(one: Rect, other: Rect) -> bool {
        one.max.x <= other.min.x
            || other.max.x <= one.min.x
            || one.max.y <= other.min.y
            || other.max.y <= one.min.y
    }

    fn check_siblings(siblings: &[Boxed], parent: Option<Rect>, checked: &mut usize) {
        for (i, one) in siblings.iter().enumerate() {
            if let Some(parent) = parent {
                assert!(
                    strictly_inside(one.area, parent),
                    "{:?} is not strictly inside {parent:?}",
                    one.area
                );
            }
            for other in &siblings[i + 1..] {
                assert!(
                    disjoint(one.area, other.area),
                    "{:?} overlaps {:?}",
                    one.area,
                    other.area
                );
            }
            *checked += 1;
            check_siblings(&one.children, Some(one.area), checked);
        }
    }

    #[test]
    fn no_two_rectangles_overlap_and_every_child_is_inside_its_parent() {
        let map = map_of(&generated_files(400));
        let mut checked = 0;

        check_siblings(&tree(&map), Some(map.extent.expand(1.0)), &mut checked);

        assert!(checked > 400, "the generated tree was too small: {checked}");
    }

    #[test]
    fn the_same_files_lay_out_to_identical_geometry() {
        let files = generated_files(200);

        assert_eq!(map_of(&files), map_of(&files));
    }

    /// The smallest horizontal or vertical gap between any two siblings.
    fn gap(areas: &[Rect]) -> f32 {
        let mut smallest = f32::MAX;
        for (i, one) in areas.iter().enumerate() {
            for other in &areas[i + 1..] {
                let dx = (other.min.x - one.max.x).max(one.min.x - other.max.x);
                let dy = (other.min.y - one.max.y).max(one.min.y - other.max.y);
                smallest = smallest.min(dx.max(dy));
            }
        }
        smallest
    }

    #[test]
    fn districts_are_further_apart_than_sub_districts_and_those_than_groups() {
        let mut files = Vec::new();
        for d in ["a", "b"] {
            for s in ["c", "d"] {
                for g in ["e", "f"] {
                    files.push(PathBuf::from(format!("src/{d}/{s}/{g}/x.rs")));
                }
            }
        }
        let map = map_of(&files);

        let districts: Vec<Rect> = map.districts.iter().map(|d| d.area).collect();
        let subs: Vec<Rect> = map.districts[0]
            .sub_districts
            .iter()
            .map(|s| s.area)
            .collect();
        let groups: Vec<Rect> = map.districts[0].sub_districts[0]
            .groups
            .iter()
            .map(|g| g.area)
            .collect();

        assert!(gap(&districts) > gap(&subs), "{districts:?} {subs:?}");
        assert!(gap(&subs) > gap(&groups), "{subs:?} {groups:?}");
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
