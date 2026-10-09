//! The walk under the projects folder.
//!
//! Two stopping rules decide the result and keep the walk cheap: it stops at
//! the first `.git`, so it never descends into a repository's `node_modules`,
//! `target` or `vendor`, and it skips entries whose name begins with `.`, so
//! caches below the projects folder cost nothing. No depth cap is imposed —
//! a cap would silently hide a project.
//!
//! `DirEntry::file_type` is used rather than `metadata`, because it does not
//! follow a symlink; that is what makes "symlinks are not followed" one
//! decision rather than two, and it is also why no symlink loop can hang the
//! scan.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::event::WorkspaceEvent;

/// What one walk found, and what it could not read on the way.
#[derive(Debug, Default)]
pub(crate) struct Discovered {
    /// The canonical path of every repository found.
    pub(crate) folders: Vec<PathBuf>,
    /// One event per directory the walk had to skip.
    pub(crate) events: Vec<WorkspaceEvent>,
}

/// Walks `projects_folder` and returns the git repositories under it.
///
/// A projects folder that does not exist contributes nothing and is not an
/// error — that is the normal state of a fresh machine. A directory that
/// cannot be read is skipped and reported, and the rest of the walk continues.
pub(crate) fn discover(projects_folder: &Path) -> Discovered {
    let mut found = Discovered::default();

    if matches!(projects_folder.try_exists(), Ok(false)) {
        return found;
    }

    let mut pending = vec![projects_folder.to_path_buf()];
    while let Some(directory) = pending.pop() {
        match read(&directory) {
            Ok(Contents::Repository) => match fs::canonicalize(&directory) {
                Ok(canonical) => found.folders.push(canonical),
                Err(error) => found.events.push(unreadable(&directory, &error)),
            },
            Ok(Contents::Subdirectories(subdirectories)) => pending.extend(subdirectories),
            Err(error) => found.events.push(unreadable(&directory, &error)),
        }
    }

    found
}

/// What one directory turned out to be.
enum Contents {
    /// It directly contains an entry named `.git`, of either kind: a git
    /// repository, and the walk does not descend into it.
    Repository,
    /// It is not a repository; these are the directories below it that the
    /// walk still has to visit.
    Subdirectories(Vec<PathBuf>),
}

fn read(directory: &Path) -> std::io::Result<Contents> {
    let mut subdirectories = Vec::new();

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();

        if name == OsStr::new(".git") {
            return Ok(Contents::Repository);
        }
        if name.as_encoded_bytes().first() == Some(&b'.') {
            continue;
        }

        // `file_type` rather than `metadata`: it does not follow the link, so
        // a symlink is skipped here instead of being walked into.
        if entry.file_type()?.is_dir() {
            subdirectories.push(entry.path());
        }
    }

    Ok(Contents::Subdirectories(subdirectories))
}

