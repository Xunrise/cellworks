//! The Workspace bounded context: the projects Cellworks knows, their
//! identity, the settings file that adds a folder by hand, and the universe
//! view that shows them all.
//!
//! Defined by docs/domain/contexts/Workspace/Workspace.md. [`scan`] is an
//! ordinary blocking function and [`start_scan`] is the thread around it, so
//! every rule of discovery can be tested without a thread, a channel or a
//! frame. Where the projects come from is a [`Sources`] the caller passes in;
//! [`Sources::for_this_user`] is the one function here that reads the
//! environment, which is what keeps every test away from the user's home
//! directory.

mod arrangement;
mod discovery;
mod event;
mod hosts;
mod launcher;
mod marks;
mod project;
mod recent;
mod registry;
mod settings;
pub mod ui;

pub use marks::{mark, marked};
pub mod theme;
mod universe;
mod words;

#[cfg(test)]
mod test_tree;

use std::path::PathBuf;
use std::sync::mpsc;

pub use arrangement::{Arrangement, ArrangementKeeper, ArrangementStore};
pub use event::WorkspaceEvent;
pub use hosts::{HostProbe, SshProbe, start_probes};
pub use launcher::{HostState, Launcher, Location, Pick, ProjectDetails, ProjectKind, Sort};
pub use project::{Project, ProjectId};
pub use recent::{RecentKeeper, RecentStore};
pub use universe::{Notice, NoticeKind, ScanState, Universe};

use settings::Platform;

/// Where the projects come from.
///
/// The binary fills this from `$HOME`; every test fills it from a temporary
/// directory, which is what keeps "no test reads the user's home directory"
/// true by construction rather than by remembering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sources {
    /// The folder walked for git repositories.
    pub projects_folder: PathBuf,
    /// The file read for folders added by hand. Never written.
    pub settings_file: PathBuf,
}

impl Sources {
    /// `$HOME/Projects` and the settings file of this platform.
    ///
    /// `None` when `$HOME` is unset or empty: there is no home directory to
    /// take either path from, and a relative projects folder would walk
    /// whatever the working directory happens to be.
    pub fn for_this_user() -> Option<Sources> {
        let home = PathBuf::from(non_empty("HOME")?);
        let xdg_config_home = non_empty("XDG_CONFIG_HOME").map(PathBuf::from);

        // Where a settings file lives genuinely differs per platform. This is
        // the only `cfg!(target_os)` in the context, and it decides a
        // directory — never how two paths compare.
        let platform = if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Xdg
        };

        Some(Sources {
            projects_folder: home.join("Projects"),
            settings_file: settings::settings_file(platform, &home, xdg_config_home.as_deref()),
        })
    }
}

fn non_empty(variable: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(variable).filter(|value| !value.is_empty())
}

/// Walks, reads and merges, and says what it found.
///
/// Blocking: called on the background thread [`start_scan`] spawns, and
/// directly from a test. Exactly one [`WorkspaceEvent::ProjectsDiscovered`] is
/// returned, last, after any failure the scan met on the way.
pub fn scan(sources: &Sources) -> Vec<WorkspaceEvent> {
    let discovered = discovery::discover(&sources.projects_folder);
    let added = settings::read(&sources.settings_file);
    let theme_words_off = added.theme_words_off;
    let (mut projects, unidentified) = registry::merge(&discovered.folders, &added.folders);
    registry::attach_district_prefixes(&mut projects, &added.district_prefixes);
    registry::attach_contexts(&mut projects, &added.contexts);

    let mut events = discovered.events;
    events.extend(added.events);
    events.extend(unidentified);

    let recent = RecentStore::beside(&sources.settings_file).load();
    if !recent.is_empty() {
        events.push(WorkspaceEvent::RecentRead(recent));
    }

    for (host, paths) in added.remote {
        for path in paths {
            projects.push(Project::remote(host.clone(), path));
        }
    }

    if theme_words_off {
        events.push(WorkspaceEvent::ThemeWordsTurnedOff);
    }

    events.push(WorkspaceEvent::ProjectsDiscovered(projects));
    events
}

