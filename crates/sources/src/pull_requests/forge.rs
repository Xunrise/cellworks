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

/// An issue, read only far enough to find a story's text in it
/// (`crate::stories`, STORY-022 requirement 9): its number, title (checked
/// for the `"<id>: "` prefix that names the story it is) and body (the
/// Gherkin-shaped prose an `As a`/`I want`/`So that` is read out of).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub number: u32,
    pub title: String,
    pub body: String,
}

/// What the forge lists about a project's pull requests and issues.
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

    /// Every issue (open or closed) matching `search`, read for its title and
    /// body. `crate::stories::read` always calls this with a story id
    /// (`"STORY-012"`, say) as `search`, never with text an outside party
    /// chose; still passed as a plain `gh` argument, never shell-interpolated.
    fn story_issues(&self, repo: &Path, search: &str) -> Result<Vec<Issue>, ForgeUnavailable>;
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

/// The fields a pull request listing needs, in the order [`TEMPLATE`] prints
/// them (requirement: size, file count and flags with the listing, STORY-022
/// requirement 2).
const JSON_FIELDS: &str = "number,state,headRefName,headRefOid,baseRefOid,author,title,additions,deletions,changedFiles,files";

/// One record per pull request: number, state, head branch, head commit, base
/// commit, author login, additions, deletions, changed files, its files
/// (themselves joined by the ASCII unit separator `\x1f`, since `gh` itself
/// prints `files` as a nested list of `{path, ...}` objects), then title last
/// — each field separated by the ASCII group separator `\x1d` and each record
/// by the ASCII record separator `\x1e`, the same three-level convention
/// [`ISSUE_TEMPLATE`] uses (record > field > unit), rather than a tab/newline
/// convention a `files` entry or a title holding a literal tab or newline
/// (both plain, unremarkable bytes in a real path or a real title) would
/// otherwise forge a second record out of, with every one of its fields —
/// including `head`/`base` — then attacker-chosen (`parse_listing`'s own
/// `looks_like_a_commit` guard only protects against what reaches `git`, not
/// against the record boundary itself moving). The `state` column is never
/// parsed back (the caller already knows which call produced the line); it is
/// kept only because `gh` always prints it.
const TEMPLATE: &str = "{{range .}}{{.number}}\u{1d}{{.state}}\u{1d}{{.headRefName}}\u{1d}{{.headRefOid}}\u{1d}{{.baseRefOid}}\u{1d}{{.author.login}}\u{1d}{{.additions}}\u{1d}{{.deletions}}\u{1d}{{.changedFiles}}\u{1d}{{range $i, $f := .files}}{{if $i}}\u{1f}{{end}}{{$f.path}}{{end}}\u{1d}{{.title}}\u{1e}{{end}}";

/// The fields one `gh issue list` listing needs, in the order [`ISSUE_TEMPLATE`]
/// prints them.
const ISSUE_JSON_FIELDS: &str = "number,title,body";

/// One record per issue: number, title and body (which can itself hold any
/// text, including tab and newline characters, since it is prose an outside
/// author wrote), each field separated by the ASCII unit separator `\x1f` and
/// each record by the ASCII record separator `\x1e`, rather than this
/// module's usual tab/newline convention, which a multi-line issue body would
/// break (a literal newline inside it would otherwise look like a second
/// record to [`parse_issues`]).
const ISSUE_TEMPLATE: &str = "{{range .}}{{.number}}\u{1f}{{.title}}\u{1f}{{.body}}\u{1e}{{end}}";

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

    fn story_issues(&self, repo: &Path, search: &str) -> Result<Vec<Issue>, ForgeUnavailable> {
        let search_in_title = format!("{search} in:title");
        let output = run(command(
            "gh",
            repo,
            &[
                "issue",
                "list",
                "--state",
                "all",
                "--search",
                &search_in_title,
                "--json",
                ISSUE_JSON_FIELDS,
                "--template",
                ISSUE_TEMPLATE,
            ],
        ))?;
        Ok(parse_issues(&output))
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

/// Parses `TEMPLATE`'s output into pull requests, one per record.
/// A record with fewer than eleven fields, a non-numeric number, additions,
/// deletions or changed-files count, or a head or base that is not a plain
/// commit hash is skipped rather than failing the whole listing (tolerant
/// parsing, same as today's non-numeric `number`). The `state` column `gh`
/// prints is ignored; `state` is the one the caller already knows it asked
/// for. `flags` is left empty here: [`super::list`] fills it once, from
/// `files`, after merging the open and merged listings.
///
/// Records are split on the ASCII record separator `\x1e`, never on a literal
/// newline: a pull request's title or one of its file paths is free text an
/// outside author fully controls, and a plain, unremarkable byte in either
/// (a literal tab or newline in a path is valid on most filesystems and
/// `gh`/GitHub never reject it) would otherwise be read as a field or record
/// boundary under a tab/newline convention. `head`/`base` are additionally
/// checked against [`looks_like_a_commit`] before they ever reach a `git`
/// argument vector elsewhere in this module: an unchecked head or base
/// starting with `-` can be read by `git` as an option rather than a
/// revision.
fn parse_listing(output: &str, state: super::PullRequestState) -> Vec<super::PullRequest> {
    output
        .split('\u{1e}')
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            let mut fields = record.splitn(11, '\u{1d}');
            let number = fields.next()?.parse().ok()?;
            let _state = fields.next()?;
            let branch = fields.next()?.to_owned();
            let head = fields.next()?.to_owned();
            let base = fields.next()?.to_owned();
            let author = fields.next()?.to_owned();
            let additions = fields.next()?.parse().ok()?;
            let deletions = fields.next()?.parse().ok()?;
            let changed_files = fields.next()?.parse().ok()?;
            let files_field = fields.next()?;
            let files = if files_field.is_empty() {
                Vec::new()
            } else {
                files_field.split('\u{1f}').map(str::to_owned).collect()
            };
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
                additions,
                deletions,
                changed_files,
                files,
                flags: std::collections::BTreeMap::new(),
            })
        })
        .collect()
}

