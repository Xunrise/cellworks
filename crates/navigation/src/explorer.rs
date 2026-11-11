//! The Explorer: the open project's whole file tree beside the map, grouped
//! by the active lens, searchable, tracking folds and uncommitted changes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use cellworks_map::ui::district_colour_at;
use cellworks_map::{District, Group, ItemKind, Node, ProjectMap, SubDistrict};
use egui::{Color32, Rect};

use crate::Navigation;

/// What a row's icon shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Badge {
    Folder,
    Extension(String),
    Symbol(SymbolBadge),
}

/// A symbol leaf's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolBadge {
    Function,
    Type,
    Uses,
    Section,
}

/// Whether a row, or something it holds, has uncommitted changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Change {
    #[default]
    None,
    Modified,
    Holds,
}

/// One row the Explorer lists.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub badge: Badge,
    /// A folder's file count (every file it holds, however deep), or a
    /// file's highest floor (line) number; `None` for a symbol leaf, or for
    /// a file holding no item with a floor.
    pub count: Option<usize>,
    /// A folder or group's district-colour swatch; `None` for a file or a
    /// symbol leaf.
    pub colour: Option<Color32>,
    pub change: Change,
    /// Whether this row has children that can be folded or unfolded.
    pub foldable: bool,
    /// Whether this row's children are currently listed.
    pub unfolded: bool,
}

/// The file tree navigator beside the map: what is folded, the filter, and
/// the camera's flight to a newly framed row.
#[derive(Clone, Debug, PartialEq)]
pub struct Explorer {
    open: bool,
    unfolded: HashSet<PathBuf>,
    filter: String,
    flight: Option<(Rect, Rect, f64)>,
    followed: Option<PathBuf>,
    scroll_to: Option<PathBuf>,
}

impl Default for Explorer {
    fn default() -> Explorer {
        Explorer {
            open: true,
            unfolded: HashSet::new(),
            filter: String::new(),
            flight: None,
            followed: None,
            scroll_to: None,
        }
    }
}