/// Runs [`scan`] on a `std::thread` and hands back the receiving end.
///
/// A send that fails is the UI having gone away; the thread ends quietly,
/// because there is no one left to report it to.
pub fn start_scan(sources: Sources) -> mpsc::Receiver<WorkspaceEvent> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        for event in scan(&sources) {
            if sender.send(event).is_err() {
                return;
            }
        }
    });

    receiver
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_tree::{TestTree, canonical};
    use std::fs;

    /// The project names `scan` reported, in the order it reported them.
    fn universe_of(sources: &Sources) -> Vec<String> {
        projects_of(&scan(sources))
            .iter()
            .map(|project| project.name.clone())
            .collect()
    }

    fn projects_of(events: &[WorkspaceEvent]) -> Vec<Project> {
        let reported: Vec<&Vec<Project>> = events
            .iter()
            .filter_map(|event| match event {
                WorkspaceEvent::ProjectsDiscovered(projects) => Some(projects),
                _ => None,
            })
            .collect();

        assert_eq!(reported.len(), 1, "a scan reports exactly once: {events:?}");
        reported[0].clone()
    }

    fn settings_naming(tree: &TestTree, folders: &[&std::path::Path]) {
        let entries: Vec<String> = folders
            .iter()
            .map(|folder| format!("  \"{}\",\n", folder.display()))
            .collect();
        tree.file(
            "settings.toml",
            &format!("added_folders = [\n{}]\n", entries.concat()),
        );
    }

    #[test]
    fn a_folder_the_settings_file_adds_is_a_project_wherever_it_lives() {
        let tree = TestTree::new();
        tree.repository("Projects/alpha");
        let infra = tree.plain_folder("work/infra");
        settings_naming(&tree, &[&infra]);

        assert_eq!(universe_of(&tree.sources()), vec!["alpha", "infra"]);
    }

    #[test]
    fn a_folder_both_discovered_and_added_is_exactly_one_project() {
        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        settings_naming(&tree, &[&alpha]);

        let projects = projects_of(&scan(&tree.sources()));

        assert_eq!(projects.len(), 1, "{projects:?}");
        assert_eq!(
            projects[0].id.local_path(),
            Some(canonical(&alpha).as_path())
        );
    }

    #[test]
    fn an_unreadable_projects_folder_still_reports_the_added_folders() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TestTree::new();
        let projects = tree.plain_folder("Projects");
        let infra = tree.plain_folder("work/infra");
        settings_naming(&tree, &[&infra]);
        fs::set_permissions(&projects, fs::Permissions::from_mode(0o000))
            .expect("take the permissions off the projects folder");

        let readable_anyway = fs::read_dir(&projects).is_ok();
        let events = scan(&tree.sources());

        fs::set_permissions(&projects, fs::Permissions::from_mode(0o755))
            .expect("give the permissions back, or the temporary directory cannot be removed");
        assert!(
            !readable_anyway,
            "this test needs a non-root user: mode 0o000 did not stop the read"
        );

        assert_eq!(
            projects_of(&events)
                .iter()
                .map(|project| project.name.clone())
                .collect::<Vec<String>>(),
            vec!["infra"]
        );
        assert!(
            events.iter().any(|event| matches!(
                event,
                WorkspaceEvent::ProjectsFolderUnreadable { path, .. } if path == &projects
            )),
            "the unreadable projects folder is reported too: {events:?}"
        );
    }

    #[test]
    fn a_shuffled_tree_comes_back_sorted_by_canonical_path() {
        let tree = TestTree::new();
        tree.repository("Projects/zulu");
        tree.repository("Projects/alpha");
        tree.repository("Projects/mike");

        assert_eq!(universe_of(&tree.sources()), vec!["alpha", "mike", "zulu"]);
    }

    #[test]
    fn an_absent_projects_folder_and_an_absent_settings_file_report_an_empty_universe() {
        let tree = TestTree::new();

        let events = scan(&tree.sources());

        assert_eq!(projects_of(&events), Vec::new());
        assert_eq!(events.len(), 1, "and nothing is reported as a failure");
    }

    #[test]
    fn the_settings_file_is_byte_identical_after_a_scan() {
        let tree = TestTree::new();
        let settings = tree.file(
            "settings.toml",
            "theme = \"dark\"\nadded_folders = [\"/nowhere/at/all\", 17]\n",
        );
        let before = fs::read(&settings).expect("the settings file is readable");

        scan(&tree.sources());

        assert_eq!(
            fs::read(&settings).expect("the settings file is still readable"),
            before,
            "this story never writes the settings file"
        );
    }

    /// STORY-078, requirements 2 and 3: a remote project from the `[hosts]`
    /// table is appended after the projects found on this Mac, and reading it
    /// never writes the settings file back — it stays the engineer's,
    /// hand-edited.
    #[test]
    fn remote_projects_follow_the_local_ones_and_the_settings_file_is_untouched() {
        let tree = TestTree::new();
        tree.repository("Projects/alpha");
        let settings = tree.file(
            "settings.toml",
            "[hosts.\"az.wsl\"]\nprojects = [\"~/work/api\"]\n",
        );
        let before = fs::read(&settings).expect("the settings file is readable");

        let projects = projects_of(&scan(&tree.sources()));

        assert_eq!(
            projects
                .iter()
                .map(|project| project.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "api"],
            "the remote project follows the local one: {projects:?}"
        );
        assert_eq!(projects[1].id.local_path(), None, "{:?}", projects[1].id);
        assert_eq!(
            projects[1].id.host(),
            Some("az.wsl"),
            "{:?}",
            projects[1].id
        );
        assert_eq!(
            fs::read(&settings).expect("the settings file is still readable"),
            before,
            "this story never writes the settings file"
        );
    }

    /// STORY-083, requirement 6: `scan` reports `ThemeWordsTurnedOff` exactly
    /// once when the settings file switches theme words off — and, per
    /// `projects_of`'s own assertion above, never at the cost of reporting
    /// `ProjectsDiscovered` more or less than once.
    #[test]
    fn scan_reports_theme_words_turned_off_when_the_setting_is_off() {
        let tree = TestTree::new();
        tree.file("settings.toml", "[launcher]\ntheme_words = false\n");

        let events = scan(&tree.sources());

        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, WorkspaceEvent::ThemeWordsTurnedOff))
                .count(),
            1,
            "exactly one ThemeWordsTurnedOff when the setting is off: {events:?}"
        );
        // Still reports exactly once, unaffected by the new event.
        projects_of(&events);
    }

    /// STORY-083, requirement 6: a settings file without the key, and no
    /// settings file at all, both leave theme words on — `scan` never reports
    /// `ThemeWordsTurnedOff` for either.
    #[test]
    fn scan_never_reports_theme_words_turned_off_without_the_setting() {
        let without_key = TestTree::new();
        without_key.file("settings.toml", "added_folders = []\n");
        let events = scan(&without_key.sources());
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, WorkspaceEvent::ThemeWordsTurnedOff)),
            "a settings file without [launcher]: {events:?}"
        );

        let without_file = TestTree::new();
        let events = scan(&without_file.sources());
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, WorkspaceEvent::ThemeWordsTurnedOff)),
            "no settings file at all: {events:?}"
        );
    }

    #[test]
    fn start_scan_reports_over_the_channel_without_the_caller_blocking() {
        let tree = TestTree::new();
        tree.repository("Projects/alpha");

        let events = start_scan(tree.sources());
        let reported = events
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the scan thread reports");

        assert_eq!(
            reported,
            WorkspaceEvent::ProjectsDiscovered(vec![Project::at(canonical(
                &tree.path().join("Projects/alpha")
            ))])
        );
    }
}
