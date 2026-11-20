# ADR-0010: Drive Claude Code sessions as Claude Code background sessions

## Status
Accepted

## Date
2026-10-19

## Context
Dispatching agents, repairs, agent pull requests and the city's reactions to sessions all need a way to give a Claude
Code session work and follow it (arc42 [4.4](../arc42/04-solution-strategy.md)). The sessions must be real CLI
sessions the engineer can open in a terminal, and they must survive restarts of Cellworks
([chapter 1](../arc42/01-introduction.md)). Cellworks must never handle the user's Claude credentials or run a local
network server ([chapter 2](../arc42/02-constraints.md)). Claude Code 2.1.300 can host sessions itself:
`claude --bg` starts a detached session and `claude agents --json` lists every interactive and background session
with its state. The STORY-029 spike started a session on a task with this command. The session's reads and writes
appeared in its transcript within four seconds, and its state went from busy/working to idle/done. Typing a follow-up
into the running session through `claude attach` worked.

## Decision
We will start sessions with `claude --bg -n <name> "<task>"`, and follow them through their transcripts and
`claude agents --json`. We will type into a session or answer its approval through `claude attach <id>`, run in a
pseudo-terminal that Cellworks embeds. Cellworks runs `claude` as a child process with a clean environment, so the
session uses the user's own Claude Code login. The task text is passed as one argument of the
process, never through a shell, and is treated as data.

## Consequences

### Positive
- Sessions are real Claude Code CLI sessions that survive restarts of Cellworks. The engineer can attach to them from
  any terminal, and Cellworks builds no host process of its own.
- Every session on the machine is visible the same way, whether or not Cellworks started it: the transcript for file
  reads, writes and pull requests, `agents --json` for busy, idle, done and blocked.
- `claude` becomes another external tool behind an adapter in `agent-sessions`, as
  [ADR-0003](0003-external-tools-as-child-processes.md) prescribes.

### Negative
- Neither `claude agents --json` nor the transcript format is a published contract. Both must be parsed tolerantly,
  and a Claude Code update can break the adapter.
- Waiting for approval shows only as a state in `agents --json`, not in the transcript. Answering an approval through
  `attach` was not exercised in the spike, because background sessions ran in the user's configured permission mode.
- `--resume <sessionId> --bg` on a session that is still running starts a copy with a new id and an auto-generated
  name. Follow-ups must go through `attach`, or the session must be stopped first and resumed with its name.
- A child that inherits a host's Claude environment (for example when Cellworks is started from a Claude session)
  fails to authenticate, so the environment must be cleared explicitly.
- A shell command is placed only by what it names: the first file or folder it mentions, `cd`, `git -C`, `--test`
  and `-p`; otherwise at the project as a whole. What it actually reads or writes is not seen.
- Newer transcripts also hold `api-request-blob` lines that replay earlier tool calls; only `assistant` and `user`
  lines count.

### Neutral
- Sessions often touch files outside their working directory. A session belongs to a project's city by the paths it
  touches, not by its cwd.

## Alternatives considered
- **A session host owned by Cellworks, holding each CLI session in a pseudo-terminal** (the prototype): rejected,
  because Claude Code now ships this host, and a host of our own would duplicate it, including survival across
  restarts.
- **The Agent SDK or headless `claude -p --output-format stream-json`**: structured events and a permission-prompt
  hook. Rejected, because the sessions are not real CLI sessions the engineer can attach to. The product owner
  rejected this direction in the prototype.
- **An external multiplexer (tmux, herdr) with Cellworks sending keys**: rejected. It adds a dependency (tmux is not
  installed on the reference machine) and gives no structured session state.

## References
- arc42: [03 Context](../arc42/03-context.md), [04 Solution strategy](../arc42/04-solution-strategy.md),
  [05 Building blocks](../arc42/05-building-blocks.md), [06 Runtime](../arc42/06-runtime.md)
- Related: [ADR-0003](0003-external-tools-as-child-processes.md),
  [ADR-0002](0002-cargo-workspace-crate-per-bounded-context.md)
- Domain: [AgentSessions](../domain/contexts/AgentSessions/AgentSessions.md)
- Spike: STORY-029
