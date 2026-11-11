# Prototype: STORY-081, steer a session from its session pane

## Confirmed behavior
- Double-clicking a session (its dot on the map or its row in the sessions sidebar of STORY-030) opens a resizable
  pane at the bottom of Cellworks, titled with the session's name. Opening another session replaces the pane's content.
- For a background session (one `claude agents --json` lists with an `id`), the pane runs `claude attach <id>` in a
  pseudo-terminal with a clean environment (ADR-0010) and shows the real Claude Code screen: colours, inverse video,
  cursor. While the pane has focus, text, paste, Enter, Backspace, Tab, arrows, Esc and Ctrl+letter go to the session,
  and the map's keys do nothing; the terminal follows the pane's size. The engineer replies and answers approvals in
  the session's own screen.
- Closing the pane (its close button, or Esc while the pane has no focus) detaches: only the attach process ends, the
  session keeps running.
- For an interactive session (desktop app or terminal; no attach id), the pane shows a live read-only conversation from
  its main transcript: the engineer's prompts, the assistant's text, tool calls as one-line entries, newest at the
  bottom. A banner says "Started outside Cellworks (interactive). Read-only here." with "Continue in the background".
- "Continue in the background" runs `claude --resume <sessionId> --bg -n "<name>"`, which starts a background copy of
  the conversation while the original keeps running; the pane switches to the copy's terminal and its banner reads
  "Background copy of <name>; close the original where it runs." A failure shows the command's output under the banner.
- Stop (confirmed, not prototyped): a Stop control in the pane header runs `claude stop <id>` for a background session;
  the session leaves the city and the sidebar.

## Feedback log
- "i wish i could open up the session within cellworks when double clicking it to see what the agent is doing and
  respond to questions if necessary" → the session pane on double-click.
- "Embedded terminal (Recommended)" → the pane is the real Claude Code screen through `claude attach`.
- "Offer to move it to the background" → interactive sessions get a read-only view and "Continue in the background",
  realised as a background copy so the original is never killed.
- "Animations and the session pane are right; freeze STORY-030 and STORY-081." → frozen.
- "Pane + Stop (Recommended)" → Stop added; the mock's card summary dropped.

## Rejected
- A conversation view rendered by Cellworks with its own reply box and approval buttons (mock v13): approvals would
  have to be screen-scraped; the real terminal shows them.
- The card above the pane with the last request, last message and trail of towers (mock v13).
- Killing the interactive original when moving it to the background: a background copy is started instead.
- Both views in tabs.

## Edge cases
- `claude attach` accepts only background sessions; an interactive session has no attach id ("No job matching ...").
- Esc inside the focused terminal goes to Claude Code (it interrupts there); detaching uses the close button, or Esc
  after clicking out of the pane.
- egui may turn Ctrl+C into a copy event; Ctrl+C must still reach the session.
- `--resume <id> --bg` on a running session prints "started a copy as <id>" and "backgrounded · <id> · <name>"; the id
  is the token after "backgrounded".
- The read-only view skips sidechain lines and text that starts with `<` (system reminders, command output) and reads
  only the last 1 MB of a transcript.

## Decisions
- Q: Double-clicking a session opens what? → A: the embedded terminal (`claude attach`).
- Q: What does double-clicking an interactive session do? → A: a read-only view with "Continue in the background".
- Q: What does STORY-081 contain? → A: the pane and Stop; no card summary.
- Q: Where do reply and answer happen? → A: in the session's own terminal in the pane.

## Evidence
- Branch `proto/session-card`, from `proto/sessions-walk` bc2687a (STORY-030 frozen there). STORY-081's prototype
  commits: 308263d (attach pane), b429015 (read-only view and continue in background).
- Files: `crates/agent-sessions/src/terminal.rs`, `crates/agent-sessions/src/lib.rs` (`Entry`, `conversation`,
  `parse_bg_id`, `continue_in_background`, `Session.attach_id`), `src/app.rs`,
  `crates/agent-sessions/examples/attach_screen.rs`, `crates/agent-sessions/examples/conversation.rs`; crates
  `portable-pty` 0.9 and `vt100` 0.18.
- Run: `attach_screen` printed the real screen of a background session (its open question and prompt box) without
  sending keys; `conversation` printed the last 15 entries of an interactive session; the user drove the pane in
  `target/release/cellworks`.
