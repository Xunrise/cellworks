//! GitHub through `gh`, diffs and worktrees through `git`: the only module in
//! this crate that spawns a process (ADR-0003, ADR-0009).
//!
//! The repository is always the child's working directory, never an
//! argument, and no shell is involved, so a project folder named like an
//! option never reaches an argument vector.

use std::path::Path;
use std::process::Command;

// Brought into scope so the `#[cfg(test)]` module below, which globs this
// module with `use super::*;`, can name it bare; this module's own code
// still spells it out as `super::PullRequestState` to match the contract.
#[cfg(test)]
use super::PullRequestState;

/// Why the forge or git could not answer: the message to show the engineer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForgeUnavailable(pub String);

/// What the forge lists about a project's pull requests.
///
/// The binary passes [`GhCli`]; every test passes a fake, so no test runs
/// `gh` or touches the network.
pub trait Forge {
    /// Every open pull request.
    fn open_pull_requests(&self, repo: &Path) -> Result<Vec<super::PullRequest>, ForgeUnavailable>;

    /// The last `limit` merged pull requests.
    fn merged_pull_requests(
        &self,
        repo: &Path,
        limit: usize,
    ) -> Result<Vec<super::PullRequest>, ForgeUnavailable>;
}

/// A pull request's diff and the temporary worktree its head is checked out
/// into.
///
/// The binary passes [`GitCli`]; every test passes a fake, so no test runs
/// git or touches the network.
pub trait Checkout {
    /// The pull request's own diff, fetching its head and base first when
    /// either is absent from `repo`.
    fn diff(
        &self,
        repo: &Path,
        pull_request: &super::PullRequest,
    ) -> Result<String, ForgeUnavailable>;

    /// Checks `commit` out into a new detached worktree at `into`.
    fn add_worktree(&self, repo: &Path, commit: &str, into: &Path) -> Result<(), ForgeUnavailable>;

    /// Removes the worktree at `into`, even when it holds changes.
    fn remove_worktree(&self, repo: &Path, into: &Path) -> Result<(), ForgeUnavailable>;
}

/// [`Forge`] by running `gh pr list` as a child process.
pub struct GhCli;

/// [`Checkout`] by running `git` as a child process.
pub struct GitCli;

/// The six fields a pull request listing needs, in the order [`TEMPLATE`]
/// prints them.
const JSON_FIELDS: &str = "number,state,headRefName,headRefOid,baseRefOid,author,title";

/// One tab-separated line per pull request: number, state, head branch, head
/// commit, base commit, author login, title. The `state` column is never
/// parsed back (the caller already knows which call produced the line); it
/// is kept only because `gh` always prints it.
const TEMPLATE: &str = "{{range .}}{{.number}}\t{{.state}}\t{{.headRefName}}\t{{.headRefOid}}\t{{.baseRefOid}}\t{{.author.login}}\t{{.title}}\n{{end}}";

impl Forge for GhCli {
    fn open_pull_requests(&self, repo: &Path) -> Result<Vec<super::PullRequest>, ForgeUnavailable> {
        let output = run(command(
            "gh",
            repo,
            &[
                "pr",
                "list",
                "--state",
                "open",
                "--limit",
                "1000",
                "--json",
                JSON_FIELDS,
                "--template",
                TEMPLATE,
            ],
        ))?;
        Ok(parse_listing(&output, super::PullRequestState::Open))
    }

    fn merged_pull_requests(
        &self,
        repo: &Path,
        limit: usize,
    ) -> Result<Vec<super::PullRequest>, ForgeUnavailable> {
        let limit = limit.to_string();
        let output = run(command(
            "gh",
            repo,
            &[
                "pr",
                "list",
                "--state",
                "merged",
                "--limit",
                &limit,
                "--json",
                JSON_FIELDS,
                "--template",
                TEMPLATE,
            ],
        ))?;
        Ok(parse_listing(&output, super::PullRequestState::Merged))
    }
}

impl Checkout for GitCli {
    fn diff(
        &self,
        repo: &Path,
        pull_request: &super::PullRequest,
    ) -> Result<String, ForgeUnavailable> {
        if !exists(repo, &pull_request.head) {
            run(command(
                "git",
                repo,
                &[
                    "fetch",
                    "-q",
                    "origin",
                    &format!("pull/{}/head", pull_request.number),
                ],
            ))?;
        }
        if !exists(repo, &pull_request.base) {
            run(command(
                "git",
                repo,
                &["fetch", "-q", "origin", &pull_request.base],
            ))?;
        }
        let range = format!("{}...{}", pull_request.base, pull_request.head);
        run(command(
            "git",
            repo,
            &["diff", "--no-color", "--no-ext-diff", "-U0", &range],
        ))
    }

    fn add_worktree(&self, repo: &Path, commit: &str, into: &Path) -> Result<(), ForgeUnavailable> {
        // Created ourselves, exclusively and readable by no one else, before
        // git ever sees it: `git worktree add` otherwise accepts an existing
        // empty directory, which on a shared machine another local user could
        // have pre-created (at a guessable path) to read the checkout as it
        // is written, or to replace a subdirectory with a symlink.
        create_private_dir(into).map_err(|error| {
            ForgeUnavailable(format!("could not create the worktree folder: {error}"))
        })?;
        let mut worktree_add = command("git", repo, &["worktree", "add", "-q", "--detach"]);
        worktree_add.arg(into).arg(commit);
        run(worktree_add)?;
        Ok(())
    }

    fn remove_worktree(&self, repo: &Path, into: &Path) -> Result<(), ForgeUnavailable> {
        let mut worktree_remove = command("git", repo, &["worktree", "remove", "--force"]);
        worktree_remove.arg(into);
        run(worktree_remove)?;
        Ok(())
    }
}

