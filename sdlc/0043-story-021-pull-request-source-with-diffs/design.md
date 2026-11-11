---
stage: design
issue: 43
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-021 Pull-request source with diffs and critical-change flags

The contract is `issue.md` in this directory (a chore: its "Done when" list). The confirmed behaviour, the operator's
decisions and the rejected options are in `prototype.md`; this note holds only what they do not pin. Context:
`docs/discovery/2026-10-10-living-city-design.md`, "Reviewing pull requests". STORY-022 to STORY-026 build on this.

## Open questions

None.

## Requirements

1. The Sources crate lists a project's pull requests: every open one, then the last 30 merged ones. Closed pull
   requests that were not merged are not listed. Each carries number, state (open or merged), title, head branch,
   head and base commit, author and story.
2. A pull request's story is the first `STORY-NNN` in its title, else in its head branch name, case-insensitive;
   otherwise none.
3. Opening a pull request yields its review: size (lines added, lines removed, files), the critical-change flags it
   raises with the files that raised each, and per changed file the towers and floors its lines land on.
4. Diffs are read lazily, only when a pull request is opened, never for the whole list.
5. The diff comes from git: `git fetch origin pull/<n>/head` when the head commit is absent, the base fetched when
   absent, then `git diff --no-color --no-ext-diff -U0 <base>...<head>`. No size limit.
6. Lines are placed on the city at the pull request's head: the head is checked out into a temporary detached
   worktree, mapped by CodeAnalysis, and removed afterwards, also when mapping fails.
7. An added line lands on the floor with its number in the tower that holds it. A removed line lands on exactly one
   tower: the innermost tower whose floors contain the new-side line it was removed before; else the tower whose last
   floor is the line just above; else outside.
8. Lines no tower holds are counted per file as outside any tower, added and removed separately, never dropped.
9. Flags come from `sdlc/critical.yaml` of the open project folder. A file raises a flag when its lowercased path
   matches one of the flag's globs; `*` matches any run of characters including `/`. A missing file means no flags;
   an unreadable line is skipped.
10. Listing and opening run on a background `std::thread` and report `PullRequestEvent::Listed`, `ListFailed`,
    `Opened`, `OpenFailed`. A missing `gh` or `git`, no login, no network or a non-GitHub `origin` gives a `*Failed`
    event with the tool's message; nothing else in the app is affected.
11. Only read-only `gh` commands run (`gh pr list`). Cellworks never writes to the forge in this story.
12. No UI and no key binding in this story; STORY-022 consumes the events.

## Decisions

- **Sources owns the adapters; the binary supplies the mapping.** Sources may not depend on CodeAnalysis (arc42 05),
  so opening takes a `&dyn Fn(&Path) -> Option<ProjectMap>` and the binary passes one built on
  `cellworks_code_analysis::load`. Rejected: a Sources → CodeAnalysis dependency (breaks the context map); mapping in
  the binary after Sources returns the raw diff (spreads review assembly over two places).
- **Two `gh` calls, open and merged with `--limit 30`.** Rejected: one `--state all --limit N` call, which also returns
  closed-unmerged pull requests and cannot give all open plus exactly 30 merged.
- **Parse `gh` output from a `--template` of tab-separated fields.** No JSON dependency is added; lines that do not
  parse are ignored (tolerant parsing, architecture guideline). Rejected: adding `serde_json` to Sources for six fields.
