//! Lanes: the connections a lens is built from, drawn between the towers.
//! Imports run as lanes on the ground between blocks, on a street grid; calls
//! arc through the air from the calling line to the called tower.
//!
//! Pure geometry: where each lane runs on the ground. Drawing is in
//! `ui::lanes`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use egui::{Pos2, Rect, pos2};

use crate::city;
use crate::graph::{self, ItemKind, ProjectMap};

/// What a lane stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LaneKind {
    Imports,
    Calls,
}

/// Which lane: its kind, where it starts and ends (blocks for imports,
/// items for calls), and for a call the index of the calling floor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LaneId {
    pub kind: LaneKind,
    pub from: PathBuf,
    pub to: PathBuf,
    pub floor: Option<usize>,
}

/// Which lanes are shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lanes {
    pub imports: bool,
    pub calls: bool,
}

/// The imports of one block from another: the lane they run in.
#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    /// The paths of the importing block and the imported one.
    pub from: PathBuf,
    pub to: PathBuf,
    /// Their ground, and the index of the district each stands in.
    pub from_area: Rect,
    pub to_area: Rect,
    pub from_district: usize,
    pub to_district: usize,
    /// Each import: the `use` block that imports, and the item imported.
    pub imports: Vec<(PathBuf, PathBuf)>,
}

/// One call: the calling item, the index of the calling floor, and the item
/// called.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub caller: PathBuf,
    pub floor: usize,
    pub callee: PathBuf,
    /// The calling line's number, and the block the caller stands on.
    pub line: usize,
    pub block: PathBuf,
    /// Where the arc starts, at the calling line, and ends, on top of the
    /// called tower: a ground point and its height.
    pub start: (Pos2, f32),
    pub end: (Pos2, f32),
}

/// Where a lane runs: its points on the ground, the height of each, and
/// whether it runs between two districts, in a highway.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<Pos2>,
    pub heights: Vec<f32>,
    pub highway: bool,
}

/// The street grid's segments: (column or row, cell, horizontal) to how many
/// lanes run along it and their width.
pub type Streets = BTreeMap<(i32, i32, bool), (usize, f32)>;

/// The lanes of one map worked out on the ground: the import flows and their
/// routes, the street pieces the routes within districts share, and the
/// calls. Worked out once per map, not once per frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Network {
    pub flows: Vec<Flow>,
    pub routes: Vec<Route>,
    pub streets: Streets,
    pub calls: Vec<Call>,
}

impl Network {
    /// The lanes of `map`.
    pub fn of(map: &ProjectMap) -> Network {
        let flows = import_flows(map);
        let routes = routes(map, &flows);
        let streets = streets(&flows, &routes);
        Network {
            calls: calls(map),
            flows,
            routes,
            streets,
        }
    }
}

/// The street grid lanes run on, in layout points.
pub const GRID: f32 = 24.0;

/// The space between the lanes of a highway, and between the highways of
/// different district pairs sharing a street.
const LANE: f32 = 3.0;
const PAIR_GAP: f32 = 5.0;

/// The import flows of `map`, one per pair of blocks, in a fixed order.
/// Tests import nothing a lane is drawn for.
pub fn import_flows(map: &ProjectMap) -> Vec<Flow> {
    let mut block_of: HashMap<&Path, (&Path, Rect, usize)> = HashMap::new();
    for (d, district) in map.districts.iter().enumerate() {
        for node in graph::nodes(district) {
            for item in &node.items {
                block_of.insert(&item.path, (&node.path, node.area, d));
            }
        }
    }
    let mut flows: BTreeMap<(&Path, &Path), Flow> = BTreeMap::new();
    for district in &map.districts {
        for item in graph::items(district) {
            if item.kind != ItemKind::Uses || item.tests {
                continue;
            }
            let Some(&(from, from_area, from_district)) = block_of.get(item.path.as_path()) else {
                continue;
            };
            for target in item.floors.iter().flat_map(|f| &f.imports) {
                let Some(&(to, to_area, to_district)) = block_of.get(target.as_path()) else {
                    continue;
                };
                if from == to {
                    continue;
                }
                flows
                    .entry((from, to))
                    .or_insert_with(|| Flow {
                        from: from.to_path_buf(),
                        to: to.to_path_buf(),
                        from_area,
                        to_area,
                        from_district,
                        to_district,
                        imports: Vec::new(),
                    })
                    .imports
                    .push((item.path.clone(), target.clone()));
            }
        }
    }
    flows.into_values().collect()
}

