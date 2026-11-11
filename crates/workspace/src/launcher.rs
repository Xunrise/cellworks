//! The launcher: the view model over the [`crate::Universe`] scan state that
//! the launcher view draws — what is searched for, which [`Location`] is
//! picked, which project is selected, the five most recently opened, and the
//! details and the state known about each project so far.
//!
//! STORY-078. `Universe` stays the scan's own state; `Launcher` is read from
//! it and from the host probes, the recent file and the project details read
//! off the frame loop, per requirements 5 to 9 of the design.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use crate::event::WorkspaceEvent;
use crate::project::{Project, ProjectId};
use crate::words::{PLAIN, THEMED, Words};

/// A left-column entry the launcher can be narrowed to: this Mac, or one
/// remote host. "Location" because "source" already names the Sources
/// context and the `Sources` struct — this is a UI word ("sources"/"HOSTS")
/// only, never this one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    ThisMac,
    Host(String),
}

/// What the main area is narrowed to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    All,
    Location(Location),
}

/// Whether a remote host answers, always a glyph paired with a word, never
/// colour alone (requirement 4). The glyph is a shape the view paints beside
/// the word (STORY-082 ASSUMED (Q-1)), so this type carries the word alone —
/// there is deliberately no `Display` writing a glyph into text a row or a
/// chip would then read out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Checking,
    Connected,
    Offline,
}

/// Sort key for ASSUMED (Q-2)'s chip and `cycle_pick` order: connected hosts
/// before checking ones before offline ones, name breaking a tie.
fn state_priority(state: HostState) -> u8 {
    match state {
        HostState::Connected => 0,
        HostState::Checking => 1,
        HostState::Offline => 2,
    }
}

/// One location chip (STORY-082 requirement 3): `pick` is what clicking it
/// narrows the main area to, `label` and `count` what it reads, `state` the
/// host's state for a host chip, `None` for "All" and "This Mac" which have
/// none.
#[derive(Clone, Debug, PartialEq)]
pub struct Chip {
    pub pick: Pick,
    pub label: String,
    pub count: usize,
    pub state: Option<HostState>,
}

/// The byte range of the first case-insensitive match of `query` in `name`,
/// for the view to underline (STORY-082 requirement 6); `None` when `query`
/// is empty, there is no match, or lowercasing `name` changes its byte
/// length — a Unicode case-folding mismatch that would make a byte index
/// found in the lowercased copy invalid against `name` itself.
pub fn match_range(name: &str, query: &str) -> Option<std::ops::Range<usize>> {
    if query.is_empty() {
        return None;
    }
    let lower_name = name.to_lowercase();
    if lower_name.len() != name.len() {
        return None;
    }
    let lower_query = query.to_lowercase();
    let start = lower_name.find(&lower_query)?;
    Some(start..start + lower_query.len())
}

/// What the inspector calls a project (requirement 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectKind {
    Repository,
    Folder,
    Remote,
}

/// A local project's branch, file count, last commit time and folder
/// modification time, read off the frame loop; each `None` until it is
/// known, and a plain folder's `files` and `committed` stay `None` rather
/// than becoming zero (requirement 9). Times are seconds since the Unix
/// epoch.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectDetails {
    pub branch: Option<String>,
    pub files: Option<usize>,
    pub committed: Option<i64>,
    pub changed: Option<i64>,
}

/// How the rows inside each location group are ordered (STORY-083
/// requirement 15). Cycled by the sort chip and Shift+Tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// By parent folder, then name: the folders read as blocks.
    #[default]
    Dir,
    /// Last opened in Cellworks first, then the never-opened by name.
    Opened,
    /// Newest last git commit first, the unknown by name.
    Commit,
    /// Newest folder modification first, the unknown by name.
    Changed,
}

impl Sort {
    /// The sort chip's text; the same with theme words on or off.
    pub fn label(self) -> &'static str {
        match self {
            Sort::Dir => "Sort: dir",
            Sort::Opened => "Sort: opened",
            Sort::Commit => "Sort: commit",
            Sort::Changed => "Sort: changed",
        }
    }

    fn next(self) -> Sort {
        match self {
            Sort::Dir => Sort::Opened,
            Sort::Opened => Sort::Commit,
            Sort::Commit => Sort::Changed,
            Sort::Changed => Sort::Dir,
        }
    }
}

/// The view model the launcher draws: what the scan has found, which
/// [`Location`] is picked, what is searched for, which project is selected,
/// the five most recently opened, and the host states and project details
/// read off the frame loop so far.
///
/// Read from [`crate::Universe`]'s scan events and from the host probes, the
/// recent file and the project details this story adds — never the other
/// way around: `Universe` stays the scan's own state, unaware this exists.
pub struct Launcher {
    home: Option<PathBuf>,
    projects: Vec<Project>,
    search: String,
    pick: Pick,
    sort: Sort,
    selected: Option<ProjectId>,
    recent: Vec<ProjectId>,
    hosts: HashMap<String, HostState>,
    details: HashMap<PathBuf, ProjectDetails>,
    /// What the view should do with the search field's focus this frame:
    /// `Some(true)` takes it (CMD), `Some(false)` gives it up (NAV), read
    /// (and cleared) once by the view. The mode itself is whether the field
    /// has focus, so there is no second state to drift (STORY-083
    /// requirement 8).
    focus_request: Option<bool>,
    /// The vocabulary the launcher currently speaks (STORY-083 requirement
    /// 6): themed by default, switched to [`PLAIN`] once the settings file's
    /// `ThemeWordsTurnedOff` is applied.
    words: &'static Words,
}

impl Launcher {
    /// A launcher with nothing found yet, for a machine whose home directory
    /// is `home` — used to show it as `~` (requirement 7); `None` when there
    /// is none to show.
    pub fn new(home: Option<PathBuf>) -> Launcher {
        Launcher {
            home,
            projects: Vec::new(),
            search: String::new(),
            pick: Pick::All,
            sort: Sort::default(),
            selected: None,
            recent: Vec::new(),
            hosts: HashMap::new(),
            details: HashMap::new(),
            focus_request: None,
            words: &THEMED,
        }
    }

