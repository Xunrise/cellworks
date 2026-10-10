//! The graph of one opened project: its districts, sub-districts, groups and
//! nodes, and the rectangle each one occupies.
//!
//! Every element is identified by the canonical path of the folder or file it
//! stands for, never by its name: two districts may carry the same label, and
//! nothing keys on a label.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Pos2, Rect};

/// One project laid out as a map. Built only by [`ProjectMap::of`].
///
/// Invariant: no two drawn rectangles overlap; the only rectangle that
/// contains another is a parent containing its own child.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectMap {
    /// The project's display name.
    pub project: String,
    /// The canonical path of the project's folder.
    pub root: PathBuf,
    /// Ordered by name, byte-wise; ties by path.
    pub districts: Vec<District>,
    /// The rectangle every district lies inside, at the layout's own scale.
    pub extent: Rect,
}

/// The top level of a project: a folder at the district level, or the
/// district named after the project that holds the files sitting there.
#[derive(Clone, Debug, PartialEq)]
pub struct District {
    pub name: String,
    pub path: PathBuf,
    pub sub_districts: Vec<SubDistrict>,
    pub nodes: Vec<Node>,
    pub area: Rect,
}

/// A folder directly inside a district.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDistrict {
    pub name: String,
    pub path: PathBuf,
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
    pub area: Rect,
}

/// A folder deeper than a sub-district, nested as the folders nest.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    pub path: PathBuf,
    pub groups: Vec<Group>,
    pub nodes: Vec<Node>,
    pub area: Rect,
}

/// A file.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub area: Rect,
}

impl ProjectMap {
    /// Whether there is nothing to draw: the project has no included file.
    pub fn is_empty(&self) -> bool {
        self.districts.is_empty()
    }

    /// Marks the nodes standing for closed folders: `closed` maps each such
    /// folder, relative to the root, to the number of files it holds, and the
    /// node's name says that number.
    pub fn close(&mut self, closed: &BTreeMap<PathBuf, usize>) {
        if closed.is_empty() {
            return;
        }
        let mark = |nodes: &mut [Node]| {
            for node in nodes {
                let relative = node.path.strip_prefix(&self.root).unwrap_or(&node.path);
                if let Some(count) = closed.get(relative) {
                    let files = if *count == 1 { "file" } else { "files" };
                    node.name = format!("{} ({count} {files})", node.name);
                }
            }
        };
        for district in &mut self.districts {
            mark(&mut district.nodes);
            for sub in &mut district.sub_districts {
                mark(&mut sub.nodes);
                let mut pending: Vec<&mut Group> = sub.groups.iter_mut().collect();
                while let Some(group) = pending.pop() {
                    mark(&mut group.nodes);
                    pending.extend(group.groups.iter_mut());
                }
            }
        }
    }
}

/// Where one element of the map is, as [`ProjectMap::find`] reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found<'a> {
    /// Its display name.
    pub name: &'a str,
    /// The rectangle it occupies, at the layout's own scale.
    pub area: Rect,
    /// Whether it is a node rather than a folder.
    pub is_node: bool,
    /// The folder that holds it; `None` for a district, which the project holds.
    pub parent: Option<&'a Path>,
}

impl ProjectMap {
    /// The element whose path is `path`, if the map holds one.
    pub fn find(&self, path: &Path) -> Option<Found<'_>> {
        self.districts.iter().find_map(|district| {
            if district.path == path {
                return Some(folder(&district.name, district.area, None));
            }
            let parent = Some(district.path.as_path());
            in_nodes(&district.nodes, path, parent).or_else(|| {
                district.sub_districts.iter().find_map(|sub| {
                    if sub.path == path {
                        return Some(folder(&sub.name, sub.area, parent));
                    }
                    let inside = Some(sub.path.as_path());
                    in_nodes(&sub.nodes, path, inside)
                        .or_else(|| in_groups(&sub.groups, path, inside))
                })
            })
        })
    }
}

