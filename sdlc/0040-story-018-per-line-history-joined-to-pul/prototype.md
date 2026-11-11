# Prototype: STORY-018 Per-line history joined to pull requests and stories

## Confirmed behavior

- Reading a project that is a git repository also reads its per-line history: for every visible file, the commit
  that last wrote each line, from `git blame --porcelain` run as a child process (ADR-0003).
- The project's commits are listed oldest first (`git log --reverse`), so a replay can step through them in order.
- Each commit carries its full id, commit time, subject, the pull request number and the story ids.
- The pull request is the last `(#N)` in the commit subject, as a squash merge on GitHub writes it
  (`feature: story-003-… (#5) (#19)` is pull request 19).
- The stories are every `STORY-NNN` the commit message names, in any case, in order of first mention, normalised to
  upper case with three digits (`story-003` becomes `STORY-003`). A commit naming no story has none.
- A floor knows its commit through its file and line number; a line not yet committed has no commit.
- Measured on the cellworks repository: 220 files and 27 commits read in about 1.1 s, blame spread over 8 threads;
  24 commits name a pull request, 18 name a story, 37,404 of 45,094 lines come from a commit naming a story.

## Feedback log

None.

## Rejected

- Asking GitHub, per pull request, for its own commits to attribute one story per line: network, slower, and it
  overlaps EPIC-012's pull-request source (STORY-021). Declined by the operator.
- Attributing only the first story a commit names: wrong for epic pull requests that squash several stories.
  Declined by the operator.
- Fetching pull request titles or URLs with `gh` in this story: the pull request source is STORY-021's. Declined by
  the operator.

## Edge cases

- Epic pull requests squash many stories into one commit (for example `EPIC-010: Lenses (#80)` names seven stories):
  each of its lines carries all seven; the why panel (STORY-019) weights shares across them. One-story pull requests,
  the Factory flow, stay exact.
- Nested pull request references in a subject (`(#5) (#19)`): the last one is the pull request merged into the
  current branch.
- Commits before the Factory flow (no `(#N)`, no story): the line has a commit and no pull request or story.
- Uncommitted lines (blame's all-zero commit id): no commit.
- A folder that is not a git repository: no history; nothing else changes.

## Decisions

- Q: How should a floor attribute stories when its commit names several? → A: All named stories; the why panel
  weights shares across them. Local only, no network.
- Q: Is the commit message enough for the pull request, or should this story also fetch titles and URLs? → A: The
  number only, from the commit subject; titles and bodies come later from EPIC-012's pull-request source (STORY-021).

## Evidence

- Branch `proto/line-history`, commit `300761d`, cut from `origin/test` at `86f8a51`.
- `crates/code-analysis/src/history.rs` (new): `git log` and parallel `git blame` parsing, `History::of_line`.
- `crates/code-analysis/examples/history.rs` (new): loads a project, prints the history summary, a tower's floors with
  commit, pull request, stories and subject, and the first commits of the replay order.
- `crates/code-analysis/src/lib.rs`: `pub mod history;`.
- Run headless on the cellworks repository:
  `cargo run --release -p cellworks-code-analysis --example history -- . visible crates/code-analysis/src/git.rs`
  showed `visible_files` and `visible` built by `ada5efe #19 STORY-003,STORY-005`; the same with
  `floor_z crates/map/src/city.rs` showed floors from `#68 EPIC-009` and `#80 EPIC-010`, each naming several stories.
