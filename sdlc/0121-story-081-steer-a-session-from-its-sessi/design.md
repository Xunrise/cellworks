---
stage: design
issue: 121
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-081, steer a session from its session pane

The contract is `issue.md` in this directory. Confirmed behaviour, feedback, rejections and edge cases are in
[prototype.md](prototype.md); the binding decision is
[ADR-0010](../../docs/adr/0010-claude-code-background-sessions.md). Evidence only: `git show 308263d` and
`git show b429015` on `proto/session-card`. Build from `test`.

## Open questions

None.

## Requirements

1. Built after STORY-029 (#51) and STORY-030 (#52) are merged: it extends STORY-030's `agent-sessions` watch,
   `SessionSources` and `SessionsView`; where the merged names differ from STORY-030's design, follow the code.
2. Double-clicking a session's dot (9 px) or its sidebar row opens a resizable bottom pane (default 340 px) titled
   "Session: <name>". Double-clicking the session already shown does nothing; another session replaces the pane. A
   double-click on a dot never also steps into the tower under it. Closing the project closes the pane.
3. A session with an attach id (the `id` of `claude agents --json`) runs `claude attach <id>` in a pseudo-terminal with
   the clean environment (HOME, PATH, USER, LANG, `TERM=xterm-256color`). The pane draws the screen in monospace
   cells with xterm 256 colours, RGB, inverse video and the cursor, and resizes the terminal to its size in cells.
4. While the terminal has focus, text, paste, Enter `\r`, Backspace 0x7f, Tab, Esc 0x1b, arrows and Ctrl+letter go to
   it (Copy as 0x03, Cut as 0x18), and the map's keys, `[` and `]` do nothing.
5. "Close (detach)", or Esc while the pane is open but the terminal has no focus, ends only the attach process; that
   Esc does nothing else that frame. "(attach ended)" shows when the child exits.
6. A session without an attach id shows a read-only conversation: the last 80 entries of its main transcript (last
   1 MB, re-read every 1 s on a background thread), prompts as "> text", assistant text, tool calls as one line
   ("Read <path>", "<Tool>: <command|pattern|path>", 80 characters), skipping sidechain lines, empty text and text
   starting with `<`. Banner: "Started outside Cellworks (interactive). Read-only here."
7. "Continue in the background" runs `claude --resume <sessionId> --bg -n <name>` on a thread; the copy's id is the
   alphanumeric token after "backgrounded". The pane then attaches to the copy with the banner "Background copy of
   <name>; close the original where it runs." A failure shows at most 200 characters of the output under the banner.
   The original is never stopped.
8. "Stop" (terminal panes only) runs `claude stop <id>` on a thread and closes the pane; the session leaves the city at
   the next listing. No confirmation.
9. Repaints are delayed: 33 ms while a terminal is shown, 500 ms for a conversation.

## Decisions

- Keep `portable-pty` 0.9 and `vt100` 0.18 (both MIT) as dependencies of `agent-sessions` only. Rejected: raw
  `openpty` through libc, `alacritty_terminal`, a hand-written VT parser.
- `SessionSources` gains `claude: PathBuf` (the program run to attach, stop and continue); tests point it at a fake
  `claude` shell script in a temp dir. Rejected: a second trait with an in-memory fake, leaving the pseudo-terminal and
  environment untested.
- One `clean_env()` in `claude.rs` serves the listing, stop, continue and the pseudo-terminal.
- Background results are new past-tense `SessionEvent` variants (`ConversationRead`, `CopyStarted`, `CopyFailed`,
  `Stopped`, `StopFailed`) on the pane's own channel. Rejected: a separate `PaneEvent` enum.
- Replying and answering happen in the session's own screen. Rejected: Cellworks' own reply box and approval buttons,
  and the card with the last request, last message and trail (mock v13).
- An interactive session moves to the background as a copy. Rejected: killing the original.
- `Terminal` drops by killing and reaping only its child, never blocking the UI thread.

## Order of work

1. Process layer: `crates/agent-sessions/Cargo.toml` (portable-pty, vt100), `crates/agent-sessions/src/claude.rs`
   (`clean_env`, `stop`, `continue_in_background`, `parse_bg_id`), `crates/agent-sessions/src/terminal.rs`
   (`Terminal::spawn`, `pump`, `screen`, `send`, `resize`, `has_ended`, `Drop`), with unit tests.
2. Session data: `crates/agent-sessions/src/lib.rs` (`Session.attach_id`, `SessionSources.claude`, the new
   `SessionEvent` variants, `SessionPane` with `open`, `drain`, `continue_in_background`, `stop`) and
   `crates/agent-sessions/src/transcript.rs` (`Entry`, `EntryKind`, `entries`, a start offset for `Tail`), with unit
   tests and a transcript fixture.
3. The pane UI: `crates/agent-sessions/src/ui.rs` (`SessionsView::open_pane`, `close_pane`, `has_keyboard`, `pane`,
   the terminal and conversation drawing, `key_bytes`, double-click on rows and dots).
4. The binary: `src/app.rs` (`sessions.pane(ui)` before the central panel, key gating, the Esc that closed the pane,
   dropping `drawn.entered` on a dot double-click, closing the pane with the project).
5. Acceptance tests: `tests/story_081_steer_a_session_from_its_session_pane.rs`.

## Risks

- The pseudo-terminal tests depend on timing and on `/bin/sh`, `cat` and `stty`; they poll with a 5 s deadline and are
  `#[cfg(unix)]`.
- `claude attach`, `--bg` output and the attach id are not a published contract (ADR-0010).
- Two new dependencies raise the Factory's dependencies flag: a person accepts them before the merge.
- Riskiest step: 4, key routing between the terminal and the map; STORY-030's and the existing key tests guard it.
- Rollback: revert the squash commit; nothing persists outside the binary, and stopped sessions keep their transcript.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass.
- `tests/story_081_steer_a_session_from_its_session_pane.rs`, headless, with a fake `claude` script; no test runs
  `claude`, opens a window or reads `$HOME`:
  `opening_a_background_session_shows_its_own_screen` (from a sidebar row and from a dot),
  `the_engineer_answers_a_session_in_its_pane` (typed text and Enter echo through `cat`; layer and selection unchanged),
  `closing_the_pane_leaves_the_session_running` (Close, and Esc after clicking the map; no `stop` recorded),
  `a_session_started_outside_cellworks_is_shown_read_only` (banner, prompt, text, one tool line; no sidechain or
  `<`-text; an appended line appears),
  `a_session_started_outside_cellworks_continues_in_the_background` (copy banner and the copy's attach), and
  `stopping_a_session` (`stop <id>` recorded, pane closed); plus `opening_another_session_replaces_the_pane` and
  `a_failed_continue_shows_the_commands_output`.
- Unit tests: `a_terminal_shows_what_its_child_prints_and_follows_its_size`, `the_child_gets_only_the_clean_environment`,
  `dropping_a_terminal_ends_its_child`, `the_copy_id_is_the_token_after_backgrounded`,
  `the_conversation_is_prompts_text_and_one_line_tool_calls`, `keys_become_terminal_bytes`.

## Docs impact

- **Business decisions:** AgentSessions: only background sessions are attached to, typed into or stopped; interactive
  sessions are read-only and never stopped; closing the pane ends only the attach (in the docs commit).
- **Glossary:** AgentSessions: "Session card" becomes "Session pane"; "Interactive session" and "Background copy" added
  (in the docs commit).
- **Guidelines:** None.
- **arc42:** chapter 3, Claude Code quirks: attach only for background sessions; the `--bg` output (in the docs commit).
- **ADR needed:** no; ADR-0010 decides the embedded pseudo-terminal.
