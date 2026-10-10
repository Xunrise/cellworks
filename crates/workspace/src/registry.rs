//! Merging the two sources into one deterministic project list.
//!
//! This is the one place where being wrong would be silent and
//! platform-shaped. The map is keyed on the filesystem's own folder identity,
//! never on a path string and never on a folded one: on a case-insensitive
//! macOS volume two spellings are one inode and so one project, and on a
//! case-sensitive Linux filesystem they are two directories and stay two.
//!
//! Insertion follows a fixed precedence — everything discovered under the
//! projects folder first, then everything the settings file added, each group
//! sorted by canonical path — so the universe is the same whichever order the
//! sources happened to be read in. The result is sorted by canonical path.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::event::WorkspaceEvent;
use crate::project::{FolderIdentity, Project};

/// Which source a path came from, and therefore which event it is reported
/// under when the filesystem will not say what folder it is.
#[derive(Clone, Copy)]
enum Source {
    ProjectsFolder,
    Settings,
}

impl Source {
    fn unreadable(self, path: PathBuf, reason: String) -> WorkspaceEvent {
        match self {
            Source::ProjectsFolder => WorkspaceEvent::ProjectsFolderUnreadable { path, reason },
            Source::Settings => WorkspaceEvent::AddedFolderSkipped { path, reason },
        }
    }
}

/// The universe the two sources add up to, sorted by canonical path, plus one
/// event for every path the filesystem would not identify.
pub(crate) fn merge(
    discovered: &[PathBuf],
    added: &[PathBuf],
) -> (Vec<Project>, Vec<WorkspaceEvent>) {
    let mut by_folder: HashMap<FolderIdentity, Project> = HashMap::new();
    let mut events = Vec::new();

    for (paths, source) in [
        (discovered, Source::ProjectsFolder),
        (added, Source::Settings),
    ] {
        let mut paths = paths.to_vec();
        paths.sort();

        for path in paths {
            match FolderIdentity::of(&path) {
                // First insertion wins: the precedence above is the tiebreak.
                Ok(folder) => {
                    by_folder.entry(folder).or_insert_with(|| Project::at(path));
                }
                Err(error) => events.push(source.unreadable(path, error.to_string())),
            }
        }
    }

    let mut projects: Vec<Project> = by_folder.into_values().collect();
    projects.sort_by(|one, other| one.id.path().cmp(other.id.path()));

    (projects, events)
}

/// Gives each project the district prefix its settings name. A setting for a
/// folder that is not a project is ignored.
pub(crate) fn attach_district_prefixes(projects: &mut [Project], prefixes: &[(PathBuf, String)]) {
    for (path, prefix) in prefixes {
        let Ok(folder) = FolderIdentity::of(path) else {
            continue;
        };
        for project in projects.iter_mut() {
            if FolderIdentity::of(project.id.path()).is_ok_and(|other| other == folder) {
                project.district_prefix = Some(prefix.clone());
            }
        }
    }
}

