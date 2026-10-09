//! The Navigation bounded context: layers, selection, movement modes, folder
//! views, the item menu, the palette, the tree, the history and keyboard focus.
//!
//! Defined by docs/domain/contexts/Navigation/Navigation.md. So far it holds
//! the layer the view is on, what is framed and the selection, and moves one
//! layer in on Enter and one layer out on Esc. It draws nothing: the binary
//! reads the keys, hands the opened project's map in, and acts on [`Moved`].

use std::path::{Path, PathBuf};

use cellworks_map::{Framing, ProjectMap};

/// A level of detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Universe,
    Project,
    Directory,
    File,
}

/// A move that reaches past the map: a project to open or to close.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Moved {
    /// Enter on the project at this path.
    Opened(PathBuf),
    /// Esc out of the open project.
    Closed,
}

/// What the view is framed on.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Framed {
    Universe,
    Project,
    Folder(PathBuf),
    Node(PathBuf),
}

/// Share of the view a framed folder fills: the folder with a margin.
const FOLDER_FILL: f32 = 0.7;

/// Share of the view a framed node fills: the inspection distance, close
/// enough to read its name.
const NODE_FILL: f32 = 0.55;

/// The layer the view is on, what is framed and what is selected.
///
/// The layer follows from what is framed. The selection is a path read on the
/// current layer: a project's folder in the Universe, an element of the open
/// project's map inside it. The district of a project's top-level files has
/// the project's own path, so the layer is what tells the two apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    framed: Framed,
    project: Option<PathBuf>,
    selection: Option<PathBuf>,
}

impl Default for Navigation {
    fn default() -> Navigation {
        Navigation {
            framed: Framed::Universe,
            project: None,
            selection: None,
        }
    }
}

impl Navigation {
    /// The layer the view is on.
    pub fn layer(&self) -> Layer {
        match self.framed {
            Framed::Universe => Layer::Universe,
            Framed::Project => Layer::Project,
            Framed::Folder(_) => Layer::Directory,
            Framed::Node(_) => Layer::File,
        }
    }

    /// What the keyboard acts on.
    pub fn selection(&self) -> Option<&Path> {
        self.selection.as_deref()
    }

    /// Puts the selection on `path`.
    pub fn select(&mut self, path: PathBuf) {
        self.selection = Some(path);
    }

    /// The project at `path` was opened: it is framed whole, nothing in it
    /// selected yet.
    pub fn opened(&mut self, path: PathBuf) {
        self.framed = Framed::Project;
        self.project = Some(path);
        self.selection = None;
    }

    /// One layer in: a selected project opens, a selected folder or node is
    /// framed. On a framed node nothing happens yet; the editor comes later.
    pub fn enter(&mut self, map: Option<&ProjectMap>) -> Option<Moved> {
        let selection = self.selection.clone()?;
        match &self.framed {
            Framed::Universe => {
                self.opened(selection.clone());
                Some(Moved::Opened(selection))
            }
            Framed::Node(_) => None,
            Framed::Project | Framed::Folder(_) => {
                let found = map?.find(&selection)?;
                self.framed = if found.is_node {
                    Framed::Node(selection)
                } else {
                    Framed::Folder(selection)
                };
                None
            }
        }
    }

    /// One layer out: from a node or folder to the folder holding it, which
    /// becomes the selection; from a district to the whole project, keeping the
    /// district selected; from the project to the Universe, selecting the
    /// project. In the Universe it stays there.
    pub fn esc(&mut self, map: Option<&ProjectMap>) -> Option<Moved> {
        match &self.framed {
            Framed::Universe => None,
            Framed::Project => {
                self.framed = Framed::Universe;
                self.selection = self.project.take();
                Some(Moved::Closed)
            }
            Framed::Folder(path) | Framed::Node(path) => {
                match map.and_then(|map| map.find(path)).and_then(|f| f.parent) {
                    Some(parent) => {
                        self.selection = Some(parent.to_path_buf());
                        self.framed = Framed::Folder(parent.to_path_buf());
                    }
                    None => {
                        self.selection = Some(path.clone());
                        self.framed = Framed::Project;
                    }
                }
                None
            }
        }
    }

    /// The part of `map` the view shows: the whole project, a folder with a
    /// margin, or a node at inspection distance.
    pub fn framing(&self, map: &ProjectMap) -> Framing {
        let whole = Framing {
            area: map.extent,
            fill: 1.0,
            scale_up: false,
        };
        let (path, fill) = match &self.framed {
            Framed::Folder(path) => (path, FOLDER_FILL),
            Framed::Node(path) => (path, NODE_FILL),
            Framed::Universe | Framed::Project => return whole,
        };
        match map.find(path) {
            Some(found) => Framing {
                area: found.area,
                fill,
                scale_up: true,
            },
            None => whole,
        }
    }
}

