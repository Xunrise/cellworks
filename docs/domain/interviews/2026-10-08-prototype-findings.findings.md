## Findings: 2026-10-08-prototype-findings (interview)

<!-- Reference scheme: line numbers of docs/discovery/2026-10-08-prototype-findings.md (Lnn). -->

Product owner decisions during analysis (2026-10-08):
- Neutral terms are the ubiquitous language (glossary, code). The Warhammer 40k words (planet, factory, machine, crate, servitor, belt, pipe) are the display theme only, recorded as aliases.
- Districts are the nodes of the Project layer. How districts are cut must be swappable later (by domain context, module, type, programming language); for now directories decide.
- Ticket Tracking, Infrastructure and Data Structures merge into one "Sources" context: loaders for non-code project types.

### Terms
| Term | Definition | Context | Source refs | Notes |
|------|------------|---------|-------------|-------|
| Universe | All projects, shown together on one map. | Map | L41, L147 | |
| Project | A top-level thing shown as a circle; identified by its path or a `jira:` / `otc:` / `data:` prefix. | Workspace | L42, L219 | theme: planet, continent |
| District | A node of the Project layer: for code, the top folder below the project's common prefix; in Jira a component, in infrastructure an environment. | Map | L43, L148 | PO: the way districts are cut is to become swappable (domain, module, type, language); directories for now |
| Sub-district | The level below a district; has its own home inside its district. | Map | L44, L68, L239 | |
| Folder | A folder; in other project types an epic, namespace, Terraform group or database. | Map | L45 | theme: factory |
| File | A file; in other project types a ticket, workload, table, message or DTO. | Map | L46 | theme: machine |
| Function | A function; in other project types a sub-task, column or field. | Map | L47 | theme: crate |
| Context cell | A unit of domain knowledge from `context-cells/` or jarvis `docs/domain/contexts/`, linked to code that uses its glossary terms. | Domain Knowledge | L48, L97-100 | |
| Lane | A connection drawn between nodes, with dots flowing upstream to downstream; has a kind. | Map | L49, L90 | theme: belt, pipe |
| Lane kind | imports, calls, context links, cell references, Jira links, infrastructure wiring, foreign keys, field types, service usage. | Map | L49 | |
| Folder view | Showing only one folder, packed by next subfolder, with outside connections on a ring. | Navigation | L50, L77-81 | alias: scope |
| Layer (of detail) | Universe → Project → Directory → File; follows from selection and zoom. | Navigation | L53, L143-150 | |
| Selection | The node the keyboard acts on; always shown with a pulsing outline. | Navigation | L54, L173 | alias: cursor |
| Movement mode | spatial (nearest node in a direction) or list order (j/k, h/l). | Navigation | L154-156 | |
| Item menu | Per-node pop-up (Space / right-click) with that node's connections and actions. | Navigation | L55, L175-180 | |
| Palette | ⌘K search over everything and list of all actions. | Navigation | L182 | |
| Tree | Per-layer explorer (E) that frames the map on the row under the cursor. | Navigation | L56, L186-204 | |
| View options | What is drawn and how it is laid out. | Map | L57, L83-89 | |
| History | Back and forward through views (project, camera, folder view, selection). | Navigation | L91 | |
| AST metrics | Functions with complexity, imports and calls per file, via tree-sitter. | Code Analysis | L95 | |
| Dependency tree | Imports and calls between files, aggregated to folders and districts. | Code Analysis | L96 | |
| Git overlay | Changed files highlighted on the map; branch and worktree switcher. | Code Analysis | L102-103 | worktrees open as their own projects |
| Editor | Embedded Neovim; opens only on explicit request, auto-saves, can pop out. | Editing | L104-109, L248 | |
| Session | A Claude Code session, drawn on the map moving to the files it touches. | Agent Sessions | L51, L113-119 | theme: servitor, servo-skull |
| Construction site | A file being created by a servitor. | Agent Sessions | L51, L116 | |
| Approval | A permission prompt from a session, answered inline. | Agent Sessions | L117, L119 | alias: blocked alert |
| Session host | Detached process that owns a session's terminal so it survives restarts. | Agent Sessions | L52, L119 | |
| Ticket | A Jira issue; coloured by status; done > 30 days hidden. | Sources | L123-127 | |
| Sync | Fetching and caching tickets; ⌘R resyncs. | Sources | L127 | |
| Workload | Live Kubernetes object, coloured by health, linked to code and manifest. | Sources | L128-132 | |
| Wiring | Links derived from `*_SERVICE_HOST`, ingress → services → databases. | Sources | L131 | |
| Table / Collection / Message / DTO | Data structures loaded from Knex migrations, Mongo, proto and DTOs. | Sources | L134-137 | |
| Service usage | Which services use which tables, collections and contracts; groups tables by service, "shared" or "not found in services". | Sources | L138-139 | |
| Rescan | Rebuilding a project's map; ⌘R or after agents write files. | Workspace | L222 | |

