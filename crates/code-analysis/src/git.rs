//! Asking git which files a repository shows: the one module that spawns a
//! process (ADR-0003).
//!
//! The repository is the child's working directory, never an argument, and no
//! shell is involved, so a project folder named like an option never reaches
//! git's argument vector.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Why git could not answer: the message to show the engineer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitUnavailable(pub String);

/// One commit as `git log` tells it, before it is joined to a pull request
/// or a story.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedCommit {
    pub sha: String,
    /// Commit time, seconds since the Unix epoch (`%ct`).
    pub time: i64,
    /// The whole raw message, subject and body.
    pub message: String,
}

/// What git says about a repository.
///
/// The binary passes [`GitCli`]; every test passes a recorded listing, so no
/// test runs real git.
pub trait GitListing {
    /// Paths relative to the repository root that git does not ignore and
    /// that exist in the worktree.
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable>;

    /// The files each of the last commits changed, relative to the
    /// repository root, newest first. None by default: a listing that only
    /// knows files knows no history.
    fn history(&self, _repository: &Path) -> Result<Vec<Vec<PathBuf>>, GitUnavailable> {
        Ok(Vec::new())
    }

    /// The project's commits, oldest first, each with its id, commit time and
    /// whole message. Empty by default.
    fn log(&self, _repository: &Path) -> Result<Vec<LoggedCommit>, GitUnavailable> {
        Ok(Vec::new())
    }

    /// The commit id that last wrote each line of each of `files`, relative to
    /// the repository root, in file order; a file blame cannot explain (binary,
    /// untracked, gone) is left out, the rest still answered. Empty by default.
    fn blame(
        &self,
        _repository: &Path,
        _files: &[PathBuf],
    ) -> BTreeMap<PathBuf, Vec<Option<String>>> {
        BTreeMap::new()
    }
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

/// The files of the last 500 commits, each commit opened by a line holding
/// only the record separator.
/// Paths come unquoted, as `ls-files -z` lists them, so `café.rs` pairs up.
const HISTORY: &[&str] = &[
    "-c",
    "core.quotePath=false",
    "log",
    "-n",
    "500",
    "--name-only",
    "--format=%x1e",
];

/// `git log`'s format for `GitCli::log`: the commit id, a unit separator,
/// the commit time, a unit separator, the whole raw message, a record
/// separator. git appends one more newline after each commit's formatted
/// output, including the last.
const LOG: &[&str] = &["log", "--reverse", "--format=%H%x1f%ct%x1f%B%x1e"];

/// How many `git blame` child processes run at once.
const BLAME_WORKERS: usize = 8;

impl GitListing for GitCli {
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        let listed = run(command(repository, LISTED))?;
        let deleted = run(command(repository, DELETED))?;
        Ok(visible(listed, deleted))
    }

    fn history(&self, repository: &Path) -> Result<Vec<Vec<PathBuf>>, GitUnavailable> {
        let mut command = command(repository, HISTORY);
        let output = command
            .output()
            .map_err(|error| GitUnavailable(format!("git could not be started: {error}")))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            return Err(GitUnavailable(message.trim().chars().take(200).collect()));
        }
        Ok(commits(&output.stdout))
    }

    fn log(&self, repository: &Path) -> Result<Vec<LoggedCommit>, GitUnavailable> {
        let mut command = command(repository, LOG);
        let output = command
            .output()
            .map_err(|error| GitUnavailable(format!("git could not be started: {error}")))?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr);
            return Err(GitUnavailable(message.trim().chars().take(200).collect()));
        }
        Ok(parse_log(&output.stdout))
    }

    fn blame(
        &self,
        repository: &Path,
        files: &[PathBuf],
    ) -> BTreeMap<PathBuf, Vec<Option<String>>> {
        if files.is_empty() {
            return BTreeMap::new();
        }
        let workers = BLAME_WORKERS.min(files.len());
        let chunk_size = files.len().div_ceil(workers);
        std::thread::scope(|scope| {
            files
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .filter_map(|file| {
                                blame_one(repository, file).map(|lines| (file.clone(), lines))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .flat_map(|handle| handle.join().unwrap_or_default())
                .collect()
        })
    }
}

/// The files of each commit in `git log --name-only --format=%x1e` output.
/// ponytail: a path holding a newline is split; quote paths with -z if one
/// ever matters.
fn commits(output: &[u8]) -> Vec<Vec<PathBuf>> {
    output
        .split(|byte| *byte == 0x1e)
        .map(|commit| {
            commit
                .split(|byte| *byte == b'\n')
                .filter(|line| !line.is_empty())
                .map(|line| PathBuf::from(OsStr::from_bytes(line)))
                .collect::<Vec<_>>()
        })
        .filter(|files| !files.is_empty())
        .collect()
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

/// The commit id of each line of `file`, relative to `repository`, from
/// `git blame --porcelain`; `None` when the blame itself fails (binary,
/// untracked, gone).
fn blame_one(repository: &Path, file: &Path) -> Option<Vec<Option<String>>> {
    let mut command = Command::new("git");
    command
        .args(["blame", "--porcelain", "--"])
        .arg(file)
        .current_dir(repository)
        .env("GIT_OPTIONAL_LOCKS", "0");
    let output = command.output().ok()?;
    output.status.success().then(|| parse_blame(&output.stdout))
}

/// `git log --reverse --format=%H%x1f%ct%x1f%B%x1e` output, oldest first:
/// each commit's id, a unit separator, its commit time, a unit separator,
/// its whole raw message, a record separator; git appends one more newline
/// after each commit's formatted output, including the last, so each
/// record but the first starts with that newline.
fn parse_log(output: &[u8]) -> Vec<LoggedCommit> {
    String::from_utf8_lossy(output)
        .split('\x1e')
        .map(|entry| entry.trim_start_matches('\n'))
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            let mut fields = entry.splitn(3, '\x1f');
            let sha = fields.next()?.to_owned();
            let time = fields.next()?.parse().ok()?;
            let message = fields
                .next()
                .unwrap_or("")
                .trim_end_matches('\n')
                .to_owned();
            Some(LoggedCommit { sha, time, message })
        })
        .collect()
}

