//! Lenses: what counts as a connection, which decides the blocks and
//! districts of the city and where they stand. Towers keep their shape and
//! the colour of their crate in every lens; folders is the default and the
//! stable home.

mod cells;
mod cluster;
mod custom;
mod flight;
mod relation;
mod shape;
mod stair;

use std::collections::BTreeMap;

pub use flight::Flight;

use crate::graph::{self, ProjectMap};
use crate::layout;

/// A way of seeing the city.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lens {
    /// Blocks are files, districts the folders the district rule names.
    #[default]
    Folders,
    /// Blocks are files; districts are files that import each other.
    Imports,
    /// Blocks are functions that call each other; districts are blocks that
    /// call each other.
    Calls,
    /// Blocks and districts are files changed in the same commits.
    CoChange,
    /// Blocks are files; districts are the bounded contexts of the domain
    /// model, joined by the context map's roads.
    Cells,
    /// The folder city, arranged by the engineer.
    Custom,
}

impl Lens {
    /// Every lens, in the order they are offered.
    pub const ALL: [Lens; 6] = [
        Lens::Folders,
        Lens::Imports,
        Lens::Calls,
        Lens::CoChange,
        Lens::Cells,
        Lens::Custom,
    ];

    /// The lens's name as the engineer reads it.
    pub fn name(self) -> &'static str {
        match self {
            Lens::Folders => "folders",
            Lens::Imports => "imports",
            Lens::Calls => "calls",
            Lens::CoChange => "co-change",
            Lens::Cells => "cells",
            Lens::Custom => "custom",
        }
    }

    /// What the lens arranges by, in one line.
    pub fn blurb(self) -> &'static str {
        match self {
            Lens::Folders => "The city as its folders lay it out.",
            Lens::Imports => {
                "What builds on what: the foundation on the ground, what imports it raised above it."
            }
            Lens::Calls => {
                "Functions that call each other stand together, whatever file they are in."
            }
            Lens::CoChange => "Files that change in the same commits stand together.",
            Lens::Cells => {
                "The bounded contexts of the domain model, joined by the context map's roads."
            }
            Lens::Custom => "Your own arrangement: drag blocks and districts where you want them.",
        }
    }

    /// What a block is in this lens.
    pub fn block(self) -> &'static str {
        match self {
            Lens::Imports => "one file, or the files of an import cycle",
            Lens::Calls => "functions that call each other",
            Lens::CoChange => "files changed together",
            Lens::Folders | Lens::Cells | Lens::Custom => "a file",
        }
    }

    /// What a district is in this lens.
    pub fn district(self) -> &'static str {
        match self {
            Lens::Folders => "a crate or folder",
            Lens::Imports => "files that import each other; each step imports the steps below",
            Lens::Calls => "blocks that call each other",
            Lens::CoChange => "blocks changed together, round the one that changes with everything",
            Lens::Cells => "a bounded context; code of none stands apart",
            Lens::Custom => "a crate or folder, where you put it",
        }
    }

    /// Whether `folders` can be seen through this lens: the cells lens only
    /// for a project that documents bounded contexts.
    pub fn offered(self, folders: &ProjectMap) -> bool {
        self != Lens::Cells
            || folders
                .contexts
                .as_ref()
                .is_some_and(|known| !known.contexts.is_empty())
    }

    /// The lanes the lens shows when it is switched to: the connections it
    /// is built from.
    pub fn lanes(self) -> crate::Lanes {
        match self {
            Lens::Imports | Lens::CoChange => crate::Lanes {
                imports: true,
                calls: false,
            },
            Lens::Calls => crate::Lanes {
                imports: false,
                calls: true,
            },
            Lens::Folders | Lens::Cells | Lens::Custom => crate::Lanes::default(),
        }
    }

    /// The lens after this one.
    pub fn next(self) -> Lens {
        let at = Lens::ALL.iter().position(|lens| *lens == self).unwrap_or(0);
        Lens::ALL[(at + 1) % Lens::ALL.len()]
    }

    /// The lens's name in the paths of the blocks and districts it makes.
    fn key(self) -> &'static str {
        match self {
            Lens::CoChange => "co-change",
            other => other.name(),
        }
    }
}

