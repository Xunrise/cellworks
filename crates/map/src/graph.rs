//! The graph of one opened project: its districts, sub-districts, groups and
//! nodes, and the rectangle each one occupies.
//!
//! Every element is identified by the canonical path of the folder or file it
//! stands for, never by its name: two districts may carry the same label, and
//! nothing keys on a label.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use egui::{Pos2, Rect};

use crate::Lens;
use crate::contexts::BoundedContexts;

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
    /// The rectangle every district and every tower lies inside, at the
    /// layout's own scale.
    pub extent: Rect,
    /// Pairs of files changed in the same commits; empty where git has no
    /// history to tell.
    pub co_changes: Vec<CoChange>,
    /// The lens the districts were made by.
    pub lens: Lens,
    /// For every item, the index of the district it stands in under the
    /// folders lens: its crate, whose colour it keeps in every lens. Empty
    /// for a map made by the folders or the custom lens, where that is its
    /// own district.
    pub homes: BTreeMap<PathBuf, usize>,
    /// The district a lens put in the middle of the others, if it did.
    pub hub: Option<usize>,
    /// The districts of a staircase, the foundation first: each step stands
    /// edge to edge beside the one before.
    pub stair: Vec<usize>,
    /// The blocks holding the files of an import cycle.
    pub cycles: BTreeSet<PathBuf>,
    /// The bounded contexts the project documents, once read; the cells
    /// lens needs them.
    pub contexts: Option<BoundedContexts>,
    /// The cells lens's roads between districts: upstream, downstream, and
    /// the relationship's type.
    pub roads: Vec<(usize, usize, String)>,
    /// Where the engineer moved districts and blocks in the custom lens.
    pub arrangement: cellworks_workspace::Arrangement,
}

/// Two files changed in the same commits, and how strongly: each commit adds
/// one over the number of files it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct CoChange {
    /// The canonical paths of the two files, the smaller first.
    pub files: (PathBuf, PathBuf),
    pub weight: f32,
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
    pub kind: DistrictKind,
    /// How high its ground is raised; everything on it stands that high.
    pub elevation: f32,
}

/// What made a district.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DistrictKind {
    /// A folder the district rule names.
    #[default]
    Folder,
    /// A cluster a relationship lens found.
    Cluster,
    /// The tests that test no code a lens placed.
    Tests,
    /// The code a lens connects to nothing.
    Outskirts,
    /// A bounded context of the domain model.
    Context,
    /// The code of no bounded context.
    NoContext,
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
    /// The functions, types and `use` blocks the file holds, in source order.
    /// Empty for a file no loader reads the code of.
    pub items: Vec<Item>,
    pub area: Rect,
}

/// A function, a type or a block of `use` statements inside a node.
///
/// Its path is the node's path joined with its qualified name
/// (`pricing.rs/Cart::total`), so it is unique within the project.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub name: String,
    pub path: PathBuf,
    pub kind: ItemKind,
    /// Whether it belongs to the tests: a test, or anything inside a test
    /// module. Tests live in the cellar.
    pub tests: bool,
    /// One per line, in source order.
    pub floors: Vec<Floor>,
    /// The ground the item stands on, at the layout's own scale.
    pub area: Rect,
    /// The height of that ground: 0, or how high its district is raised.
    pub base: f32,
}

/// What an item is in its source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Function,
    Test,
    Type,
    Uses,
}

/// One line of an item.
#[derive(Clone, Debug, PartialEq)]
pub struct Floor {
    /// The line number in the file, from 1.
    pub number: usize,
    pub text: String,
    /// Columns of leading whitespace, a tab counting as four.
    pub indent: usize,
    /// Columns up to the end of the last character that is not whitespace.
    pub width: usize,
    /// Where each token on the line starts and how many columns it spans.
    pub tokens: Vec<Token>,
    /// The paths of the items this line calls.
    pub calls: Vec<PathBuf>,
    /// The paths of the items this line imports: the names a `use` line
    /// brings in, resolved to the item each stands for.
    pub imports: Vec<PathBuf>,
}

