# Prototype: STORY-021 Pull-request source with diffs and critical-change flags

Issue #43. A vertical punch-through from the forge to the city: pull requests listed through `gh`, their diffs read
through `git`, the critical-change flags matched from `sdlc/critical.yaml`, and the added and removed lines placed on
the towers and floors of the city that CodeAnalysis builds at the pull request's head. Run headlessly against the
cellworks repository's own pull requests.

## Confirmed behavior

- The project's pull requests are listed through `gh pr list --state all`: number, state (open or merged), title, head
  branch, head and base commits, author.
- The list holds the open pull requests and the last 30 merged ones.
- A pull request's story is the first `STORY-NNN` in its title, else in its head branch name (case-insensitive,
  `feature/83-story-058-…` gives `STORY-058`). A pull request with neither implements no story.
- A pull request's diff is read with git, not the forge: its head is fetched with `git fetch origin pull/<n>/head` when
  the commit is not present, its base when not present, then `git diff -U0 <base>...<head>`.
- From the diff, each file has the line numbers it adds (new-side) and the lines it removes (old-side), each removed
  line with the new-side line it was removed before.
- A pull request's size is the sum of added and removed lines and the number of files.
- The critical-change flags are read from the project's `sdlc/critical.yaml` (one flag per line under `flags:`, one
  quoted glob per `- ` line). A file raises a flag when its path, lowercased, matches one of the flag's globs; `*`
  matches any run of characters, `/` included. Each raised flag carries the files that raised it.
- The added and removed lines are mapped onto towers and floors of the city at the pull request's head: the head is
  checked out into a temporary detached worktree, mapped by CodeAnalysis, and each changed file's towers are found by
  its path. An added line lands on the floor with its number; a removed line lands on the tower whose floors span the
  line it was removed before.
- Lines that no tower holds (blank lines and comments between items, files no loader reads, files in hidden folders
  such as `.github/`) are counted as outside any tower, not dropped.
- Observed on real pull requests: #117 (open, STORY-058, 17 files, +1374/-103, flag `dependencies` from `Cargo.lock`
  and `crates/code-analysis/Cargo.toml`; lines on towers such as `typescript.rs/items`, floors 106-148); #113
  (merged promotion, +37394/-414, 179 files, no story); #116 (merged, STORY-080, two files under `.github/`, all lines
  outside any tower).

## Feedback log

None.

## Rejected

- Reading diffs with `gh pr diff`: GitHub refuses diffs above 20,000 lines (HTTP 406 on #113), and one call per pull
  request made listing 15 pull requests with their sizes take 22 s instead of 2 s with git.
- Mapping lines onto the city of the open checkout: line numbers drift wherever the checkout differs from the pull
  request's head.
- Finding the story in the pull request's body and commit trailers too: declined; title and branch only.
- Listing only open pull requests, or every pull request without a limit: declined in favour of open plus the last 30
  merged.

## Edge cases

- Diff too large for the forge (#113): handled by git; no limit.
- Deleted file: named only on the `---` side; its removed lines are kept under its old path.
- Added file: named only on the `+++` side.
- Pull request without a story (epic and promotion pull requests such as #98, #113): story is none.
- File in a hidden folder or a file no loader reads (`Cargo.lock`): no towers; its lines count as outside any tower.
- Project without `sdlc/critical.yaml`: no flags.
- Removed lines at the boundary between two towers may match both in the prototype's span rule; the build attributes
  each removed line to exactly one tower.

## Decisions

- Q: Keep the split, `gh` lists and `git` diffs? → A: Yes, gh lists, git diffs.
- Q: Which city do a pull request's lines land on? → A: The city at the pull request's head.
- Q: How is a pull request's story found? → A: `STORY-NNN` in the title or branch.
- Q: Which pull requests are listed? → A: The open ones plus the last 30 merged; diffs read lazily when a pull request
  is opened, on a background thread.
- Q: Is the punch-through the behaviour wanted, freeze it? → A: Yes, freeze it.

## Evidence

- Branch `proto/pr-source` off `origin/test` (86f8a51), worktree `../cellworks-proto-pr-source`.
- Files: `crates/sources/src/pull_requests.rs` (gh listing via `--template`, git diff, unified-diff parser,
  `critical.yaml` reader, glob matcher), `crates/sources/src/lib.rs` (module), `examples/pr_review.rs` (headless
  driver: list, fetch head into a detached worktree, `cellworks_code_analysis::load`, `ProjectMap::items_under`).
- Run: `cargo run --example pr_review -- . 117 <scratch-dir>` and `cargo run --example pr_review -- .` in the worktree,
  against `squer-solutions/cellworks` with the operator's `gh` login.
