//! The relationship lenses: blocks and districts rebuilt from imports, calls
//! or co-change (ADR-0005).
//!
//! A unit is what the lens connects: a function for calls, a file for imports
//! and co-change. Units joined by the relationship form blocks, blocks joined
//! by it form districts, each named after what holds it together. Tests hang
//! under the district whose code they call most; code the lens connects to
//! nothing stands in the outskirts.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use egui::Rect;

use super::Lens;
use super::cluster::{Weights, communities, link};
use crate::graph::{self, District, DistrictKind, Item, ItemKind, Node, NodeKind, ProjectMap};

/// What a lens connects, with the items it carries.
pub(crate) struct Unit {
    /// The file the unit comes from.
    pub file: PathBuf,
    pub items: Vec<Item>,
    /// The kind of the node it comes from.
    pub kind: NodeKind,
}

/// How a lens groups its units into blocks.
pub(crate) type Blocks = fn(&[Unit], &Weights) -> Vec<Option<usize>>;

/// The districts of `folders` seen through the relationship `lens`, not yet
/// laid out; `blocks` groups the units into blocks.
pub(crate) fn rebuild(folders: &ProjectMap, lens: Lens, blocks_of: Blocks) -> Vec<District> {
    let root = &folders.root;
    let place = |parts: &[String]| {
        parts
            .iter()
            .fold(root.join(".lens").join(lens.key()), |p, part| p.join(part))
    };
    let (units, tests) = units(folders, lens);
    let (weights, score) = links(folders, lens, &units);
    let labels = blocks_of(&units, &weights);

    let mut blocks: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut outskirts: BTreeMap<PathBuf, (NodeKind, Vec<Item>)> = BTreeMap::new();
    for (unit, label) in labels.iter().enumerate() {
        match label {
            Some(label) => blocks.entry(*label).or_default().push(unit),
            None => outskirts
                .entry(units[unit].file.clone())
                .or_insert_with(|| (units[unit].kind, Vec::new()))
                .1
                .extend(units[unit].items.iter().cloned()),
        }
    }
    let blocks: Vec<Vec<usize>> = blocks.into_values().collect();
    let block_of: HashMap<usize, usize> = blocks
        .iter()
        .enumerate()
        .flat_map(|(b, members)| members.iter().map(move |u| (*u, b)))
        .collect();
    let mut between = Weights::new();
    for (&(a, b), &w) in &weights {
        if let (Some(&x), Some(&y)) = (block_of.get(&a), block_of.get(&b)) {
            link(&mut between, x, y, w);
        }
    }
    let mut districts_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut lone: Vec<usize> = Vec::new();
    for (block, label) in communities(blocks.len(), &between).into_iter().enumerate() {
        match label {
            Some(label) => districts_of.entry(label).or_default().push(block),
            None => lone.push(block),
        }
    }
    // A block linked to no other block stands in the outskirts, unless no
    // block links to another at all: then each is a district of its own,
    // rather than leaving the lens nothing to show.
    if districts_of.is_empty() {
        for block in lone.drain(..) {
            districts_of.insert(usize::MAX - block, vec![block]);
        }
    }

    let block_node = |members: &[usize], path: PathBuf| {
        let mut items = Vec::new();
        let mut files: BTreeMap<&Path, usize> = BTreeMap::new();
        for u in members {
            *files.entry(&units[*u].file).or_default() += 1;
            items.extend(units[*u].items.iter().cloned());
        }
        // A block of one kind of file stands as that kind; a mixed one as
        // code.
        let mut kinds = members.iter().map(|u| units[*u].kind);
        let first = kinds.next().unwrap_or_default();
        let kind = if kinds.all(|kind| kind == first) {
            first
        } else {
            NodeKind::Code
        };
        let main = files
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(file, _)| name_of(file))
            .unwrap_or_default();
        let name = match (files.len(), lens) {
            (0 | 1, _) => main,
            // In the imports lens, files sharing a block import each other
            // in a loop.
            (n, Lens::Imports) => format!("cycle: {main} +{}", n - 1),
            (n, _) => format!("{main} +{}", n - 1),
        };
        Node {
            name,
            path,
            items,
            area: Rect::NOTHING,
            kind,
        }
    };

    let mut districts: Vec<District> = Vec::new();
    for (d, members) in districts_of.values().enumerate() {
        let nodes: Vec<Node> = members
            .iter()
            .enumerate()
            .map(|(j, b)| block_node(&blocks[*b], place(&[d.to_string(), j.to_string()])))
            .collect();
        // What may name it: the functions of its blocks for calls, their
        // files, code or not, otherwise.
        let candidates: Vec<PathBuf> = members
            .iter()
            .flat_map(|b| &blocks[*b])
            .flat_map(|u| match lens {
                Lens::Calls => units[*u]
                    .items
                    .iter()
                    .map(|item| item.path.clone())
                    .collect(),
                _ => vec![units[*u].file.clone()],
            })
            .collect();
        districts.push(District {
            name: hub_name(lens, &candidates, &score),
            path: place(&[d.to_string()]),
            sub_districts: Vec::new(),
            nodes,
            area: Rect::NOTHING,
            kind: DistrictKind::Cluster,
            elevation: 0.0,
        });
    }

    // Tests hang under the district whose code they call most.
    let district_of: HashMap<&Path, usize> = districts
        .iter()
        .enumerate()
        .flat_map(|(d, district)| {
            district
                .nodes
                .iter()
                .flat_map(move |n| n.items.iter().map(move |i| (i.path.as_path(), d)))
        })
        .collect();
    let mut unplaced: Vec<Node> = Vec::new();
    let mut hung: Vec<(usize, Node)> = Vec::new();
    for (j, (file, items)) in tests.into_iter().enumerate() {
        let mut votes: BTreeMap<usize, usize> = BTreeMap::new();
        for target in items.iter().flat_map(|i| &i.floors).flat_map(|f| &f.calls) {
            if let Some(d) = district_of.get(target.as_path()) {
                *votes.entry(*d).or_default() += 1;
            }
        }
        let node = Node {
            name: name_of(&file),
            path: place(&["tests".to_owned(), j.to_string()]),
            items,
            area: Rect::NOTHING,
            kind: NodeKind::Code,
        };
        match votes
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        {
            Some((d, _)) => hung.push((d, node)),
            None => unplaced.push(node),
        }
    }
    for (d, node) in hung {
        districts[d].nodes.push(node);
    }
    if !unplaced.is_empty() {
        districts.push(District {
            name: "tests".to_owned(),
            path: place(&["tests".to_owned()]),
            sub_districts: Vec::new(),
            nodes: unplaced,
            area: Rect::NOTHING,
            kind: DistrictKind::Tests,
            elevation: 0.0,
        });
    }

    // The outskirts: units the lens connects to nothing, by file, and blocks
    // linked to no other block.
    let mut far: Vec<Node> = lone
        .iter()
        .enumerate()
        .map(|(j, b)| {
            block_node(
                &blocks[*b],
                place(&["outskirts".to_owned(), format!("linked-{j}")]),
            )
        })
        .collect();
    far.extend(
        outskirts
            .into_iter()
            .enumerate()
            .map(|(j, (file, (kind, items)))| Node {
                name: name_of(&file),
                path: place(&["outskirts".to_owned(), j.to_string()]),
                items,
                area: Rect::NOTHING,
                kind,
            }),
    );
    if !far.is_empty() {
        districts.push(District {
            name: "outskirts".to_owned(),
            path: place(&["outskirts".to_owned()]),
            sub_districts: Vec::new(),
            nodes: far,
            area: Rect::NOTHING,
            kind: DistrictKind::Outskirts,
            elevation: 0.0,
        });
    }
    districts
}