/// `git blame --porcelain` output: one commit id per final line, in file
/// order; the all-zero id (an uncommitted line) is `None`. A commit's
/// detail block (author, committer, summary, filename) appears only the
/// first time that commit is mentioned in the whole output; every line
/// still gets its own `<sha> <orig-line> <final-line> [<count>]` header, so
/// a later line of a commit already seen has no detail block, just the
/// header and the tab-prefixed content line straight after.
fn parse_blame(output: &[u8]) -> Vec<Option<String>> {
    let lines: Vec<&[u8]> = output.split(|&byte| byte == b'\n').collect();
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.is_empty() {
            i += 1;
            continue;
        }
        let header = String::from_utf8_lossy(line);
        let Some(sha) = header.split_whitespace().next() else {
            i += 1;
            continue;
        };
        let sha = sha.to_owned();
        result.push((!sha.bytes().all(|byte| byte == b'0')).then(|| sha.clone()));
        i += 1;
        if seen.insert(sha) {
            while i < lines.len() && !lines[i].starts_with(b"\t") {
                i += 1;
            }
        }
        if i < lines.len() {
            i += 1; // the content line
        }
    }
    result
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
    fn the_history_lists_the_files_of_each_commit() {
        let output = b"\x1e\n\nsrc/a.rs\nsrc/b.rs\n\x1e\n\nREADME.md\n\x1e\n\n";

        assert_eq!(
            commits(output),
            [
                vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")],
                vec![PathBuf::from("README.md")],
            ]
        );
    }

    #[test]
    fn the_log_lists_commits_oldest_first_with_their_whole_message() {
        let output: &[u8] = b"aaaaaaa1\x1f1000\x1fchore: a (#1)\n\x1e\nbbbbbbb2\x1f2000\x1fsecond change (#5) (#19)\n\nRefs: STORY-7, STORY-003\n\x1e\n";

        let commits = parse_log(output);

        assert_eq!(
            commits,
            vec![
                LoggedCommit {
                    sha: "aaaaaaa1".to_owned(),
                    time: 1000,
                    message: "chore: a (#1)".to_owned(),
                },
                LoggedCommit {
                    sha: "bbbbbbb2".to_owned(),
                    time: 2000,
                    message: "second change (#5) (#19)\n\nRefs: STORY-7, STORY-003".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn blame_gives_one_commit_per_line_in_order() {
        let output = "c1 1 1 1\n\
author A\nauthor-mail <a@x>\nauthor-time 1000\nauthor-tz +0000\n\
committer A\ncommitter-mail <a@x>\ncommitter-time 1000\ncommitter-tz +0000\n\
summary first\nfilename file.txt\n\tline one\n\
c2 2 2 1\n\
author B\nauthor-mail <b@x>\nauthor-time 2000\nauthor-tz +0000\n\
committer B\ncommitter-mail <b@x>\ncommitter-time 2000\ncommitter-tz +0000\n\
summary second\nprevious c0 file.txt\nfilename file.txt\n\tline two\n\
c1 1 3 1\n\tline three\n\
0000000000000000000000000000000000000000 4 4 1\n\
author Not Committed Yet\nauthor-mail <not.committed.yet>\nauthor-time 1700000000\nauthor-tz +0000\n\
committer Not Committed Yet\ncommitter-mail <not.committed.yet>\ncommitter-time 1700000000\ncommitter-tz +0000\n\
summary Uncommitted changes\nfilename file.txt\n\tline four\n";

        let lines = parse_blame(output.as_bytes());

        assert_eq!(
            lines,
            vec![
                Some("c1".to_owned()),
                Some("c2".to_owned()),
                Some("c1".to_owned()),
                None,
            ],
            "line 3 repeats commit c1 without its detail block (already seen); line 4 is the all-zero uncommitted id"
        );
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