impl Explorer {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
    }

    /// Clears every fold and the filter; keeps whether the panel is open —
    /// called when a different project is opened.
    pub fn reset(&mut self) {
        self.unfolded.clear();
        self.filter.clear();
        self.flight = None;
        self.followed = None;
        self.scroll_to = None;
    }

    /// The rows `map`'s districts, sub-districts, groups, nodes and items
    /// make, folders first, depth-first, respecting which folders are
    /// unfolded; a non-empty filter (case-insensitive substring over a row's
    /// name) unfolds everything and keeps only a matching file or symbol row
    /// plus the folders leading to it. `changed` marks a file `Change::Modified`
    /// and every folder holding it `Change::Holds`.
    pub fn rows(&self, map: &ProjectMap, changed: &HashSet<PathBuf>) -> Vec<Row> {
        let trimmed = self.filter.trim();
        let filter = (!trimmed.is_empty()).then(|| trimmed.to_lowercase());
        let mut out = Vec::new();
        for district in &map.districts {
            let folder = FolderRef::District(district);
            if filter
                .as_deref()
                .is_some_and(|f| !folder_has_match(&folder, f))
            {
                continue;
            }
            walk_folder(&folder, 0, self, changed, filter.as_deref(), map, &mut out);
        }
        out
    }

    /// Folds `path` if it is unfolded, or unfolds it if it is folded. No-op
    /// for a path nothing is nested under.
    pub fn fold(&mut self, path: &Path) {
        if !self.unfolded.remove(path) {
            self.unfolded.insert(path.to_path_buf());
        }
    }

    /// Unfolds every ancestor folder of `path`, so its row would be listed
    /// (by an unfiltered `rows` call).
    pub fn reveal(&mut self, map: &ProjectMap, path: &Path) {
        let mut at = path.to_path_buf();
        while let Some(parent) = map.find(&at).and_then(|found| found.parent) {
            self.unfolded.insert(parent.to_path_buf());
            at = parent.to_path_buf();
        }
    }

    /// `reveal`s `path` and marks it to be scrolled into view, unless it was
    /// the last path followed.
    pub fn follow(&mut self, map: &ProjectMap, path: &Path) {
        if self.followed.as_deref() == Some(path) {
            return;
        }
        self.reveal(map, path);
        self.scroll_to = Some(path.to_path_buf());
        self.followed = Some(path.to_path_buf());
    }

    /// Takes the row to scroll into view, if one is pending.
    pub fn take_scroll_to(&mut self) -> Option<PathBuf> {
        self.scroll_to.take()
    }

    /// Moves the cursor `by` rows down the current list (`rows(map, changed)`,
    /// in order), or up when negative, stopping at the first and the last;
    /// frames the row it lands on via `navigation.frame`. No-op on an empty
    /// row list. A selection not among the rows starts from before the first
    /// row, so the step, whichever way it goes, lands on the first row
    /// (unlike `Navigation::move_in_tree`, which sends a selection outside
    /// the current layer to the far end in the direction stepped away from;
    /// the Explorer's cursor has no such outside position to step away from,
    /// so it always lands back at the top).
    pub fn step(
        &mut self,
        map: &ProjectMap,
        changed: &HashSet<PathBuf>,
        navigation: &mut Navigation,
        by: isize,
    ) {
        let rows = self.rows(map, changed);
        let Some(last) = rows.len().checked_sub(1) else {
            return;
        };
        let current = rows
            .iter()
            .position(|row| Some(row.path.as_path()) == navigation.selection());
        let index = match current {
            Some(index) => index.saturating_add_signed(by).min(last),
            None => 0,
        };
        let path = rows[index].path.clone();
        navigation.frame(map, path.clone());
        // So the map-selection change `frame` just caused doesn't re-trigger
        // `follow`'s one-time scroll for a row already visible.
        self.followed = Some(path);
    }

    /// Starts flying the camera from `from` to `to`, beginning at `now`.
    pub fn fly(&mut self, from: Rect, to: Rect, now: f64) {
        self.flight = Some((from, to, now));
    }

    /// The camera `duration` seconds into the flight `fly` started, at `now`;
    /// `None` once it has landed (the flight is then cleared).
    pub fn flying(&mut self, now: f64, duration: f64) -> Option<Rect> {
        let (from, to, started) = self.flight?;
        let t = ((now - started) / duration) as f32;
        if t >= 1.0 {
            self.flight = None;
            return Some(to);
        }
        Some(flown(from, to, t.max(0.0)))
    }
}

/// `t` (0 to 1) of the way from `from` to `to`'s corner and size; exactly
/// `to` at `t` = 1, `from` at `t` = 0.
pub fn flown(from: Rect, to: Rect, t: f32) -> Rect {
    let t = t.clamp(0.0, 1.0);
    Rect::from_min_size(
        from.min + (to.min - from.min) * t,
        from.size() + (to.size() - from.size()) * t,
    )
}

/// Shifts `area` left by a ground margin equal to `panel_width` screen points
/// at `scale` points per ground unit, so that centring the result in a view
/// `window_width` points wide puts `area` itself centred in the part of that
/// view to the right of a panel `panel_width` points wide — unless
/// `area == whole` (the whole-project overview) or `window_width` is less
/// than twice `panel_width`.
pub fn beside(area: Rect, whole: Rect, window_width: f32, scale: f32, panel_width: f32) -> Rect {
    if area == whole || window_width < 2.0 * panel_width {
        return area;
    }
    let margin = panel_width / scale;
    area.translate(egui::vec2(-margin, 0.0))
}

/// A folder-shaped element of the map: a district, a sub-district or a
/// group, each nesting the next and holding nodes the same way.
enum FolderRef<'a> {
    District(&'a District),
    SubDistrict(&'a SubDistrict),
    Group(&'a Group),
}

impl<'a> FolderRef<'a> {
    fn path(&self) -> &'a Path {
        match self {
            FolderRef::District(district) => &district.path,
            FolderRef::SubDistrict(sub) => &sub.path,
            FolderRef::Group(group) => &group.path,
        }
    }

    fn name(&self) -> &'a str {
        match self {
            FolderRef::District(district) => &district.name,
            FolderRef::SubDistrict(sub) => &sub.name,
            FolderRef::Group(group) => &group.name,
        }
    }

    fn nodes(&self) -> &'a [Node] {
        match self {
            FolderRef::District(district) => &district.nodes,
            FolderRef::SubDistrict(sub) => &sub.nodes,
            FolderRef::Group(group) => &group.nodes,
        }
    }

    fn child_folders(&self) -> Vec<FolderRef<'a>> {
        match self {
            FolderRef::District(district) => district
                .sub_districts
                .iter()
                .map(FolderRef::SubDistrict)
                .collect(),
            FolderRef::SubDistrict(sub) => sub.groups.iter().map(FolderRef::Group).collect(),
            FolderRef::Group(group) => group.groups.iter().map(FolderRef::Group).collect(),
        }
    }
}