/// The columns one token occupies on its line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub start: usize,
    pub len: usize,
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

impl ProjectMap {
    /// The rectangle of every element of the map, however deep.
    pub(crate) fn rects_mut(&mut self) -> Vec<&mut Rect> {
        fn nodes<'a>(nodes: &'a mut [Node], into: &mut Vec<&'a mut Rect>) {
            for node in nodes {
                into.push(&mut node.area);
                into.extend(node.items.iter_mut().map(|item| &mut item.area));
            }
        }
        fn groups<'a>(inside: &'a mut [Group], into: &mut Vec<&'a mut Rect>) {
            for group in inside {
                into.push(&mut group.area);
                nodes(&mut group.nodes, into);
                groups(&mut group.groups, into);
            }
        }
        let mut rects = Vec::new();
        for district in &mut self.districts {
            rects.push(&mut district.area);
            nodes(&mut district.nodes, &mut rects);
            for sub in &mut district.sub_districts {
                rects.push(&mut sub.area);
                nodes(&mut sub.nodes, &mut rects);
                groups(&mut sub.groups, &mut rects);
            }
        }
        rects
    }
}

/// Every item of `district`, however deep.
pub(crate) fn items(district: &District) -> Vec<&Item> {
    fn in_groups<'a>(groups: &'a [Group], into: &mut Vec<&'a Item>) {
        for group in groups {
            into.extend(group.nodes.iter().flat_map(|node| &node.items));
            in_groups(&group.groups, into);
        }
    }
    let mut all: Vec<&Item> = district.nodes.iter().flat_map(|node| &node.items).collect();
    for sub in &district.sub_districts {
        all.extend(sub.nodes.iter().flat_map(|node| &node.items));
        in_groups(&sub.groups, &mut all);
    }
    all
}

/// Every node of `district`, however deep.
pub(crate) fn nodes(district: &District) -> Vec<&Node> {
    fn in_groups<'a>(groups: &'a [Group], into: &mut Vec<&'a Node>) {
        for group in groups {
            into.extend(&group.nodes);
            in_groups(&group.groups, into);
        }
    }
    let mut all: Vec<&Node> = district.nodes.iter().collect();
    for sub in &district.sub_districts {
        all.extend(&sub.nodes);
        in_groups(&sub.groups, &mut all);
    }
    all
}

/// Every node of `districts`, however deep.
pub(crate) fn nodes_mut(districts: &mut [District]) -> Vec<&mut Node> {
    fn in_groups<'a>(groups: &'a mut [Group], into: &mut Vec<&'a mut Node>) {
        for group in groups {
            into.extend(group.nodes.iter_mut());
            in_groups(&mut group.groups, into);
        }
    }
    let mut all = Vec::new();
    for district in districts {
        all.extend(district.nodes.iter_mut());
        for sub in &mut district.sub_districts {
            all.extend(sub.nodes.iter_mut());
            in_groups(&mut sub.groups, &mut all);
        }
    }
    all
}