/// Parses [`ISSUE_TEMPLATE`]'s output into issues: records separated by the
/// ASCII record separator `\x1e`, each holding number, title and body
/// separated by the ASCII unit separator `\x1f`. A record with fewer than
/// three fields, or a non-numeric number, is skipped rather than failing the
/// whole read (tolerant parsing, same convention as [`parse_listing`]).
fn parse_issues(output: &str) -> Vec<Issue> {
    output
        .split('\u{1e}')
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            let mut fields = record.splitn(3, '\u{1f}');
            let number = fields.next()?.parse().ok()?;
            let title = fields.next()?.to_owned();
            let body = fields.next()?.to_owned();
            Some(Issue {
                number,
                title,
                body,
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
        assert_eq!(pulls[0].additions, 12);
        assert_eq!(pulls[0].deletions, 4);
        assert_eq!(pulls[0].changed_files, 3);
        assert_eq!(
            pulls[0].files,
            vec![
                "crates/sources/src/pull_requests/mod.rs".to_owned(),
                "crates/sources/src/pull_requests/forge.rs".to_owned(),
                "Cargo.lock".to_owned(),
            ],
            "files are split on the unit separator, not the comma or pipe a path could hold"
        );
        assert!(
            pulls[0].flags.is_empty(),
            "the listing parser never fills flags itself, list() does: {:?}",
            pulls[0].flags
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
        assert!(
            pulls[0].files.is_empty(),
            "an empty files field is no files, not one empty path: {:?}",
            pulls[0].files
        );
        assert_eq!(
            pulls[1].files,
            vec!["src/release.rs".to_owned(), "CHANGELOG.md".to_owned()]
        );
        assert_eq!(pulls[1].additions, 20);
        assert_eq!(pulls[1].deletions, 3);
        assert_eq!(pulls[1].changed_files, 2);
    }

    #[test]
    fn a_non_numeric_additions_column_is_skipped_like_a_non_numeric_number() {
        let output = "41\u{1d}OPEN\u{1d}feature/x\u{1d}1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b\u{1d}9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e\u{1d}xunrise\u{1d}many\u{1d}2\u{1d}2\u{1d}Cargo.lock\u{1d}title\u{1e}";

        let pulls = parse_listing(output, PullRequestState::Open);

        assert!(pulls.is_empty(), "{pulls:?}");
    }

    #[test]
    fn a_literal_tab_or_newline_in_a_file_path_or_title_never_forges_a_second_record() {
        // Both bytes are valid, unremarkable in a real path or title; neither
        // may be read as a field or record boundary the way the old
        // tab/newline convention would have read them.
        let output = "41\u{1d}OPEN\u{1d}feature/x\u{1d}1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b\u{1d}9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e\u{1d}xunrise\u{1d}3\u{1d}1\u{1d}1\u{1d}weird\tpath\nhere\u{1d}STORY-012: a\ttitle\nwith a newline\u{1e}";

        let pulls = parse_listing(output, PullRequestState::Open);

        assert_eq!(pulls.len(), 1, "{pulls:?}");
        assert_eq!(pulls[0].files, vec!["weird\tpath\nhere".to_owned()]);
        assert_eq!(pulls[0].title, "STORY-012: a\ttitle\nwith a newline");
    }

    #[test]
    fn issues_are_parsed_from_the_gh_template_output_body_newlines_and_all() {
        let output = include_str!("../../tests/fixtures/gh_issue_list.txt");

        let issues = parse_issues(output);

        assert_eq!(
            issues.iter().map(|issue| issue.number).collect::<Vec<_>>(),
            vec![83, 90],
            "the malformed trailing record must not appear and must not panic: {issues:?}"
        );
        assert_eq!(
            issues[0].title,
            "STORY-012: Give engineers a price breakdown"
        );
        assert!(
            issues[0].body.contains("As a shopper comparing prices"),
            "a multi-line body survives intact: {:?}",
            issues[0].body
        );
        assert!(issues[0].body.lines().count() >= 4, "{:?}", issues[0].body);
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