/// Units joined by the relationship, by modularity.
pub(crate) fn clustered(units: &[Unit], weights: &Weights) -> Vec<Option<usize>> {
    communities(units.len(), weights)
}

/// The units of `folders` for `lens`, and the test items of each file.
fn units(folders: &ProjectMap, lens: Lens) -> (Vec<Unit>, Vec<(PathBuf, Vec<Item>)>) {
    let mut units = Vec::new();
    let mut tests = Vec::new();
    for district in &folders.districts {
        for node in graph::nodes(district) {
            let (test, code): (Vec<Item>, Vec<Item>) =
                node.items.iter().cloned().partition(|item| item.tests);
            if !test.is_empty() {
                tests.push((node.path.clone(), test));
            }
            match lens {
                Lens::Calls => units.extend(code.into_iter().map(|item| Unit {
                    file: node.path.clone(),
                    items: vec![item],
                    kind: node.kind,
                })),
                // A file of only tests has nothing left to place.
                _ if code.is_empty() && !node.items.is_empty() => {}
                _ => units.push(Unit {
                    file: node.path.clone(),
                    items: code,
                    kind: node.kind,
                }),
            }
        }
    }
    (units, tests)
}

/// The weights between units, and how strongly each file or function holds
/// its neighbours together: how often it is imported or called, or how much
/// it changes with others.
pub(crate) fn links(
    folders: &ProjectMap,
    lens: Lens,
    units: &[Unit],
) -> (Weights, HashMap<PathBuf, f32>) {
    let mut weights = Weights::new();
    let mut score: HashMap<PathBuf, f32> = HashMap::new();
    let mut unit_of: HashMap<&Path, usize> = HashMap::new();
    for (u, unit) in units.iter().enumerate() {
        unit_of.entry(unit.file.as_path()).or_insert(u);
        for item in &unit.items {
            unit_of.insert(&item.path, u);
        }
    }
    match lens {
        Lens::Calls => {
            for (u, unit) in units.iter().enumerate() {
                for target in unit
                    .items
                    .iter()
                    .flat_map(|i| &i.floors)
                    .flat_map(|f| &f.calls)
                {
                    if let Some(&v) = unit_of.get(target.as_path()) {
                        link(&mut weights, u, v, 1.0);
                        *score.entry(target.clone()).or_default() += 1.0;
                    }
                }
            }
        }
        Lens::Imports => {
            for (u, unit) in units.iter().enumerate() {
                let uses = unit.items.iter().filter(|i| i.kind == ItemKind::Uses);
                for target in uses.flat_map(|i| &i.floors).flat_map(|f| &f.imports) {
                    if let Some(&v) = unit_of.get(target.as_path()) {
                        link(&mut weights, u, v, 1.0);
                        *score.entry(units[v].file.clone()).or_default() += 1.0;
                    }
                }
            }
        }
        Lens::CoChange => {
            for pair in &folders.co_changes {
                let (a, b) = (&pair.files.0, &pair.files.1);
                if let (Some(&u), Some(&v)) = (unit_of.get(a.as_path()), unit_of.get(b.as_path())) {
                    link(&mut weights, u, v, pair.weight);
                    *score.entry(a.clone()).or_default() += pair.weight;
                    *score.entry(b.clone()).or_default() += pair.weight;
                }
            }
        }
        Lens::Folders | Lens::Cells | Lens::Custom => {}
    }
    (weights, score)
}

/// A district's name: what holds it together most.
/// A district's name: of `candidates`, what holds it together most by
/// `score`.
fn hub_name(lens: Lens, candidates: &[PathBuf], score: &HashMap<PathBuf, f32>) -> String {
    let hub = candidates
        .iter()
        .max_by(|a, b| {
            let (x, y) = (score.get(*a).unwrap_or(&0.0), score.get(*b).unwrap_or(&0.0));
            x.total_cmp(y).then(b.cmp(a))
        })
        .map(|path| name_of(path))
        .unwrap_or_default();
    match lens {
        // Its qualified name, `Pool::new`, so two `new`s are told apart.
        Lens::Calls => format!("around {hub}()"),
        Lens::CoChange => format!("changes with {hub}"),
        _ => format!("around {hub}"),
    }
}

/// The last component of `path`.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}