/// The calls of `map` between towers above the ground, in a fixed order.
pub fn calls(map: &ProjectMap) -> Vec<Call> {
    let items: HashMap<&Path, &crate::Item> = map
        .districts
        .iter()
        .flat_map(graph::items)
        .map(|item| (item.path.as_path(), item))
        .collect();
    let mut calls = Vec::new();
    for node in map.districts.iter().flat_map(graph::nodes) {
        for item in &node.items {
            if item.kind == ItemKind::Uses || city::in_cellar(item) {
                continue;
            }
            for (floor, line) in item.floors.iter().enumerate() {
                for target in &line.calls {
                    let Some(callee) = items.get(target.as_path()) else {
                        continue;
                    };
                    if callee.path != item.path && !city::in_cellar(callee) {
                        let (start, _) = city::floor_span(item, floor);
                        calls.push(Call {
                            caller: item.path.clone(),
                            floor,
                            callee: callee.path.clone(),
                            line: line.number,
                            block: node.path.clone(),
                            start: (start, city::floor_z(item, floor)),
                            end: (callee.area.center(), city::height(callee)),
                        });
                    }
                }
            }
        }
    }
    calls
}

/// Where each of `flows` runs on the ground. Within a district a lane runs
/// along the importer's grid row and the imported block's grid column, so
/// lanes on one street merge. Between two districts every lane of the pair
/// runs out along the row of the lower district's centre and in along the
/// column of the higher's, each in a lane of its own, side by side: a
/// highway.
pub fn routes(map: &ProjectMap, flows: &[Flow]) -> Vec<Route> {
    let snap = |v: f32| (v / GRID).round() * GRID;
    let pair = |flow: &Flow| {
        let (a, b) = (flow.from_district, flow.to_district);
        (a.min(b), a.max(b))
    };
    // The lanes on each row and each column, grouped by the district at the
    // other end, so the lanes of one pair lie together.
    let mut rows: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    let mut columns: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for (i, flow) in flows.iter().enumerate() {
        let (low, high) = pair(flow);
        if low != high {
            rows.entry(low).or_default().push((high, i));
            columns.entry(high).or_default().push((low, i));
        }
    }
    let (row_offset, column_offset) = (offsets(&rows), offsets(&columns));
    flows
        .iter()
        .enumerate()
        .map(|(i, flow)| {
            let (a, b) = (flow.from_area.center(), flow.to_area.center());
            let (low, high) = pair(flow);
            let raised = |d: usize| {
                map.districts
                    .get(d)
                    .map_or(0.0, |district| district.elevation)
            };
            if low == high {
                let (row, column) = (snap(a.y), snap(b.x));
                return Route {
                    points: vec![a, pos2(a.x, row), pos2(column, row), pos2(column, b.y), b],
                    heights: vec![raised(low); 5],
                    highway: false,
                };
            }
            if let Some(route) = stairs(map, flow, row_offset.get(&i).copied().unwrap_or(0.0)) {
                return route;
            }
            // Laid out from the lower district's side, then turned to run
            // from the importer to the imported.
            let (near, far) = if flow.from_district == low {
                (a, b)
            } else {
                (b, a)
            };
            let row = map.districts[low].area.center().y + row_offset[&i];
            let column = map.districts[high].area.center().x + column_offset[&i];
            let mut points = vec![
                near,
                pos2(near.x, row),
                pos2(column, row),
                pos2(column, far.y),
                far,
            ];
            if flow.from_district != low {
                points.reverse();
            }
            let mut heights = vec![0.0; points.len()];
            heights[0] = raised(flow.from_district);
            heights[4] = raised(flow.to_district);
            Route {
                points,
                heights,
                highway: true,
            }
        })
        .collect()
}