/// The number of files `folder` holds, however deep.
fn count_files(folder: &FolderRef) -> usize {
    folder.nodes().len()
        + folder
            .child_folders()
            .iter()
            .map(count_files)
            .sum::<usize>()
}

/// Whether `folder` holds, however deep, a file in `changed`.
fn holds_changed(folder: &FolderRef, changed: &HashSet<PathBuf>) -> bool {
    folder
        .nodes()
        .iter()
        .any(|node| changed.contains(&node.path))
        || folder
            .child_folders()
            .iter()
            .any(|child| holds_changed(child, changed))
}

/// Case-insensitive substring match of `name` against `filter`, already
/// lower-cased.
fn matches_name(name: &str, filter: &str) -> bool {
    name.to_lowercase().contains(filter)
}

/// Whether `node` itself, or one of its non-test items, matches `filter`.
fn file_has_match(node: &Node, filter: &str) -> bool {
    matches_name(&node.name, filter)
        || node
            .items
            .iter()
            .filter(|item| !item.tests)
            .any(|item| matches_name(&item.name, filter))
}

/// Whether `folder` holds, however deep, a file matching `filter`.
fn folder_has_match(folder: &FolderRef, filter: &str) -> bool {
    folder
        .nodes()
        .iter()
        .any(|node| file_has_match(node, filter))
        || folder
            .child_folders()
            .iter()
            .any(|child| folder_has_match(child, filter))
}

/// The symbol badge an item's kind shows, or `None` for a kind that lists no
/// row of its own: a test (already excluded by its `tests` flag in practice)
/// and free-form text, which a line-numbered body does not suit a single
/// leaf row for.
fn symbol_badge(kind: ItemKind) -> Option<SymbolBadge> {
    match kind {
        ItemKind::Function => Some(SymbolBadge::Function),
        ItemKind::Type => Some(SymbolBadge::Type),
        ItemKind::Uses => Some(SymbolBadge::Uses),
        ItemKind::Section => Some(SymbolBadge::Section),
        ItemKind::Test | ItemKind::Text => None,
    }
}

/// The extension `path` shows in its badge, without the leading dot; empty
/// when it has none.
fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_owned()
}

/// Appends `folder`'s own row, at `depth`, to `out`, then, while it is
/// unfolded (always, while `filter` is active), its child folders and its
/// nodes, folders first.
fn walk_folder(
    folder: &FolderRef,
    depth: usize,
    explorer: &Explorer,
    changed: &HashSet<PathBuf>,
    filter: Option<&str>,
    map: &ProjectMap,
    out: &mut Vec<Row>,
) {
    let path = folder.path().to_path_buf();
    let count = count_files(folder);
    let unfolded = filter.is_some() || explorer.unfolded.contains(&path);
    let change = if holds_changed(folder, changed) {
        Change::Holds
    } else {
        Change::None
    };
    out.push(Row {
        path: path.clone(),
        name: folder.name().to_owned(),
        depth,
        badge: Badge::Folder,
        count: Some(count),
        colour: Some(district_colour_at(map, &path)),
        change,
        foldable: count > 0,
        unfolded,
    });
    if !unfolded {
        return;
    }
    for child in folder.child_folders() {
        if filter.is_some_and(|f| !folder_has_match(&child, f)) {
            continue;
        }
        walk_folder(&child, depth + 1, explorer, changed, filter, map, out);
    }
    for node in folder.nodes() {
        if filter.is_some_and(|f| !file_has_match(node, f)) {
            continue;
        }
        walk_node(node, depth + 1, explorer, changed, filter, out);
    }
}

