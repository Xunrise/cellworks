# Domain

Cellworks makes an engineer's projects, code, Claude Code sessions, tickets, infrastructure and data navigable as one
zoomable map with one set of navigation mechanics.

| Where | What |
|---|---|
| [`context-map.md`](context-map.md) | The authoritative map — every context and the relationships between them. Start here. |
| [`glossary.md`](glossary.md) | Shared terms, the same-word-different-meaning collisions, and an index into every context's own glossary. |
| [`contexts/`](contexts/) | One folder per bounded context: its description and its own glossary. |
| [`interviews/`](interviews/) | Discovery sources and their analysis records. |

## Bounded contexts

| Context | What it covers | Terms |
|---|---|---|
| [Map](contexts/Map/Map.md) | The universe of projects as one zoomable map | [glossary](contexts/Map/glossary.md) |
| [Navigation](contexts/Navigation/Navigation.md) | How the user moves through the map with keyboard and mouse: layers of detail, a selection the keyboard acts on, Enter and Esc to move one layer in or out, a per-node item menu, a global palette, a per-layer tree, and back/forward history | [glossary](contexts/Navigation/glossary.md) |
| [CodeAnalysis](contexts/CodeAnalysis/CodeAnalysis.md) | Understanding a code project: per-file AST metrics, imports and calls aggregated into dependency trees, and git state including branches and worktrees | [glossary](contexts/CodeAnalysis/glossary.md) |
| [DomainKnowledge](contexts/DomainKnowledge/DomainKnowledge.md) | Domain knowledge as context cells: parsed from domain documents and linked to the code that uses their glossary terms | [glossary](contexts/DomainKnowledge/glossary.md) |
| [Editing](contexts/Editing/Editing.md) | Editing files inside Cellworks with an embedded Neovim that opens only on explicit request, auto-saves, and can pop out into its own windows | [glossary](contexts/Editing/glossary.md) |
| [AgentSessions](contexts/AgentSessions/AgentSessions.md) | Claude Code sessions on the machine, shown as units on the map that move to the files they touch | [glossary](contexts/AgentSessions/glossary.md) |
| [Sources](contexts/Sources/Sources.md) | Loaders for project types that are not code: Jira, infrastructure and data structures | [glossary](contexts/Sources/glossary.md) |
| [Workspace](contexts/Workspace/Workspace.md) | Which projects Cellworks knows and how it runs: automatic project discovery, adding folders, persisted settings, rescans, and rebuild and restart from inside the app | [glossary](contexts/Workspace/glossary.md) |
