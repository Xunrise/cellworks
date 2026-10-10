//! The Navigation bounded context: layers, selection, movement modes, folder
//! views, the item menu, the palette, the tree, the history and keyboard focus.
//!
//! Defined by docs/domain/contexts/Navigation/Navigation.md. So far it holds
//! the layer the view is on, what is framed and the selection, moves one
//! layer in on Enter and one layer out on Esc, and jumps to an element. It draws nothing: the binary
//! reads the keys, hands the opened project's map in, and acts on [`Moved`].

use std::path::{Path, PathBuf};

use cellworks_map::{Framing, ProjectMap};
use egui::{Pos2, Rect};

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

/// Share of the view, across its width or its height, an element must span
/// for zooming to put the user on it. Below both fills above, so an element
/// framed with Enter counts as filling the view.
const FILLS: f32 = 0.5;

/// Between the parts of the breadcrumb.
const CRUMB: &str = " › ";

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
    /// The part of the map the user zoomed to, filling the view; `None` while
    /// the view frames what is framed.
    camera: Option<Rect>,
    /// Whether zooming leaves the layer and the selection alone.
    locked: bool,
}

impl Default for Navigation {
    fn default() -> Navigation {
        Navigation {
            framed: Framed::Universe,
            project: None,
            selection: None,
            camera: None,
            locked: false,
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
        self.camera = None;
        self.framed = Framed::Project;
        self.project = Some(path);
        self.selection = None;
    }

    /// The open project was read again into `map`: a framed or selected path
    /// it no longer holds is let go, so the project is framed whole and
    /// nothing in it is selected.
    pub fn remapped(&mut self, map: &ProjectMap) {
        if let Framed::Folder(path) | Framed::Node(path) = &self.framed
            && map.find(path).is_none()
        {
            self.framed = Framed::Project;
        }
        if self.framed != Framed::Universe
            && self
                .selection
                .as_ref()
                .is_some_and(|path| map.find(path).is_none())
        {
            self.selection = None;
        }
    }

    /// One layer in: a selected project opens, a selected folder is shown in
    /// its folder view, a selected node is framed. On a framed node nothing
    /// happens yet; the editor comes later.
    pub fn enter(&mut self, map: Option<&ProjectMap>) -> Option<Moved> {
        let selection = self.selection.clone()?;
        self.camera = None;
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

    /// One layer out: from a node to the folder view of the folder holding it,
    /// keeping the node selected; from a folder view to the whole project,
    /// selecting the district the folder lies in; from the project to the
    /// Universe, selecting the project. In the Universe it stays there.
    pub fn esc(&mut self, map: Option<&ProjectMap>) -> Option<Moved> {
        self.camera = None;
        match &self.framed {
            Framed::Universe => None,
            Framed::Project => {
                self.framed = Framed::Universe;
                self.selection = self.project.take();
                Some(Moved::Closed)
            }
            Framed::Node(path) => {
                let parent = map.and_then(|map| map.find(path)).and_then(|f| f.parent);
                self.selection = Some(path.clone());
                self.framed = match parent {
                    Some(parent) => Framed::Folder(parent.to_path_buf()),
                    None => Framed::Project,
                };
                None
            }
            Framed::Folder(path) => {
                self.selection = Some(match map {
                    Some(map) => district_of(map, path),
                    None => path.clone(),
                });
                self.framed = Framed::Project;
                None
            }
        }
    }

    /// Moves straight to the element at `path`: the folder holding it is shown
    /// in its folder view and it is selected. A district is selected on the
    /// whole project. A path the map does not hold changes nothing.
    pub fn jump(&mut self, map: &ProjectMap, path: PathBuf) {
        let Some(found) = map.find(&path) else {
            return;
        };
        self.camera = None;
        self.framed = match found.parent {
            Some(parent) => Framed::Folder(parent.to_path_buf()),
            None => Framed::Project,
        };
        self.selection = Some(path);
    }

    /// The folder whose folder view is shown: the framed folder, or the folder
    /// holding the framed node. `None` on the whole project and the Universe.
    pub fn folder_view<'a>(&'a self, map: &'a ProjectMap) -> Option<&'a Path> {
        match &self.framed {
            Framed::Folder(path) => Some(path),
            Framed::Node(path) => map.find(path).and_then(|found| found.parent),
            Framed::Universe | Framed::Project => None,
        }
    }

    /// Whether zooming leaves the layer and the selection alone.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Switches between the layer following the zoom and the layer locked.
    /// Neither the layer, the selection nor the view changes.
    pub fn toggle_lock(&mut self) {
        self.locked = !self.locked;
    }

    /// The user zoomed around the point `around` so that `view` fills the
    /// view, both at the layout's scale.
    ///
    /// Unless the layer is locked, the layer follows: with the whole project
    /// in view it is Project, with the district of the selection selected;
    /// otherwise the innermost shown element under `around` that spans most of
    /// the view is framed, and selected when it is a node or when the
    /// selection lies outside it. Over empty space nothing changes.
    pub fn zoomed(&mut self, map: &ProjectMap, view: Rect, around: Pos2) {
        if self.framed == Framed::Universe {
            return;
        }
        self.camera = Some(view);
        if self.locked {
            return;
        }
        if view.contains_rect(map.extent) {
            self.framed = Framed::Project;
            self.selection = self.selection.as_deref().map(|path| district_of(map, path));
            return;
        }
        let Some(path) = self.filling(map, view, around) else {
            return;
        };
        let Some(found) = map.find(&path) else {
            return;
        };
        if found.is_node {
            self.selection = Some(path.clone());
            self.framed = Framed::Node(path);
        } else {
            let selection_inside = self
                .selection
                .as_deref()
                .is_some_and(|selected| holds(map, &path, selected));
            if !selection_inside {
                self.selection = Some(path.clone());
            }
            self.framed = Framed::Folder(path);
        }
    }

    /// The innermost element under `around` that spans most of `view`, among
    /// those shown: inside the folder view or around it.
    fn filling(&self, map: &ProjectMap, view: Rect, around: Pos2) -> Option<PathBuf> {
        let folder_view = self.folder_view(map).map(Path::to_path_buf);
        let chain = map.at(around);
        let shown = |path: &Path| match &folder_view {
            None => true,
            Some(folder) => holds(map, folder, path) || holds(map, path, folder),
        };
        chain
            .into_iter()
            .filter(|path| shown(path))
            .rfind(|path| {
                map.find(path).is_some_and(|found| {
                    (found.area.width() / view.width()).max(found.area.height() / view.height())
                        >= FILLS
                })
            })
            .map(Path::to_path_buf)
    }

    /// Where the view is, as the user reads it: "Universe", then the open
    /// project's name and the names in its `map` down to what is framed.
    pub fn breadcrumb(&self, project: &str, map: Option<&ProjectMap>) -> String {
        let mut crumbs = vec!["Universe".to_owned()];
        if self.framed == Framed::Universe {
            return crumbs.join(CRUMB);
        }
        crumbs.push(project.to_owned());
        if let Some(map) = map {
            let mut names = Vec::new();
            let mut at = match &self.framed {
                Framed::Folder(path) | Framed::Node(path) => map.find(path),
                Framed::Universe | Framed::Project => None,
            };
            while let Some(found) = at {
                names.push(found.name.to_owned());
                at = found.parent.and_then(|parent| map.find(parent));
            }
            crumbs.extend(names.into_iter().rev());
        }
        crumbs.join(CRUMB)
    }

    /// The part of `map` the view shows: the whole project, a folder with a
    /// margin, or a node at inspection distance.
    pub fn framing(&self, map: &ProjectMap) -> Framing {
        if let Some(area) = self.camera {
            return Framing {
                area,
                fill: 1.0,
                scale_up: true,
            };
        }
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

/// Whether the folder at `folder` is `path` or holds it, however deep.
fn holds(map: &ProjectMap, folder: &Path, path: &Path) -> bool {
    let mut at = Some(path);
    while let Some(path) = at {
        if path == folder {
            return true;
        }
        at = map.find(path).and_then(|found| found.parent);
    }
    false
}

/// The district `path` lies in: the folder above it that the project holds.
fn district_of(map: &ProjectMap, path: &Path) -> PathBuf {
    let mut district = path;
    while let Some(parent) = map.find(district).and_then(|found| found.parent) {
        district = parent;
    }
    district.to_path_buf()
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
    fn esc_has_one_step_per_layer() {
        let map = shop();
        let mut navigation = in_project();
        navigation.select(at("catalog/pricing/rules"));
        navigation.enter(Some(&map));
        navigation.select(at("catalog/pricing/rules/discount.rs"));
        navigation.enter(Some(&map));

        assert_eq!(navigation.esc(Some(&map)), None);
        assert_eq!(navigation.layer(), Layer::Directory);
        assert_eq!(
            navigation.folder_view(&map),
            Some(at("catalog/pricing/rules").as_path())
        );
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/pricing/rules/discount.rs").as_path())
        );

        assert_eq!(navigation.esc(Some(&map)), None);
        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.folder_view(&map), None);
        assert_eq!(navigation.selection(), Some(at("catalog").as_path()));

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
        assert_eq!(navigation.folder_view(&map), Some(Path::new(ROOT)));

        navigation.esc(Some(&map));
        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.selection(), Some(Path::new(ROOT)));
        assert_eq!(navigation.esc(Some(&map)), Some(Moved::Closed));
    }

    #[test]
    fn a_jump_opens_the_folder_view_holding_the_element() {
        let map = shop();
        let mut navigation = in_project();
        navigation.select(at("catalog/pricing/rules"));
        navigation.enter(Some(&map));

        navigation.jump(&map, at("orders/order.rs"));
        assert_eq!(navigation.layer(), Layer::Directory);
        assert_eq!(navigation.folder_view(&map), Some(at("orders").as_path()));
        assert_eq!(
            navigation.selection(),
            Some(at("orders/order.rs").as_path())
        );

        navigation.jump(&map, at("catalog"));
        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.selection(), Some(at("catalog").as_path()));

        let before = navigation.clone();
        navigation.jump(&map, at("nowhere.rs"));
        assert_eq!(navigation, before);
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

    /// Zooms so that `path`'s rectangle spans `share` of the view across,
    /// around its middle for a node and its name for a folder, where nothing
    /// inside it lies.
    fn zoom_onto(navigation: &mut Navigation, map: &ProjectMap, path: &str, share: f32) {
        let found = map.find(&at(path)).unwrap();
        let area = found.area;
        let view = Rect::from_center_size(area.center(), area.size() / share);
        let around = if found.is_node {
            area.center()
        } else {
            area.min + egui::vec2(4.0, 4.0)
        };
        navigation.zoomed(map, view, around);
    }

    #[test]
    fn zooming_steps_through_the_folders_down_to_the_node_and_back_out() {
        let map = shop();
        let mut navigation = in_project();

        zoom_onto(&mut navigation, &map, "catalog/pricing", 0.8);
        assert_eq!(navigation.layer(), Layer::Directory);
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/pricing").as_path())
        );

        zoom_onto(
            &mut navigation,
            &map,
            "catalog/pricing/rules/discount.rs",
            0.8,
        );
        assert_eq!(navigation.layer(), Layer::File);

        zoom_onto(&mut navigation, &map, "catalog", 0.8);
        assert_eq!(navigation.layer(), Layer::Directory);
        assert_eq!(navigation.folder_view(&map), Some(at("catalog").as_path()));
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/pricing/rules/discount.rs").as_path()),
            "the selection stays when the folder holds it"
        );

