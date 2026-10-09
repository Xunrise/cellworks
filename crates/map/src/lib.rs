//! The Map bounded context: the source-agnostic graph of districts, groups,
//! nodes, items and lanes, its layout and its rendering.
//!
//! Defined by docs/domain/contexts/Map/Map.md. A loader hands over a project's
//! included files; [`ProjectMap::of`] cuts them into districts with a
//! [`DistrictStrategy`] and lays them out, and [`ui::project`] draws the
//! result. Items and lanes arrive with the stories that need them.

mod districts;
mod graph;
mod layout;
pub mod ui;

use std::path::{Path, PathBuf};

pub use districts::{
    Directories, DistrictStrategy, PREFIX_RULES, PrefixRule, default_prefix_rule, prefix_rule,
};
pub use graph::{District, Group, Node, ProjectMap, SubDistrict};

impl ProjectMap {
    /// The map of a project: `strategy` cuts `files`, relative to `root`, into
    /// districts, and the layout places them.
    pub fn of(
        project: &str,
        root: &Path,
        files: &[PathBuf],
        strategy: &dyn DistrictStrategy,
    ) -> ProjectMap {
        let mut districts = strategy.cut(project, root, files);
        let extent = layout::lay_out(&mut districts);

        ProjectMap {
            project: project.to_owned(),
            root: root.to_path_buf(),
            districts,
            extent,
        }
    }
}
