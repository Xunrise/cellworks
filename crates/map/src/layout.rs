//! The city layout: every district a town around a free plaza, every folder
//! a quarter of the folder holding it, the districts beside each other.
//!
//! A block is a node: its buildings stand on it like a town
//! ([`city::place`]); a node without code is a low, narrow block. At every
//! level the same arrangement places what a folder holds ([`arrange`]):
//! blocks and folders whose code reaches each other (calls and imports into
//! the other's files) stand together, a folder's documents gather in one
//! quarter, and districts settle beside the districts their code reaches. A
//! district keeps a free plaza at its centre, its ground reaching as far on
//! every side of it; a district's own blocks of only tests lie in its cellar
//! under the plaza and take no ground.
//!
//! Invariants: every block holds its buildings, whole, whichever files their
//! parts are written in; no two blocks, no two folders of one folder and no
//! two districts overlap; every block lies inside its folder's area, and
//! every folder inside the area of the folder holding it. A district's plaza
//! is the centre of its area.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::arrange::{self, Piece, arrange, seed};
use crate::city;
use crate::graph::{self, District, Group, Node, NodeKind};
use crate::lens::cluster::{Weights, link};

/// The strip a name takes on screen at most.
pub(crate) const TITLE: f32 = 22.0;

/// The block of a node without code.
const BARE_BLOCK: Vec2 = vec2(24.0, 6.0);

/// The narrowest block of a node with code.
const NARROWEST_BLOCK: f32 = 48.0;

/// The half-width of the plaza left free at a district's centre, the
/// streets between blocks and folders, and the margin from the outermost of
/// them to the district's edge.
const PLAZA: f32 = 14.0;
const BLOCK_GAP: f32 = 14.0;
const DISTRICT_MARGIN: f32 = 10.0;

/// The margin from the outermost block or folder of a folder to its edge.
const FOLDER_MARGIN: f32 = 8.0;

/// The streets between districts.
const DISTRICT_GAP: f32 = 60.0;

/// How strongly two documents of one folder are bound, so they gather.
const LIBRARY: f32 = 4.0;

/// For each node, how often its code reaches each other node: the calls and
/// imports of its items into the items standing there.
type Reaches = HashMap<PathBuf, BTreeMap<PathBuf, f32>>;

/// What the layout knows of the whole project while it lays out a part of
/// it: how often each node reaches each other, and the project's root, which
/// the seeds of folders and blocks are taken relative to, so a project lays
/// out the same wherever it lies.
struct Known<'a> {
    reaches: Reaches,
    root: &'a Path,
}

/// What a laid-out block, folder or district is to the one holding it: the
/// piece it stands as, the nodes it holds, how often its code reaches each
/// node, and whether it is a document.
struct Part {
    piece: Piece,
    held: Vec<PathBuf>,
    reaches: BTreeMap<PathBuf, f32>,
    document: bool,
}

impl Part {
    /// `parts` together as one piece of `size`, as high as the highest.
    fn merged(size: Vec2, path: &Path, root: &Path, parts: Vec<Part>) -> Part {
        let mut held = Vec::new();
        let mut reaches = BTreeMap::new();
        let mut height: f32 = 0.0;
        for part in parts {
            held.extend(part.held);
            height = height.max(part.piece.height);
            for (node, n) in part.reaches {
                *reaches.entry(node).or_default() += n;
            }
        }
        Part {
            piece: piece(size, height, path, root),
            held,
            reaches,
            document: false,
        }
    }
}

/// The piece of the folder or block at `path` in the project at `root`.
fn piece(size: Vec2, height: f32, path: &Path, root: &Path) -> Piece {
    let relative = path.strip_prefix(root).unwrap_or(path);
    Piece {
        size,
        weight: 0.0,
        room: 0.0,
        height,
        seed: seed(relative.as_os_str().as_encoded_bytes()),
    }
}

