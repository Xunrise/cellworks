//! The Navigation bounded context: layers, selection, movement modes, folder
//! views, the item menu, the palette, the tree, the history and keyboard focus.
//!
//! Defined by docs/domain/contexts/Navigation/Navigation.md. So far it holds
//! the layer the view is on, what is framed and the selection, moves one
//! layer in on Enter and one layer out on Esc, jumps to an element, follows
//! the zoom and lists the current layer for the tree, which [`ui::tree`]
//! draws. It draws nothing: the binary
//! reads the keys, hands the opened project's map in, and acts on [`Moved`].

pub mod explorer;
pub mod ui;

use std::path::{Path, PathBuf};

use cellworks_map::city::Toward;
use cellworks_map::{Element, Framing, Item, LaneId, Pivot, ProjectMap};
use egui::{Pos2, Rect, Vec2};

/// A level of detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Universe,
    Project,
    Directory,
    File,
    /// Inside a tower, floor by floor.
    Item,
}

/// A move that reaches past the map: a project to open or to close.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Moved {
    /// Enter on the project at this path.
    Opened(PathBuf),
    /// Esc out of the open project.
    Closed,
    /// Esc, the back control or the button left the open review.
    LeftReview,
}

/// Reviewing a pull request: the engineer picked a row from its list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewMode {
    pub number: u32,
    pub picked: Option<PathBuf>,
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

/// The closest Enter frames a folder or a node with towers on it: close
/// enough to read a tower token by token, not so close that its floors drift
/// apart.
const FRAMED_CLOSEST: f32 = 2.4;

/// Between the parts of the breadcrumb.
const CRUMB: &str = " › ";

/// The layer the view is on, what is framed and what is selected.
///
/// The layer follows from what is framed. The selection is a path read on the
/// current layer: a project's folder in the Universe, an element of the open
/// project's map inside it. The district of a project's top-level files has
/// the project's own path, so the layer is what tells the two apart.
#[derive(Clone, Debug, PartialEq)]
pub struct Navigation {
    framed: Framed,
    project: Option<PathBuf>,
    selection: Option<PathBuf>,
    /// The part of the map the user zoomed to, filling the view; `None` while
    /// the view frames what is framed.
    camera: Option<Rect>,
    /// Whether zooming leaves the layer and the selection alone. Locked from
    /// the start: zooming only moves the camera until asked to do more.
    locked: bool,
    /// The tower stepped inside, and the floor selected in it, from 0. The
    /// city stays framed as it was beneath it.
    inside: Option<(PathBuf, usize)>,
    /// The facade the camera turns to while it is turned, and the point it
    /// pivots on, gliding towards that facade.
    facing: Option<Facing>,
    /// Whether the view is turned underground, the cellar within reach.
    underground: bool,
    /// Whether a cable runs from each mast to the building it calls.
    cables: bool,
    /// The lane picked, kept lit until another click.
    lane: Option<LaneId>,
    /// The pull request under review, if any; at most one at a time.
    review: Option<ReviewMode>,
}

/// The tower whose facade a turned camera faces, and where its pivot is: a
/// ground point and a height, on their way to the facade's.
#[derive(Clone, Debug, PartialEq)]
struct Facing {
    tower: PathBuf,
    ground: Pos2,
    z: f32,
    /// The height the view is scrolled to on a facade taller than the view,
    /// and how far it can scroll: from `low` to `high`.
    aim: f32,
    low: f32,
    high: f32,
    /// How far the camera has turned.
    turn: f32,
}

/// Where each step from a faced facade leads, by the towers' names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hints {
    pub here: String,
    pub left: Option<String>,
    pub right: Option<String>,
    pub behind: Option<String>,
    pub front: Option<String>,
}

/// How many floors one scroll step moves a tall facade.
const SCROLL_FLOORS: f32 = 5.0;

/// How far the camera must have turned before the arrows walk between
/// facades.
const WALKING_FROM: f32 = 0.5;

/// How quickly the pivot glides to a new facade: the share of the way left
/// it covers in a second is `1 - e^-GLIDE`.
const GLIDE: f32 = 10.0;

