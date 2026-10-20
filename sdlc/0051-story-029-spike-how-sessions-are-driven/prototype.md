# Prototype: STORY-029, how sessions are driven

## Confirmed behavior
- `claude --bg -n <name> "<task>"`, run with a clean environment (`HOME`, `PATH`, `USER`, `TERM` only), starts a real
  Claude Code session detached from any terminal and prints its short id.
- The session's file reads and writes appear in its transcript under `~/.claude/projects` within about four seconds
  (`Read` and `Write` tool calls with `file_path` and timestamp).
- `claude agents --json` lists the session with its id, session id, cwd, name, kind, status (busy, idle) and state
  (working, done, blocked), and follows it from busy/working to idle/done.
- Typing a follow-up into the running session through `claude attach <id>` in a pseudo-terminal works; ending the
  attach leaves the session running.
- Pull request URLs are found in the result of a `gh pr create` Bash call in existing transcripts (cellworks PR #116).
- Decision recorded as ADR-0010 with backflow into arc42 chapters 3, 4, 5, 6 and 9 and the AgentSessions domain docs.

## Feedback log
- "I renewed the CLI login; continue the STORY-029 spike." → the live part of the spike ran: start, observe, state,
  attach.
- "claude --bg (Recommended)" → ADR-0010 drafted for Claude Code background sessions.
- "ADR-0010 approved as drafted; write it with the backflow and freeze STORY-029." → ADR and backflow committed.
- "ADR only (Recommended)" → the build lands the docs and no code.

## Rejected
- A session host owned by Cellworks holding a pseudo-terminal per session: Claude Code now ships this host.
- The Agent SDK or headless `claude -p --output-format stream-json`: not a real CLI session the engineer can attach
  to; rejected by the product owner in the first prototype.
- tmux or herdr with Cellworks sending keys: an extra dependency (tmux is not installed) and no structured state.
- A thin session adapter in `agent-sessions` as part of this story: the code arrives with STORY-030 and STORY-031.

## Edge cases
- A child process that inherits the Claude desktop app's environment (`CLAUDE_CODE_SIMPLE`, host auth variables)
  fails with "Login expired"; a clean environment authenticates with the user's CLI login.
- An expired CLI login leaves a background session in state `blocked`; the user must log in.
- `claude --resume <sessionId> --bg` on a session that is still running (idle/done) starts a copy with a new id and
  an auto-generated name; stop it first or type through attach.
- Background sessions run in the user's configured permission mode; `--permission-mode default` did not produce an
  approval wait, so answering an approval through attach was not exercised.
- Sessions write outside their cwd (proto worktrees, memory, scratchpad): a session belongs to a city by the paths it
  touches.
- Reads through Bash (`cat`, `sed -n`, `grep`) are not file events.

## Decisions
- Q: How should Cellworks drive Claude Code sessions? → A: Claude Code background sessions (`claude --bg`,
  `claude agents --json`, `claude attach`), ADR-0010.
- Q: Approve ADR-0010 as drafted? → A: Approved.
- Q: What should the pipeline build for #51? → A: ADR only; no code.

## Evidence
- Branch `proto/session-host` off `origin/test` 86f8a51; commit `docs: ingest STORY-029 prototype decisions`.
- Files: `docs/adr/0010-claude-code-background-sessions.md`, `docs/arc42/03-context.md`, `04-solution-strategy.md`,
  `05-building-blocks.md`, `06-runtime.md`, `09-decisions.md`, `docs/domain/contexts/AgentSessions/AgentSessions.md`,
  `glossary.md`.
- Run from a shell: `env -i HOME=$HOME PATH=$PATH USER=$USER TERM=xterm-256color claude --bg -n pricing-fix --model
  haiku "<task>"`, polled with `claude agents --json`, transcript read with a throwaway Python script, follow-up typed
  through `claude attach` in a Python `pty`. Spike sessions and files were removed afterwards.
