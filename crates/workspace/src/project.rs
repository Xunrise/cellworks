//! What a project is, and how the filesystem is asked whether two paths are
//! the same folder.
//!
//! Two different questions, answered by two different mechanisms. A local
//! project's *identity* is its canonical path, because a path survives a
//! restart and is what the settings file and the glossary's `Project` both
//! key on; a remote project's identity is the host and path as the settings
//! file wrote them, because this machine cannot canonicalize another one's
//! filesystem. Whether two paths are the *same* folder is the filesystem's own
//! answer — the device and inode it reports — so letter case follows the
//! volume rather than a rule written into the code, and it is asked only of
//! local paths. An inode is not stable enough to persist, which is why it is
//! not the identity.

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// A project's identity: a folder on this machine, named by its canonical
/// path with symlinks and `.`/`..` resolved — never the spelling it was added
/// with — or a folder on a remote host, named by the path as the settings
/// file wrote it and never canonicalized, because this machine cannot resolve
/// another one's symlinks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectId {
    Local(PathBuf),
    Remote { host: String, path: PathBuf },
}

impl ProjectId {
    /// The canonical path this project is identified by, for a local project
    /// only: `None` forces every caller to decide what a remote id means to
    /// it, rather than treating a remote path as a folder on this machine.
    pub fn local_path(&self) -> Option<&Path> {
        match self {
            ProjectId::Local(path) => Some(path),
            ProjectId::Remote { .. } => None,
        }
    }

    /// The host this project lives on, for a remote project only.
    pub fn host(&self) -> Option<&str> {
        match self {
            ProjectId::Local(_) => None,
            ProjectId::Remote { host, .. } => Some(host),
        }
    }

    /// Reads back an id written by [`Display`](std::fmt::Display): `/abs/path`
    /// as a local id, `host:path` as a remote one. `None` when `text` is
    /// neither — no `:`, or an empty host or path either side of the first one.
    pub fn parse(text: &str) -> Option<ProjectId> {
        if text.starts_with('/') {
            return Some(ProjectId::Local(PathBuf::from(text)));
        }
        let (host, path) = text.split_once(':')?;
        if host.is_empty() || path.is_empty() {
            return None;
        }
        Some(ProjectId::Remote {
            host: host.to_owned(),
            path: PathBuf::from(path),
        })
    }
}

impl std::fmt::Display for ProjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectId::Local(path) => write!(f, "{}", path.display()),
            ProjectId::Remote { host, path } => write!(f, "{host}:{}", path.display()),
        }
    }
}

/// One project of the universe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    /// Its identity: a local folder or a remote host and path.
    pub id: ProjectId,
    /// Its display name: the last component of its path.
    pub name: String,
    /// The district prefix rule its settings name, as written there. Only the
    /// Map knows what the name means.
    pub district_prefix: Option<String>,
    /// The folders, relative to the project, its settings say implement each
    /// bounded context; empty where the settings say nothing.
    pub contexts: std::collections::BTreeMap<String, Vec<PathBuf>>,
}

impl Project {
    /// The project whose folder is at `canonical_path`.
    ///
    /// The caller passes a path that [`std::fs::canonicalize`] has already
    /// returned; this constructor does not touch the filesystem, so the view
    /// and its tests can build a project without one.
    pub fn at(canonical_path: PathBuf) -> Project {
        let name = name_of(&canonical_path);

        Project {
            id: ProjectId::Local(canonical_path),
            name,
            district_prefix: None,
            contexts: Default::default(),
        }
    }

    /// The project at `path` on `host`, named after the last component of
    /// `path` the same way [`Project::at`] names a local one.
    pub fn remote(host: String, path: PathBuf) -> Project {
        let name = name_of(&path);

        Project {
            id: ProjectId::Remote { host, path },
            name,
            district_prefix: None,
            contexts: Default::default(),
        }
    }
}

/// The last component of `path`, or the path itself when it has none.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
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
            project.id.local_path(),
            Some(Path::new("/Users/engineer/Projects/alpha"))
        );
    }

    /// STORY-078: a remote project is identified by `host:path`, never a
    /// local folder, so the compiler forces every caller of `path()` to
    /// decide what a remote id means to it instead.
    #[test]
    fn a_remote_id_has_no_local_path() {
        let id = Project::remote("az.wsl".to_owned(), PathBuf::from("~/work/api")).id;

        assert_eq!(id.local_path(), None);
        assert_eq!(id.host(), Some("az.wsl"));
    }

    /// STORY-078: a local id round-trips through `/abs/path` and a remote one
    /// through `host:path`, which is how the registry and the launcher keep
    /// an id across a restart.
    #[test]
    fn an_id_reads_back_what_it_wrote() {
        let local = Project::at(PathBuf::from("/Users/engineer/Projects/alpha")).id;
        let remote = Project::remote("az.wsl".to_owned(), PathBuf::from("~/work/api")).id;

        assert_eq!(ProjectId::parse(&local.to_string()), Some(local));
        assert_eq!(ProjectId::parse(&remote.to_string()), Some(remote));
    }
}
