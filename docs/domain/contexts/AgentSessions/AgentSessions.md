# Context: AgentSessions

Claude Code sessions on the machine, shown as units on the map that move to the files they touch. Sessions are Claude Code background sessions, created, attached, detached and answered from Cellworks ([ADR-0010](../../../adr/0010-claude-code-background-sessions.md)).

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- AI engineer / operator — runs and steers several sessions
- Claude Code — the agent CLI
- Background session — a Claude Code session hosted by Claude Code itself, detached from any terminal

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Every Claude Code session on the machine appears, found by tailing `~/.claude/projects/**.jsonl` (L113).
- Animations: movement to files read or written, construction sites for files being created, blocked and approval alerts (L114-117).
- Each tool call is an action of the session's unit, by kind: reading (Read), editing (Edit, MultiEdit, Write,
  NotebookEdit), running (Bash; "testing" when the command tests), searching (Grep, Glob, and shell searches such as
  `grep` or `find`), delegating (Task) and fetching (WebFetch, WebSearch). The unit walks along the streets to what the
  action touches and works there: a rising scaffold on the tower with the written floors lit, a scan beam, a work pulse
  that turns green or red on the command's result, a radar sweep over a folder. A command lands on the folder or file
  it names (`cd`, `git -C`, `--test`, `-p`), else on the project's gate at the front corner of the map, where calls
  that leave the map land too. Rapid calls queue and play out one by one; a backlog is cut to the newest few, so a
  unit never lags far behind. A resting unit shows a marker: "!" waiting for the engineer, dots thinking, "z" idle, a
  tick when done. With reduced motion units step instead of walking and their marks stand still.
- An agent list in a corner and a session pane (L118).
- Only background sessions are attached to, typed into or stopped; interactive sessions are read-only and never stopped from Cellworks; closing the session pane ends only the attach (STORY-081).
- Real Claude Code CLI sessions, run as Claude Code background sessions: create, attach, detach, answer approvals inline (L119). Sessions keep running when Cellworks restarts (L223).
- A chat panel or stream-json protocol was rejected in favour of real CLI sessions (L246).
