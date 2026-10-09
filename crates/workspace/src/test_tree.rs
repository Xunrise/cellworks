//! The tree a unit test raises in a temporary directory.
//!
//! A test tree cannot be committed: it needs real `.git` directories, and git
//! will not let this repository hold a nested one. So every test builds its
//! own at run time, under a [`tempfile::TempDir`] that is removed when the
//! test ends — including when it unwinds.
//!
//! No test reaches the user's home directory or the real application support
//! directory: [`TestTree::sources`] is the only `Sources` any test here uses.

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::Sources;

/// A temporary directory a test fills with folders, repositories and files.
pub(crate) struct TestTree {
    root: TempDir,
}

impl TestTree {
    pub(crate) fn new() -> TestTree {
        TestTree {
            root: TempDir::new().expect("a temporary directory for the test tree"),
        }
    }

    /// The tree's root. Every relative path below is taken from here.
    pub(crate) fn path(&self) -> &Path {
        self.root.path()
    }

    /// Where a scan of this tree looks: `<root>/Projects` and
    /// `<root>/settings.toml`.
    ///
    /// Neither is created — a test that wants them says so, which is what lets
    /// the absent-folder and absent-file cases be written at all.
    pub(crate) fn sources(&self) -> Sources {
        Sources {
            projects_folder: self.path().join("Projects"),
            settings_file: self.path().join("settings.toml"),
        }
    }

    /// A directory that is not a git repository.
    pub(crate) fn plain_folder(&self, relative: &str) -> PathBuf {
        let path = self.path().join(relative);
        fs::create_dir_all(&path).expect("create a folder in the test tree");
        path
    }

    /// A git repository: a directory whose `.git` is a directory.
    pub(crate) fn repository(&self, relative: &str) -> PathBuf {
        let path = self.plain_folder(relative);
        fs::create_dir(path.join(".git")).expect("create .git in the test tree");
        path
    }

    /// A linked worktree: a directory whose `.git` is a file.
    pub(crate) fn worktree_repository(&self, relative: &str) -> PathBuf {
        let path = self.plain_folder(relative);
        fs::write(path.join(".git"), "gitdir: ../main/.git/worktrees/linked\n")
            .expect("create a .git file in the test tree");
        path
    }

    /// A regular file, with its parent directories.
    pub(crate) fn file(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create a folder in the test tree");
        }
        fs::write(&path, text).expect("write a file in the test tree");
        path
    }

    /// A symbolic link at `relative` pointing at `target`.
    pub(crate) fn symlink(&self, target: &Path, relative: &str) -> PathBuf {
        let path = self.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create a folder in the test tree");
        }
        std::os::unix::fs::symlink(target, &path).expect("create a symlink in the test tree");
        path
    }
}

/// The canonical path of `path`, for a test that wants to say which folder it
/// expects.
///
/// A temporary directory is reached through a symlink on macOS
/// (`/var` to `/private/var`), so an expected path has to be canonicalised the
/// same way the code under test canonicalises what it found.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("the path exists and is readable")
}