    /// The vocabulary currently in effect, for the view to read its words
    /// from (STORY-083 requirement 6). `pub(crate)`, not `pub`, because
    /// [`Words`] itself is `pub(crate)` — only `ui.rs`, inside this crate,
    /// reaches it this way.
    ///
    /// Not yet called outside this file's own tests — step 2 wires `ui.rs`
    /// to it — hence `#[allow(dead_code)]`.
    #[allow(dead_code)]
    pub(crate) fn words(&self) -> &'static Words {
        self.words
    }

    /// Folds one scan event into the state. Every variant of `WorkspaceEvent`
    /// is matched: most belong to `Universe` alone and are ignored here.
    pub fn apply(&mut self, event: WorkspaceEvent) {
        match event {
            WorkspaceEvent::ProjectsDiscovered(projects) => self.projects = projects,
            WorkspaceEvent::HostProbed { host, connected } => {
                let state = if connected {
                    HostState::Connected
                } else {
                    HostState::Offline
                };
                self.hosts.insert(host, state);
            }
            WorkspaceEvent::RecentRead(ids) => self.recent = ids,
            WorkspaceEvent::ThemeWordsTurnedOff => self.words = &PLAIN,
            WorkspaceEvent::HostSkipped { .. }
            | WorkspaceEvent::ProjectsFolderUnreadable { .. }
            | WorkspaceEvent::AddedFolderSkipped { .. }
            | WorkspaceEvent::SettingsUnreadable { .. }
            | WorkspaceEvent::ArrangementRead { .. } => {}
        }
    }

    /// Narrows the main area to `text`, matched case-insensitively against a
    /// project's name or its displayed path (requirement 8).
    pub fn search(&mut self, text: String) {
        self.search = text;
    }

    /// The text currently searched for.
    pub fn search_text(&self) -> &str {
        &self.search
    }

    /// Narrows the main area to `pick`, and drops the selection if it is no
    /// longer among the projects the new pick shows (STORY-082 requirement 4's
    /// last sentence) — through this method rather than only `cycle_pick`,
    /// since a chip click changes the pick the same way.
    pub fn pick(&mut self, pick: Pick) {
        self.pick = pick;
        if let Some(selected) = &self.selected
            && !self.visible().iter().any(|project| &project.id == selected)
        {
            self.selected = None;
        }
    }

    /// What the main area is currently narrowed to.
    pub fn picked(&self) -> &Pick {
        &self.pick
    }

    /// The order the rows inside each group are in.
    pub fn sort(&self) -> Sort {
        self.sort
    }

    /// Moves to the next sort, wrapping from the last back to the first.
    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
    }

    /// Selects `id`, or clears the selection.
    pub fn select(&mut self, id: Option<ProjectId>) {
        self.selected = id;
    }

    /// The id selected, if any.
    pub fn selected(&self) -> Option<&ProjectId> {
        self.selected.as_ref()
    }

    /// The project selected, if any and if it is still among those found.
    pub fn selected_project(&self) -> Option<&Project> {
        self.selected.as_ref().and_then(|id| self.project(id))
    }

    /// Moves the selection `by` rows through every group, wrapping at either end (requirement 11's ↑/↓). Nothing while
    /// nothing is shown.
    pub fn move_selection(&mut self, by: isize) {
        let order = self.visible_order();
        if order.is_empty() {
            return;
        }
        let at = self
            .selected
            .as_ref()
            .and_then(|id| order.iter().position(|shown| shown == id));
        let len = order.len() as isize;
        let next = match at {
            Some(at) => (at as isize + by).rem_euclid(len),
            None if by >= 0 => 0,
            None => len - 1,
        };
        self.selected = Some(order[next as usize].clone());
    }

    /// Moves the selection `by` rows through every group like
    /// [`Launcher::move_selection`], but stopping at the first and last row
    /// instead of wrapping: a page move (STORY-083 requirement 14).
    pub fn move_selection_clamped(&mut self, by: isize) {
        let order = self.visible_order();
        if order.is_empty() {
            return;
        }
        let at = self
            .selected
            .as_ref()
            .and_then(|id| order.iter().position(|shown| shown == id));
        let last = order.len() as isize - 1;
        let next = match at {
            Some(at) => (at as isize + by).clamp(0, last),
            None if by >= 0 => 0,
            None => last,
        };
        self.selected = Some(order[next as usize].clone());
    }

    /// Every selectable id, in the order the view draws the group rows: what
    /// `move_selection` steps through. The recent row is left out: it never
    /// shows the selection, so stepping onto it would make the highlight jump.
    fn visible_order(&self) -> Vec<ProjectId> {
        self.groups()
            .into_iter()
            .flat_map(|(_, projects)| projects.into_iter().map(|project| project.id.clone()))
            .collect()
    }

    /// Records that `id` was opened: moves it to the front of the recent
    /// list, adding it if it was not already there, and keeps at most five
    /// (requirement 6).
    pub fn opened(&mut self, id: ProjectId) {
        self.recent.retain(|existing| existing != &id);
        self.recent.insert(0, id);
        self.recent.truncate(5);
    }

    /// Records `project`'s details, read off the frame loop.
    pub fn detailed(&mut self, project: PathBuf, details: ProjectDetails) {
        self.details.insert(project, details);
    }

    /// `id`'s details, for a local project whose read has come back.
    pub fn details_of(&self, id: &ProjectId) -> Option<&ProjectDetails> {
        id.local_path().and_then(|path| self.details.get(path))
    }

    /// What the inspector calls `id` (requirement 9): a remote project is
    /// always `Remote`; a local one is a `Repository` once its branch is
    /// known, else a plain `Folder` — including before its details arrive,
    /// which is indistinguishable from a folder until they do.
    pub fn kind(&self, id: &ProjectId) -> ProjectKind {
        match id {
            ProjectId::Remote { .. } => ProjectKind::Remote,
            ProjectId::Local(_) => {
                if self
                    .details_of(id)
                    .is_some_and(|details| details.branch.is_some())
                {
                    ProjectKind::Repository
                } else {
                    ProjectKind::Folder
                }
            }
        }
    }

    /// Where `id` lives, as the preview pane names it after its kind
    /// (STORY-082 requirement 7's "`<kind> · <location>`"): "This Mac" for a
    /// local project, the host's name for a remote one. Never the host's
    /// state — that is what the host's chip says, and it is not a location.
    pub fn location_of(&self, id: &ProjectId) -> String {
        match id {
            ProjectId::Local(_) => self.words.this_mac_location.to_owned(),
            ProjectId::Remote { host, .. } => host.clone(),
        }
    }

    /// Every remote host with at least one project, by name, with its
    /// current state (requirement 5): a host named in the settings file with
    /// no project of its own never reaches this list, because it is derived
    /// from the projects actually found rather than kept separately.
    pub fn hosts(&self) -> Vec<(&str, HostState)> {
        let names: BTreeSet<&str> = self
            .projects
            .iter()
            .filter_map(|project| project.id.host())
            .collect();
        names
            .into_iter()
            .map(|name| {
                (
                    name,
                    self.hosts.get(name).copied().unwrap_or(HostState::Checking),
                )
            })
            .collect()
    }

    /// How many projects the current pick and search leave standing — the
    /// "<k> matches" count (STORY-082 requirement 2).
    pub fn match_count(&self) -> usize {
        self.visible().len()
    }

    /// A remote host's current state, "checking" for one never probed yet —
    /// shared by [`Launcher::location_chips`] and [`Launcher::open_label`]
    /// (STORY-082 requirement 3 and requirement 8).
    fn host_state(&self, host: &str) -> HostState {
        self.hosts.get(host).copied().unwrap_or(HostState::Checking)
    }

    /// The location chip row: "All", "This Mac", then one chip per remote
    /// host with a project, ordered by state (connected, then checking, then
    /// offline) and by name within a state, per ASSUMED (Q-2) — the only
    /// order that reaches the acceptance fixture's connected "wsl" before the
    /// offline "devbox". Counts are totals over every project found, never
    /// narrowed by the current pick or search (Decision 3).
    pub fn location_chips(&self) -> Vec<Chip> {
        let mut chips = vec![
            Chip {
                pick: Pick::All,
                label: self.words.all_chip.to_owned(),
                count: self.projects.len(),
                state: None,
            },
            Chip {
                pick: Pick::Location(Location::ThisMac),
                label: self.words.this_mac_label.to_owned(),
                count: self
                    .projects
                    .iter()
                    .filter(|project| project.id.local_path().is_some())
                    .count(),
                state: None,
            },
        ];

        let names: BTreeSet<&str> = self
            .projects
            .iter()
            .filter_map(|project| project.id.host())
            .collect();
        let mut hosts: Vec<(&str, HostState)> = names
            .into_iter()
            .map(|name| (name, self.host_state(name)))
            .collect();
        hosts.sort_by_key(|(name, state)| (state_priority(*state), *name));

        for (host, state) in hosts {
            let count = self
                .projects
                .iter()
                .filter(|project| project.id.host() == Some(host))
                .count();
            chips.push(Chip {
                pick: Pick::Location(Location::Host(host.to_owned())),
                label: host.to_owned(),
                count,
                state: Some(state),
            });
        }

        chips
    }

    /// Moves the pick to the chip after the current one in
    /// [`Launcher::location_chips`] order, wrapping from the last host back
    /// to "All" (STORY-082 requirement 4). Routed through [`Launcher::pick`]
    /// rather than setting the field directly, so the invisible-selection
    /// rule applies here too.
    pub fn cycle_pick(&mut self) {
        let order: Vec<Pick> = self
            .location_chips()
            .into_iter()
            .map(|chip| chip.pick)
            .collect();
        let at = order
            .iter()
            .position(|pick| pick == &self.pick)
            .unwrap_or(0);
        let next = (at + 1) % order.len();
        self.pick(order[next].clone());
    }

    /// Whether `id` can be opened at all: only a local project, since remote
    /// opening is not built this story regardless of the host's state
    /// (STORY-082 Decision 1).
    pub fn can_open(&self, id: &ProjectId) -> bool {
        id.local_path().is_some()
    }

    /// The Open button's label for `id` (STORY-082 requirement 8): a local
    /// project, or a remote one on a connected host, reads "Open · Enter"
    /// even though a remote project never actually opens (`ui.rs` supplies
    /// the explaining hover text); any other remote host reads "connect the
    /// host first".
    pub fn open_label(&self, id: &ProjectId) -> &'static str {
        let connected = match id.host() {
            None => true,
            Some(host) => self.host_state(host) == HostState::Connected,
        };
        self.words.open_label(connected)
    }

    /// `id`'s branch label for a row or the inspector: the known branch, or
    /// "folder" for a project whose details came back without one — which is
    /// what "folder" means (STORY-082 requirement 5: "its branch, or
    /// 'folder' when it has none").
    ///
    /// `None` while nothing is known: a local project whose details have not
    /// been read yet (git runs only in the selected project, STORY-078), and
    /// a remote project, which never has details at all. The row then shows
    /// no branch rather than claiming the project has none.
    pub fn branch_label(&self, id: &ProjectId) -> Option<String> {
        let details = self.details_of(id)?;
        Some(
            details
                .branch
                .clone()
                .unwrap_or_else(|| self.words.branchless.to_owned()),
        )
    }

    /// Empties the search field, as Esc does (STORY-082 requirement 9/10).
    pub fn clear_search(&mut self) {
        self.search.clear();
    }

    /// Asks the search field to take focus (CMD mode) on the next frame.
    pub fn request_focus(&mut self) {
        self.focus_request = Some(true);
    }

    /// Asks the search field to give up focus (NAV mode) on the next frame.
    pub fn request_unfocus(&mut self) {
        self.focus_request = Some(false);
    }

    /// Reads and clears the focus request, so the view acts on each exactly
    /// once.
    pub fn take_focus_request(&mut self) -> Option<bool> {
        self.focus_request.take()
    }

    /// Selects the first (`false`) or last (`true`) row shown, as g and G do.
    /// Nothing while nothing is shown.
    pub fn select_end(&mut self, last: bool) {
        let order = self.visible_order();
        let end = if last { order.last() } else { order.first() };
        if let Some(id) = end.cloned() {
            self.selected = Some(id);
        }
    }

    /// Up to five of the most recently opened projects, most recent first,
    /// skipping an id no longer among those found (requirement 6), and
    /// narrowed by the current pick and search like every other row
    /// (STORY-082 requirement 4: picking a location must hide a recent
    /// project that is not on it, the same as it hides one from its group).
    pub fn recent_row(&self) -> Vec<&Project> {
        self.recent
            .iter()
            .filter_map(|id| self.project(id))
            .filter(|project| self.matches_pick(project) && self.matches_search(project))
            .take(5)
            .collect()
    }

    /// `project`'s path exactly as a row or the inspector shows it: the local
    /// home folder as `~`, or a remote path exactly as the settings file wrote
    /// it.
    pub fn path_of(&self, project: &Project) -> String {
        self.displayed_path(project)
    }

    /// Every project currently shown, one group per location (STORY-082
    /// requirement 5): a single "This Mac" group holding every local project,
    /// however many folders they come from, then one group per remote host,
    /// ordered by name (STORY-078 requirement 7's host order, unchanged).
    /// Each group is narrowed by the current pick and search (requirements 1
    /// and 8).
    ///
    /// Inside a group the rows follow the current [`Sort`]; by default local
    /// projects keep STORY-078's order — by parent folder, then by name — so
    /// the folders still read as blocks, and each row names its own folder
    /// through the path beside it.
    pub fn groups(&self) -> Vec<(String, Vec<&Project>)> {
        let mut local: Vec<&Project> = Vec::new();
        let mut remote: BTreeMap<&str, Vec<&Project>> = BTreeMap::new();

        for project in self.visible() {
            match &project.id {
                ProjectId::Local(_) => local.push(project),
                ProjectId::Remote { host, .. } => {
                    remote.entry(host.as_str()).or_default().push(project)
                }
            }
        }

        let mut groups = Vec::new();
        if !local.is_empty() {
            self.sort_rows(&mut local);
            groups.push((self.words.this_mac_label.to_owned(), local));
        }
        for (host, mut projects) in remote {
            self.sort_rows(&mut projects);
            groups.push((host.to_owned(), projects));
        }
        groups
    }

    /// Orders one group's rows by the current sort. `Dir` is the parent
    /// folder for a local project and just the name for a remote one (a
    /// remote host has no folders of ours); `Commit` and `Changed` have
    /// nothing to read for a remote project, so those fall back to the name.
    fn sort_rows(&self, rows: &mut [&Project]) {
        let time = |project: &Project, read: fn(&ProjectDetails) -> Option<i64>| {
            std::cmp::Reverse(self.details_of(&project.id).and_then(read))
        };
        match self.sort {
            Sort::Dir => rows.sort_by_key(|project| {
                let parent = project
                    .id
                    .local_path()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf);
                (parent, project.name.clone())
            }),
            Sort::Opened => rows.sort_by_key(|project| {
                let place = self.recent.iter().position(|id| id == &project.id);
                (place.unwrap_or(usize::MAX), project.name.clone())
            }),
            Sort::Commit => rows.sort_by_key(|project| {
                (
                    time(project, |details| details.committed),
                    project.name.clone(),
                )
            }),
            Sort::Changed => rows.sort_by_key(|project| {
                (
                    time(project, |details| details.changed),
                    project.name.clone(),
                )
            }),
        }
    }

    /// The folder `project` sits in, as a row's path shows it (`~/Projects`),
    /// for the view's folder subheading; `None` for a remote project.
    pub fn folder_of(&self, project: &Project) -> Option<String> {
        let path = project.id.local_path()?;
        Some(self.tilde(path.parent().unwrap_or(Path::new("/"))))
    }

    /// `path` as the engineer wrote it, with this machine's home folder shown
    /// as `~` (requirement 7). Used for a local project's displayed path —
    /// never for a remote one, whose path is shown exactly as the settings
    /// file wrote it, a different machine's folder.
    fn tilde(&self, path: &Path) -> String {
        let Some(home) = &self.home else {
            return path.display().to_string();
        };
        match path.strip_prefix(home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        }
    }

    /// `project`'s path as a row or the inspector shows it.
    fn displayed_path(&self, project: &Project) -> String {
        match &project.id {
            ProjectId::Local(path) => self.tilde(path),
            ProjectId::Remote { path, .. } => path.display().to_string(),
        }
    }

    /// The project `id` names, if it is still among those found.
    fn project(&self, id: &ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| &project.id == id)
    }

    /// Every project the current pick and search leave standing.
    fn visible(&self) -> Vec<&Project> {
        self.projects
            .iter()
            .filter(|project| self.matches_pick(project))
            .filter(|project| self.matches_search(project))
            .collect()
    }

    fn matches_pick(&self, project: &Project) -> bool {
        match &self.pick {
            Pick::All => true,
            Pick::Location(Location::ThisMac) => project.id.local_path().is_some(),
            Pick::Location(Location::Host(host)) => project.id.host() == Some(host.as_str()),
        }
    }

    fn matches_search(&self, project: &Project) -> bool {
        if self.search.is_empty() {
            return true;
        }
        let needle = self.search.to_lowercase();
        project.name.to_lowercase().contains(&needle)
            || self
                .displayed_path(project)
                .to_lowercase()
                .contains(&needle)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{Project, WorkspaceEvent};

    /// The name of every project every group of `launcher` currently shows,
    /// group by group flattened into one list, in the order `groups` returns
    /// them.
    fn group_names(launcher: &Launcher) -> Vec<String> {
        launcher
            .groups()
            .iter()
            .flat_map(|(_, projects)| projects.iter().map(|project| project.name.clone()))
            .collect()
    }

    fn group_labels(launcher: &Launcher) -> Vec<String> {
        launcher
            .groups()
            .iter()
            .map(|(label, _)| label.clone())
            .collect()
    }

    /// The arrows walk the group rows as drawn, top to bottom: a project in
    /// the recent row is stepped onto at its place in its group, never first.
    #[test]
    fn arrow_order_walks_the_group_rows_as_drawn() {
        let mut launcher = Launcher::new(None);
        let a = Project::at(PathBuf::from("/p/a"));
        let b = Project::at(PathBuf::from("/p/b"));
        let c = Project::at(PathBuf::from("/p/c"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            a.clone(),
            b.clone(),
            c.clone(),
        ]));
        launcher.opened(c.id.clone());

        let order = launcher.visible_order();
        assert_eq!(order, vec![a.id.clone(), b.id.clone(), c.id.clone()]);

        launcher.select(Some(a.id.clone()));
        launcher.move_selection(1);
        assert_eq!(launcher.selected(), Some(&b.id));
        launcher.move_selection(1);
        assert_eq!(launcher.selected(), Some(&c.id));
    }

    /// STORY-078, requirement 6: opening a project already in the recent row
    /// moves it to the front rather than listing it twice, and only the five
    /// most recently opened are kept.
    #[test]
    fn opening_again_moves_a_project_to_the_front_and_five_are_kept() {
        let mut launcher = Launcher::new(None);
        let projects: Vec<Project> = (0..6)
            .map(|n| Project::at(PathBuf::from(format!("/p/project-{n}"))))
            .collect();
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(projects.clone()));
        for project in &projects {
            launcher.opened(project.id.clone());
        }

        launcher.opened(projects[2].id.clone());

        let recent: Vec<String> = launcher
            .recent_row()
            .iter()
            .map(|project| project.name.clone())
            .collect();
        assert_eq!(recent.len(), 5, "{recent:?}");
        assert_eq!(
            recent[0], "project-2",
            "opening it again brings it to the front"
        );
        assert!(
            !recent.contains(&"project-0".to_owned()),
            "the oldest of six falls off five: {recent:?}"
        );
    }

    /// STORY-082, requirement 5: one group per location — every local
    /// project is in the single "This Mac" group however many folders they
    /// come from, then one group per host, ordered by name. Inside "This
    /// Mac" the projects keep STORY-078's order, by parent folder then by
    /// name.
    #[test]
    fn groups_are_this_mac_then_hosts() {
        let mut launcher = Launcher::new(None);
        // STORY-083: this STORY-082 test pins STORY-082's own plain text
        // ("This Mac"); theme words now default on, so it opts into plain
        // words explicitly, the same way the STORY-082 acceptance tests do.
        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            Project::at(PathBuf::from("/Users/engineer/scratch/tool")),
            Project::at(PathBuf::from("/Users/engineer/Projects/cellworks")),
            Project::remote("az.wsl".to_owned(), PathBuf::from("~/work/api")),
        ]));

        assert_eq!(
            group_labels(&launcher),
            vec!["This Mac".to_owned(), "az.wsl".to_owned()],
            "two local folders are one location, not two groups"
        );
        assert_eq!(
            group_names(&launcher),
            vec!["cellworks".to_owned(), "tool".to_owned(), "api".to_owned()],
            "~/Projects before ~/scratch, by parent folder then by name"
        );
    }

    /// STORY-078, requirement 7: the home folder is shown as `~` on a
    /// project's row, like the universe's own empty-state message already
    /// does for the projects folder — the folder a local project lives in is
    /// named there, now that the group heading names its location instead.
    #[test]
    fn home_is_shown_as_a_tilde() {
        let mut launcher = Launcher::new(Some(PathBuf::from("/Users/engineer")));
        let project = Project::at(PathBuf::from("/Users/engineer/Projects/cellworks"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![project.clone()]));

        assert_eq!(
            launcher.path_of(&project),
            "~/Projects/cellworks".to_owned()
        );
    }

    /// STORY-078, requirement 1 and the "This Mac" location: picking it
    /// leaves only the projects that are not on a remote host.
    #[test]
    fn picking_this_mac_hides_remote_projects() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            Project::at(PathBuf::from("/Users/engineer/Projects/cellworks")),
            Project::remote("az.wsl".to_owned(), PathBuf::from("~/work/api")),
        ]));

        launcher.pick(Pick::Location(Location::ThisMac));

        assert_eq!(group_names(&launcher), vec!["cellworks".to_owned()]);
    }

    /// STORY-078, requirement 8: a search matches the project's name or its
    /// displayed path, case-insensitively, whichever one it is found in.
    #[test]
    fn search_matches_the_name_or_the_path_ignoring_case() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            Project::at(PathBuf::from("/Users/engineer/Projects/cellworks")),
            Project::at(PathBuf::from("/Users/engineer/work/shop")),
        ]));

        launcher.search("SHOP".to_owned());
        assert_eq!(
            group_names(&launcher),
            vec!["shop".to_owned()],
            "matches the name, ignoring case"
        );

        launcher.search("ENGINEER/WORK".to_owned());
        assert_eq!(
            group_names(&launcher),
            vec!["shop".to_owned()],
            "matches the path, ignoring case"
        );
    }

    // STORY-082 below: the keyboard-first palette's view-model additions.
    // The fixture mirrors the design's Proof section: cellworks and
    // The-Factory on this Mac, factory-evals on the connected host "wsl",
    // surface-kit on the offline host "devbox".

    /// The STORY-082 Proof fixture, shared by the tests below: two local
    /// projects, one project on a connected host, one on an offline host.
    ///
    /// STORY-083: theme words default on now, but these tests pin STORY-082's
    /// own plain text ("This Mac"), so this fixture opts into plain words
    /// explicitly — the same way the STORY-082 acceptance tests do.
    fn four_projects() -> Launcher {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            Project::at(PathBuf::from("/Users/engineer/Projects/cellworks")),
            Project::at(PathBuf::from("/Users/engineer/Projects/The-Factory")),
            Project::remote("wsl".to_owned(), PathBuf::from("~/src/factory-evals")),
            Project::remote("devbox".to_owned(), PathBuf::from("/srv/surface-kit")),
        ]));
        launcher.apply(WorkspaceEvent::HostProbed {
            host: "wsl".to_owned(),
            connected: true,
        });
        launcher.apply(WorkspaceEvent::HostProbed {
            host: "devbox".to_owned(),
            connected: false,
        });
        launcher
    }

    /// STORY-082 requirement 6: the first case-insensitive match, as a byte
    /// range the view can underline.
    #[test]
    fn match_range_finds_the_first_case_insensitive_match() {
        assert_eq!(match_range("The-Factory", "fac"), Some(4..7));
        assert_eq!(match_range("The-Factory", "FAC"), Some(4..7));
    }

    /// STORY-082 requirement 6: no match and an empty query both give
    /// `None`; a name whose lowercase form changes byte length (a Unicode
    /// case-folding mismatch) also gives `None`, rather than panicking on a
    /// byte index that no longer lines up with `name`.
    #[test]
    fn match_range_gives_none_for_no_match_empty_query_or_a_length_changing_lowercase() {
        assert_eq!(match_range("cellworks", "zzz"), None, "no match");
        assert_eq!(
            match_range("cellworks", ""),
            None,
            "an empty query matches nothing"
        );
        // 'İ' (U+0130) lowercases to "i" followed by a combining dot above,
        // one byte longer than the original two-byte character.
        assert_eq!(
            match_range("İstanbul", "ist"),
            None,
            "a length-changing lowercase guards against a byte-index mismatch"
        );
    }

    /// STORY-082 requirement 2: "<k> matches" counts distinct projects
    /// currently visible, for the current pick and search together.
    #[test]
    fn match_count_counts_the_projects_currently_visible() {
        let mut launcher = four_projects();

        launcher.search("fac".to_owned());

        assert_eq!(launcher.match_count(), 3);
    }

    /// STORY-082 requirement 3 and ASSUMED (Q-2): chips read "All", "This
    /// Mac", then the hosts ordered by state (connected before offline)
    /// rather than by name — "devbox" would sort before "wsl"
    /// alphabetically, but the connected host must be reached first.
    #[test]
    fn location_chips_lists_all_this_mac_then_hosts_by_state_not_name() {
        let launcher = four_projects();

        let chips: Vec<(Pick, String, usize, Option<HostState>)> = launcher
            .location_chips()
            .iter()
            .map(|chip| {
                (
                    chip.pick.clone(),
                    chip.label.clone(),
                    chip.count,
                    chip.state,
                )
            })
            .collect();

        assert_eq!(
            chips,
            vec![
                (Pick::All, "All".to_owned(), 4, None),
                (
                    Pick::Location(Location::ThisMac),
                    "This Mac".to_owned(),
                    2,
                    None
                ),
                (
                    Pick::Location(Location::Host("wsl".to_owned())),
                    "wsl".to_owned(),
                    1,
                    Some(HostState::Connected)
                ),
                (
                    Pick::Location(Location::Host("devbox".to_owned())),
                    "devbox".to_owned(),
                    1,
                    Some(HostState::Offline)
                ),
            ]
        );
    }

    /// `location_chips`'s chip for the local location ("This Mac" or its
    /// themed name), wherever it sits in the row.
    fn this_mac_chip_label(launcher: &Launcher) -> String {
        launcher
            .location_chips()
            .into_iter()
            .find(|chip| chip.pick == Pick::Location(Location::ThisMac))
            .expect("a launcher with a local project has a This Mac chip")
            .label
    }

    /// STORY-083 requirements 6 to 8: a new launcher speaks theme words by
    /// default — the local location's chip reads "Local net" (themed; the
    /// view draws it upper case per requirement 10) — and switches to
    /// STORY-082's unchanged "This Mac" once `ThemeWordsTurnedOff` is
    /// applied. `can_open` is a behaviour, not a word, and does not move with
    /// the switch.
    #[test]
    fn theme_words_switch_the_local_location_s_name_without_touching_can_open() {
        let mut launcher = Launcher::new(None);
        let local = Project::at(PathBuf::from("/Users/engineer/Projects/cellworks"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![local.clone()]));

        assert!(
            this_mac_chip_label(&launcher)
                .to_lowercase()
                .contains("local net"),
            "theme words are on by default: {:?}",
            this_mac_chip_label(&launcher)
        );
        assert!(launcher.can_open(&local.id));

        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);

        assert_eq!(
            this_mac_chip_label(&launcher),
            "This Mac",
            "STORY-082's plain text, exactly, once theme words are off"
        );
        assert!(
            launcher.can_open(&local.id),
            "switching the words never touches what can be opened"
        );
    }

    /// STORY-082 Decision 3: chip counts are unfiltered totals, unchanged by
    /// a query.
    #[test]
    fn location_chips_counts_are_unchanged_by_a_search() {
        let mut launcher = four_projects();

        launcher.search("fac".to_owned());

        let counts: Vec<usize> = launcher
            .location_chips()
            .iter()
            .map(|chip| chip.count)
            .collect();
        assert_eq!(counts, vec![4, 2, 1, 1]);
    }

    /// STORY-082 requirement 4: Tab visits the chips in `location_chips`
    /// order — "This Mac" first, the one remote host second — and wraps from
    /// the last chip back to "All".
    #[test]
    fn cycle_pick_steps_through_this_mac_then_the_one_host_then_wraps() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            Project::at(PathBuf::from("/Users/engineer/Projects/cellworks")),
            Project::remote("wsl".to_owned(), PathBuf::from("~/src/factory-evals")),
        ]));
        launcher.apply(WorkspaceEvent::HostProbed {
            host: "wsl".to_owned(),
            connected: true,
        });
        assert_eq!(launcher.picked(), &Pick::All);

        launcher.cycle_pick();
        assert_eq!(launcher.picked(), &Pick::Location(Location::ThisMac));

        launcher.cycle_pick();
        assert_eq!(
            launcher.picked(),
            &Pick::Location(Location::Host("wsl".to_owned()))
        );

        launcher.cycle_pick();
        assert_eq!(
            launcher.picked(),
            &Pick::All,
            "the last chip wraps back to All"
        );
    }

    /// STORY-082 requirement 4's last sentence: changing the pick drops a
    /// selection that is no longer visible under it.
    #[test]
    fn cycle_pick_drops_a_selection_no_longer_visible() {
        let mut launcher = four_projects();
        launcher.pick(Pick::Location(Location::Host("wsl".to_owned())));
        let factory_evals = Project::remote("wsl".to_owned(), PathBuf::from("~/src/factory-evals"));
        launcher.select(Some(factory_evals.id.clone()));
        assert_eq!(launcher.selected(), Some(&factory_evals.id));

        // The next chip after "wsl" is "devbox" (STORY-082's Q-2 order),
        // which does not show a wsl project.
        launcher.cycle_pick();

        assert_eq!(
            launcher.selected(),
            None,
            "factory-evals is no longer visible once the pick moves off wsl"
        );
    }

    /// STORY-082 requirement 8, STORY-083 requirement 8: Open is enabled, and
    /// reads "Open project" (STORY-083's plain wording, opted into the same
    /// way the STORY-082 acceptance tests do, since theme words now default
    /// on), for a local project.
    #[test]
    fn can_open_and_open_label_for_a_local_project() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        let local = Project::at(PathBuf::from("/Users/engineer/Projects/cellworks"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![local.clone()]));

        assert!(launcher.can_open(&local.id));
        assert_eq!(launcher.open_label(&local.id), "Open project");
    }

    /// STORY-082 requirement 8, STORY-083 requirement 8: a remote project on
    /// a host that is not connected — offline, or never probed at all
    /// ("checking" or unknown) — cannot be opened, and the label asks to
    /// connect the host first (STORY-083's plain wording).
    #[test]
    fn can_open_and_open_label_for_a_remote_project_on_an_unconnected_host() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        let offline = Project::remote("devbox".to_owned(), PathBuf::from("/srv/surface-kit"));
        let unknown = Project::remote("ghost".to_owned(), PathBuf::from("/srv/unknown"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            offline.clone(),
            unknown.clone(),
        ]));
        launcher.apply(WorkspaceEvent::HostProbed {
            host: "devbox".to_owned(),
            connected: false,
        });
        // "ghost" is never probed at all, so its state defaults to checking.

        for id in [&offline.id, &unknown.id] {
            assert!(!launcher.can_open(id), "{id:?}");
            assert_eq!(launcher.open_label(id), "Connect the host first", "{id:?}");
        }
    }

    /// STORY-082 Decision 1: remote opening is not built this story, so Open
    /// stays disabled even on a connected host, though its label reads the
    /// same as a local project's — `ui.rs` adds its own hover text to
    /// explain why, which is not this function's job.
    #[test]
    fn can_open_is_false_for_a_remote_project_even_on_a_connected_host() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        let connected = Project::remote("wsl".to_owned(), PathBuf::from("~/src/factory-evals"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![connected.clone()]));
        launcher.apply(WorkspaceEvent::HostProbed {
            host: "wsl".to_owned(),
            connected: true,
        });

        assert!(!launcher.can_open(&connected.id));
        assert_eq!(launcher.open_label(&connected.id), "Open project");
    }

    /// STORY-082 requirement 5: "folder" means the project has no branch, so
    /// there is no label at all until its details have been read — a row
    /// whose branch is merely unknown says nothing rather than calling a
    /// repository a folder.
    #[test]
    fn branch_label_is_silent_until_details_arrive_then_names_the_branch() {
        let mut launcher = Launcher::new(None);
        let local = Project::at(PathBuf::from("/Users/engineer/Projects/cellworks"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![local.clone()]));

        assert_eq!(
            launcher.branch_label(&local.id),
            None,
            "nothing is known about the branch yet"
        );

        launcher.detailed(
            local.id.local_path().unwrap().to_path_buf(),
            ProjectDetails {
                branch: Some("main".to_owned()),
                files: Some(2),
                ..ProjectDetails::default()
            },
        );
        assert_eq!(launcher.branch_label(&local.id), Some("main".to_owned()));
    }

    /// STORY-082 requirement 5: a local project whose details came back
    /// without a branch is the one thing that reads "folder".
    #[test]
    fn branch_label_is_folder_for_a_project_read_without_a_branch() {
        let mut launcher = Launcher::new(None);
        // "folder" is STORY-082's plain word; theme words default on now, so
        // this test opts into plain words explicitly to pin it (THEMED's
        // branch-less word is "block").
        launcher.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        let local = Project::at(PathBuf::from("/Users/engineer/scratch/tool"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![local.clone()]));

        launcher.detailed(
            local.id.local_path().unwrap().to_path_buf(),
            ProjectDetails::default(),
        );

        assert_eq!(launcher.branch_label(&local.id), Some("folder".to_owned()));
    }

    /// STORY-082 requirement 5: a remote project never has details read for
    /// it, so its row shows no branch — it is not claimed to be a folder
    /// either.
    #[test]
    fn branch_label_is_silent_for_a_remote_project() {
        let mut launcher = Launcher::new(None);
        let remote = Project::remote("wsl".to_owned(), PathBuf::from("~/src/factory-evals"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![remote.clone()]));

        assert_eq!(launcher.branch_label(&remote.id), None);
    }

    /// STORY-082 requirement 7: the preview pane's location is where the
    /// project lives — this Mac, or the host's name — never the host's
    /// connection state, which the host's chip carries.
    #[test]
    fn location_of_is_this_mac_or_the_host_name() {
        let launcher = four_projects();
        let local = Project::at(PathBuf::from("/Users/engineer/Projects/cellworks"));
        let remote = Project::remote("wsl".to_owned(), PathBuf::from("~/src/factory-evals"));

        assert_eq!(launcher.location_of(&local.id), "This Mac");
        assert_eq!(
            launcher.location_of(&remote.id),
            "wsl",
            "the connected host is named, not its state"
        );
    }

    /// STORY-082 requirement 9's "Esc clear": clearing the search empties
    /// the query and lists every project again.
    #[test]
    fn clear_search_empties_the_query_and_lists_every_project() {
        let mut launcher = four_projects();
        launcher.search("fac".to_owned());
        assert_eq!(
            group_names(&launcher).len(),
            3,
            "the search narrowed the list"
        );

        launcher.clear_search();

        assert_eq!(launcher.search_text(), "");
        let mut names = group_names(&launcher);
        names.sort();
        let mut expected: Vec<String> =
            ["The-Factory", "cellworks", "factory-evals", "surface-kit"]
                .iter()
                .map(|name| (*name).to_owned())
                .collect();
        expected.sort();
        assert_eq!(names, expected, "every project is listed again");
    }

    /// STORY-083 requirement 8: the launcher opens in NAV (no focus request),
    /// and each request is read exactly once.
    #[test]
    fn take_focus_request_reads_each_request_once() {
        let mut launcher = Launcher::new(None);
        assert_eq!(launcher.take_focus_request(), None, "NAV at launch");

        launcher.request_focus();
        assert_eq!(launcher.take_focus_request(), Some(true));
        assert_eq!(launcher.take_focus_request(), None);

        launcher.request_unfocus();
        assert_eq!(launcher.take_focus_request(), Some(false));
        assert_eq!(launcher.take_focus_request(), None);
    }

    #[test]
    fn select_end_picks_the_first_and_last_row_shown() {
        let mut launcher = Launcher::new(None);
        launcher.select_end(true);
        assert!(
            launcher.selected().is_none(),
            "nothing shown, nothing picked"
        );

        let a = Project::at(PathBuf::from("/p/a"));
        let b = Project::at(PathBuf::from("/p/b"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            a.clone(),
            b.clone(),
        ]));
        launcher.select_end(true);
        assert_eq!(launcher.selected(), Some(&b.id));
        launcher.select_end(false);
        assert_eq!(launcher.selected(), Some(&a.id));
    }

    /// A launcher over `paths`, all local, with theme words as they are.
    fn local_launcher(paths: &[&str]) -> Launcher {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(
            paths
                .iter()
                .map(|p| Project::at(PathBuf::from(p)))
                .collect(),
        ));
        launcher
    }

    fn timed(launcher: &mut Launcher, path: &str, committed: Option<i64>, changed: Option<i64>) {
        launcher.detailed(
            PathBuf::from(path),
            ProjectDetails {
                committed,
                changed,
                ..ProjectDetails::default()
            },
        );
    }

    /// STORY-083 requirement 15: dir orders by parent folder, then name.
    #[test]
    fn dir_sort_orders_by_parent_folder_then_name() {
        let launcher = local_launcher(&["/b/x", "/a/z", "/a/y"]);

        assert_eq!(launcher.sort(), Sort::Dir);
        assert_eq!(group_names(&launcher), ["y", "z", "x"]);
    }

    /// Requirement 15: opened puts the recent list first, in its order, then
    /// the never-opened by name.
    #[test]
    fn opened_sort_follows_the_recent_list_then_names() {
        let mut launcher = local_launcher(&["/p/a", "/p/b", "/p/c", "/p/d"]);
        launcher.opened(Project::at(PathBuf::from("/p/c")).id);
        launcher.opened(Project::at(PathBuf::from("/p/d")).id);
        launcher.cycle_sort();

        assert_eq!(launcher.sort(), Sort::Opened);
        assert_eq!(group_names(&launcher), ["d", "c", "a", "b"]);
    }

    /// Requirement 15: commit puts the newest commit first and a project
    /// with no known commit time last, by name.
    #[test]
    fn commit_sort_puts_the_newest_commit_first_and_unknown_last() {
        let mut launcher = local_launcher(&["/p/a", "/p/b", "/p/c", "/p/d"]);
        timed(&mut launcher, "/p/b", Some(100), None);
        timed(&mut launcher, "/p/c", Some(200), None);
        timed(&mut launcher, "/p/d", None, None);
        launcher.cycle_sort();
        launcher.cycle_sort();

        assert_eq!(launcher.sort(), Sort::Commit);
        assert_eq!(group_names(&launcher), ["c", "b", "a", "d"]);
    }

    /// Requirement 15: changed puts the newest folder modification first and
    /// the unknown last, by name.
    #[test]
    fn changed_sort_puts_the_newest_modification_first_and_unknown_last() {
        let mut launcher = local_launcher(&["/p/a", "/p/b", "/p/c"]);
        timed(&mut launcher, "/p/a", None, Some(10));
        timed(&mut launcher, "/p/c", None, Some(20));
        for _ in 0..3 {
            launcher.cycle_sort();
        }

        assert_eq!(launcher.sort(), Sort::Changed);
        assert_eq!(group_names(&launcher), ["c", "a", "b"]);
    }

    /// Requirement 15: the sort cycles dir, opened, commit, changed and back,
    /// and its chip text is the same in either vocabulary.
    #[test]
    fn the_sort_cycles_and_reads_the_same_in_both_vocabularies() {
        let mut launcher = Launcher::new(None);
        let mut seen = Vec::new();
        for _ in 0..5 {
            seen.push(launcher.sort().label());
            launcher.cycle_sort();
        }
        assert_eq!(
            seen,
            [
                "Sort: dir",
                "Sort: opened",
                "Sort: commit",
                "Sort: changed",
                "Sort: dir"
            ]
        );
        let mut plain = Launcher::new(None);
        plain.apply(WorkspaceEvent::ThemeWordsTurnedOff);
        assert_eq!(plain.sort().label(), "Sort: dir");
    }

    /// Requirement 15: `visible_order` follows `groups` under every sort, so
    /// j and k still walk the rows as drawn.
    #[test]
    fn walking_the_rows_follows_the_current_sort() {
        let mut launcher = local_launcher(&["/p/a", "/p/b", "/p/c"]);
        timed(&mut launcher, "/p/c", Some(5), None);
        launcher.cycle_sort();
        launcher.cycle_sort();

        launcher.select_end(false);
        assert_eq!(launcher.selected_project().unwrap().name, "c");
        launcher.move_selection(1);
        assert_eq!(launcher.selected_project().unwrap().name, "a");
    }

    /// Requirement 14: a page move steps that many rows and stops at either
    /// end instead of wrapping.
    #[test]
    fn a_page_move_clamps_at_the_first_and_last_row() {
        let mut launcher = local_launcher(&["/p/a", "/p/b", "/p/c", "/p/d", "/p/e"]);

        launcher.move_selection_clamped(3);
        assert_eq!(launcher.selected_project().unwrap().name, "a");
        launcher.move_selection_clamped(3);
        assert_eq!(launcher.selected_project().unwrap().name, "d");
        launcher.move_selection_clamped(3);
        assert_eq!(launcher.selected_project().unwrap().name, "e");
        launcher.move_selection_clamped(-3);
        assert_eq!(launcher.selected_project().unwrap().name, "b");
        launcher.move_selection_clamped(-3);
        assert_eq!(launcher.selected_project().unwrap().name, "a");
    }

    /// Requirement 16: a search naming a folder lists every project in it,
    /// because search matches the displayed path.
    #[test]
    fn a_folder_search_lists_the_whole_folder() {
        let mut launcher = local_launcher(&["/h/Projects/a", "/h/Projects/b", "/h/Other/c"]);
        launcher.search("projects".to_owned());

        assert_eq!(group_names(&launcher), ["a", "b"]);
        let projects = launcher.groups().remove(0).1;
        assert_eq!(
            launcher.folder_of(projects[0]),
            Some("/h/Projects".to_owned())
        );
    }
}
