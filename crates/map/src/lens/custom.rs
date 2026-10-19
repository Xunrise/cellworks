//! The custom lens: the folder city with every district and block moved
//! where the engineer put them (ADR-0006). A district grows to keep a block
//! moved out of it.

use egui::vec2;

use crate::graph::{self, ProjectMap};
use crate::layout;

/// How far a district reaches past a block moved to its edge.
const MARGIN: f32 = 10.0;

/// `folders` with its arrangement applied.
pub(crate) fn arranged(folders: &ProjectMap) -> ProjectMap {
    let mut map = folders.clone();
    let by = |path: &std::path::Path| {
        let relative = path.strip_prefix(&folders.root).unwrap_or(path);
        folders
            .arrangement
            .moves
            .get(relative)
            .map(|(x, y)| vec2(*x, *y))
    };
    for district in &mut map.districts {
        if let Some(offset) = by(&district.path) {
            layout::translate(district, offset);
        }
        let mut area = district.area;
        for node in graph::nodes_mut(std::slice::from_mut(district)) {
            if let Some(offset) = by(&node.path) {
                node.area = node.area.translate(offset);
                for item in &mut node.items {
                    item.area = item.area.translate(offset);
                }
                area = area.union(node.area.expand(MARGIN));
            }
        }
        district.area = area;
    }
    map.extent = map.ground();
    map
}
