---
stage: design
issue: 51
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-029, how sessions are driven

The contract is `issue.md` in this directory. The spike ran in the prototype session
([prototype.md](prototype.md)); its outcome is ADR-0010, already written with the operator.

## Open questions

None.

## Requirements

1. The change lands ADR-0010 and its backflow, taken from the prototype's `docs: ingest STORY-029 prototype decisions`
   commit, unchanged.
2. No source code, test or workflow changes.

## Decisions

- Docs only. Rejected: a thin `agent-sessions` adapter (start, `agents --json`, transcript file events) in this story,
  because STORY-030 and STORY-031 carry that behaviour with their own scenarios and tests.
- Claude Code background sessions drive sessions (ADR-0010). Rejected: a Cellworks-owned PTY session host, the Agent
  SDK or stream-json, tmux or herdr; reasons in the ADR.
- `CLAUDE.md` still said "how sessions are driven is still open" and now points at ADR-0010, a backflow the Docs
  impact below missed. `context/05-case/AGENTS.md` carries the same stale open question, but it is the product's
  business context: `hooks/guard-paths.sh` blocks a build session from editing it, and it needs its own pull request.
  Reported as a finding instead of changed here.
- Chapter 6.3's sequence still drew the approval prompt arriving from the transcript, which ADR-0010's Negative
  consequences deny: waiting for approval shows only as a state in `claude agents --json`. The sequence now polls
  `agents --json` for that state, as 6.4 and chapter 3 already do, and a bullet says so. A departure from
  Requirement 1 (the docs commit taken unchanged) in one detail, taken because STORY-030 and STORY-031 build the
  `agent-sessions` adapter from these chapters and would otherwise wire the approval alert to an event that never
  arrives.
- The freeze stage's own commit (`5183574`, "design: story-029-spike-how-sessions-are-driven") already carried the
  eight docs files byte-identical to the frozen docs commit (`4a552b0`, "docs: ingest STORY-029 prototype
  decisions"). Cherry-picking `4a552b0` onto the branch produced an empty patch, so step 1 needed no further commit;
  confirmed by diffing each of the eight files between the two commits.

## Order of work

1. [x] Take the docs commit onto the feature branch: `docs/adr/0010-claude-code-background-sessions.md`,
   `docs/arc42/03-context.md`, `docs/arc42/04-solution-strategy.md`, `docs/arc42/05-building-blocks.md`,
   `docs/arc42/06-runtime.md`, `docs/arc42/09-decisions.md`, `docs/domain/contexts/AgentSessions/AgentSessions.md`,
   `docs/domain/contexts/AgentSessions/glossary.md`.

## Risks

- `docs/arc42/09-decisions.md` conflicts with ADR-0008 and ADR-0009 from parallel stories at merge; keep all entries,
  sorted by number.
- Nothing runtime changes. Rollback: revert the squash commit.

## Proof

- `python3 "$JARVIS_TOOLING/scripts/check-docs.py" adr services links` prints `0 error(s), 0 warning(s)`.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
  unchanged.
- The issue's "done when": ADR-0010 records the chosen way and the rejected ones; starting a session with a task and
  observing its reads and writes is shown in prototype.md under Confirmed behavior.

## Docs impact

- **Business decisions:** None.
- **Glossary:** AgentSessions glossary: "Session host" replaced by "Background session" (already in the docs commit).
- **Guidelines:** None.
- **arc42:** chapters 3 (Claude Code and transcripts), 4.4, 5.2 (`agent-sessions`), 6.3 and 6.4, 9 (already in the
  docs commit).
- **ADR needed:** yes, ADR-0010, written with the operator through adr-writing and in the docs commit.
- **CLAUDE.md:** the Claude Code entry under "Foreign systems" now records background sessions and ADR-0010 instead
  of the open question.
- **Business context:** `context/05-case/AGENTS.md` lists the same open question under "Open, and the plan for each".
  It must lose that entry, but only in its own pull request (see Decisions).
