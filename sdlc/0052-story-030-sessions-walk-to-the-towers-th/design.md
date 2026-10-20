---
stage: design
issue: 52
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-030, sessions walk to the towers they work on

The contract is `issue.md` in this directory. Confirmed behaviour, feedback, rejections and edge cases are in
[prototype.md](prototype.md); the binding decision is
[ADR-0010](../../docs/adr/0010-claude-code-background-sessions.md). The prototype on `proto/sessions-walk` is evidence
only (`git show 2e05e9f:crates/agent-sessions/src/lib.rs`, `git diff origin/test 2e05e9f -- src/app.rs`); build from
`test`.

## Open questions

Resolved by @Xunrise on 2026-10-20: approved for the build. The critical-area hold (payments) is a false positive:
the design's "checkout" means a git checkout (a worktree or the main folder of the repository), matched by
`*checkout*` in sdlc/critical.yaml. No payment, billing or checkout-flow code is touched.

None.

## Requirements

1. Built after STORY-029 (#51) is merged: this story amends the chapter 3 text that story adds.
2. Only sessions that `claude agents --json` lists as running appear; one that ends leaves the city and the sidebar.
3. Events come from the main transcript `<sessionId>.jsonl` and `<sessionId>/subagents/*.jsonl`. `Read` is a read at
   `input.offset` (default 1). `Edit` is a write whose line is where `new_string` sits in the file on disk, and `added`
   is the lines of `new_string` minus the lines of `old_string`, at least 0. `MultiEdit` sums `added` and takes the
   first edit's line. `Write` is a write at line 1 that adds nothing. Every `dir/name.ext` token of a `Bash` command or a
   `Grep` / `Glob` path that names an existing file is a read without a line. Relative paths resolve against the line's
   `cwd`. Unknown tools, blocks and unparsable lines are ignored. A session keeps its last 50 events.
4. An event is in the city when its file's checkout shares the open project's git common dir (any worktree). Paths are
   stored relative to their own checkout root. For a project that is not a git repository, a path prefix decides.
5. A session stands at the latest event that lands on a tower: the smallest item whose floors hold the line; without a
   line, the file's first function; else the file's node. A file the city lacks leaves it at the last placed tower.
6. State: `state` waiting or blocked → Waiting; `status` busy or `state` working → Working; else Idle.
7. Animation runs on frame time: a 1.5 s eased walk along x then y at ground level with a fading trail, a 0.4 s climb,
   then a bustle (slow cyan drift reading, faster with yellow sparks writing), a yellow pulse when waiting. A session
   seen for the first time appears at its tower. Repaints are always delayed: 33 ms while something moves, else 500 ms.
8. Writing draws `added` marked floors on top of the tower while the session works there; the map is not re-read.
9. Sessions at one tower fan out around it (about 13 by 7 px), labels stacked 15 px apart to its right, with a leader
   line and a dark box behind each.
10. Sidebar (`Panel::left`, 300 px, only while a project is open and the app has session sources): heading
    "Sessions"; summary "N running · M need you · K in this city", or the listing error; search over name, repo and
    branch; groups Needs you / Working / Idle for this city, then Other projects, each newest first, empty groups left
    out. A row: status dot, name, "repo · branch", up to two lines of the last assistant text (110 characters), state
    and age ("now", "N min", "N h", "N d", "-").
11. Clicking a dot (9 px) or an in-city row selects the session without selecting a tower; a row click and `[` / `]`
    also centre the city on its tower. Esc clears a session selection before anything else. While the search field
    has focus, the map's letter and bracket keys do nothing.

## Decisions

- Everything lives in `agent-sessions` (watch, parsing, placement, `ui` module with the sidebar and the overlay); the
  binary holds one `SessionsView`. Rejected: the prototype's five fields and painting code in `src/app.rs`.
- `claude agents --json` sits behind a trait `AgentList`; `ClaudeCli` runs it with `env_clear()` plus `HOME`, `PATH`,
  `USER`, `TERM`. The transcripts folder and the poll interval are passed in (`SessionSources`). Rejected: reading
  `$HOME` and running `claude` inside the crate, which tests could not replace.
- `serde_json = "1"` becomes a direct dependency of `agent-sessions` only (already in `Cargo.lock` through eframe).
  Rejected: a hand-written parser for nested JSON lines.
- Repository identity, common dir and branch are read from git's files (`.git` folder, or `gitdir:` plus `commondir`,
  and `HEAD`). Rejected: `git rev-parse` per checkout, another child process and test seam.
