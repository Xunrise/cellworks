# Context Map

## Bounded Contexts

### 1. Map
> The universe of projects as one zoomable map.

### 2. Navigation
> How the user moves through the map with keyboard and mouse: layers of detail, a selection the keyboard acts on, Enter and Esc to move one layer in or out, a per-node item menu, a global palette, a per-layer tree, and back/forward history.

### 3. CodeAnalysis
> Understanding a code project: per-file AST metrics, imports and calls aggregated into dependency trees, and git state including branches and worktrees.

### 4. DomainKnowledge
> Domain knowledge as context cells: parsed from domain documents and linked to the code that uses their glossary terms.

### 5. Editing
> Editing files inside Cellworks with an embedded Neovim that opens only on explicit request, auto-saves, and can pop out into its own windows.

### 6. AgentSessions
> Claude Code sessions on the machine, shown as units on the map that move to the files they touch.

### 7. Sources
> Loaders for project types that are not code: Jira, infrastructure and data structures.

### 8. Workspace
> Which projects Cellworks knows and how it runs: automatic project discovery, adding folders, persisted settings, rescans, and rebuild and restart from inside the app.

---

## Relationships

| # | Upstream | Downstream | Type | Clarity | Notes |
|---|----------|------------|------|---------|-------|
| 1 | Map | CodeAnalysis | Published Language (PL) | confirmed | Code analysis emits files, functions, imports and calls as Map nodes, items and lanes |
| 2 | Map | Sources | Published Language (PL) | confirmed | Jira, infrastructure and data loaders emit Map nodes and lanes |
| 3 | Map | DomainKnowledge | Published Language (PL) | confirmed | Context cells and cell-reference lanes |
| 4 | Map | AgentSessions | Conformist (CF) | confirmed | Sessions are placed on the map at the nodes of the files they touch |
| 5 | Map | Navigation | Conformist (CF) | confirmed | Selection, folder view and camera act on Map nodes as they are |
| 6 | Workspace | Map | Open Host Service (OHS) | confirmed | Project identity and rescans |
| 7 | Workspace | Sources | Open Host Service (OHS) | confirmed | `jira:` / `otc:` / `data:` prefixes select the loader |
| 8 | CodeAnalysis | DomainKnowledge | Conformist (CF) | confirmed | Code text is matched against glossary terms |
| 9 | CodeAnalysis | Workspace | Conformist (CF) | confirmed | Worktrees open as their own projects |
| 10 | AgentSessions | Workspace | Conformist (CF) | confirmed | Folders sessions worked in become projects; session writes trigger a rescan |
| 11 | Editing | Navigation | Open Host Service (OHS) | confirmed | Open a node in the editor; keyboard focus handoff |
| 12 | Sources | Navigation | Conformist (CF) | needs review | "Show code" jumps from a workload to its code project |

---

## Summary

- **Total Bounded Contexts:** 8
- **Total Relationships:** 12
- **Relationship Breakdown:** Conformist: 6, Open Host Service: 3, Published Language: 3