/// Lays out every district of the project at `root` and returns the
/// project's extent.
pub(crate) fn lay_out(districts: &mut [District], root: &Path) -> Rect {
    let known = Known {
        reaches: reaches(districts),
        root,
    };
    let parts: Vec<Part> = districts
        .iter_mut()
        .map(|district| lay_out_district(district, &known))
        .collect();
    let street = (DISTRICT_GAP / 2.0, DISTRICT_GAP / 2.0);
    let corners = settle(&parts, None, street);
    let mut extent = Rect::NOTHING;
    for (district, corner) in districts.iter_mut().zip(corners) {
        let offset = corner - district.area.min;
        translate(district, offset);
        extent = extent.union(district.area);
    }
    extent
}

/// How often the code of each node of `districts` reaches each other node.
fn reaches(districts: &[District]) -> Reaches {
    let nodes: Vec<&Node> = districts.iter().flat_map(graph::nodes).collect();
    let node_of: HashMap<&Path, &Path> = nodes
        .iter()
        .flat_map(|node| {
            node.items
                .iter()
                .map(|i| (i.path.as_path(), node.path.as_path()))
        })
        .collect();
    nodes
        .iter()
        .map(|node| {
            let mut reached: BTreeMap<PathBuf, f32> = BTreeMap::new();
            let targets = node
                .items
                .iter()
                .flat_map(|i| &i.floors)
                .flat_map(|f| f.calls.iter().chain(&f.imports));
            for target in targets {
                if let Some(other) = node_of.get(target.as_path())
                    && *other != node.path
                {
                    *reached.entry(other.to_path_buf()).or_default() += 1.0;
                }
            }
            (node.path.clone(), reached)
        })
        .collect()
}

/// Where each of `parts` stands, by its corner, after `first` if given:
/// each beside what its code reaches most, a folder's documents together.
fn settle(parts: &[Part], first: Option<Piece>, street: (f32, f32)) -> Vec<Pos2> {
    let skip = usize::from(first.is_some());
    let index: HashMap<&Path, usize> = parts
        .iter()
        .enumerate()
        .flat_map(|(i, part)| part.held.iter().map(move |node| (node.as_path(), i + skip)))
        .collect();
    let mut affinity = Weights::new();
    for (i, part) in parts.iter().enumerate() {
        for (node, n) in &part.reaches {
            if let Some(&j) = index.get(node.as_path()) {
                link(&mut affinity, i + skip, j, *n);
            }
        }
    }
    let documents: Vec<usize> = (0..parts.len()).filter(|i| parts[*i].document).collect();
    for (k, a) in documents.iter().enumerate() {
        for b in &documents[k + 1..] {
            link(&mut affinity, a + skip, b + skip, LIBRARY);
        }
    }
    let pieces: Vec<Piece> = first
        .into_iter()
        .chain(parts.iter().map(|part| part.piece))
        .collect();
    arrange(&pieces, &affinity, street)
}

/// Lays `district` out and returns it as a part: a plaza first, what the
/// district holds around it, its area reaching as far on every side of the
/// plaza.
fn lay_out_district(district: &mut District, known: &Known) -> Part {
    let (mut cellar, mut ground): (Vec<&mut Node>, Vec<&mut Node>) =
        district.nodes.iter_mut().partition(|node| tests_only(node));
    let mut parts: Vec<Part> = district
        .sub_districts
        .iter_mut()
        .map(|sub| {
            let part = folder(&mut sub.groups, &mut sub.nodes, &sub.path, known);
            sub.area = Rect::from_min_size(Pos2::ZERO, part.piece.size);
            part
        })
        .collect();
    parts.extend(ground.iter_mut().map(|node| block(node, known)));
    let plaza = Piece {
        size: Vec2::splat(2.0 * PLAZA),
        weight: f32::INFINITY,
        ..piece(Vec2::ZERO, 0.0, &district.path, known.root)
    };
    let corners = settle(&parts, Some(plaza), (BLOCK_GAP, BLOCK_GAP / 2.0));
    let centre = Rect::from_min_size(corners[0], plaza.size).center();
    let (at_subs, at_blocks) = corners[1..].split_at(district.sub_districts.len());
    let mut extent = Rect::from_center_size(centre, plaza.size);
    for (sub, corner) in district.sub_districts.iter_mut().zip(at_subs) {
        let offset = *corner - sub.area.min;
        sub.area = sub.area.translate(offset);
        shift_groups(&mut sub.groups, offset);
        shift_nodes(&mut sub.nodes, offset);
        extent = extent.union(sub.area);
    }
    for (node, corner) in ground.iter_mut().zip(at_blocks) {
        let offset = *corner - node.area.min;
        shift_nodes(std::slice::from_mut(*node), offset);
        extent = extent.union(node.area);
    }
    let cellar_radii: Vec<f32> = cellar
        .iter_mut()
        .map(|node| block(node, known).piece.size.length() / 2.0)
        .collect();
    for (node, at) in cellar.iter_mut().zip(spiral(&cellar_radii, 0.0, BLOCK_GAP)) {
        move_node(node, centre + at.to_vec2());
    }
    district.area = arrange::about(extent, centre).expand(DISTRICT_MARGIN);
    let mut part = Part::merged(district.area.size(), &district.path, known.root, parts);
    for node in &cellar {
        part.held.push(node.path.clone());
    }
    part
}