- Transcripts are tailed from a byte offset per file; a shorter file is read again. The summary and the newest
  timestamp fold forward over each poll's new lines only, and the per-session cache is dropped once `claude agents
  --json` stops listing the session, so no transcript text stays resident or is parsed twice. Rejected: re-reading
  whole files every 2 s, and keeping every line read so far to re-derive the summary over all of it.
- Floors being written are an overlay. Rejected: rescanning the map on writes (a separate concern).
- `start()` gets sources from `SessionSources::for_this_user()`; `over()` gets none, so existing tests see no sidebar;
  `with_sessions(..)` adds them in tests.

## Order of work

1. Parsing and repositories: `crates/agent-sessions/Cargo.toml` (egui, serde_json; dev tempfile),
   `crates/agent-sessions/src/claude.rs` (`AgentList`, `ClaudeCli`, `ClaudeUnavailable`, `parse_agents`),
   `crates/agent-sessions/src/checkout.rs` (`checkout_root`, `common_dir`, `branch`, `repo_name`),
   `crates/agent-sessions/src/transcript.rs` (`Tail`, `tool_events`, `summary`, `epoch`), fixtures under
   `crates/agent-sessions/tests/fixtures/`, with their unit tests.
2. Sessions and the watch: `crates/agent-sessions/src/lib.rs` (`Session`, `FileEvent`, `SessionState`,
   `SessionEvent { Listed, ListingFailed }`, `SessionSources`, `start_watch`, `place`, `groups`, `fan`, `ago`), with
   unit tests.
3. The view: `crates/agent-sessions/src/ui.rs` (`SessionsView`: `watching`, `drain`, `sidebar`, `paint`, selection and
   accessors, `DrawnSession`).
4. The binary: `src/app.rs` (fields, `with_sessions`, `open`, `apply_pending`, `sessions()`, the left panel, `paint`
   after the map, dot and row selection, `[` / `]`, Esc, keys ignored while the search has focus), and
   `tests/common/mod.rs` (`shop_tree`).
5. Acceptance tests: `tests/story_030_sessions_walk_to_the_towers_they_work_on.rs`, one per scenario.

## Risks

- Neither `claude agents --json` nor the transcript format is a published contract; a Claude Code update shows as
  sessions that stop moving. Recorded fixtures pin today's shape.
- The Bash token rule misses `cat README.md` (no slash) and counts a file merely mentioned.
- Riskiest step: 4, the key routing in `draw`; the existing acceptance tests guard it.
- egui_kittest's `Harness::run` follows only immediate repaints; an immediate `request_repaint()` would make every
  session test hit `max_steps`.
- Rollback: revert the squash commit; nothing persists outside the binary.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass.
- `tests/story_030_sessions_walk_to_the_towers_they_work_on.rs`: `a_session_walks_to_the_tower_it_reads`,
  `naming_a_file_in_a_command_counts_as_reading_it`, `writing_adds_floors_while_the_session_writes`,
  `a_session_waiting_for_the_engineer_asks_for_attention`, `sessions_at_the_same_tower_stand_apart`,
  `a_session_working_in_another_checkout_of_the_project_is_in_the_city`,
  `a_session_in_another_project_is_not_in_this_city`, `selecting_a_session_in_the_sidebar_brings_it_into_view`,
  `a_session_that_ended_leaves_the_city` — headless, with a recorded agent list and transcripts in a temp folder; no
  test runs `claude` or reads `$HOME`.
- Unit tests: `agents_are_read_tolerantly`, `tool_calls_become_file_events`, `a_tail_reads_only_new_complete_lines`,
  `the_summary_is_the_last_assistant_text_and_newest_time`, `checkouts_of_one_repository_share_a_common_dir`,
  `a_session_stands_at_the_smallest_tower_holding_its_line`, `groups_fan_and_ago`.

## Docs impact

- **Business decisions:** None.
- **Glossary:** AgentSessions glossary: "Sessions sidebar" and "Waiting" (in the docs commit).
- **Guidelines:** None.
- **arc42:** chapter 3, Claude Code and transcripts: Bash, Grep and Glob paths, subagent transcripts, the waiting
  state (in the docs commit).
- **ADR needed:** no; ADR-0010 binds it.
