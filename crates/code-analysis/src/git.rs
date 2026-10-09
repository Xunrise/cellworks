//! Asking git which files a repository shows: the one module that spawns a
//! process (ADR-0003).
//!
//! The repository is the child's working directory, never an argument, and no
//! shell is involved, so a project folder named like an option never reaches
//! git's argument vector.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Why git could not answer: the message to show the engineer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitUnavailable(pub String);

/// What git says about a repository.
///
/// The binary passes [`GitCli`]; every test passes a recorded listing, so no
/// test runs real git.
pub trait GitListing {
    /// Paths relative to the repository root that git does not ignore and
    /// that exist in the worktree.
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable>;
}

/// [`GitListing`] by running `git ls-files` as a child process.
pub struct GitCli;

/// Tracked files plus untracked files no ignore rule covers.
const LISTED: &[&str] = &[
    "ls-files",
    "-z",
    "--cached",
    "--others",
    "--exclude-standard",
];

/// Files still in the index but gone from the worktree.
const DELETED: &[&str] = &["ls-files", "-z", "--deleted"];

impl GitListing for GitCli {
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        let listed = run(command(repository, LISTED))?;
        let deleted = run(command(repository, DELETED))?;
        Ok(visible(listed, deleted))
    }
}

/// What git listed, without the files gone from the worktree, sorted and
/// without duplicates.
///
/// A set rather than `--deduplicate`, which older gits reject. A nested
/// repository is listed as `name/`, a folder rather than a file.
/// ponytail: submodules are listed as one file each; read them when a story
/// needs their contents.
fn visible(listed: Vec<PathBuf>, deleted: Vec<PathBuf>) -> Vec<PathBuf> {
    let deleted: BTreeSet<PathBuf> = deleted.into_iter().collect();
    let visible: BTreeSet<PathBuf> = listed
        .into_iter()
        .filter(|path| !deleted.contains(path) && !path.as_os_str().as_bytes().ends_with(b"/"))
        .collect();
    visible.into_iter().collect()
}

/// The child process for one call: fixed arguments, the repository as the
/// working directory, and no index lock taken from a session working there.
fn command(repository: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .args(arguments)
        .current_dir(repository)
        .env("GIT_OPTIONAL_LOCKS", "0");
    command
}

fn run(mut command: Command) -> Result<Vec<PathBuf>, GitUnavailable> {
    let output = command
        .output()
        .map_err(|error| GitUnavailable(format!("git could not be started: {error}")))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message: String = message.trim().chars().take(200).collect();
        return Err(GitUnavailable(message));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(OsStr::from_bytes(path)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deleted_file_a_duplicate_and_a_nested_repository_are_not_visible() {
        let paths = |names: &[&str]| names.iter().map(PathBuf::from).collect::<Vec<_>>();

        let files = visible(
            paths(&["src/b.rs", "gone.rs", "src/a.rs", "src/b.rs", "nested/"]),
            paths(&["gone.rs"]),
        );

        assert_eq!(files, paths(&["src/a.rs", "src/b.rs"]));
    }

    #[test]
    fn the_repository_is_the_working_directory_and_never_an_argument() {
        let repository = Path::new("/projects/--upload-pack=evil");

        let command = command(repository, LISTED);

        assert_eq!(command.get_program(), "git");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            LISTED.iter().map(OsStr::new).collect::<Vec<_>>()
        );
        assert_eq!(command.get_current_dir(), Some(repository));
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "GIT_OPTIONAL_LOCKS" && value == Some(OsStr::new("0")))
        );
    }
}