/// Lays out a folder's own `groups` and `nodes` from the origin, each group
/// a folder of its own, and returns the folder at `path` as a part.
fn folder(groups: &mut [Group], nodes: &mut [Node], path: &Path, known: &Known) -> Part {
    let mut parts: Vec<Part> = groups
        .iter_mut()
        .map(|group| {
            let part = folder(&mut group.groups, &mut group.nodes, &group.path, known);
            group.area = Rect::from_min_size(Pos2::ZERO, part.piece.size);
            part
        })
        .collect();
    parts.extend(nodes.iter_mut().map(|node| block(node, known)));
    let corners = settle(&parts, None, (BLOCK_GAP * 0.6, BLOCK_GAP * 0.6));
    let extent = parts
        .iter()
        .zip(&corners)
        .fold(Rect::NOTHING, |e, (part, corner)| {
            e.union(Rect::from_min_size(*corner, part.piece.size))
        });
    let origin = if extent.is_finite() {
        extent.min
    } else {
        Pos2::ZERO
    };
    let inward = Vec2::splat(FOLDER_MARGIN) - origin.to_vec2();
    let (at_groups, at_nodes) = corners.split_at(groups.len());
    for (group, corner) in groups.iter_mut().zip(at_groups) {
        let offset = *corner - group.area.min + inward;
        group.area = group.area.translate(offset);
        shift_groups(&mut group.groups, offset);
        shift_nodes(&mut group.nodes, offset);
    }
    for (node, corner) in nodes.iter_mut().zip(at_nodes) {
        let offset = *corner - node.area.min + inward;
        shift_nodes(std::slice::from_mut(node), offset);
    }
    let size = if extent.is_finite() {
        extent.size()
    } else {
        Vec2::ZERO
    };
    Part::merged(
        size + Vec2::splat(2.0 * FOLDER_MARGIN),
        path,
        known.root,
        parts,
    )
}

/// Whether every tower on `node` belongs to the tests.
fn tests_only(node: &Node) -> bool {
    !node.items.is_empty() && node.items.iter().all(city::in_cellar)
}

/// Places the buildings of `node` on its block at the origin, the block left
/// in `node.area` until it is moved into place, and returns the block as a
/// part.
fn block(node: &mut Node, known: &Known) -> Part {
    let size = if node.items.is_empty() {
        BARE_BLOCK
    } else {
        city::place(&mut node.items, node.kind, NARROWEST_BLOCK)
    };
    node.area = Rect::from_min_size(Pos2::ZERO, size);
    let height = node
        .items
        .iter()
        .filter(|item| !city::in_cellar(item))
        .map(city::height)
        .fold(0.0, f32::max);
    Part {
        piece: piece(size, height, &node.path, known.root),
        held: vec![node.path.clone()],
        reaches: known.reaches.get(&node.path).cloned().unwrap_or_default(),
        document: node.kind == NodeKind::Document,
    }
}

/// Moves `node` and its towers so its block is centred on `centre`.
fn move_node(node: &mut Node, centre: Pos2) {
    let offset = centre - node.area.center();
    shift_nodes(std::slice::from_mut(node), offset);
}