        navigation.zoomed(&map, map.extent.expand(10.0), map.extent.center());
        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.selection(), Some(at("catalog").as_path()));
    }

    #[test]
    fn a_locked_layer_only_moves_the_view() {
        let map = shop();
        let mut navigation = in_project();
        navigation.toggle_lock();

        zoom_onto(&mut navigation, &map, "catalog/item.rs", 0.8);

        assert_eq!(navigation.layer(), Layer::Project);
        assert_eq!(navigation.selection(), None);
        assert_eq!(
            navigation.framing(&map).area.center(),
            map.find(&at("catalog/item.rs")).unwrap().area.center()
        );
    }

    #[test]
    fn enter_and_esc_drop_the_zoom() {
        let map = shop();
        let mut navigation = in_project();
        zoom_onto(&mut navigation, &map, "orders", 0.2);
        navigation.select(at("catalog"));

        navigation.enter(Some(&map));

        assert_eq!(
            navigation.framing(&map).area,
            map.find(&at("catalog")).unwrap().area
        );
    }

    #[test]
    fn the_breadcrumb_names_every_folder_down_to_what_is_framed() {
        let map = shop();
        let mut navigation = Navigation::default();
        assert_eq!(navigation.breadcrumb("shop", None), "Universe");

        navigation.opened(PathBuf::from(ROOT));
        assert_eq!(navigation.breadcrumb("shop", None), "Universe › shop");

        navigation.jump(&map, at("README.md"));
        navigation.select(at("README.md"));
        navigation.enter(Some(&map));
        assert_eq!(
            navigation.breadcrumb("shop", Some(&map)),
            "Universe › shop › shop › README.md"
        );
    }
}