impl Default for Navigation {
    fn default() -> Navigation {
        Navigation {
            framed: Framed::Universe,
            project: None,
            selection: None,
            camera: None,
            locked: true,
            inside: None,
            facing: None,
            underground: false,
            cables: false,
            lane: None,
            review: None,
        }
    }
}

impl Navigation {
    /// The layer the view is on.
    pub fn layer(&self) -> Layer {
        if self.inside.is_some() {
            return Layer::Item;
        }
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

    /// Lets go of the selection: empty ground was picked.
    pub fn deselect(&mut self) {
        self.selection = None;
    }

    /// Picks `lane`, which stays lit with what it carries; picking it again
    /// lets it go.
    pub fn pick_lane(&mut self, lane: LaneId) {
        self.lane = if self.lane.as_ref() == Some(&lane) {
            None
        } else {
            Some(lane)
        };
    }

    /// Lets the picked lane go.
    pub fn drop_lane(&mut self) {
        self.lane = None;
    }

    /// The lane picked, if any.
    pub fn picked_lane(&self) -> Option<&LaneId> {
        self.lane.as_ref()
    }

    /// Moves the selection to the next tower on the same block, from the last
    /// back to the first; from a selected block, to its first tower.
    pub fn next_in_block(&mut self, map: &ProjectMap) {
        let Some(selected) = self.selection.as_deref() else {
            return;
        };
        let Some(found) = map.find(selected) else {
            return;
        };
        let (block, current) = match found.kind {
            Element::Item => (found.parent, Some(selected)),
            Element::Node => (Some(selected), None),
            Element::Folder => return,
        };
        let Some(items) = block.and_then(|block| map.items_of(block)) else {
            return;
        };
        let next = match current.and_then(|c| items.iter().position(|item| item.path == c)) {
            Some(index) => (index + 1) % items.len(),
            None => 0,
        };
        if let Some(item) = items.get(next) {
            self.selection = Some(item.path.clone());
        }
    }

    /// The project at `path` was opened: it is framed whole, nothing in it
    /// selected yet.
    pub fn opened(&mut self, path: PathBuf) {
        self.camera = None;
        self.inside = None;
        self.facing = None;
        self.framed = Framed::Project;
        self.project = Some(path);
        self.selection = None;
        self.review = None;
    }

    /// The open project was read again into `map`: a framed or selected path
    /// it no longer holds is let go, so the project is framed whole and
    /// nothing in it is selected.
    pub fn remapped(&mut self, map: &ProjectMap) {
        if let Some((item, floor)) = &mut self.inside {
            match map.find(item).map(|found| found.kind) {
                Some(Element::Item) => {
                    let floors = item_floors(map, item);
                    *floor = (*floor).min(floors.saturating_sub(1));
                }
                _ => self.inside = None,
            }
        }
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
    /// its folder view, a selected node is framed, and a selected tower is
    /// stepped inside, on its first floor. Inside a tower, or on a framed node
    /// with nothing on it selected, nothing happens yet; the editor comes
    /// later.
    pub fn enter(&mut self, map: Option<&ProjectMap>) -> Option<Moved> {
        let selection = self.selection.clone()?;
        if self.inside.is_some() {
            return None;
        }
        if let Some(map) = map
            && map.find(&selection).map(|found| found.kind) == Some(Element::Item)
        {
            self.inside = Some((selection, 0));
            return None;
        }
        self.camera = None;
        match &self.framed {
            Framed::Universe => {
                self.opened(selection.clone());
                Some(Moved::Opened(selection))
            }
            Framed::Node(_) => None,
            Framed::Project | Framed::Folder(_) => {
                let found = map?.find(&selection)?;
                self.framed = if found.kind == Element::Node {
                    Framed::Node(selection)
                } else {
                    Framed::Folder(selection)
                };
                None
            }
        }
    }

    /// Leaves an open review first, before any other step, even from inside a
    /// tower. Otherwise, one layer out: from inside a tower back to the city
    /// as it was, the tower still selected; from a node to the folder view of
    /// the folder holding it, keeping the node selected; from a folder view to
    /// the whole project, selecting the district the folder lies in; from the
    /// project to the Universe, selecting the project. In the Universe it
    /// stays there.
    pub fn esc(&mut self, map: Option<&ProjectMap>) -> Option<Moved> {
        if let Some(moved) = self.leave_review() {
            return Some(moved);
        }
        if self.inside.take().is_some() {
            return None;
        }
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
    /// in its folder view, or for a tower the node it stands on is framed, and
    /// it is selected. A district is selected on the
    /// whole project. A path the map does not hold changes nothing.
    pub fn jump(&mut self, map: &ProjectMap, path: PathBuf) {
        let Some(found) = map.find(&path) else {
            return;
        };
        self.camera = None;
        self.inside = None;
        self.framed = match (found.kind, found.parent) {
            (Element::Item, Some(node)) => Framed::Node(node.to_path_buf()),
            (_, Some(parent)) => Framed::Folder(parent.to_path_buf()),
            (_, None) => Framed::Project,
        };
        self.selection = Some(path);
    }

    /// Frames the element at `path` the way entering it would — a folder at its
    /// own area, a file's node at inspection distance — and selects it,
    /// regardless of what is currently framed: unlike `enter`, this is not
    /// blocked by a node already being framed, so the Explorer's cursor can move
    /// from file to file, or from a file to a folder, without an Esc between.
    /// Nothing while no project is open (`Framed::Universe`), inside a tower, or
    /// when `path` is not on `map`.
    pub fn frame(&mut self, map: &ProjectMap, path: PathBuf) {
        if self.framed == Framed::Universe || self.inside.is_some() {
            return;
        }
        let Some(found) = map.find(&path) else {
            return;
        };
        self.camera = None;
        self.selection = Some(path.clone());
        self.framed = if found.kind == Element::Node {
            Framed::Node(path)
        } else {
            Framed::Folder(path)
        };
    }

    /// Enters review mode for pull request `number`: called once the
    /// review's head has been opened as a project, after [`Navigation::opened`]
    /// so its own clearing of a previous review does not immediately undo
    /// this.
    pub fn enter_review(&mut self, number: u32) {
        self.review = Some(ReviewMode {
            number,
            picked: None,
        });
    }

    /// The review in progress, if any.
    pub fn review(&self) -> Option<&ReviewMode> {
        self.review.as_ref()
    }

    /// Marks `path` picked in the review's changed or impacted tower list and
    /// moves the view to it, keeping the Project layer: the review's city
    /// stays whole-project-framed throughout. Nothing while no review is
    /// open.
    pub fn pick_in_review(&mut self, map: &ProjectMap, path: PathBuf) {
        let Some(review) = &mut self.review else {
            return;
        };
        review.picked = Some(path.clone());
        let Some(found) = map.find(&path) else {
            return;
        };
        let centre = found.area.center();
        self.select(path);
        let visible = self.camera.unwrap_or(map.extent);
        self.centre_on(visible, centre);
    }

    /// Leaves the review, if one is open: `Some(Moved::LeftReview)` when one
    /// was, `None` otherwise, so the caller can tell whether leaving actually
    /// happened.
    pub fn leave_review(&mut self) -> Option<Moved> {
        self.review.take().map(|_| Moved::LeftReview)
    }

    /// The camera has turned `turn` of the way from the corner towards a
    /// facade, `visible` at the layout's scale in view. Decides the facade
    /// it pivots on, keeping the one it turned to while it stays turned: the
    /// selected tower, else the facade kept, else the tower `pointed` at,
    /// else the one `nearest` the middle of the view. A facade taller than
    /// the view, `half` its height at the layout's scale, is seen from its top
    /// down, where its first line is, and scrolls. Glides the pivot `dt`
    /// seconds further towards that facade and keeps the camera centred on
    /// it. Returns whether it is still gliding.
    #[allow(clippy::too_many_arguments)]
    pub fn face(
        &mut self,
        map: &ProjectMap,
        turn: f32,
        pointed: Option<&Path>,
        nearest: Option<&Path>,
        visible: Rect,
        half: f32,
        dt: f32,
    ) -> bool {
        if turn <= 0.0 || self.framed == Framed::Universe {
            self.facing = None;
            return false;
        }
        let is_tower = |path: &&Path| map.item(path).is_some();
        let kept = self.facing.as_ref().map(|facing| facing.tower.as_path());
        let Some(tower) = self
            .selection
            .as_deref()
            .filter(is_tower)
            .or(kept.filter(is_tower))
            .or(pointed.filter(is_tower))
            .or(nearest.filter(is_tower))
            .and_then(|path| map.item(path))
        else {
            self.facing = None;
            return false;
        };
        // The facade's height, from the ground up or, in the cellar, down.
        let reach = cellworks_map::city::reach(tower);
        let (bottom, top) = if cellworks_map::city::in_cellar(tower) {
            (tower.base - reach, tower.base)
        } else {
            (tower.base, tower.base + reach)
        };
        let (low, high) = if top - bottom > 2.0 * half {
            (bottom + half, top - half)
        } else {
            let middle = (bottom + top) / 2.0;
            (middle, middle)
        };
        let aim = match &self.facing {
            Some(facing) if facing.tower == tower.path => facing.aim,
            _ => high,
        }
        .clamp(low, high);
        let target = (tower.area.center(), aim);
        let (ground, z) = self
            .facing
            .as_ref()
            .map_or(target, |facing| (facing.ground, facing.z));
        let k = 1.0 - (-dt * GLIDE).exp();
        let ground = ground + (target.0 - ground) * k;
        let z = z + (target.1 - z) * k;
        let gliding = (target.0 - ground).length() > 0.01 || (target.1 - z).abs() > 0.01;
        self.facing = Some(Facing {
            tower: tower.path.clone(),
            ground,
            z,
            aim,
            low,
            high,
            turn,
        });
        if let Some(camera) = &mut self.camera {
            let off = ground - camera.center();
            *camera = camera.translate(off * k);
        } else {
            self.camera = Some(visible.translate((ground - visible.center()) * k));
        }
        gliding || (ground - visible.center()).length() > 0.01
    }

    /// Whether the camera has turned far enough towards a facade for the
    /// arrows to walk between facades.
    pub fn is_walking(&self) -> bool {
        self.facing
            .as_ref()
            .is_some_and(|facing| facing.turn >= WALKING_FROM)
    }

    /// One step from the faced facade `toward` a neighbour on its block:
    /// left and right face the next tower along the rows; behind and in
    /// front first scroll a facade taller than the view up or down, and once
    /// it is scrolled to its end face the nearest tower on the row behind or
    /// in front. Nothing past the block's ends.
    pub fn walk(&mut self, map: &ProjectMap, toward: Toward) {
        if !self.is_walking() {
            return;
        }
        let Some(facing) = &mut self.facing else {
            return;
        };
        let Some(tower) = map.item(&facing.tower) else {
            return;
        };
        let step = SCROLL_FLOORS * cellworks_map::city::floor_height(tower);
        match toward {
            Toward::Behind if facing.aim < facing.high - 0.01 => {
                facing.aim = (facing.aim + step).min(facing.high);
                return;
            }
            Toward::Front if facing.aim > facing.low + 0.01 => {
                facing.aim = (facing.aim - step).max(facing.low);
                return;
            }
            _ => {}
        }
        let block = map
            .find(&facing.tower)
            .and_then(|found| found.parent)
            .and_then(|node| map.items_of(node))
            .unwrap_or_default();
        let block: Vec<&Item> = block.iter().collect();
        if let Some(next) = cellworks_map::city::neighbour(&block, tower, toward) {
            self.selection = Some(next.path.clone());
        }
    }

    /// Where each step from the faced facade leads, while the arrows walk.
    pub fn hints(&self, map: &ProjectMap) -> Option<Hints> {
        let facing = self.facing.as_ref().filter(|_| self.is_walking())?;
        let tower = map.item(&facing.tower)?;
        let block = map
            .find(&facing.tower)
            .and_then(|found| found.parent)
            .and_then(|node| map.items_of(node))
            .unwrap_or_default();
        let block: Vec<&Item> = block.iter().collect();
        let name = |toward| {
            cellworks_map::city::neighbour(&block, tower, toward).map(|item| item.name.clone())
        };
        Some(Hints {
            here: tower.name.clone(),
            left: name(Toward::Left),
            right: name(Toward::Right),
            behind: name(Toward::Behind),
            front: name(Toward::Front),
        })
    }

    /// The facade the camera pivots on while it is turned.
    pub fn pivot(&self, map: &ProjectMap) -> Option<Pivot> {
        let facing = self.facing.as_ref()?;
        let tower = map.item(&facing.tower)?;
        Some(Pivot {
            ground: facing.ground,
            z: facing.z,
            along_x: tower.area.width() >= tower.area.height(),
        })
    }

    /// Whether the view is turned underground.
    pub fn is_underground(&self) -> bool {
        self.underground
    }

    /// Turns the view underground, the cellar within reach and the city
    /// above stepped back, or back above the ground.
    pub fn toggle_underground(&mut self) {
        self.underground = !self.underground;
    }

    /// Whether the cables from the masts are shown.
    pub fn shows_cables(&self) -> bool {
        self.cables
    }

    /// Shows the cables from each mast to the building it calls, or hides
    /// them.
    pub fn toggle_cables(&mut self) {
        self.cables = !self.cables;
    }

    /// The tower whose facade the camera turns to, while it is turned.
    pub fn facing(&self) -> Option<&Path> {
        self.facing.as_ref().map(|facing| facing.tower.as_path())
    }

    /// The tower stepped inside and its selected floor, from 0.
    pub fn inside(&self) -> Option<(&Path, usize)> {
        self.inside
            .as_ref()
            .map(|(item, floor)| (item.as_path(), *floor))
    }

    /// Steps inside the tower at `path`, selecting it, on its first floor. A
    /// path that is not a tower of `map` changes nothing.
    pub fn step_inside(&mut self, map: &ProjectMap, path: PathBuf) {
        if map.find(&path).map(|found| found.kind) == Some(Element::Item) {
            self.selection = Some(path.clone());
            self.inside = Some((path, 0));
        }
    }

    /// Inside a tower, moves the selected floor `by` floors up the source,
    /// or down when negative, stopping at the first and the last.
    pub fn move_floor(&mut self, map: &ProjectMap, by: isize) {
        if let Some((item, floor)) = &mut self.inside {
            let last = item_floors(map, item).saturating_sub(1);
            *floor = floor.saturating_add_signed(by).min(last);
        }
    }

    /// Inside a tower, selects floor `floor`, from 0.
    pub fn pick_floor(&mut self, floor: usize) {
        if let Some((_, selected)) = &mut self.inside {
            *selected = floor;
        }
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
        if found.kind == Element::Node {
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

    /// The map was turned `quarters` quarters about `centre`: the part of it
    /// in view turns with it, so the same spot stays in view. The layer and
    /// the selection stay as they are.
    pub fn turned(&mut self, centre: Pos2, quarters: u8) {
        // The pivot's ground point turned with the map; the next frame finds it again.
        self.facing = None;
        if let Some(camera) = &mut self.camera {
            for _ in 0..quarters % 4 {
                *camera = cellworks_map::turn_rect(*camera, centre);
            }
        }
    }

    /// Moves the view, `visible` at the layout's scale, by `by`. Neither the
    /// layer nor the selection changes.
    pub fn pan(&mut self, visible: Rect, by: Vec2) {
        if self.framed != Framed::Universe {
            self.camera = Some(visible.translate(by));
            // A turned camera lets go of the facade it faced and finds the
            // one nearest where it moved to.
            self.facing = None;
        }
    }

    /// Moves the view, `visible` at the layout's scale, so that `at` is in its
    /// centre, on the Project layer so that whatever lies there is shown. The
    /// selection stays.
    pub fn centre_on(&mut self, visible: Rect, at: Pos2) {
        if self.framed != Framed::Universe {
            self.framed = Framed::Project;
        }
        self.pan(visible, at - visible.center());
    }

    /// Brings the whole project into view, on the Project layer, keeping the
    /// selection.
    pub fn fit(&mut self) {
        if self.framed != Framed::Universe {
            self.camera = None;
            self.framed = Framed::Project;
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

    /// What the current layer holds, as the tree lists it: the districts on
    /// the Project layer, what the folder view holds on the Directory and File
    /// layers, and nothing in the Universe.
    pub fn rows<'a>(&self, map: &'a ProjectMap) -> Vec<(&'a str, &'a Path)> {
        match &self.framed {
            Framed::Universe => Vec::new(),
            Framed::Project => map.held_by(None).unwrap_or_default(),
            Framed::Folder(_) | Framed::Node(_) => {
                map.held_by(self.folder_view(map)).unwrap_or_default()
            }
        }
    }

    /// Moves the selection `by` rows down the tree, or up when negative,
    /// stopping at the first and the last row. A selection not in the tree
    /// moves to the first row going down and the last going up.
    pub fn move_in_tree(&mut self, map: &ProjectMap, by: isize) {
        let rows = self.rows(map);
        let Some(last) = rows.len().checked_sub(1) else {
            return;
        };
        let current = rows
            .iter()
            .position(|(_, path)| Some(*path) == self.selection.as_deref());
        let row = match current {
            Some(row) => row.saturating_add_signed(by).min(last),
            None if by < 0 => last,
            None => 0,
        };
        self.selection = Some(rows[row].1.to_path_buf());
    }

    /// The name of what the tree lists: the open project, or the folder of
    /// the folder view.
    pub fn listed<'a>(&self, map: &'a ProjectMap) -> &'a str {
        self.folder_view(map)
            .and_then(|folder| map.find(folder))
            .map_or(&map.project, |found| found.name)
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
            if let Some(found) = self.inside.as_ref().and_then(|(item, _)| map.find(item)) {
                crumbs.push(found.name.to_owned());
            }
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
                closest: f32::INFINITY,
                headroom: 0.0,
            };
        }
        let whole = Framing {
            area: map.extent,
            fill: 1.0,
            closest: 1.0,
            headroom: map.headroom(map.extent),
        };
        let (path, fill) = match &self.framed {
            Framed::Folder(path) => (path, FOLDER_FILL),
            Framed::Node(path) => (path, NODE_FILL),
            Framed::Universe | Framed::Project => return whole,
        };
        match map.find(path) {
            Some(found) => {
                let headroom = map.headroom(found.area);
                // Only towers drift apart when drawn too close.
                let closest = if headroom > 0.0 {
                    FRAMED_CLOSEST
                } else {
                    f32::INFINITY
                };
                Framing {
                    area: found.area,
                    fill,
                    closest,
                    headroom,
                }
            }
            None => whole,
        }
    }
}

/// How many floors the tower at `item` has.
fn item_floors(map: &ProjectMap, item: &Path) -> usize {
    map.item(item).map_or(0, |item| item.floors.len())
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
            "catalog/pricing/base.rs",
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
        assert!(navigation.framing(&map).closest <= 1.0);

        navigation.select(at("catalog"));
        navigation.enter(Some(&map));
        let folder = navigation.framing(&map);
        navigation.select(at("catalog/item.rs"));
        navigation.enter(Some(&map));
        let node = navigation.framing(&map);

        assert!(folder.closest > 1.0 && node.closest > 1.0);
        assert!(node.fill < folder.fill);
    }

    /// Zooms so that `path`'s rectangle spans `share` of the view across,
    /// around its middle for a node and just inside its left edge for a folder,
    /// where nothing inside it lies.
    fn zoom_onto(navigation: &mut Navigation, map: &ProjectMap, path: &str, share: f32) {
        let found = map.find(&at(path)).unwrap();
        let area = found.area;
        let view = Rect::from_center_size(area.center(), area.size() / share);
        let around = if found.kind == Element::Node {
            area.center()
        } else {
            egui::pos2(area.min.x + 0.5, area.center().y)
        };
        navigation.zoomed(map, view, around);
    }

    #[test]
    fn zooming_steps_through_the_folders_down_to_the_node_and_back_out() {
        let map = shop();
        let mut navigation = in_project();
        navigation.toggle_lock();

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
        assert!(navigation.is_locked(), "locked by default");

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
    fn the_tree_lists_the_current_layer_and_moves_the_selection() {
        let map = shop();
        let mut navigation = in_project();
        let names = |navigation: &Navigation| {
            navigation
                .rows(&map)
                .into_iter()
                .map(|(name, _)| name.to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&navigation), ["catalog", "orders", "shop"]);
        assert_eq!(navigation.listed(&map), "shop");

        navigation.move_in_tree(&map, 1);
        assert_eq!(navigation.selection(), Some(at("catalog").as_path()));
        navigation.move_in_tree(&map, 1);
        assert_eq!(navigation.selection(), Some(at("orders").as_path()));
        navigation.move_in_tree(&map, 5);
        assert_eq!(navigation.selection(), Some(Path::new(ROOT)));

        navigation.select(at("catalog"));
        navigation.enter(Some(&map));
        assert_eq!(names(&navigation), ["pricing", "item.rs"]);
        assert_eq!(navigation.listed(&map), "catalog");
        navigation.move_in_tree(&map, -1);
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/item.rs").as_path())
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

    #[test]
    fn esc_leaves_a_review_before_any_other_step_even_inside_a_tower() {
        let map = shop();
        let mut navigation = in_project();
        navigation.inside = Some((at("catalog/pricing/rules/discount.rs"), 0));
        navigation.enter_review(41);

        assert_eq!(navigation.esc(Some(&map)), Some(Moved::LeftReview));
        assert_eq!(navigation.review(), None);
        assert_eq!(
            navigation.inside(),
            Some((at("catalog/pricing/rules/discount.rs").as_path(), 0)),
            "esc leaves the review as one step, not also popping a layer"
        );
    }

    #[test]
    fn esc_without_a_review_falls_through_to_the_usual_layer_it_already_had() {
        let mut navigation = in_project();

        assert_eq!(navigation.esc(None), Some(Moved::Closed));
        assert_eq!(navigation.layer(), Layer::Universe);
    }

    #[test]
    fn opening_a_project_ends_a_review() {
        let mut navigation = in_project();
        navigation.enter_review(7);

        navigation.opened(PathBuf::from(ROOT));

        assert_eq!(navigation.review(), None);
    }

    #[test]
    fn pick_in_review_marks_the_row_and_moves_the_view_to_it() {
        let map = shop();
        let mut navigation = in_project();
        navigation.enter_review(41);

        navigation.pick_in_review(&map, at("catalog/pricing/rules/discount.rs"));

        assert_eq!(
            navigation.review().unwrap().picked,
            Some(at("catalog/pricing/rules/discount.rs"))
        );
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/pricing/rules/discount.rs").as_path())
        );
        assert_eq!(
            navigation.layer(),
            Layer::Project,
            "the review's city stays whole-project-framed"
        );
    }

    #[test]
    fn pick_in_review_does_nothing_while_no_review_is_open() {
        let map = shop();
        let mut navigation = in_project();
        let before = navigation.clone();

        navigation.pick_in_review(&map, at("catalog/pricing/rules/discount.rs"));

        assert_eq!(navigation, before);
    }

    /// STORY-089: `frame` shows a folder in its folder view and a node framed
    /// at inspection distance, selecting either; unlike `enter`, it keeps
    /// moving even when the view is already framed on a node, so the
    /// Explorer's cursor can jump from file to file without an Esc between.
    #[test]
    fn framing_a_folder_shows_its_folder_view_and_a_node_its_building() {
        let map = shop();
        let mut navigation = in_project();

        navigation.frame(&map, at("catalog"));
        assert_eq!(navigation.layer(), Layer::Directory);
        assert_eq!(navigation.folder_view(&map), Some(at("catalog").as_path()));
        assert_eq!(
            navigation.framing(&map).area,
            map.find(&at("catalog")).unwrap().area
        );
        assert_eq!(navigation.selection(), Some(at("catalog").as_path()));

        navigation.frame(&map, at("catalog/pricing/rules/discount.rs"));
        assert_eq!(navigation.layer(), Layer::File);
        assert_eq!(
            navigation.framing(&map).area,
            map.find(&at("catalog/pricing/rules/discount.rs"))
                .unwrap()
                .area
        );
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/pricing/rules/discount.rs").as_path())
        );

        // Framing another file from an already-framed node still moves: the
        // "entering a framed node does nothing" rule of `enter` does not hold
        // `frame` back.
        navigation.frame(&map, at("orders/order.rs"));
        assert_eq!(
            navigation.framing(&map).area,
            map.find(&at("orders/order.rs")).unwrap().area
        );
        assert_eq!(
            navigation.selection(),
            Some(at("orders/order.rs").as_path())
        );

        // And framing a folder from a framed node too.
        navigation.frame(&map, at("catalog/pricing"));
        assert_eq!(
            navigation.folder_view(&map),
            Some(at("catalog/pricing").as_path())
        );
        assert_eq!(
            navigation.selection(),
            Some(at("catalog/pricing").as_path())
        );
    }

    /// STORY-089: `frame` changes nothing in the Universe, inside a tower, or
    /// for a path the map does not hold.
    #[test]
    fn framing_does_nothing_in_the_universe_inside_a_tower_or_off_the_map() {
        let map = shop();

        let mut in_universe = Navigation::default();
        let before = in_universe.clone();
        in_universe.frame(&map, at("catalog"));
        assert_eq!(in_universe, before, "the Universe has no map to frame on");

        // `shop()` holds no items (built via `ProjectMap::of`, which maps no
        // code), so a map with one tower is built here to actually exercise
        // "inside a tower": an Item's path is its file's path joined with its
        // qualified name.
        let towered = {
            // `of_code` keys `items` by the file's path relative to `root`,
            // but an item's own path is its file's absolute path joined with
            // its qualified name.
            let file = PathBuf::from("catalog/pricing/rules/discount.rs");
            let item = cellworks_map::Item {
                name: "discount".to_owned(),
                path: at("catalog/pricing/rules/discount.rs").join("discount"),
                kind: cellworks_map::ItemKind::Function,
                tests: false,
                floors: vec![cellworks_map::Floor {
                    number: 1,
                    text: "fn discount() {}".to_owned(),
                    indent: 0,
                    width: 16,
                    tokens: Vec::new(),
                    calls: Vec::new(),
                    imports: Vec::new(),
                }],
                area: egui::Rect::NOTHING,
                base: 0.0,
                declaration: Default::default(),
            };
            let strategy = cellworks_map::Directories {
                prefix: cellworks_map::default_prefix_rule(),
            };
            let files: Vec<PathBuf> = [
                "catalog/pricing/base.rs",
                "catalog/pricing/rules/discount.rs",
                "catalog/pricing/rules/tax.rs",
                "catalog/item.rs",
                "orders/order.rs",
                "README.md",
            ]
            .iter()
            .map(PathBuf::from)
            .collect();
            let items = std::collections::BTreeMap::from([(file, vec![item])]);
            ProjectMap::of_code(
                "shop",
                Path::new(ROOT),
                &files,
                &strategy,
                items,
                &Default::default(),
            )
        };
        let tower = at("catalog/pricing/rules/discount.rs").join("discount");
        let mut inside_tower = in_project();
        inside_tower.step_inside(&towered, tower);
        assert!(inside_tower.inside().is_some(), "a real tower was entered");
        let before = inside_tower.clone();
        inside_tower.frame(&towered, at("catalog"));
        assert_eq!(inside_tower, before, "inside a tower, frame does nothing");

        let mut off_map = in_project();
        let before = off_map.clone();
        off_map.frame(&map, at("nowhere.rs"));
        assert_eq!(
            off_map, before,
            "a path the map does not hold changes nothing"
        );
    }
}
