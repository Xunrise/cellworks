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
- An agent list in a corner and a session card (L118).
- Real Claude Code CLI sessions, run as Claude Code background sessions: create, attach, detach, answer approvals inline (L119). Sessions keep running when Cellworks restarts (L223).
- A chat panel or stream-json protocol was rejected in favour of real CLI sessions (L246).