impl ProjectMap {
    /// The items the element at `path` holds: an item itself, or every item
    /// on a block, or in a folder or district, however deep. Empty when the
    /// map holds no such element.
    pub fn items_under(&self, path: &Path) -> Vec<&Item> {
        if let Some(item) = self.item(path) {
            return vec![item];
        }
        fn in_groups<'a>(groups: &'a [Group], path: &Path, all: bool, into: &mut Vec<&'a Item>) {
            for group in groups {
                let all = all || group.path == path;
                in_nodes(&group.nodes, path, all, into);
                in_groups(&group.groups, path, all, into);
            }
        }
        fn in_nodes<'a>(nodes: &'a [Node], path: &Path, all: bool, into: &mut Vec<&'a Item>) {
            for node in nodes.iter().filter(|node| all || node.path == path) {
                into.extend(&node.items);
            }
        }
        let mut under = Vec::new();
        for district in &self.districts {
            let all = district.path == path;
            in_nodes(&district.nodes, path, all, &mut under);
            for sub in &district.sub_districts {
                let all = all || sub.path == path;
                in_nodes(&sub.nodes, path, all, &mut under);
                in_groups(&sub.groups, path, all, &mut under);
            }
        }
        under
    }

    /// The item standing nearest the ground point `point`; one in the cellar
    /// only with `cellar`.
    pub fn item_nearest(&self, point: Pos2, cellar: bool) -> Option<&Item> {
        self.districts
            .iter()
            .flat_map(items)
            .filter(|item| cellar || !item.tests)
            .min_by(|a, b| {
                let distance = |item: &Item| (item.area.center() - point).length();
                distance(a).total_cmp(&distance(b))
            })
    }

    /// The item at `path`, if the map holds one.
    pub fn item(&self, path: &Path) -> Option<&Item> {
        let node = self.find(path)?.parent?;
        self.items_of(node)?.iter().find(|item| item.path == path)
    }

    /// The items standing on the node at `node`, in source order; `None` when
    /// the map holds no such node.
    pub fn items_of(&self, node: &Path) -> Option<&[Item]> {
        fn in_groups<'a>(groups: &'a [Group], node: &Path) -> Option<&'a Node> {
            groups.iter().find_map(|group| {
                group
                    .nodes
                    .iter()
                    .find(|n| n.path == node)
                    .or_else(|| in_groups(&group.groups, node))
            })
        }
        self.districts
            .iter()
            .find_map(|district| {
                district.nodes.iter().find(|n| n.path == node).or_else(|| {
                    district.sub_districts.iter().find_map(|sub| {
                        sub.nodes
                            .iter()
                            .find(|n| n.path == node)
                            .or_else(|| in_groups(&sub.groups, node))
                    })
                })
            })
            .map(|node| node.items.as_slice())
    }
}

/// Where one element of the map is, as [`ProjectMap::find`] reports it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found<'a> {
    /// Its display name.
    pub name: &'a str,
    /// The rectangle it occupies, at the layout's own scale.
    pub area: Rect,
    /// Whether it is a folder, a node or an item.
    pub kind: Element,
    /// The folder or node that holds it; `None` for a district, which the
    /// project holds.
    pub parent: Option<&'a Path>,
}

/// The kinds of element a map holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Folder,
    Node,
    Item,
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
    /// The paths of the elements under `point`, the district first and the
    /// innermost last: the district, and when `point` is on a block, the
    /// folders down to that block's node and the node; off every block, the
    /// folders whose rectangle holds it. Empty when `point` lies between
    /// districts.
    pub fn at(&self, point: Pos2) -> Vec<&Path> {
        /// The folders down to the node whose block holds `point`, and that
        /// node, appended to `chain`; `false`, with `chain` as it was, when no
        /// block below holds it.
        fn down<'a>(
            groups: &'a [Group],
            nodes: &'a [Node],
            point: Pos2,
            chain: &mut Vec<&'a Path>,
        ) -> bool {
            if let Some(node) = nodes.iter().find(|n| n.area.contains(point)) {
                chain.push(&node.path);
                return true;
            }
            groups.iter().any(|group| {
                chain.push(&group.path);
                let found = down(&group.groups, &group.nodes, point, chain);
                if !found {
                    chain.pop();
                }
                found
            })
        }
        // A district is round: its square overlaps its neighbours' at the
        // corners.
        let within = |d: &&District| (d.area.center() - point).length() <= d.area.width() / 2.0;
        let Some(district) = self.districts.iter().find(within) else {
            return Vec::new();
        };
        let mut chain = vec![district.path.as_path()];
        if down(&[], &district.nodes, point, &mut chain) {
            return chain;
        }
        for sub in &district.sub_districts {
            chain.push(&sub.path);
            if down(&sub.groups, &sub.nodes, point, &mut chain) {
                return chain;
            }
            chain.pop();
        }
        // Off every block: the folders whose rectangle holds `point`, the
        // first one at each level where rectangles overlap.
        let mut groups: &[Group] = &[];
        if let Some(sub) = district
            .sub_districts
            .iter()
            .find(|s| s.area.contains(point))
        {
            chain.push(&sub.path);
            groups = &sub.groups;
        }
        while let Some(group) = groups.iter().find(|g| g.area.contains(point)) {
            chain.push(&group.path);
            groups = &group.groups;
        }
        chain
    }
}