/// Appends `node`'s own row, at `depth`, to `out`, then, while it is
/// unfolded (always, while `filter` is active), its non-test, badge-bearing
/// items, each matching `filter` while one is active.
fn walk_node(
    node: &Node,
    depth: usize,
    explorer: &Explorer,
    changed: &HashSet<PathBuf>,
    filter: Option<&str>,
    out: &mut Vec<Row>,
) {
    let unfolded = filter.is_some() || explorer.unfolded.contains(&node.path);
    let change = if changed.contains(&node.path) {
        Change::Modified
    } else {
        Change::None
    };
    let foldable = node.items.iter().any(|item| !item.tests);
    let count = node
        .items
        .iter()
        .flat_map(|item| item.floors.iter())
        .map(|floor| floor.number)
        .max();
    out.push(Row {
        path: node.path.clone(),
        name: node.name.clone(),
        depth,
        badge: Badge::Extension(extension(&node.path)),
        count,
        colour: None,
        change,
        foldable,
        unfolded,
    });
    if !unfolded {
        return;
    }
    for item in node.items.iter().filter(|item| !item.tests) {
        let Some(badge) = symbol_badge(item.kind) else {
            continue;
        };
        if filter.is_some_and(|f| !matches_name(&item.name, f)) {
            continue;
        }
        out.push(Row {
            path: item.path.clone(),
            name: item.name.clone(),
            depth: depth + 1,
            badge: Badge::Symbol(badge),
            count: None,
            colour: None,
            change: Change::None,
            foldable: false,
            unfolded: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    use cellworks_map::{Directories, ProjectMap, default_prefix_rule};

    use super::*;
    use crate::Navigation;

    const ROOT: &str = "/p/shop";

    /// `shop` with the district `catalog`, its sub-district `pricing`, the
    /// group `rules` and the node `discount.rs`, plus the district `orders`:
    /// the same depth-three shape as `crate::tests::shop`, so a sub-district
    /// and a group are genuinely distinct from each other.
    fn shop() -> ProjectMap {
        let files: Vec<PathBuf> = [
            "catalog/pricing/base.rs",
            "catalog/pricing/rules/discount.rs",
            "catalog/item.rs",
            "orders/order.rs",
        ]
        .iter()
        .map(PathBuf::from)
        .collect();
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of("shop", Path::new(ROOT), &files, &strategy)
    }

    /// A map with no district at all: a project with no included file.
    fn empty_map() -> ProjectMap {
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of("shop", Path::new(ROOT), &[], &strategy)
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
    fn rows_list_folders_first_and_count_files() {
        let map = shop();
        let explorer = Explorer::default();

        let rows = explorer.rows(&map, &HashSet::new());

        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(
            names,
            ["catalog", "orders"],
            "nothing is unfolded yet, so only the top level shows"
        );
        let catalog = rows
            .iter()
            .find(|row| row.path == at("catalog"))
            .expect("a catalog row");
        assert!(matches!(catalog.badge, Badge::Folder));
        assert_eq!(
            catalog.count,
            Some(3),
            "base.rs, discount.rs and item.rs, however deep"
        );
        let orders = rows
            .iter()
            .find(|row| row.path == at("orders"))
            .expect("an orders row");
        assert_eq!(orders.count, Some(1));
    }

    #[test]
    fn a_filter_keeps_matches_and_the_folders_leading_to_them() {
        let map = shop();
        let mut explorer = Explorer::default();

        explorer.set_filter("DISCOUNT".to_owned());
        let rows = explorer.rows(&map, &HashSet::new());

        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(
            names,
            ["catalog", "pricing", "rules", "discount.rs"],
            "case-insensitive, keeps the match and the folders leading to it, nothing else"
        );
    }

    #[test]
    fn a_changed_file_marks_itself_and_every_folder_holding_it() {
        let map = shop();
        let mut explorer = Explorer::default();
        let changed_file = at("catalog/pricing/rules/discount.rs");
        let changed: HashSet<PathBuf> = [changed_file.clone()].into_iter().collect();
        // Reveal the changed file's row, since nothing is unfolded by default.
        explorer.reveal(&map, &changed_file);

        let rows = explorer.rows(&map, &changed);
        let row = |path: &Path| {
            rows.iter()
                .find(|row| row.path == path)
                .unwrap_or_else(|| panic!("no row for {}", path.display()))
        };

        assert_eq!(row(&changed_file).change, Change::Modified);
        assert_eq!(row(&at("catalog/pricing/rules")).change, Change::Holds);
        assert_eq!(row(&at("catalog/pricing")).change, Change::Holds);
        assert_eq!(row(&at("catalog")).change, Change::Holds);
        assert_eq!(
            row(&at("orders")).change,
            Change::None,
            "orders holds no changed file"
        );
    }

    #[test]
    fn stepping_stops_at_the_ends_and_frames_the_row() {
        let map = shop();
        let mut navigation = in_project();
        let explorer_rows = Explorer::default().rows(&map, &HashSet::new());
        assert_eq!(explorer_rows.len(), 2, "catalog and orders at the top");
        let mut explorer = Explorer::default();

        explorer.step(&map, &HashSet::new(), &mut navigation, -5);
        assert_eq!(
            navigation.selection(),
            Some(explorer_rows[0].path.as_path()),
            "stops at the first row"
        );
        assert_eq!(
            navigation.framing(&map).area,
            map.find(&explorer_rows[0].path).unwrap().area
        );

        explorer.step(&map, &HashSet::new(), &mut navigation, 100);
        assert_eq!(
            navigation.selection(),
            Some(explorer_rows[1].path.as_path()),
            "stops at the last row"
        );
        assert_eq!(
            navigation.framing(&map).area,
            map.find(&explorer_rows[1].path).unwrap().area
        );

        // No-op on an empty row list.
        let empty = empty_map();
        let before = navigation.clone();
        explorer.step(&empty, &HashSet::new(), &mut navigation, 1);
        assert_eq!(navigation, before, "nothing to step onto");
    }

    #[test]
    fn following_a_selection_unfolds_its_ancestors_once() {
        let map = shop();
        let mut explorer = Explorer::default();
        let target = at("catalog/pricing/rules/discount.rs");

        explorer.follow(&map, &target);
        assert_eq!(
            explorer.take_scroll_to(),
            Some(target.clone()),
            "following scrolls to the path"
        );
        assert_eq!(
            explorer.take_scroll_to(),
            None,
            "taken once, nothing is left to take"
        );
        let unfolded = explorer.rows(&map, &HashSet::new());
        assert!(
            unfolded
                .iter()
                .any(|row| row.path == at("catalog/pricing/rules")),
            "following unfolds the ancestors: {unfolded:?}"
        );

        explorer.follow(&map, &target);
        assert_eq!(
            explorer.take_scroll_to(),
            None,
            "re-following the same path scrolls nothing"
        );

        let other = at("orders/order.rs");
        explorer.follow(&map, &other);
        assert_eq!(
            explorer.take_scroll_to(),
            Some(other),
            "a different path scrolls again"
        );
    }

    #[test]
    fn beside_shifts_folder_and_node_framing_but_not_the_whole_project_nor_a_narrow_view() {
        let area = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(30.0, 40.0));
        let whole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 500.0));
        let scale = 2.0;
        let panel_width = 320.0;
        let window_width = 1200.0;

        let shifted = beside(area, whole, window_width, scale, panel_width);
        let expected = area.translate(egui::vec2(-panel_width / scale, 0.0));
        assert_eq!(shifted, expected);

        assert_eq!(
            beside(whole, whole, window_width, scale, panel_width),
            whole,
            "the whole-project overview is never shifted"
        );

        let narrow = 2.0 * panel_width - 1.0;
        assert_eq!(
            beside(area, whole, narrow, scale, panel_width),
            area,
            "a view narrower than twice the panel is left alone"
        );
    }

    #[test]
    fn flown_reaches_the_target_at_one() {
        let from = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0));
        let to = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(40.0, 20.0));

        assert_eq!(flown(from, to, 0.0), from, "the start, at t = 0");
        assert_eq!(flown(from, to, 1.0), to, "exactly the target, at t = 1");
    }
}