/// Gives each project the bounded contexts its settings map to folders.
pub(crate) fn attach_contexts(
    projects: &mut [Project],
    contexts: &[(PathBuf, std::collections::BTreeMap<String, Vec<PathBuf>>)],
) {
    for (path, folders) in contexts {
        let Ok(folder) = FolderIdentity::of(path) else {
            continue;
        };
        for project in projects.iter_mut() {
            if FolderIdentity::of(project.id.path()).is_ok_and(|other| other == folder) {
                project.contexts = folders.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_tree::{TestTree, canonical};
    use std::slice::from_ref;

    fn names(projects: &[Project]) -> Vec<&str> {
        projects
            .iter()
            .map(|project| project.name.as_str())
            .collect()
    }

    #[test]
    fn one_folder_reached_from_both_sources_is_one_project() {
        let tree = TestTree::new();
        let alpha = canonical(&tree.repository("Projects/alpha"));

        let (projects, events) = merge(from_ref(&alpha), from_ref(&alpha));

        assert_eq!(projects, vec![Project::at(alpha)]);
        assert!(events.is_empty());
    }

    #[test]
    fn two_folders_the_filesystem_calls_different_stay_two_projects() {
        let tree = TestTree::new();
        let alpha = canonical(&tree.repository("Projects/alpha"));
        let beta = canonical(&tree.repository("Projects/beta"));

        let (projects, _) = merge(&[alpha, beta], &[]);

        assert_eq!(names(&projects), vec!["alpha", "beta"]);
    }

    #[test]
    fn the_universe_is_the_same_whichever_order_the_sources_were_read_in() {
        let tree = TestTree::new();
        let alpha = canonical(&tree.repository("Projects/alpha"));
        let beta = canonical(&tree.repository("Projects/beta"));
        let infra = canonical(&tree.plain_folder("work/infra"));

        let (one, _) = merge(&[alpha.clone(), beta.clone()], from_ref(&infra));
        let (other, _) = merge(&[beta, alpha], &[infra]);

        assert_eq!(one, other);
    }

    #[test]
    fn the_universe_is_sorted_by_canonical_path() {
        let tree = TestTree::new();
        let zulu = canonical(&tree.repository("Projects/zulu"));
        let mike = canonical(&tree.repository("Projects/mike"));
        let alpha = canonical(&tree.repository("Projects/alpha"));

        let (projects, _) = merge(&[zulu, mike, alpha], &[]);

        assert_eq!(names(&projects), vec!["alpha", "mike", "zulu"]);
    }

    #[test]
    fn a_path_the_filesystem_will_not_identify_is_reported_rather_than_unwrapped() {
        let tree = TestTree::new();
        let gone = tree.path().join("Projects/gone");

        let (projects, events) = merge(from_ref(&gone), &[]);

        assert!(projects.is_empty());
        assert!(
            matches!(
                events.as_slice(),
                [WorkspaceEvent::ProjectsFolderUnreadable { path, .. }] if path == &gone
            ),
            "{events:?}"
        );
    }

    #[test]
    fn an_added_folder_the_filesystem_will_not_identify_is_reported_as_skipped() {
        let tree = TestTree::new();
        let gone = tree.path().join("work/gone");

        let (projects, events) = merge(&[], from_ref(&gone));

        assert!(projects.is_empty());
        assert!(
            matches!(
                events.as_slice(),
                [WorkspaceEvent::AddedFolderSkipped { path, .. }] if path == &gone
            ),
            "{events:?}"
        );
    }

    #[test]
    fn a_district_prefix_reaches_the_project_it_names_and_no_other() {
        let tree = TestTree::new();
        let alpha = canonical(&tree.repository("Projects/alpha"));
        let beta = canonical(&tree.repository("Projects/beta"));
        let (mut projects, _) = merge(&[alpha.clone(), beta], &[]);

        attach_district_prefixes(&mut projects, &[(alpha, "none".to_owned())]);

        assert_eq!(projects[0].district_prefix.as_deref(), Some("none"));
        assert_eq!(projects[1].district_prefix, None);
    }

    /// Letter case follows the filesystem, so this test asks the filesystem
    /// first and then asserts. It is meaningful on the macOS runner and on the
    /// Linux runner, and it fails the moment a case rule is written into the
    /// code.
    #[test]
    fn letter_case_follows_the_filesystem_rather_than_a_rule_in_the_code() {
        let tree = TestTree::new();
        let upper = tree.repository("Projects/Alpha");
        let lower = tree.path().join("Projects/alpha");

        let volume_is_case_insensitive = lower.is_dir();
        let spellings: Vec<PathBuf> = if volume_is_case_insensitive {
            // Both spellings canonicalise on this volume; feed both in.
            vec![canonical(&upper), canonical(&lower)]
        } else {
            vec![
                canonical(&upper),
                canonical(&tree.repository("Projects/alpha")),
            ]
        };

        let (projects, _) = merge(&spellings, &[]);

        if volume_is_case_insensitive {
            assert_eq!(
                projects.len(),
                1,
                "the volume is case-insensitive, so Alpha and alpha are one project: {projects:?}"
            );
        } else {
            assert_eq!(
                projects.len(),
                2,
                "the volume is case-sensitive, so Alpha and alpha are two projects: {projects:?}"
            );
        }
    }
}
