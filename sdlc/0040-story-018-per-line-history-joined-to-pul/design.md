---
stage: design
issue: 40
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-018 Per-line history joined to pull requests and stories

The contract is `issue.md` in this directory: its "Done when" list is what the change must do. The confirmed
prototype is `prototype.md` beside this note; its Decisions and Rejected sections bind this design. Decision record:
ADR-0008.

## Open questions

None.

## Requirements

1. `ProjectMap` gains `history: LineHistory` (map crate), empty by default and for a folder that is not a git
   repository. `LineHistory { commits: Vec<Commit>, lines: BTreeMap<PathBuf, Vec<Option<usize>>> }`;
   `Commit { sha, time, summary, pull_request: Option<u32>, stories: Vec<String> }`.
2. `commits` are oldest first. Every `Some(index)` in `lines` indexes `commits`. A line index is `Floor.number - 1`.
3. `LineHistory::of_line(file, number) -> Option<&Commit>`: `number` from 1, as `Floor.number`; 0, past the end, an
   unknown file or an uncommitted line give `None`.
4. `lines` is keyed by the same path form the Map uses for a floor's file (check `ProjectMap::of_code` and
   `co_change`, and key by that form), so a floor's commit is found from its item's file and `Floor.number`.
5. Pull request: the last `(#N)` of the subject (first message line); `None` without one or for a malformed one.
   Stories: every `STORY-` followed by digits in the whole message, any case, first-mention order, de-duplicated,
   upper case with at least three digits (`story-3` and `story-003` are `STORY-003`).
6. git output is read only through `GitListing` in `git.rs`, the one module that spawns a process: new methods `log`
   and `blame` whose defaults return empty answers. The repository is the working directory; a file path follows
   `--`. Blame of a file that fails (binary, untracked, gone) leaves that file out without failing the rest; a
   failing `log` leaves `history` empty. No new event.
7. History is read inside `load()` on the load thread, never on the UI thread. Nothing is fetched from GitHub; no `gh`.
8. Additive only: `Floor`, the parsers in `rust/` and `plain.rs`, and the existing `history` (co-change) method are
   untouched. The prototype's `pub mod history` and `examples/history.rs` do not ship.

## Decisions

- Types in the map crate as `ProjectMap.history`, next to `co_changes`; rejected a field on `Floor` (STORY-058 is
  changing the parsers that build floors) and a separate event (the loader already hands over one `ProjectMap`).
- A line holds an index into `commits`; rejected a sha string per line (45k lines on this repository).
- `GitCli::blame` spreads files over about 8 `std::thread::scope` workers inside the method; rejected requiring
  `&(dyn GitListing + Sync)` on `load`, which breaks the `Cell`-based recorded listings in tests.
- Output parsing (`parse_log`, `parse_blame`) lives in `git.rs`; the join with pull request and story extraction in a
  private `history.rs` in code-analysis; rejected parsing git formats outside `git.rs`.
- All stories a commit names are kept; rejected asking GitHub per pull request and crediting the first story only
  (operator decision, see `prototype.md`).
- The pull request is a number only; titles and URLs come from STORY-021; rejected fetching them with `gh` here.
- `git log --reverse --format=%H%x1f%ct%x1f%B%x1e` and `git blame --porcelain -- <file>`; rejected `--line-porcelain`
  (repeats every header per line, several times the output).

## Order of work

1. Map types: `crates/map/src/history.rs` (new) with `Commit`, `LineHistory`, `of_line`; re-export in
   `crates/map/src/lib.rs`; `pub history: LineHistory` on `ProjectMap` in `crates/map/src/graph.rs`, initialised empty
   where `ProjectMap` is built; fix any `ProjectMap { .. }` literal elsewhere. Unit tests for `of_line`.
2. git access: `crates/code-analysis/src/git.rs` gains `LoggedCommit`, `GitListing::log`, `GitListing::blame` with
   empty defaults, the `GitCli` implementations, `parse_log` and `parse_blame`, with unit tests on recorded output.
3. Join and wiring: `crates/code-analysis/src/history.rs` (new, private) with `join`, `pull_request`, `stories`;
   `crates/code-analysis/src/lib.rs` calls `log` and `blame` in `load()` when the folder is a repository and sets
   `map.history`; update the crate and `load` doc comments. Unit tests for the join and an acceptance test through
   `load` with a recorded listing.

## Risks

- Path keying (requirement 4) is the riskiest step: a mismatch makes every lookup `None` while all unit tests pass;
  the acceptance test through `load` must look a floor up from the loaded map's own item.
- Load time grows by about 1 s on this repository and linearly with file count; mark the ceiling with a `ponytail:`
  comment (upgrade path: blame lazily per opened district).
- Rollback: revert the squash commit; the field is additive and nothing reads it yet.

## Proof

- `cargo test --workspace` passes, including:
  - `map::history`: `a_floor_finds_the_commit_that_last_wrote_it`, `line_zero_past_the_end_and_unknown_files_have_no_commit`.
  - `git.rs`: `blame_gives_one_commit_per_line_in_order` (repeated groups without headers, all-zero id is none),
    `the_log_lists_commits_oldest_first_with_their_whole_message`.
  - `history.rs`: `the_pull_request_is_the_last_number_in_the_subject`, `every_story_the_message_names_is_kept`
    (seven for an epic message, `story-3` is `STORY-003`), `uncommitted_lines_and_unknown_commits_have_none`.
  - `lib.rs` acceptance: `each_floor_knows_its_commit_pull_request_and_stories` (recorded listing, lookup from the
    loaded item's file and floor number), `a_folder_without_git_has_no_history`.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all --check` clean;
  `SHELLCHECK_OPTS=--exclude=SC2016 actionlint` zero findings. No test runs real git.

## Docs impact

- **Business decisions** — None.
- **Glossary** — Commit (shared glossary), Floor provenance and Replay (CodeAnalysis glossary); written at freeze.
- **Guidelines** — None.
- **arc42** — `09-decisions.md`: index line for ADR-0008; written at freeze.
- **ADR needed** — yes: ADR-0008 "Per-line provenance from git blame, joined to pull requests and stories by commit
  message", written at freeze.