impl ProjectMap {
    /// The paths of the elements whose rectangle holds `point`, the district
    /// first and the innermost last. Empty when `point` lies between districts.
    pub fn at(&self, point: Pos2) -> Vec<&Path> {
        fn nodes<'a>(nodes: &'a [Node], point: Pos2, chain: &mut Vec<&'a Path>) {
            chain.extend(
                nodes
                    .iter()
                    .filter(|n| n.area.contains(point))
                    .map(|n| n.path.as_path()),
            );
        }
        fn groups<'a>(inside: &'a [Group], point: Pos2, chain: &mut Vec<&'a Path>) {
            for group in inside.iter().filter(|g| g.area.contains(point)) {
                chain.push(&group.path);
                groups(&group.groups, point, chain);
                nodes(&group.nodes, point, chain);
            }
        }
        let mut chain = Vec::new();
        for district in self.districts.iter().filter(|d| d.area.contains(point)) {
            chain.push(district.path.as_path());
            for sub in district
                .sub_districts
                .iter()
                .filter(|s| s.area.contains(point))
            {
                chain.push(&sub.path);
                groups(&sub.groups, point, &mut chain);
                nodes(&sub.nodes, point, &mut chain);
            }
            nodes(&district.nodes, point, &mut chain);
        }
        chain
    }
}

fn folder<'a>(name: &'a str, area: Rect, parent: Option<&'a Path>) -> Found<'a> {
    Found {
        name,
        area,
        is_node: false,
        parent,
    }
}

fn in_nodes<'a>(nodes: &'a [Node], path: &Path, parent: Option<&'a Path>) -> Option<Found<'a>> {
    nodes
        .iter()
        .find(|node| node.path == path)
        .map(|node| Found {
            name: &node.name,
            area: node.area,
            is_node: true,
            parent,
        })
}

fn in_groups<'a>(groups: &'a [Group], path: &Path, parent: Option<&'a Path>) -> Option<Found<'a>> {
    groups.iter().find_map(|group| {
        if group.path == path {
            return Some(folder(&group.name, group.area, parent));
        }
        let inside = Some(group.path.as_path());
        in_nodes(&group.nodes, path, inside).or_else(|| in_groups(&group.groups, path, inside))
    })
}

#[cfg(test)]
mod tests {
    use crate::{Directories, default_prefix_rule};

    use super::*;

    #[test]
    fn find_reports_each_element_with_the_folder_holding_it() {
        let files: Vec<PathBuf> = ["src/catalog/pricing/rules/discount.rs", "src/catalog/a.rs"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of("shop", Path::new("/p"), &files, &strategy);
        let district = Path::new("/p/src/catalog");
        let sub = district.join("pricing");
        let group = sub.join("rules");

        let found = |path: &Path| map.find(path).map(|f| (f.is_node, f.parent));
        assert_eq!(found(district), Some((false, None)));
        assert_eq!(found(&sub), Some((false, Some(district))));
        assert_eq!(found(&group), Some((false, Some(sub.as_path()))));
        assert_eq!(
            found(&group.join("discount.rs")),
            Some((true, Some(group.as_path())))
        );
        assert_eq!(found(&district.join("a.rs")), Some((true, Some(district))));
        assert_eq!(found(Path::new("/p/src")), None);
        assert_eq!(map.find(&sub).map(|f| f.name), Some("pricing"));
    }

    #[test]
    fn at_reports_the_elements_under_a_point_outermost_first() {
        let files: Vec<PathBuf> = ["src/catalog/pricing/rules/discount.rs", "src/orders/a.rs"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of("shop", Path::new("/p"), &files, &strategy);
        let node = Path::new("/p/src/catalog/pricing/rules/discount.rs");

        let chain = map.at(map.find(node).unwrap().area.center());

        assert_eq!(
            chain,
            [
                Path::new("/p/src/catalog"),
                Path::new("/p/src/catalog/pricing"),
                Path::new("/p/src/catalog/pricing/rules"),
                node,
            ]
        );
        assert!(map.at(map.extent.max + egui::vec2(1.0, 1.0)).is_empty());
    }
}