fn unreadable(path: &Path, error: &std::io::Error) -> WorkspaceEvent {
    WorkspaceEvent::ProjectsFolderUnreadable {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_tree::{TestTree, canonical};

    /// The canonical paths `discover` found, sorted, so a test can state them.
    fn folders_under(tree: &TestTree) -> Vec<PathBuf> {
        let mut found = discover(&tree.sources().projects_folder);
        found.folders.sort();
        assert!(
            found.events.is_empty(),
            "the walk reported {:?}",
            found.events
        );
        found.folders
    }

    #[test]
    fn every_repository_under_the_projects_folder_is_found() {
        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        let beta = tree.repository("Projects/beta");

        assert_eq!(
            folders_under(&tree),
            vec![canonical(&alpha), canonical(&beta)]
        );
    }

    #[test]
    fn a_repository_whose_git_entry_is_a_file_is_found() {
        let tree = TestTree::new();
        let linked = tree.worktree_repository("Projects/linked");

        assert_eq!(folders_under(&tree), vec![canonical(&linked)]);
    }

    #[test]
    fn a_repository_nested_in_a_repository_is_not_a_second_project() {
        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        tree.repository("Projects/alpha/vendor/inner");

        assert_eq!(folders_under(&tree), vec![canonical(&alpha)]);
    }

    #[test]
    fn a_folder_that_is_not_a_repository_is_not_a_project() {
        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        tree.plain_folder("Projects/notes");

        assert_eq!(folders_under(&tree), vec![canonical(&alpha)]);
    }

    #[test]
    fn a_repository_below_a_plain_folder_is_found() {
        let tree = TestTree::new();
        let service = tree.repository("Projects/acme/service");

        assert_eq!(folders_under(&tree), vec![canonical(&service)]);
    }

    #[test]
    fn a_repository_inside_a_dotted_folder_is_not_a_project() {
        let tree = TestTree::new();
        tree.repository("Projects/.cache/alpha");

        assert_eq!(folders_under(&tree), Vec::<PathBuf>::new());
    }

    #[test]
    fn a_symlink_to_a_sibling_repository_is_not_a_second_project() {
        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        tree.symlink(&alpha, "Projects/alpha-link");

        assert_eq!(folders_under(&tree), vec![canonical(&alpha)]);
    }

    #[test]
    fn a_symlink_to_its_own_ancestor_does_not_hang_the_walk() {
        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        let projects = tree.path().join("Projects");
        tree.symlink(&projects, "Projects/upwards");

        assert_eq!(folders_under(&tree), vec![canonical(&alpha)]);
    }

    #[test]
    fn an_absent_projects_folder_contributes_nothing_and_is_not_an_error() {
        let tree = TestTree::new();

        let found = discover(&tree.sources().projects_folder);

        assert!(found.folders.is_empty());
        assert!(
            found.events.is_empty(),
            "a fresh machine has no projects folder, and that is not a failure"
        );
    }

    #[test]
    fn a_projects_folder_that_cannot_be_read_is_reported() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TestTree::new();
        let projects = tree.plain_folder("Projects");
        fs::set_permissions(&projects, fs::Permissions::from_mode(0o000))
            .expect("take the permissions off the projects folder");

        // Mode 0o000 means nothing to root, so without this probe the test
        // would go green in a root container without touching the error path
        // at all.
        let readable_anyway = fs::read_dir(&projects).is_ok();
        let found = discover(&projects);

        fs::set_permissions(&projects, fs::Permissions::from_mode(0o755))
            .expect("give the permissions back, or the temporary directory cannot be removed");
        assert!(
            !readable_anyway,
            "this test needs a non-root user: mode 0o000 did not stop the read"
        );

        assert!(found.folders.is_empty());
        assert!(
            matches!(
                found.events.as_slice(),
                [WorkspaceEvent::ProjectsFolderUnreadable { path, .. }] if path == &projects
            ),
            "expected one ProjectsFolderUnreadable naming the folder, got {:?}",
            found.events
        );
    }

    #[test]
    fn a_subdirectory_that_cannot_be_read_does_not_stop_the_walk() {
        use std::os::unix::fs::PermissionsExt;

        let tree = TestTree::new();
        let alpha = tree.repository("Projects/alpha");
        let closed = tree.plain_folder("Projects/closed");
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000))
            .expect("take the permissions off the subdirectory");

        let readable_anyway = fs::read_dir(&closed).is_ok();
        let found = discover(&tree.sources().projects_folder);

        fs::set_permissions(&closed, fs::Permissions::from_mode(0o755))
            .expect("give the permissions back, or the temporary directory cannot be removed");
        assert!(
            !readable_anyway,
            "this test needs a non-root user: mode 0o000 did not stop the read"
        );

        assert_eq!(found.folders, vec![canonical(&alpha)]);
        assert!(
            matches!(
                found.events.as_slice(),
                [WorkspaceEvent::ProjectsFolderUnreadable { path, .. }] if path == &closed
            ),
            "expected one ProjectsFolderUnreadable naming the subdirectory, got {:?}",
            found.events
        );
    }
}
