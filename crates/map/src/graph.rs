//! The graph of one opened project: its districts, sub-districts, groups and
//! nodes, and the rectangle each one occupies.
//!
//! Every element is identified by the canonical path of the folder or file it
//! stands for, never by its name: two districts may carry the same label, and
//! nothing keys on a label.

use std::path::PathBuf;

use egui::Rect;

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
}