/// The route of `flow` down (or up) a staircase: across each step to the
/// edge it shares with the next, straight down or up that edge, and on, so
/// it never crosses open air or passes under a step. `None` unless both its
/// districts are steps of `map`'s staircase.
fn stairs(map: &ProjectMap, flow: &Flow, lane: f32) -> Option<Route> {
    let from = map.stair.iter().position(|d| *d == flow.from_district)?;
    let to = map.stair.iter().position(|d| *d == flow.to_district)?;
    let steps: Vec<usize> = if from <= to {
        map.stair[from..=to].to_vec()
    } else {
        map.stair[to..=from].iter().rev().copied().collect()
    };
    let mut points = vec![flow.from_area.center()];
    let mut heights = vec![map.districts[flow.from_district].elevation];
    for pair in steps.windows(2) {
        let (leave, arrive) = crossing(map, pair[0], pair[1], lane)?;
        let (last, level) = (points[points.len() - 1], heights[heights.len() - 1]);
        points.extend([pos2(last.x, leave.y), leave, arrive]);
        heights.extend([level, level, map.districts[pair[1]].elevation]);
    }
    let (last, level) = (points[points.len() - 1], heights[heights.len() - 1]);
    let end = flow.to_area.center();
    points.extend([pos2(end.x, last.y), end]);
    heights.extend([level, level]);
    Some(Route {
        points,
        heights,
        highway: true,
    })
}

/// Where a lane leaves step `p` and arrives on step `q`: the middle of the
/// edge they share, `lane` along it, on either side of the gap between them.
fn crossing(map: &ProjectMap, p: usize, q: usize, lane: f32) -> Option<(Pos2, Pos2)> {
    let (a, b) = (map.districts[p].area, map.districts[q].area);
    let middle = |a0: f32, a1: f32, b0: f32, b1: f32| {
        let (low, high) = (a0.max(b0), a1.min(b1));
        ((low + high) / 2.0 + lane).clamp(low + 2.0, (high - 2.0).max(low + 2.0))
    };
    // Steps stand `STEP_GAP` apart; anything a few gaps close shares an edge.
    const NEAR: f32 = crate::lens::STEP_GAP * 3.0;
    if (b.max.x - a.min.x).abs() < NEAR {
        let y = middle(a.min.y, a.max.y, b.min.y, b.max.y);
        Some((pos2(a.min.x, y), pos2(b.max.x, y)))
    } else if (a.max.x - b.min.x).abs() < NEAR {
        let y = middle(a.min.y, a.max.y, b.min.y, b.max.y);
        Some((pos2(a.max.x, y), pos2(b.min.x, y)))
    } else if (b.max.y - a.min.y).abs() < NEAR {
        let x = middle(a.min.x, a.max.x, b.min.x, b.max.x);
        Some((pos2(x, a.min.y), pos2(x, b.max.y)))
    } else if (a.max.y - b.min.y).abs() < NEAR {
        let x = middle(a.min.x, a.max.x, b.min.x, b.max.x);
        Some((pos2(x, a.max.y), pos2(x, b.min.y)))
    } else {
        None
    }
}

/// For lanes grouped by street, each lane's offset across its street: the
/// lanes of one district pair side by side, a wider gap between pairs, the
/// whole bundle centred on the street.
fn offsets(streets: &BTreeMap<usize, Vec<(usize, usize)>>) -> HashMap<usize, f32> {
    let mut offset = HashMap::new();
    for lanes in streets.values() {
        let mut lanes = lanes.clone();
        lanes.sort();
        let pairs = lanes.windows(2).filter(|w| w[0].0 != w[1].0).count();
        let width = lanes.len().saturating_sub(1) as f32 * LANE + pairs as f32 * PAIR_GAP;
        let mut at = -width / 2.0;
        for (k, (other, lane)) in lanes.iter().enumerate() {
            if k > 0 && lanes[k - 1].0 != *other {
                at += PAIR_GAP;
            }
            offset.insert(*lane, at);
            at += LANE;
        }
    }
    offset
}

