//! The imports lens as a staircase of what builds on what.
//!
//! A block is one file, or the files of an import cycle: a strongly
//! connected set. Within a district the blocks stand in rows by import depth,
//! what imports nothing else of the district at the back. Between districts
//! the foundation stands on the ground; every level that imports the ones
//! below stands edge to edge beside the step before, alternately along
//! ground -x and ground -y (up-left and up-right on screen), one level
//! higher. Levels follow the net import direction, so imports both ways do
//! not lift both districts.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use super::cluster::Weights;
use super::relation::Unit;
use crate::graph::{District, ItemKind};
use crate::layout;

/// How much higher each level stands than the one below it.
pub(crate) const LEVEL: f32 = 90.0;

/// The space left between two steps, and between rows within a district.
pub(crate) const STEP_GAP: f32 = 6.0;
const ROW_GAP: f32 = 12.0;

/// The import cycles of `units`: units that import each other in a loop
/// share a label, every other unit with an import its own. `None` for a
/// unit with no import either way.
pub(crate) fn cycles(units: &[Unit], weights: &Weights) -> Vec<Option<usize>> {
    let mut unit_of: HashMap<&Path, usize> = HashMap::new();
    for (u, unit) in units.iter().enumerate() {
        for item in &unit.items {
            unit_of.insert(&item.path, u);
        }
    }
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); units.len()];
    for (u, unit) in units.iter().enumerate() {
        let uses = unit.items.iter().filter(|item| item.kind == ItemKind::Uses);
        for target in uses
            .flat_map(|item| &item.floors)
            .flat_map(|floor| &floor.imports)
        {
            if let Some(&v) = unit_of.get(target.as_path())
                && v != u
                && !out[u].contains(&v)
            {
                out[u].push(v);
            }
        }
    }
    let component = strongly_connected(&out);
    let mut linked = vec![false; units.len()];
    for &(a, b) in weights.keys() {
        linked[a] = true;
        linked[b] = true;
    }
    (0..units.len())
        .map(|u| linked[u].then_some(component[u]))
        .collect()
}

/// Tarjan's strongly connected components of the graph `out`: the component
/// of each node.
pub(crate) fn strongly_connected(out: &[Vec<usize>]) -> Vec<usize> {
    struct Walk<'a> {
        out: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        component: Vec<usize>,
        components: usize,
    }
    fn visit(walk: &mut Walk<'_>, v: usize) {
        walk.index[v] = Some(walk.next);
        walk.low[v] = walk.next;
        walk.next += 1;
        walk.stack.push(v);
        walk.on_stack[v] = true;
        for &w in walk.out[v].iter() {
            match walk.index[w] {
                None => {
                    visit(walk, w);
                    walk.low[v] = walk.low[v].min(walk.low[w]);
                }
                Some(index) if walk.on_stack[w] => walk.low[v] = walk.low[v].min(index),
                Some(_) => {}
            }
        }
        if Some(walk.low[v]) == walk.index[v] {
            while let Some(w) = walk.stack.pop() {
                walk.on_stack[w] = false;
                walk.component[w] = walk.components;
                if w == v {
                    break;
                }
            }
            walk.components += 1;
        }
    }
    let n = out.len();
    // ponytail: recursive; an explicit stack if a project's imports chain
    // deeper than the thread's stack allows.
    let mut walk = Walk {
        out,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        component: vec![0; n],
        components: 0,
    };
    for v in 0..n {
        if walk.index[v].is_none() {
            visit(&mut walk, v);
        }
    }
    walk.component
}

