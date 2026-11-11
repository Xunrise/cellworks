# Prototype: STORY-030, sessions walk to the towers they work on

## Confirmed behavior
- Every Claude Code session that `claude agents --json` lists as running appears; a session that ends leaves the city.
- A session is in a project's city when the files it touches belong to the same git repository, in any checkout: the
  main folder or any worktree (`.claude/worktrees/<x>`, `../<repo>-proto-<x>`). Paths are made relative to their own
  checkout and looked up in the open city.
- File events come from the session's main transcript and its subagent transcripts (`<sessionId>/subagents/*.jsonl`):
  `Read` is a read; `Edit`, `MultiEdit`, `Write` are writes; and any existing file named in a `Bash`, `Grep` or `Glob`
  call counts as a read.
- A session stands at the tower whose lines hold the touched line (smallest span); without a line, at the file's first
  function, else at the file's block. A file the city does not have keeps the session at the last tower it could place.
- Sessions walk from tower to tower (about 1.5 s along the streets, then climb), bustle about the tower while they work
  (slow cyan drift when reading, faster with yellow sparks when writing), and writing grows the tower by the floors
  being written.
- A session whose state is `waiting` or `blocked` stays at its last tower and pulses yellow: it waits for the engineer.
- Sessions at the same tower fan out around it, with stacked labels, leader lines and a dark box behind each label.
- Clicking a session's dot selects it (ring under the dot) without selecting the tower; Esc clears the selection.
- A left sidebar lists every running session on the machine: header "Sessions", summary "N running · M need you · K in
  this city", a search over name, project and branch, groups Needs you / Working / Idle for this city, then Other
  projects. Each row: status dot, name, "repo · branch", the start of the last assistant message, state and time since
  last activity. Clicking a row in this city selects the session and centres the camera on its tower; `[` and `]` step
  through the sessions in this city.

## Feedback log
- "claude --bg (Recommended)" → sessions are observed through transcripts and `claude agents --json` (ADR-0010).
- "Tools + Bash paths (Recommended)" → file paths named in Bash, Grep and Glob calls count as reads.
- "Live ones, any checkout (Recommended)" → only running sessions; any checkout of the repository counts.
- "Pulse at its tower (Recommended)" → a waiting session pulses at the last tower it touched.
- "there is some overlap, also the agents should be selectable. Design a side bar menu for having an overview as well.
  I want to replace me working in claude code desktop with that menu" → fan-out with stacked labels, click to select,
  the sessions sidebar (mock v13, then the egui prototype).
- "New story in EPIC-014 (Recommended)" → steering (reply, answer, attach, stop) moved to STORY-081; + New to STORY-031.
- "the sidebar works ifne, but i wish i could open up the session within cellworks when double clicking it to see what
  the agent is doing and respond to questions if necessary. Also i'd love some animations on the agents within, so when
  they move to a new file, they walk over, when they are working on a file, they are busing about on that file, etc."
  → walking and working animations (this story); the session pane on double-click (STORY-081).
- "Animations and the session pane are right; freeze STORY-030 and STORY-081." → frozen.

## Rejected
- Only the dedicated file tools as the signal: about 7 events per session per day, so sessions mostly stand still.
- Showing sessions that ended in the last day (greyed out): only running sessions are shown.
- Only sessions in the open project's own folder: worktree sessions belong to the city too.
- Labels drawn over each other at a shared tower (v12 and the first egui build).
- Reply, answer, attach, stop and new-session controls in this story: they belong to STORY-081 and STORY-031.

## Edge cases
- Sessions read mostly through Bash (60 to 200 path mentions a day against about 7 Read calls) and through subagents.
- A file that exists only in a worktree is not in the open checkout's city: the session stays at its last placed tower.
- Files with no tower (for example `.claude/work-tracking.conf`): a session with only such events has no dot, but is
  listed in the sidebar.
- Sessions that touch no file of this repository are listed under Other projects with their repository name (the
  parent of `git rev-parse --git-common-dir`, else the cwd's last component).
- A session with no transcript yet shows an empty message and no age.
- `claude agents --json` reports `status` busy/idle and `state` working/done/waiting/blocked; either may be missing.
- `claude` must run with a clean environment (ADR-0010).

## Decisions
- Q: What moves a session to a tower? → A: file tools plus file paths named in Bash, Grep and Glob calls.
- Q: Which sessions appear in a project's city? → A: running ones, from any checkout of the repository.
- Q: How does a session that needs the engineer show? → A: it pulses at the last tower it touched.
- Q: Where do reply, answer, attach and stop go? → A: a new story, STORY-081; + New joins STORY-031.
- Q: Does the v13 sidebar look right? → A: yes.

## Evidence
- Branch `proto/sessions-walk` off `origin/test` 86f8a51: commits 7c9bae7 (punch-through), 7d7e982 (sidebar, selection,
  fan-out), 2e05e9f (animations); 308263d and b429015 are STORY-081's session pane.
- Files: `crates/agent-sessions/src/lib.rs`, `crates/agent-sessions/Cargo.toml`, `src/app.rs`,
  `crates/agent-sessions/examples/sessions_walk.rs`.
- Run: `cargo run --release -p cellworks-agent-sessions --example sessions_walk` printed 15 running sessions, 8 in the
  cellworks city, the sidebar groups and the fan-out; the user drove `target/release/cellworks` on the cellworks city.
- Mocks: Claude Design canvas "Cellworks mind palace mockups", pages "Sessions in the city (v12, STORY-030)" and
  "Sessions sidebar (agents v13, STORY-030)", fed by real sessions (`cw-sessions.json`).