/// The pieces of grid street the lanes within districts share, and how many
/// imports run along each: one piece of `GRID` from its grid corner, along
/// ground x or along ground y.
pub fn streets(flows: &[Flow], routes: &[Route]) -> Streets {
    let cell = |v: f32| (v / GRID).round() as i32;
    let mut pieces = BTreeMap::new();
    for (flow, route) in flows.iter().zip(routes).filter(|(_, r)| !r.highway) {
        let carried = flow.imports.len();
        for leg in route.points[1..4].windows(2) {
            let (start, end) = (leg[0], leg[1]);
            let along_x = (end.y - start.y).abs() < 1e-3;
            let (from, to) = if along_x {
                (cell(start.x.min(end.x)), cell(start.x.max(end.x)))
            } else {
                (cell(start.y.min(end.y)), cell(start.y.max(end.y)))
            };
            for k in from..to {
                let key = if along_x {
                    (k, cell(start.y), true)
                } else {
                    (cell(start.x), k, false)
                };
                let piece = pieces.entry(key).or_insert((0, route.heights[0]));
                piece.0 += carried;
            }
        }
    }
    pieces
}

/// How long `points` is on the ground.
pub fn length(points: &[Pos2]) -> f32 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow(from: (usize, Rect), to: (usize, Rect)) -> Flow {
        Flow {
            from: PathBuf::from(format!("/p/{}", from.0)),
            to: PathBuf::from(format!("/p/{}", to.0)),
            from_area: from.1,
            to_area: to.1,
            from_district: from.0,
            to_district: to.0,
            imports: vec![(PathBuf::from("/p/use"), PathBuf::from("/p/x"))],
        }
    }

    fn map_of(districts: &[Rect]) -> ProjectMap {
        let mut map = crate::ProjectMap::of(
            "p",
            Path::new("/p"),
            &[],
            &crate::Directories {
                prefix: crate::default_prefix_rule(),
            },
        );
        map.districts = districts
            .iter()
            .enumerate()
            .map(|(n, area)| graph::District {
                name: n.to_string(),
                path: PathBuf::from(format!("/p/d{n}")),
                sub_districts: Vec::new(),
                nodes: Vec::new(),
                area: *area,
                kind: graph::DistrictKind::Folder,
                elevation: 0.0,
            })
            .collect();
        map
    }

    #[test]
    fn lanes_between_two_districts_run_side_by_side_and_lanes_within_one_at_right_angles() {
        let left = Rect::from_center_size(pos2(0.0, 0.0), egui::vec2(100.0, 100.0));
        let right = Rect::from_center_size(pos2(400.0, 300.0), egui::vec2(100.0, 100.0));
        let map = map_of(&[left, right]);
        let block = |x: f32, y: f32| Rect::from_center_size(pos2(x, y), egui::vec2(10.0, 10.0));
        let flows = [
            flow((0, block(-20.0, 10.0)), (1, block(410.0, 290.0))),
            flow((0, block(20.0, -10.0)), (1, block(380.0, 320.0))),
            flow((0, block(-20.0, 10.0)), (0, block(30.0, 30.0))),
        ];

        let routes = routes(&map, &flows);

        assert!(routes[0].highway && routes[1].highway && !routes[2].highway);
        // The trunks of the two highway lanes run parallel, a lane apart.
        let (a, b) = (&routes[0].points, &routes[1].points);
        assert!((a[1].y - b[1].y).abs() >= LANE - 1e-3, "{a:?} {b:?}");
        assert!((a[2].x - b[2].x).abs() >= LANE - 1e-3);
        for route in &routes {
            for leg in route.points.windows(2) {
                let d = leg[1] - leg[0];
                assert!(
                    d.x.abs() < 1e-3 || d.y.abs() < 1e-3,
                    "a leg at an angle: {leg:?}"
                );
            }
        }
    }

    #[test]
    fn lanes_within_a_district_on_one_street_merge() {
        let block = |x: f32, y: f32| Rect::from_center_size(pos2(x, y), egui::vec2(10.0, 10.0));
        let map = map_of(&[Rect::from_center_size(Pos2::ZERO, egui::vec2(400.0, 400.0))]);
        let flows = [
            flow((0, block(0.0, 0.0)), (0, block(96.0, 48.0))),
            flow((0, block(0.0, 0.0)), (0, block(96.0, 96.0))),
        ];

        let pieces = streets(&flows, &routes(&map, &flows));

        assert_eq!(
            pieces.get(&(0, 0, true)).map(|p| p.0),
            Some(2),
            "{pieces:?}"
        );
    }
}