/// Centres for circles of `radii`, in order, along a spiral out from the
/// origin: each takes the first place on the spiral where it keeps `gap` from
/// every circle placed before it. With no `hole`, the first sits at the
/// origin. The cellar's blocks hang under a plaza so.
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
    fn no_two_blocks_folders_or_districts_overlap_and_every_folder_holds_what_it_holds() {
        let map = map_of(&generated_files(400));
        let holdings = holdings(&map);
        let blocks: Vec<Rect> = map
            .districts
            .iter()
            .flat_map(graph::nodes)
            .map(|node| node.area)
            .collect();

        assert_eq!(blocks.len(), 400);
        for (i, one) in blocks.iter().enumerate() {
            for other in &blocks[i + 1..] {
                assert!(disjoint(*one, *other), "{one:?} overlaps {other:?}");
            }
        }
        for (i, one) in map.districts.iter().enumerate() {
            for other in &map.districts[i + 1..] {
                assert!(
                    disjoint(one.area, other.area),
                    "{:?} {:?}",
                    one.area,
                    other.area
                );
            }
        }
        assert!(holdings.len() > 10, "the generated tree was too small");
        for (folder, held) in holdings {
            for (i, inner) in held.iter().enumerate() {
                assert!(
                    folder.contains_rect(*inner),
                    "{inner:?} is outside {folder:?}"
                );
                for other in &held[i + 1..] {
                    assert!(disjoint(*inner, *other), "{inner:?} overlaps {other:?}");
                }
            }
        }
    }

    #[test]
    fn a_districts_plaza_is_the_centre_of_its_area_and_left_free() {
        let map = map_of(&generated_files(200));

        for district in &map.districts {
            let plaza = Rect::from_center_size(district.area.center(), Vec2::splat(2.0 * PLAZA));
            let subs = district.sub_districts.iter().map(|s| s.area);
            for held in subs.chain(district.nodes.iter().map(|n| n.area)) {
                assert!(
                    disjoint(plaza, held),
                    "{held:?} stands on the plaza {plaza:?}"
                );
            }
        }
    }

    #[test]
    fn blocks_whose_code_reaches_each_other_stand_together() {
        // a.rs calls into c.rs; b.rs reaches nothing.
        let files: Vec<PathBuf> = ["src/x/a.rs", "src/x/b.rs", "src/x/c.rs", "src/x/d.rs"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let item = |file: &str, name: &str, calls: Vec<PathBuf>| crate::Item {
            name: name.to_owned(),
            path: PathBuf::from(format!("/p/shop/{file}/{name}")),
            kind: crate::ItemKind::Function,
            tests: false,
            floors: vec![crate::Floor {
                number: 1,
                text: String::new(),
                indent: 0,
                width: 40,
                tokens: Vec::new(),
                calls,
                imports: Vec::new(),
            }],
            area: Rect::NOTHING,
            base: 0.0,
            declaration: Default::default(),
        };
        let callee = PathBuf::from("/p/shop/src/x/c.rs/f");
        let items: BTreeMap<PathBuf, Vec<crate::Item>> = [
            (
                "src/x/a.rs",
                vec![item("src/x/a.rs", "f", vec![callee.clone(); 5])],
            ),
            ("src/x/b.rs", vec![item("src/x/b.rs", "f", Vec::new())]),
            ("src/x/c.rs", vec![item("src/x/c.rs", "f", Vec::new())]),
            ("src/x/d.rs", vec![item("src/x/d.rs", "f", Vec::new())]),
        ]
        .into_iter()
        .map(|(file, items)| (PathBuf::from(file), items))
        .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of_code(
            "shop",
            Path::new("/p/shop"),
            &files,
            &strategy,
            items,
            &BTreeMap::new(),
        );
        let at = |file: &str| map.find(&Path::new("/p/shop").join(file)).unwrap().area;
        let apart = |a: Rect, b: Rect| (a.center() - b.center()).length();

        let (a, b, c) = (at("src/x/a.rs"), at("src/x/b.rs"), at("src/x/c.rs"));
        assert!(apart(a, c) < apart(b, c), "a {a:?} b {b:?} c {c:?}");
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