### Contexts
- **Map** — The universe of projects as a zoomable, physics-laid-out map of districts, factories, machines and crates joined by lanes, rendered with level of detail.
  - Participants: code architect, all personas
  - Concepts: universe, district, sub-district, folder, file, function, lane, view options, district strategy (directories for now)
  - Behaviour: physics layout with grouped forces and district homes (L65-71, L239-240); LOD rendering (L72-76); drag and pan (L82); view options (L83-89)
- **Navigation** — How the user moves through the map: layers, selection, keyboard movement, Enter/Esc, item menu, palette, tree, history.
  - Participants: every persona; keyboard and mouse
  - Concepts: layer, selection, movement mode, folder view, item menu, palette, tree, history
  - Behaviour: L141-215; edge cases L231-236
- **Code Analysis** — Understanding a code project: AST metrics, dependency trees, git status and worktrees.
  - Participants: code architect
  - Concepts: AST metrics, dependency tree, git overlay, worktree
  - Behaviour: L95-103
- **Domain Knowledge** — Context cells parsed from domain docs and linked to code by glossary terms.
  - Participants: code architect
  - Concepts: context cell, glossary term
  - Behaviour: L97-100; deferred: ADRs/arc42 as cells (L263), bounded-context boundaries (L260)
- **Editing** — Embedded Neovim on explicit request, auto-saving, pop-out windows.
  - Participants: AI engineer, code architect
  - Concepts: editor, pop-out
  - Behaviour: L104-109, L167, L171, L235
- **Agent Sessions** — Claude Code sessions on the machine as servitors: discovered from transcripts, animated on the map, created, attached, detached and answered from Cellworks.
  - Participants: AI engineer / operator, Claude Code
  - Concepts: session, session host, construction site, approval
  - Behaviour: L113-119, L223
- **Sources** — Loaders for non-code project types that turn an external system into nodes and links for the map: Jira (Atlassian remote MCP, own OAuth client), infrastructure (Terraform plus read-only Kubernetes), data structures (Knex, Mongo, proto, DTOs, service usage).
  - Participants: ticket/ops persona, data persona, Atlassian, Kubernetes, Terraform
  - Concepts: ticket, epic, component, sub-task, sync, environment, workload, wiring, health, table, collection, message, DTO, service usage
  - Behaviour: L121-139, L237-238
- **Workspace** — Which projects exist and how Cellworks runs: discovery under ~/Projects and from Claude sessions, settings, rescan, rebuild and restart.
  - Participants: every persona, onboarding colleague
  - Concepts: project, settings, rescan
  - Behaviour: L219-224, L228-230

### Interactions
| From context | To context | Ref | What flows |
|--------------|------------|-----|------------|
| Workspace | Map | L42, L219 | project identity (path or prefix) selects a loader |
| Workspace | Sources | L42 | `jira:` / `otc:` / `data:` project identity |
| Code Analysis | Map | L95-96 | files, functions, imports and calls as machines, crates and lanes |
| Domain Knowledge | Map | L48, L100 | context cells and cell-reference lanes |
| Code Analysis | Domain Knowledge | L100 | code text matched against glossary terms |
| Sources | Map | L124-139 | tickets, workloads, tables and the like as nodes, with Jira links, wiring, foreign keys, field types as lanes |
| Sources | Code Analysis | L132 | "show code" jump from workload to its code project |
| Agent Sessions | Map | L114-117 | files touched, construction sites, alerts |
| Agent Sessions | Workspace | L219, L222 | folders sessions worked in become projects; writes trigger rescan |
| Navigation | Map | L152-171 | camera framing, folder view, selection |
| Navigation | Editing | L167, L171, L235 | open file in editor; keyboard focus handoff |
| Code Analysis | Navigation | L102 | worktrees opened as their own projects |