/// The city of `folders`, the folders lens as read and laid out, seen
/// through `lens`.
pub fn relens(folders: &ProjectMap, lens: Lens) -> ProjectMap {
    let mut map = folders.clone();
    map.lens = lens;
    map.homes = homes(folders);
    if lens == Lens::Folders {
        return map;
    }
    if lens == Lens::Custom {
        let mut map = custom::arranged(folders);
        map.lens = lens;
        map.homes = homes(folders);
        return map;
    }
    if lens == Lens::Cells {
        let Some(known) = &folders.contexts else {
            return map;
        };
        let (districts, roads) = cells::cells(folders, known);
        map.districts = districts;
        map.roads = roads;
        map.extent = map.ground();
        return map;
    }
    let blocks: relation::Blocks = match lens {
        Lens::Imports => stair::cycles,
        _ => relation::clustered,
    };
    let mut districts = relation::rebuild(folders, lens, blocks);
    layout::lay_out(&mut districts);
    let weights = between_districts(folders, lens, &districts);
    match lens {
        Lens::CoChange => map.hub = shape::orbit(&mut districts, &weights),
        Lens::Imports => {
            let core = shape::core(&districts);
            for d in &core {
                stair::rows(&mut districts[*d]);
            }
            let edges = imports_between(&districts);
            map.stair = stair::staircase(&mut districts, &core, &edges);
            map.cycles = districts
                .iter()
                .flat_map(|district| &district.nodes)
                .filter(|node| {
                    let mut files: Vec<&std::path::Path> =
                        node.items.iter().filter_map(|i| i.path.parent()).collect();
                    files.dedup();
                    files.len() > 1
                })
                .map(|node| node.path.clone())
                .collect();
        }
        _ => shape::network(&mut districts, &weights),
    }
    shape::aside(&mut districts);
    map.districts = districts;
    map.extent = map.ground();
    map
}

/// For every item of `folders`, the index of the district it stands in
/// there: its crate, whose colour it keeps in every lens.
fn homes(folders: &ProjectMap) -> BTreeMap<std::path::PathBuf, usize> {
    folders
        .districts
        .iter()
        .enumerate()
        .flat_map(|(d, district)| {
            graph::items(district)
                .into_iter()
                .map(move |item| (item.path.clone(), d))
        })
        .collect()
}

/// How many imports run from each district of `districts` to each other:
/// directed, importer first.
fn imports_between(districts: &[graph::District]) -> BTreeMap<(usize, usize), f32> {
    let district_of: std::collections::HashMap<&std::path::Path, usize> = districts
        .iter()
        .enumerate()
        .flat_map(|(d, district)| {
            district
                .nodes
                .iter()
                .flat_map(move |n| n.items.iter().map(move |i| (i.path.as_path(), d)))
        })
        .collect();
    let mut edges = BTreeMap::new();
    for (d, district) in districts.iter().enumerate() {
        let uses = district
            .nodes
            .iter()
            .flat_map(|n| &n.items)
            .filter(|i| i.kind == crate::ItemKind::Uses);
        for target in uses.flat_map(|i| &i.floors).flat_map(|f| &f.imports) {
            if let Some(&to) = district_of.get(target.as_path())
                && to != d
            {
                *edges.entry((d, to)).or_insert(0.0) += 1.0;
            }
        }
    }
    edges
}

/// How strongly the lens links each pair of `districts`.
fn between_districts(
    folders: &ProjectMap,
    lens: Lens,
    districts: &[graph::District],
) -> cluster::Weights {
    let units: Vec<relation::Unit> = districts
        .iter()
        .enumerate()
        .flat_map(|(d, district)| {
            district.nodes.iter().map(move |node| relation::Unit {
                // The district stands in for the file: links between files of
                // two districts add up between the districts.
                file: std::path::PathBuf::from(d.to_string()),
                items: node.items.clone(),
            })
        })
        .collect();
    let district_of_unit: Vec<usize> = districts
        .iter()
        .enumerate()
        .flat_map(|(d, district)| std::iter::repeat_n(d, district.nodes.len()))
        .collect();
    let file_district: std::collections::HashMap<&std::path::Path, usize> = districts
        .iter()
        .enumerate()
        .flat_map(|(d, district)| {
            district
                .nodes
                .iter()
                .flat_map(|n| &n.items)
                .filter_map(move |i| i.path.parent().map(|f| (f, d)))
        })
        .collect();
    let mut weights = cluster::Weights::new();
    if lens == Lens::CoChange {
        for pair in &folders.co_changes {
            if let (Some(&a), Some(&b)) = (
                file_district.get(pair.files.0.as_path()),
                file_district.get(pair.files.1.as_path()),
            ) {
                cluster::link(&mut weights, a, b, pair.weight);
            }
        }
        return weights;
    }
    let (unit_weights, _) = relation::links(folders, lens, &units);
    for (&(a, b), &w) in &unit_weights {
        cluster::link(&mut weights, district_of_unit[a], district_of_unit[b], w);
    }
    weights
}