/// Creates `path` as a new, empty directory, readable and writable by no one
/// but its owner, failing if anything is already there (never following a
/// pre-existing entry, symlink or otherwise).
#[cfg(unix)]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().create(path)
}

/// Whether `commit` is present in `repo`, ignoring `git cat-file`'s own exit
/// code beyond that boolean: only a fetch failing is surfaced as an error.
fn exists(repo: &Path, commit: &str) -> bool {
    command("git", repo, &["cat-file", "-e", commit])
        .status()
        .is_ok_and(|status| status.success())
}

/// Parses `TEMPLATE`'s tab-separated output into pull requests, one per line.
/// A line with fewer than seven fields, a non-numeric number, or a head or
/// base that is not a plain commit hash is skipped rather than failing the
/// whole listing (tolerant parsing). The `state` column `gh` prints is
/// ignored; `state` is the one the caller already knows it asked for.
///
/// `head`/`base` are checked against [`looks_like_a_commit`] before they ever
/// reach a `git` argument vector elsewhere in this module: a pull request's
/// title is the one field an outside author fully controls, and a title
/// holding a literal newline would otherwise split `output.lines()` into a
/// forged second record whose every field, head and base included, is
/// attacker-chosen — and an unchecked head or base starting with `-` can be
/// read by `git` as an option rather than a revision.
fn parse_listing(output: &str, state: super::PullRequestState) -> Vec<super::PullRequest> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(7, '\t');
            let number = fields.next()?.parse().ok()?;
            let _state = fields.next()?;
            let branch = fields.next()?.to_owned();
            let head = fields.next()?.to_owned();
            let base = fields.next()?.to_owned();
            let author = fields.next()?.to_owned();
            let title = fields.next()?.to_owned();
            if !looks_like_a_commit(&head) || !looks_like_a_commit(&base) {
                return None;
            }
            let story = super::story_of(&title).or_else(|| super::story_of(&branch));
            Some(super::PullRequest {
                number,
                state,
                title,
                branch,
                head,
                base,
                author,
                story,
            })
        })
        .collect()
}

/// Whether `text` is shaped like a git commit hash: lower-case hex, long
/// enough to be an abbreviation, short enough for any hash algorithm git
/// uses. Required before `head`/`base` reach a `git` argument vector, since
/// unlike a fixed argument such a value is data from the forge and must never
/// be let through looking like an option (a leading `-`) or a shell token.
fn looks_like_a_commit(text: &str) -> bool {
    (7..=64).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The child process for one call: fixed arguments, the repository as the
/// working directory.
fn command(program: &str, repository: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new(program);
    command.args(arguments).current_dir(repository);
    command
}

/// Runs `command`, returning its stdout on success. A spawn failure or a
/// non-zero exit becomes a [`ForgeUnavailable`] carrying the tool's own
/// message: the spawn error as `std::io::Error` prints it, or the process's
/// stderr trimmed and capped at 200 characters (`crates/code-analysis/src/git.rs`'s
/// convention).
fn run(mut command: Command) -> Result<String, ForgeUnavailable> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .map_err(|error| ForgeUnavailable(format!("{program} could not be started: {error}")))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(ForgeUnavailable(message.trim().chars().take(200).collect()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::*;

    #[test]
    fn open_pull_requests_are_parsed_from_the_gh_template_output() {
        let output = include_str!("../../tests/fixtures/gh_pr_list_open.txt");

        let pulls = parse_listing(output, PullRequestState::Open);

        assert_eq!(pulls.len(), 2, "{pulls:?}");
        assert_eq!(pulls[0].number, 117);
        assert_eq!(pulls[0].state, PullRequestState::Open);
        assert_eq!(pulls[0].branch, "feature/92-story-058-pr-source");
        assert_eq!(pulls[0].head, "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b");
        assert_eq!(pulls[0].base, "9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e");
        assert_eq!(pulls[0].author, "xunrise");
        assert_eq!(pulls[0].title, "STORY-058: Pull-request source with diffs");
        assert_eq!(
            pulls[0].story.as_deref(),
            Some("STORY-058"),
            "the story is found in the title"
        );
        assert_eq!(pulls[1].number, 124);
        assert_eq!(
            pulls[1].story, None,
            "neither the title nor the branch of this one names a story"
        );
    }

    #[test]
    fn a_malformed_line_in_the_merged_listing_is_skipped_rather_than_panicking() {
        let output = include_str!("../../tests/fixtures/gh_pr_list_merged.txt");

        let pulls = parse_listing(output, PullRequestState::Merged);

        assert_eq!(
            pulls.iter().map(|p| p.number).collect::<Vec<_>>(),
            vec![113, 116],
            "the malformed trailing line must not appear and must not panic: {pulls:?}"
        );
        assert_eq!(
            pulls[0].story, None,
            "a plain promotion title names no story"
        );
        assert_eq!(
            pulls[1].story.as_deref(),
            Some("STORY-080"),
            "the story is found in this one's title"
        );
        assert!(
            pulls.iter().all(|p| p.state == PullRequestState::Merged),
            "{pulls:?}"
        );
    }

    #[test]
    fn the_repository_is_the_working_directory_and_never_an_argument() {
        let repository = Path::new("/projects/--upload-pack=evil");

        for program in ["gh", "git"] {
            let built = command(program, repository, &["status", "--porcelain"]);

            assert_eq!(built.get_program(), program, "{program}");
            assert_eq!(
                built.get_args().collect::<Vec<_>>(),
                [OsStr::new("status"), OsStr::new("--porcelain")],
                "{program}"
            );
            assert_eq!(built.get_current_dir(), Some(repository), "{program}");
        }
    }
}
