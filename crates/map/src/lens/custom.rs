//! The custom lens: the folder city with every district and block moved
//! where the engineer put them (ADR-0006). A district grows to keep a block
//! moved out of it.

use egui::{Rect, Vec2, vec2};

use crate::graph::{self, ProjectMap};
use crate::layout;

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
        let centre = district.area.center();
        let before = district.area.width() / 2.0;
        let mut radius = before;
        for node in graph::nodes_mut(std::slice::from_mut(district)) {
            if let Some(offset) = by(&node.path) {
                node.area = node.area.translate(offset);
                for item in &mut node.items {
                    item.area = item.area.translate(offset);
                }
            }
            for corner in [
                node.area.left_top(),
                node.area.right_top(),
                node.area.left_bottom(),
                node.area.right_bottom(),
            ] {
                radius = radius.max((corner - centre).length() + 10.0);
            }
        }
        if radius > before {
            district.area = Rect::from_center_size(centre, Vec2::splat(2.0 * radius));
        }
    }
    map.extent = map.ground();
    map
}
