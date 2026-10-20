# AgentSessions — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| Approval | A permission prompt from a session, answered inline. | prototype-findings L117, L119 | shown as a blocked alert |
| Construction site | A file being created by a session. | prototype-findings L51, L116 |  |
| Session | A Claude Code session, shown on the map moving to the files it touches. | prototype-findings L51, L113-119 | theme: servitor, servo-skull |
| Session pane | The pane at the bottom of Cellworks showing one session: its own Claude Code screen, or a read-only conversation. | prototype-findings L118; STORY-081 prototype | replaces "session card"; theme: servitor card |
| Background session | A Claude Code session hosted by Claude Code itself, so it survives restarts of Cellworks; attached and detached from Cellworks or any terminal. | prototype-findings L52, L119, L223; ADR-0010 | replaces the prototype's session host |
| Sessions sidebar | The list of every running session on the machine: those that need the engineer, are working or are idle in this city, then those in other projects. | STORY-030 prototype |  |
| Waiting | A session whose state is waiting or blocked: it stays at the last tower it touched and asks for the engineer. | STORY-030 prototype | `claude agents --json` state |
| Interactive session | A session started in a terminal or the desktop app; it cannot be attached and is read-only from Cellworks. | STORY-081 prototype | no attach id in `claude agents --json` |
| Background copy | A background session started from an interactive one with its conversation; the original keeps running. | STORY-081 prototype | `claude --resume <id> --bg` |
