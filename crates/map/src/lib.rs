//! The Map bounded context: the source-agnostic graph of districts, groups,
//! nodes, items and lanes, its layout and its rendering.
//!
//! Defined by docs/domain/contexts/Map/Map.md. A loader hands over a project's
//! included files; [`ProjectMap::of`] cuts them into districts with a
//! [`DistrictStrategy`] and lays them out, and [`ui::project`] draws the
//! result: a city whose items stand as towers on their node's block
//! ([`city`]). Lanes arrive with the stories that need them.

pub mod city;
mod contexts;
mod districts;
mod graph;
mod lanes;
mod layout;
mod lens;
mod rules;
pub mod ui;
mod view;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Pos2, Rect, vec2};

pub use contexts::{BoundedContext, BoundedContexts, Road};
pub use districts::{Directories, DistrictStrategy};
pub use graph::{
    CoChange, District, DistrictKind, Element, Floor, Found, Group, Item, ItemKind, Node,
    ProjectMap, SubDistrict, Token,
};
pub use lanes::{LaneId, LaneKind, Lanes, Network};
pub use lens::{Flight, Lens, relens};
pub use rules::{PREFIX_RULES, PrefixRule, default_prefix_rule, prefix_rule};
pub use view::{Pivot, View, turn_at};

impl ProjectMap {
    /// The map of a project: `strategy` cuts `files`, relative to `root`, into
    /// districts, and the layout places them.
    pub fn of(
        project: &str,
        root: &Path,
        files: &[PathBuf],
        strategy: &dyn DistrictStrategy,
    ) -> ProjectMap {
        ProjectMap::of_code(project, root, files, strategy, BTreeMap::new())
    }

    /// The map of a project whose code was read: like [`ProjectMap::of`], with
    /// `items` keyed by the file, relative to `root`, that holds them.
    pub fn of_code(
        project: &str,
        root: &Path,
        files: &[PathBuf],
        strategy: &dyn DistrictStrategy,
        mut items: BTreeMap<PathBuf, Vec<Item>>,
    ) -> ProjectMap {
        let mut districts = strategy.cut(project, root, files);
        for node in graph::nodes_mut(&mut districts) {
            let relative = node.path.strip_prefix(root).unwrap_or(&node.path);
            if let Some(held) = items.remove(relative) {
                node.items = held;
            }
        }
        layout::lay_out(&mut districts);
        let mut map = ProjectMap {
            project: project.to_owned(),
            root: root.to_path_buf(),
            districts,
            extent: Rect::NOTHING,
            co_changes: Vec::new(),
            lens: Lens::Folders,
            homes: BTreeMap::new(),
            hub: None,
            stair: Vec::new(),
            cycles: Default::default(),
            contexts: None,
            roads: Vec::new(),
            arrangement: Default::default(),
        };
        map.extent = map.ground();
        map
    }

    /// The rectangle the districts' ground covers, without the towers rising
    /// above it.
    pub fn ground(&self) -> Rect {
        self.districts
            .iter()
            .fold(Rect::NOTHING, |ground, district| {
                ground.union(district.area)
            })
    }

    /// Shows the map from the next side: the ground turned a quarter
    /// clockwise about its centre, `quarters` times. Every element keeps its
    /// path; towers keep their shape and stand up the screen as before.
    pub fn turn(&mut self, quarters: u8) {
        let centre = self.ground().center();
        for _ in 0..quarters % 4 {
            for rect in self.rects_mut() {
                *rect = turn_rect(*rect, centre);
            }
        }
        self.extent = self.ground();
    }

    /// How high the tallest tower standing on `area` rises.
    pub fn headroom(&self, area: Rect) -> f32 {
        self.districts
            .iter()
            .filter(|district| district.area.intersects(area))
            .flat_map(graph::items)
            .filter(|item| item.area.intersects(area))
            .map(city::height)
            .fold(0.0, f32::max)
    }
}

/// `rect` turned a quarter clockwise on screen about `centre`.
pub fn turn_rect(rect: Rect, centre: Pos2) -> Rect {
    let quarter = |p: Pos2| centre + vec2(centre.y - p.y, p.x - centre.x);
    Rect::from_two_pos(quarter(rect.min), quarter(rect.max))
}

/// Which part of a map a frame shows, and how large.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Framing {
    /// The rectangle to show, at the layout's own scale.
    pub area: Rect,
    /// The share of the view's width or height it fills, whichever is tighter.
    pub fill: f32,
    /// The largest scale it may be drawn at: screen points per layout point.
    pub closest: f32,
    /// How high the towers on `area` rise above it, kept in view too.
    pub headroom: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_quarter_turns_bring_the_map_back() {
        let files: Vec<PathBuf> = ["a/x.rs", "a/y/z.rs", "b/w.rs"]
            .iter()
            .map(PathBuf::from)
            .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of("shop", Path::new("/p"), &files, &strategy);
        let mut turned = map.clone();

        turned.turn(1);
        let block = |map: &ProjectMap| map.find(Path::new("/p/a/x.rs")).map(|f| f.area);
        let (before, after) = (block(&map).unwrap(), block(&turned).unwrap());
        assert!((after.width() - before.height()).abs() < 1e-3);
        turned.turn(3);

        assert!((block(&turned).unwrap().min - before.min).length() < 1e-2);
    }
}
