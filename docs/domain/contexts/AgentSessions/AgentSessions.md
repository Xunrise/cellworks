# Context: AgentSessions

Claude Code sessions on the machine, shown as units on the map that move to the files they touch. Sessions run in detachable session hosts and are created, attached, detached and answered from Cellworks.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- AI engineer / operator — runs and steers several sessions
- Claude Code — the agent CLI
- Session host — detached process owning a session terminal

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Every Claude Code session on the machine appears, found by tailing `~/.claude/projects/**.jsonl` (L113).
- Animations: movement to files read or written, construction sites for files being created, blocked and approval alerts (L114-117).
- An agent list in a corner and a session card (L118).
- Real Claude Code CLI sessions in detachable hosts: create, attach, detach, answer approvals inline (L119). Sessions keep running when Cellworks restarts (L223).
- A chat panel or stream-json protocol was rejected in favour of real CLI sessions (L246).

## Tactical model

> Status: not yet discovered
> The tactical model emerges during implementation. A story adds an
> entry here only for a new aggregate, a changed aggregate boundary or
> a new invariant — the code is the rest of the model. Do not
> pre-populate from the discovery source: tactical choices are made at
> code time, not at analysis time.

### Aggregates
_No entries yet._

### Entities (non-aggregate roots)
_No entries yet._

### Value objects
_No entries yet._

### Domain services
_No entries yet._
