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
/// colour alone (requirement 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Checking,
    Connected,
    Offline,
}

impl std::fmt::Display for HostState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostState::Checking => write!(f, "◌ checking"),
            HostState::Connected => write!(f, "● connected"),
            HostState::Offline => write!(f, "○ offline"),
        }
    }
}

/// What the inspector calls a project (requirement 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectKind {
    Repository,
    Folder,
    Remote,
}

/// A local project's branch and file count, read off the frame loop; each
/// `None` until it is known, and a plain folder's `files` stays `None`
/// rather than becoming zero (requirement 9).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectDetails {
    pub branch: Option<String>,
    pub files: Option<usize>,
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
    selected: Option<ProjectId>,
    recent: Vec<ProjectId>,
    hosts: HashMap<String, HostState>,
    details: HashMap<PathBuf, ProjectDetails>,
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
            selected: None,
            recent: Vec::new(),
            hosts: HashMap::new(),
            details: HashMap::new(),
        }
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

    /// Narrows the main area to `pick`.
    pub fn pick(&mut self, pick: Pick) {
        self.pick = pick;
    }

    /// What the main area is currently narrowed to.
    pub fn picked(&self) -> &Pick {
        &self.pick
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

    /// Moves the selection `by` rows through the recent row followed by every
    /// group, wrapping at either end (requirement 11's ↑/↓). Nothing while
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

    /// Every id shown, the recent row first then every group, in the order
    /// the view draws them: what `move_selection` steps through. A project
    /// shown twice (recent and in its group) is listed once, at its first
    /// place, so each keypress moves to a different row.
    fn visible_order(&self) -> Vec<ProjectId> {
        let mut seen = BTreeSet::new();
        self.recent_row()
            .into_iter()
            .map(|project| project.id.clone())
            .chain(
                self.groups().into_iter().flat_map(|(_, projects)| {
                    projects.into_iter().map(|project| project.id.clone())
                }),
            )
            .filter(|id| seen.insert(id.clone()))
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

    /// Records `project`'s branch and file count, read off the frame loop.
    pub fn detailed(&mut self, project: PathBuf, branch: Option<String>, files: Option<usize>) {
        self.details
            .insert(project, ProjectDetails { branch, files });
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

    /// `id`'s state, shown on every row: "this Mac" for a local project, or
    /// its host's state (requirement 4's last sentence).
    pub fn state_of(&self, id: &ProjectId) -> String {
        match id {
            ProjectId::Local(_) => "this Mac".to_owned(),
            ProjectId::Remote { host, .. } => self
                .hosts
                .get(host)
                .copied()
                .unwrap_or(HostState::Checking)
                .to_string(),
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

    /// Up to five of the most recently opened projects, most recent first,
    /// skipping an id no longer among those found (requirement 6).
    pub fn recent_row(&self) -> Vec<&Project> {
        self.recent
            .iter()
            .filter_map(|id| self.project(id))
            .take(5)
            .collect()
    }

    /// `project`'s path exactly as a row or the inspector shows it: the local
    /// home folder as `~`, or a remote path exactly as the settings file wrote
    /// it.
    pub fn path_of(&self, project: &Project) -> String {
        self.displayed_path(project)
    }

    /// Every project currently shown, grouped by its parent folder (the home
    /// folder shown as `~`) or, for a remote project, by its host: local
    /// groups first, ordered by path, then hosts, ordered by name
    /// (requirement 7), each narrowed by the current pick and search
    /// (requirements 1 and 8).
    pub fn groups(&self) -> Vec<(String, Vec<&Project>)> {
        let mut local: BTreeMap<PathBuf, Vec<&Project>> = BTreeMap::new();
        let mut remote: BTreeMap<&str, Vec<&Project>> = BTreeMap::new();

        for project in self.visible() {
            match &project.id {
                ProjectId::Local(path) => {
                    let parent = path.parent().unwrap_or(Path::new("/")).to_path_buf();
                    local.entry(parent).or_default().push(project);
                }
                ProjectId::Remote { host, .. } => {
                    remote.entry(host.as_str()).or_default().push(project)
                }
            }
        }

        let mut groups = Vec::new();
        for (parent, mut projects) in local {
            projects.sort_by(|one, other| one.name.cmp(&other.name));
            groups.push((self.tilde(&parent), projects));
        }
        for (host, mut projects) in remote {
            projects.sort_by(|one, other| one.name.cmp(&other.name));
            groups.push((host.to_owned(), projects));
        }
        groups
    }

    /// `path` as the engineer wrote it, with this machine's home folder shown
    /// as `~` (requirement 7). Used for a local project's group label and its
    /// displayed path alike — never for a remote one, whose path is shown
    /// exactly as the settings file wrote it, a different machine's folder.
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

    /// STORY-078, requirement 11: a project in both the recent row and its
    /// group is stepped through once, so ↓/↑ never spend a keypress on a
    /// duplicate.
    #[test]
    fn arrow_order_lists_each_project_once() {
        let mut launcher = Launcher::new(None);
        let a = Project::at(PathBuf::from("/p/a"));
        let b = Project::at(PathBuf::from("/p/b"));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            a.clone(),
            b.clone(),
        ]));
        launcher.opened(a.id.clone());

        let order = launcher.visible_order();
        assert_eq!(order, vec![a.id.clone(), b.id.clone()]);

        launcher.select(Some(b.id.clone()));
        launcher.move_selection(1);
        assert_eq!(launcher.selected(), Some(&a.id));
        launcher.move_selection(1);
        assert_eq!(launcher.selected(), Some(&b.id));
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

    /// STORY-078, requirement 7: a local project's group is its parent
    /// folder, a remote project's its host; the local groups come first,
    /// ordered by path, then the hosts, ordered by name.
    #[test]
    fn groups_are_parent_folders_then_hosts() {
        let mut launcher = Launcher::new(None);
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![
            Project::at(PathBuf::from("/Users/engineer/scratch/tool")),
            Project::at(PathBuf::from("/Users/engineer/Projects/cellworks")),
            Project::remote("az.wsl".to_owned(), PathBuf::from("~/work/api")),
        ]));

        assert_eq!(
            group_labels(&launcher),
            vec![
                "/Users/engineer/Projects".to_owned(),
                "/Users/engineer/scratch".to_owned(),
                "az.wsl".to_owned(),
            ]
        );
    }

    /// STORY-078, requirement 7: the home folder is shown as `~`, like the
    /// universe's own empty-state message already does for the projects
    /// folder.
    #[test]
    fn home_is_shown_as_a_tilde() {
        let mut launcher = Launcher::new(Some(PathBuf::from("/Users/engineer")));
        launcher.apply(WorkspaceEvent::ProjectsDiscovered(vec![Project::at(
            PathBuf::from("/Users/engineer/Projects/cellworks"),
        )]));

        assert_eq!(group_labels(&launcher), vec!["~/Projects".to_owned()]);
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
}