#[cfg(test)]
mod tests {
    use cellworks_map::{Directories, default_prefix_rule};

    use super::*;

    const ROOT: &str = "/p/shop";

    /// `shop` with the district `catalog`, its sub-district `pricing`, the
    /// group `rules` and the node `discount.rs`, plus files at the top.
    fn shop() -> ProjectMap {
        let files: Vec<PathBuf> = [
            "catalog/pricing/rules/discount.rs",
            "catalog/pricing/rules/tax.rs",
            "catalog/item.rs",
            "orders/order.rs",
            "README.md",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of("shop", Path::new(ROOT), &files, &strategy)
    }

    fn at(relative: &str) -> PathBuf {
        Path::new(ROOT).join(relative)
    }

    fn in_project() -> Navigation {
        let mut navigation = Navigation::default();
        navigation.opened(PathBuf::from(ROOT));
        navigation
    }

    #[test]
    fn enter_on_a_selected_project_opens_it() {
        let mut navigation = Navigation::default();
        navigation.select(PathBuf::from(ROOT));

        assert_eq!(
            navigation.enter(None),
            Some(Moved::Opened(PathBuf::from(ROOT)))
        );
        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.selection(), None);
    }

    #[test]
    fn enter_without_a_selection_changes_nothing() {
        let mut navigation = Navigation::default();

        assert_eq!(navigation.enter(None), None);
        assert_eq!(navigation, Navigation::default());
    }

    #[test]
    fn enter_frames_folders_on_the_directory_layer_and_nodes_on_the_file_layer() {
        let map = shop();
        let mut navigation = in_project();

        for (path, layer) in [
            ("catalog", Layer::Directory),
            ("catalog/pricing", Layer::Directory),
            ("catalog/pricing/rules", Layer::Directory),
            ("catalog/pricing/rules/discount.rs", Layer::File),
        ] {
            navigation.select(at(path));
            assert_eq!(navigation.enter(Some(&map)), None);
            assert_eq!(navigation.layer(), layer, "{path}");
            assert_eq!(
                navigation.framing(&map).area,
                map.find(&at(path)).unwrap().area
            );
        }
    }

    #[test]
    fn enter_on_a_framed_node_changes_nothing() {
        let map = shop();
        let mut navigation = in_project();
        navigation.select(at("catalog/pricing/rules/discount.rs"));
        navigation.enter(Some(&map));
        let before = navigation.clone();

        assert_eq!(navigation.enter(Some(&map)), None);
        assert_eq!(navigation, before);
    }

    #[test]
    fn esc_walks_out_to_the_universe_one_level_at_a_time() {
        let map = shop();
        let mut navigation = in_project();
        navigation.select(at("catalog/pricing/rules/discount.rs"));
        navigation.enter(Some(&map));

        for (layer, selected) in [
            (Layer::Directory, at("catalog/pricing/rules")),
            (Layer::Directory, at("catalog/pricing")),
            (Layer::Directory, at("catalog")),
            (Layer::Project, at("catalog")),
        ] {
            assert_eq!(navigation.esc(Some(&map)), None);
            assert_eq!(navigation.layer(), layer);
            assert_eq!(navigation.selection(), Some(selected.as_path()));
        }

        assert_eq!(navigation.esc(Some(&map)), Some(Moved::Closed));
        assert_eq!(navigation.layer(), Layer::Universe);
        assert_eq!(navigation.selection(), Some(Path::new(ROOT)));

        assert_eq!(navigation.esc(None), None);
        assert_eq!(navigation.layer(), Layer::Universe);
    }

    #[test]
    fn the_district_of_top_level_files_is_told_apart_from_the_project() {
        let map = shop();
        let mut navigation = in_project();
        navigation.select(at("README.md"));
        navigation.enter(Some(&map));

        navigation.esc(Some(&map));
        assert_eq!(navigation.layer(), Layer::Directory);
        assert_eq!(navigation.selection(), Some(Path::new(ROOT)));

        navigation.esc(Some(&map));
        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.esc(Some(&map)), Some(Moved::Closed));
    }

    #[test]
    fn a_node_is_framed_closer_than_a_folder_and_the_project_is_never_scaled_up() {
        let map = shop();
        let mut navigation = in_project();
        assert!(!navigation.framing(&map).scale_up);

        navigation.select(at("catalog"));
        navigation.enter(Some(&map));
        let folder = navigation.framing(&map);
        navigation.select(at("catalog/item.rs"));
        navigation.enter(Some(&map));
        let node = navigation.framing(&map);

        assert!(folder.scale_up && node.scale_up);
        assert!(node.fill < folder.fill);
    }
}
