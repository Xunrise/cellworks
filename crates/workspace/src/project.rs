//! What a project is, and how the filesystem is asked whether two paths are
//! the same folder.
//!
//! Two different questions, answered by two different mechanisms. A project's
//! *identity* is its canonical path, because a path survives a restart and is
//! what the settings file and the glossary's `Project` both key on. Whether
//! two paths are the *same* folder is the filesystem's own answer — the device
//! and inode it reports — so letter case follows the volume rather than a rule
//! written into the code. An inode is not stable enough to persist, which is
//! why it is not the identity.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// A project's identity: the canonical path of its folder, with symlinks and
/// `.`/`..` resolved — never the spelling the folder was added with.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(PathBuf);

impl ProjectId {
    /// The canonical path this project is identified by.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// One project of the universe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// The canonical path of its folder.
    pub id: ProjectId,
    /// Its display name: the last component of that path.
    pub name: String,
    /// The district prefix rule its settings name, as written there. Only the
    /// Map knows what the name means.
    pub district_prefix: Option<String>,
}

impl Project {
    /// The project whose folder is at `canonical_path`.
    ///
    /// The caller passes a path that [`std::fs::canonicalize`] has already
    /// returned; this constructor does not touch the filesystem, so the view
    /// and its tests can build a project without one.
    pub fn at(canonical_path: PathBuf) -> Project {
        let name = canonical_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| canonical_path.to_string_lossy().into_owned());

        Project {
            id: ProjectId(canonical_path),
            name,
            district_prefix: None,
        }
    }
}

/// The filesystem's answer to "are these two paths the same folder": the
/// device and inode it reports for them.
///
/// On a case-insensitive macOS volume `Alpha` and `alpha` are one inode and so
/// one project; on a case-sensitive Linux filesystem they are two inodes and
/// stay two projects. No `cfg!(target_os)` decides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FolderIdentity {
    device: u64,
    inode: u64,
}

impl FolderIdentity {
    /// Asks the filesystem which folder `path` is.
    ///
    /// `Err` when it cannot be read; the caller reports that as the event its
    /// source calls for, and never unwraps it.
    pub(crate) fn of(path: &Path) -> io::Result<FolderIdentity> {
        let metadata = std::fs::metadata(path)?;

        Ok(FolderIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_tree::TestTree;

    #[test]
    fn two_paths_to_one_directory_are_the_same_folder() {
        let tree = TestTree::new();
        let directory = tree.plain_folder("Projects/alpha");
        tree.plain_folder("Projects/beta");
        let roundabout = tree.path().join("Projects/./beta/../alpha");

        assert_eq!(
            FolderIdentity::of(&directory).expect("the directory is readable"),
            FolderIdentity::of(&roundabout).expect("the directory is readable"),
            "one folder reached two ways is one folder"
        );
    }

    #[test]
    fn two_directories_are_not_the_same_folder() {
        let tree = TestTree::new();
        let alpha = tree.plain_folder("Projects/alpha");
        let beta = tree.plain_folder("Projects/beta");

        assert_ne!(
            FolderIdentity::of(&alpha).expect("the directory is readable"),
            FolderIdentity::of(&beta).expect("the directory is readable"),
        );
    }

    #[test]
    fn a_folder_that_cannot_be_read_yields_an_error_rather_than_a_panic() {
        let tree = TestTree::new();

        assert!(FolderIdentity::of(&tree.path().join("absent")).is_err());
    }

    #[test]
    fn a_project_is_named_after_the_last_component_of_its_canonical_path() {
        let project = Project::at(PathBuf::from("/Users/engineer/Projects/alpha"));

        assert_eq!(project.name, "alpha");
        assert_eq!(
            project.id.path(),
            Path::new("/Users/engineer/Projects/alpha")
        );
    }
}