impl ProjectMap {
    /// What the folder at `folder` holds, folders first, as names and paths;
    /// the districts when `folder` is `None`. `None` when the map holds no
    /// such folder.
    pub fn held_by(&self, folder: Option<&Path>) -> Option<Vec<(&str, &Path)>> {
        fn entries<'a>(
            folders: impl Iterator<Item = (&'a String, &'a PathBuf)>,
            nodes: &'a [Node],
        ) -> Vec<(&'a str, &'a Path)> {
            folders
                .chain(nodes.iter().map(|node| (&node.name, &node.path)))
                .map(|(name, path)| (name.as_str(), path.as_path()))
                .collect()
        }
        fn in_groups<'a>(groups: &'a [Group], folder: &Path) -> Option<Vec<(&'a str, &'a Path)>> {
            groups.iter().find_map(|group| {
                if group.path == folder {
                    let inner = group.groups.iter().map(|g| (&g.name, &g.path));
                    return Some(entries(inner, &group.nodes));
                }
                in_groups(&group.groups, folder)
            })
        }
        let Some(folder) = folder else {
            let districts = self.districts.iter().map(|d| (&d.name, &d.path));
            return Some(entries(districts, &[]));
        };
        self.districts.iter().find_map(|district| {
            if district.path == folder {
                let subs = district.sub_districts.iter().map(|s| (&s.name, &s.path));
                return Some(entries(subs, &district.nodes));
            }
            district.sub_districts.iter().find_map(|sub| {
                if sub.path == folder {
                    let groups = sub.groups.iter().map(|g| (&g.name, &g.path));
                    return Some(entries(groups, &sub.nodes));
                }
                in_groups(&sub.groups, folder)
            })
        })
    }
}

fn folder<'a>(name: &'a str, area: Rect, parent: Option<&'a Path>) -> Found<'a> {
    Found {
        name,
        area,
        kind: Element::Folder,
        parent,
    }
}

fn in_nodes<'a>(nodes: &'a [Node], path: &Path, parent: Option<&'a Path>) -> Option<Found<'a>> {
    // An item's path is its file's path and more; a block a lens made holds
    // items of other paths.
    let node = nodes.iter().find(|node| {
        node.path == path
            || (path.starts_with(&node.path) && node.items.iter().any(|i| i.path == path))
    });
    let node = match node {
        Some(node) => node,
        None => nodes
            .iter()
            .find(|node| node.items.iter().any(|i| i.path == path))?,
    };
    if node.path == path {
        return Some(Found {
            name: &node.name,
            area: node.area,
            kind: Element::Node,
            parent,
        });
    }
    let item = node.items.iter().find(|item| item.path == path)?;
    Some(Found {
        name: &item.name,
        area: item.area,
        kind: Element::Item,
        parent: Some(&node.path),
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

        let found = |path: &Path| map.find(path).map(|f| (f.kind == Element::Node, f.parent));
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
    fn held_by_lists_what_each_folder_holds_folders_first() {
        let files: Vec<PathBuf> = [
            "src/catalog/pricing/rules/discount.rs",
            "src/catalog/pricing/base.rs",
            "src/catalog/a.rs",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of("shop", Path::new("/p"), &files, &strategy);
        let names = |folder: Option<&str>| {
            map.held_by(folder.map(Path::new))
                .map(|held| held.into_iter().map(|(name, _)| name).collect::<Vec<_>>())
        };

        assert_eq!(names(None), Some(vec!["catalog"]));
        assert_eq!(names(Some("/p/src/catalog")), Some(vec!["pricing", "a.rs"]));
        assert_eq!(
            names(Some("/p/src/catalog/pricing")),
            Some(vec!["rules", "base.rs"])
        );
        assert_eq!(
            names(Some("/p/src/catalog/pricing/rules")),
            Some(vec!["discount.rs"])
        );
        assert_eq!(names(Some("/p/src/catalog/a.rs")), None);
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