/// The depth of each of `n` nodes over the directed edges `from → to` of
/// `edges` with a positive weight: 0 for a node that imports nothing, else
/// one more than the deepest it imports. Only the net direction of each pair
/// counts, so a loop never lifts.
fn depths(n: usize, edges: &BTreeMap<(usize, usize), f32>) -> Vec<usize> {
    let mut depth = vec![0usize; n];
    for _ in 0..n {
        let mut moved = false;
        for (&(from, to), &w) in edges {
            let back = edges.get(&(to, from)).copied().unwrap_or(0.0);
            if w > back && depth[from] < depth[to] + 1 && depth[to] < n {
                depth[from] = depth[to] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    depth
}

/// How far an axis-aligned rectangle reaches along either screen diagonal.
fn span(rect: Rect) -> f32 {
    (rect.width() + rect.height()) / 2f32.sqrt()
}

/// Re-places the blocks of `district` in rows by import depth, what imports
/// nothing else of the district at the back, rows running across the screen;
/// the district becomes the square around them, centred where it was.
pub(crate) fn rows(district: &mut District) {
    let n = district.nodes.len();
    if n == 0 {
        return;
    }
    let block_of: HashMap<&Path, usize> = district
        .nodes
        .iter()
        .enumerate()
        .flat_map(|(b, node)| node.items.iter().map(move |item| (item.path.as_path(), b)))
        .collect();
    let mut edges: BTreeMap<(usize, usize), f32> = BTreeMap::new();
    for (b, node) in district.nodes.iter().enumerate() {
        let uses = node.items.iter().filter(|item| item.kind == ItemKind::Uses);
        for target in uses
            .flat_map(|item| &item.floors)
            .flat_map(|floor| &floor.imports)
        {
            if let Some(&t) = block_of.get(target.as_path())
                && t != b
            {
                *edges.entry((b, t)).or_default() += 1.0;
            }
        }
    }
    let depth = depths(n, &edges);
    let mut rows: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (b, d) in depth.iter().enumerate() {
        rows.entry(*d).or_default().push(b);
    }
    let centre = district.area.center();
    let across = vec2(1.0, -1.0) / 2f32.sqrt();
    let toward = vec2(1.0, 1.0) / 2f32.sqrt();
    let deep = |row: &[usize], nodes: &[crate::Node]| {
        row.iter().map(|b| span(nodes[*b].area)).fold(0.0, f32::max)
    };
    let total: f32 = rows
        .values()
        .map(|row| deep(row, &district.nodes) + ROW_GAP)
        .sum::<f32>()
        - ROW_GAP;
    let mut at_depth = -total / 2.0;
    let mut extent = Rect::NOTHING;
    for row in rows.values() {
        let row_deep = deep(row, &district.nodes);
        let width: f32 = row
            .iter()
            .map(|b| span(district.nodes[*b].area) + ROW_GAP)
            .sum::<f32>()
            - ROW_GAP;
        let mut at = -width / 2.0;
        for b in row {
            let node = &mut district.nodes[*b];
            let w = span(node.area);
            let target = centre + across * (at + w / 2.0) + toward * (at_depth + row_deep / 2.0);
            let offset = target - node.area.center();
            node.area = node.area.translate(offset);
            for item in &mut node.items {
                item.area = item.area.translate(offset);
            }
            extent = extent.union(node.area);
            at += w + ROW_GAP;
        }
        at_depth += row_deep + ROW_GAP;
    }
    let reach = [
        extent.left_top(),
        extent.right_top(),
        extent.left_bottom(),
        extent.right_bottom(),
    ]
    .iter()
    .map(|corner| (*corner - centre).abs().max_elem())
    .fold(0.0, f32::max);
    district.area = Rect::from_center_size(centre, Vec2::splat(2.0 * (reach + 10.0)));
}

/// Stands the districts `core` of `districts` as a staircase by the net
/// directed import weights `edges` between them, raising each and what
/// stands on it; returns the districts in stair order, the foundation first.
pub(crate) fn staircase(
    districts: &mut [District],
    core: &[usize],
    edges: &BTreeMap<(usize, usize), f32>,
) -> Vec<usize> {
    let all = depths(districts.len(), edges);
    // Number the levels that occur, so no level is left empty.
    let mut levels: Vec<usize> = core.iter().map(|d| all[*d]).collect();
    levels.sort_unstable();
    levels.dedup();
    let level_of = |d: usize| levels.iter().position(|l| *l == all[d]).unwrap_or(0);
    let mut order: Vec<usize> = core.to_vec();
    order.sort_by_key(|d| (level_of(*d), *d));
    let mut previous: Option<(Pos2, f32, usize)> = None;
    let mut along_x = false;
    let mut at: HashMap<usize, Pos2> = HashMap::new();
    for &d in &order {
        let level = level_of(d);
        let r = districts[d].area.width() / 2.0;
        let footprint = match previous {
            None => Pos2::ZERO,
            Some((before, before_r, before_level)) => {
                if level != before_level {
                    along_x = !along_x;
                }
                // Edge to edge beside the step before, the front edges in one
                // line, so the staircase's edges run straight.
                let front = before_r - r;
                if along_x {
                    pos2(before.x - (before_r + r + STEP_GAP), before.y + front)
                } else {
                    pos2(before.x + front, before.y - (before_r + r + STEP_GAP))
                }
            }
        };
        at.insert(d, footprint);
        previous = Some((footprint, r, level));
        raise(&mut districts[d], level as f32 * LEVEL);
    }
    for (d, centre) in at {
        let offset = centre - districts[d].area.center();
        layout::translate(&mut districts[d], offset);
    }
    order
}

/// Raises `district` and everything standing on it to `height`.
fn raise(district: &mut District, height: f32) {
    district.elevation = height;
    for node in crate::graph::nodes_mut(std::slice::from_mut(district)) {
        for item in &mut node.items {
            item.base = height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_in_a_loop_share_a_component_and_the_rest_stand_alone() {
        // 0 → 1 → 2 → 0 is a loop; 3 → 0 is not part of it.
        let out = vec![vec![1], vec![2], vec![0], vec![0]];

        let component = strongly_connected(&out);

        assert_eq!(component[0], component[1]);
        assert_eq!(component[1], component[2]);
        assert_ne!(component[3], component[0]);
    }

    #[test]
    fn only_the_net_direction_lifts() {
        let mut edges = BTreeMap::new();
        edges.insert((1, 0), 3.0);
        edges.insert((0, 1), 1.0);
        edges.insert((2, 1), 1.0);

        assert_eq!(depths(3, &edges), [0, 1, 2]);
    }

    fn square(n: usize, side: f32) -> District {
        District {
            name: format!("d{n}"),
            path: std::path::PathBuf::from(format!("/p/d{n}")),
            sub_districts: Vec::new(),
            nodes: Vec::new(),
            area: Rect::from_center_size(Pos2::ZERO, Vec2::splat(side)),
            kind: crate::DistrictKind::Cluster,
            elevation: 0.0,
        }
    }

    #[test]
    fn each_level_stands_edge_to_edge_beside_the_one_below_and_higher() {
        let mut districts = vec![square(0, 100.0), square(1, 60.0), square(2, 80.0)];
        let mut edges = BTreeMap::new();
        edges.insert((1, 0), 1.0);
        edges.insert((2, 1), 1.0);

        let order = staircase(&mut districts, &[0, 1, 2], &edges);

        assert_eq!(order, [0, 1, 2]);
        assert_eq!(
            districts.iter().map(|d| d.elevation).collect::<Vec<_>>(),
            [0.0, LEVEL, 2.0 * LEVEL]
        );
        for pair in order.windows(2) {
            let (a, b) = (districts[pair[0]].area, districts[pair[1]].area);
            assert!(
                !a.intersects(b.shrink(0.5)),
                "steps do not overlap: {a:?} {b:?}"
            );
            let gap = (a.distance_to_pos(b.center()) - b.width() / 2.0).abs();
            assert!(gap <= STEP_GAP + 1e-3, "edge to edge: {gap}");
        }
        // Up-left, then up-right: along -x, then along -y, front edges in
        // line.
        assert!(districts[1].area.center().x < districts[0].area.center().x);
        assert!((districts[1].area.max.y - districts[0].area.max.y).abs() < 1e-3);
        assert!(districts[2].area.center().y < districts[1].area.center().y);
        assert!((districts[2].area.max.x - districts[1].area.max.x).abs() < 1e-3);
    }
}