- **git for diffs, `gh` for the list** (operator). Rejected: `gh pr diff`, which fails above 20,000 lines (HTTP 406 on
  #113) and took 22 s instead of 2 s for 15 pull requests.
- **City at the head** (operator). Rejected: the open checkout's city, whose line numbers drift.
- **Story from title or branch only** (operator). Rejected: the body and commit trailers.
- **One tower per removed line** (requirement 7). Rejected: the prototype's span rule, which counted a removal at the
  boundary of two towers on both.
- **Diff paths from the `---`/`+++` lines with `strip_prefix("a/")`/`("b/")` once.** Rejected: `trim_start_matches`,
  which eats `a/a/x`; parsing the `diff --git` header, which is ambiguous with spaces in paths.
- **A `--- `/`+++ ` line is a file header only between hunks** (review finding): the prefix alone cannot tell a header
  from content, because a removed source line whose own text starts with `-- ` (a SQL, Lua, Haskell or Ada comment)
  arrives as `--- <text>`, and an added one starting with `++ ` as `+++ <text>`. Read as a header it would enter the
  line's text into `Diff::files` as a phantom file, taking the hunk's remaining lines with it — a wrong `Size::files`
  and lines placed against a path no project map holds, against requirements 3 and 8. So `parse_hunk_header` now keeps
  the line counts it used to discard, and `parse_diff` counts down what each hunk still owes: inside a body the
  `-`/`+` branches win, outside one the header branches do. A line that cannot belong to a body (a `diff --git`
  arriving early) resyncs the counters to zero, so a malformed hunk header loses only its own hunk. Rejected:
  requiring the `---` to be preceded by a `diff --git` line, which is absent from `git diff` output fed through
  filters and still cannot resolve a `+++ ` directly after a `--- `.
- **No rename detection** (no `-M`): a rename is a delete plus an add. Rejected for now: `-M`, which needs old/new path
  pairs no consumer asks for yet.
- **`PullRequest` carries no size.** Size, flags and placements belong to `Review`, because diffs are lazy
  (requirement 4). Rejected: computing size during listing, one diff per listed pull request.
- **The temporary worktree lives under the OS temp directory and is removed by a drop guard.** Rejected: a worktree
  inside the project folder (would appear on the open city and in `git status`).
- **The prototype's `examples/pr_review.rs` is deleted**; the acceptance test replaces it.
- **The worktree folder is created exclusively, with `0o700` permissions, before `git worktree add` ever sees it, and
  its path carries a random component beyond the pid and pull-request number** (security review finding): `git
  worktree add` accepts an existing *empty* directory, so a predictable, world-creatable path would let another local
  user on a shared machine pre-stage it — reading the checkout as git writes it, or replacing a subdirectory with a
  symlink. Creating it ourselves first, exclusively, makes that race fail safely (`OpenFailed`) instead of writing
  into a directory we do not own; the random component means there is nothing worth guessing in the first place.
  Rejected: relying on the temp directory's own permissions alone, which does not stop a pre-staged symlink.
- **A listed pull request's head and base commits are validated as plain hex before any `git` call**
  (`forge::looks_like_a_commit`, security review finding): `headRefOid`/`baseRefOid` are ordinarily git-computed
  hashes, but a title holding a literal newline splits `gh`'s template output into a forged extra record whose every
  field — head and base included — is the pull request author's own text, and an unvalidated value starting with `-`
  reaches `git diff`/`git fetch`/`git worktree add` as an option, not a revision. A line failing this check is
  skipped like any other malformed line (tolerant parsing, requirement already in place for a short line). Rejected:
  trusting `gh`'s JSON shape to rule this out, which it does not — the vector is a `\n` inside a string field, not a
  JSON-structure break.

## Order of work

1. [x] Diff parsing and critical-change flags: `crates/sources/src/pull_requests/diff.rs` (`parse_diff`, `Diff`,
   `FileChange`, `Removal`, `Size`), `crates/sources/src/pull_requests/critical.rs` (`read_flags`, `glob_matches`,
   `flags_raised`), with unit tests; `crates/sources/src/pull_requests/mod.rs` declaring them;
   `crates/sources/src/lib.rs` declaring `pull_requests`. Replaces the prototype's `pull_requests.rs`.
2. [x] Adapters: `crates/sources/src/pull_requests/forge.rs` with traits `Forge` and `Checkout`, `ForgeUnavailable`,
   `GhCli` (two listing calls, template parsing) and `GitCli` (fetch when absent, diff, `worktree add --detach`,
   `worktree remove --force`); the repository is the child's working directory and argument vectors are fixed.
   Recorded fixtures `crates/sources/tests/fixtures/gh_pr_list_open.txt`, `gh_pr_list_merged.txt`;
   `crates/sources/tests/git_cli_against_real_git.rs`.
3. [x] Placement: `crates/sources/src/pull_requests/placement.rs` (`place(&Diff, &ProjectMap, root)` to
   `Vec<FilePlacement>`, requirements 7 and 8), with unit tests on a hand-built `ProjectMap`.
4. [x] Review assembly, story and events: in `crates/sources/src/pull_requests/mod.rs` the types `PullRequest`,
   `PullRequestState`, `Review`, `PullRequestEvent`, `story_of`, blocking `list` and `open`, and `start_list`,
   `start_open` threads; re-exports in `crates/sources/src/lib.rs`; acceptance test
   `crates/sources/tests/story_021_pull_request_source.rs`.
5. [x] Binary wiring and cleanup: `src/review.rs` (the map-a-checkout function over `cellworks_code_analysis::load`,
   `GitCli` and `cellworks_map::default_prefix_rule()`, and a helper that starts opening a review with `GhCli` and
   the Sources `GitCli`), its module line in `src/lib.rs`; delete `examples/pr_review.rs`; the Docs impact lines that
   the build writes.

## Risks

- `gh` not installed or not logged in, or `origin` not on GitHub: only this source fails, with a message in
  `ListFailed`/`OpenFailed`; covered by tests with failing fakes.
- Fetching `pull/<n>/head` writes objects and `FETCH_HEAD` into the user's repository. It creates no branch and moves
  no ref the user owns; arc42 03 is corrected accordingly.
- The temporary worktree of a large repository costs seconds on the background thread; a crash can leave one behind,
  which `git worktree prune` clears. This is the riskiest step (step 5 together with step 2).
- Removed-line attribution at tower boundaries is a heuristic, documented on `place`.
- Rollback: revert the squash commit; nothing else reads the new module until STORY-022.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: all
  pass, no warnings. No test runs `gh` or touches the network.
- `crates/sources/tests/story_021_pull_request_source.rs`:
  - `open_and_merged_pull_requests_are_listed_with_their_story` — fake forge, `start_list` yields `Listed` with the
    open ones first, then merged, stories from title and branch, none for an epic pull request.
  - `an_opened_pull_request_has_its_size_flags_and_lines_on_towers` — fake checkout with a recorded diff touching
    `Cargo.lock` and a Rust file, fake map: `Opened` with size, flag `dependencies` naming `Cargo.lock`, added floors
    and removed lines on the right towers, lines outside towers counted.
  - `a_failing_forge_reports_the_list_as_failed` and `a_failing_fetch_reports_the_open_as_failed`.
  - `the_temporary_worktree_is_removed_when_mapping_fails`.
- Unit tests: `diff.rs` (modified, added, deleted and binary files; removal at file start and end; `a/a/x`; two hunks;
  changed lines whose own content starts with `-` or `+`, which requirement 8 forbids dropping; changed lines reading
  like a `--- `/`+++ ` header, asserting the file count so no phantom file passes; the next file still found after
  such a hunk; a `\ No newline at end of file` marker owing no line),
  `critical.rs` (`*` across `/`, case-insensitive, missing file, malformed line), `story_of` (title, branch, title
  wins, none, `story-` without digits), `forge.rs` (recorded `gh` output, malformed line ignored, fixed argument
  vectors), `placement.rs` (two towers, boundary removal on exactly one tower, removal after last floor, nested
  towers pick the innermost, file without towers).
- `crates/sources/tests/git_cli_against_real_git.rs`: `GitCli::diff` and worktree add/remove on a temporary repository
  with two commits, no network.
- Manual smoke, not in CI: against `squer-solutions/cellworks` #117 (17 files, +1374/-103, flag `dependencies`),
  #113 (+37394/-414, no story) and #116 (two `.github/` files, all lines outside towers), as recorded in `prototype.md`.

## Docs impact

- **Business decisions:** `docs/domain/contexts/Sources/Sources.md`: a pull request implements at most one story, found
  as `STORY-NNN` in its title or branch; the list holds open pull requests and the last 30 merged.
- **Glossary:** `docs/domain/contexts/Sources/glossary.md`: Pull request, Critical-change flag, Review; the Sources
  count in `docs/domain/glossary.md`.
- **Guidelines:** `docs/services/cellworks/guidelines/architecture.md`, External tools: `gh` joins the child-process
  tools, read-only commands only.
- **arc42:** `03-context.md`: GitHub through `gh` in the diagram, the neighbour table and 3.2; the git entry no longer
  says Cellworks never fetches (it fetches pull-request heads). `05-building-blocks.md`: the Sources row names the
  pull-request source. `09-decisions.md`: the ADR-0009 line.
- **ADR needed:** yes, ADR-0009 "Pull requests through gh and git", recorded at freeze with the operator.
