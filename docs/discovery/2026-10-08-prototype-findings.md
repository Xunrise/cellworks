# Cellworks prototype: findings for domain discovery

Source: the working sessions in which Cellworks was prototyped, from 2026-10-06 to 2026-10-08. They were conversations
between the product owner (the user, a software engineer at SQUER) and Claude Code.

- The first prototype is on `main` of `squer-solutions/cellworks`.
- The keyboard-navigation prototype is on branch `proto/vim` in `~/Projects/cellworks-proto-proto-vim` (not pushed).

This document is the input for `domain-interview`. It records what the product owner asked for, what was confirmed after
trying it, and what was rejected. The prototypes are evidence, not a design to keep.

## Purpose

Cellworks makes the work an engineer does with AI agents across many projects humanly readable and navigable in one
place, as a game-like map. The intent is to replace using the Claude Code CLI directly:

- all projects in one view;
- Claude Code sessions visible as units on the map, created and steered from it;
- navigation of files, code, AI conversations, terminals, git, tickets, infrastructure and data structures, all with the
  same map mechanics.

The theme is the Warhammer 40k Adeptus Mechanicus ("machine spirit"). Projects are planets, folders are factories,
files are machines, and agents are servitors (servo-skulls). The visual metaphor is Factorio-like factories you can
zoom into.

## Personas

- **Code architect**: understands a codebase's structure, modules, dependencies and health, and where bounded-context
  boundaries are drawn well or badly.
- **AI engineer / operator**: runs several Claude Code sessions, watches what they touch, and answers their permission
  prompts.
- **Ticket and ops view**: navigates Jira tickets and live infrastructure (Open Telekom Cloud: Terraform plus
  Kubernetes) the same way as code.
- **Data view**: navigates a system's data structures (tables, collections, contracts, DTOs) and who uses them.
- **Onboarding colleague**: gets started quickly with their own Claude session (README, CLAUDE.md, setup script).

## Core concepts (candidate ubiquitous language)

| Term | Meaning |
|---|---|
| Universe | all projects |
| Project / planet / continent | a top-level thing shown as a circle. A code project is a folder. A Jira project is `jira:<site>/<KEY>`, an infrastructure project `otc:<folder>`, a data-structures project `data:<repo>` |
| District | the top folder below the common prefix of a project, or a component in Jira, or an environment in infrastructure |
| Sub-district | the next level down |
| Factory | a folder (or an epic, a Kubernetes namespace, a Terraform group, a database) |
| Machine | a file (or a ticket, a workload, a table, a message, a DTO) |
| Crate | a function (or a sub-task, a column, a field) |
| Context cell | a unit of domain knowledge, from `context-cells/` or a jarvis `docs/domain/contexts/`, linked to the code that uses its glossary terms |
| Lane / belt / pipe | a connection drawn between nodes, with dots flowing from upstream to downstream. Kinds: imports, calls, context links, cell references, Jira links, infrastructure wiring, foreign keys, field types, service usage |
| Folder view (scope) | showing only one folder, laid out tidily, with everything it connects to on a ring around it |
| Servitor | a Claude Code session drawn as a servo-skull that moves to the files it touches; files it writes are construction sites |
| Session host | a detached process that owns a Claude Code session's terminal, so sessions survive restarts (like herdr) |
| Layer of detail | Universe → Project → Directory → File (see Navigation) |
| Selection / cursor | the node the keyboard acts on |
| Item menu | the per-node menu (Space or right-click) |
| Tree | the per-layer explorer (E) |
| View options | what is drawn and how it is laid out |

## Confirmed behaviour

### Map and layout

- Projects are planets in a universe. Zooming out of one project and into the next is seamless.
- Double-clicking a planet locks into it and hides the others; Esc unlocks.
- Physics layout in the style of a graph explorer:
  - repulsion only within a capped range;
  - forces only within a group;
  - districts anchored apart, sub-districts with their own home inside their district;
  - files inside a folder clustered by AST affinity: shared users, shared dependencies, name tokens.
- **Constraint:** clicking around must not inflate the map. Verified on a project of about 20k files.
- Clear margins between districts and sub-districts, so the folder hierarchy reads at a glance.
- Level-of-detail rendering:
  - labels stay readable at every zoom, including during transitions;
  - names are enough at overview zoom;
  - different shapes per file type;
  - rounded corners.
- Folder view:
  - double-click a district, sub-district or folder;
  - members are packed in boxes by their next subfolder, and outside connections sit on a ring as lanes;
  - lanes are drawn behind nodes;
  - double-click goes deeper.
- Districts, folders and folder boxes can be dragged. Middle-drag always pans.
- View options:
  - connections between files, folders or districts;
  - imports, calls, context links and cell references on or off;
  - a minimum strength;
  - only around the selection;
  - organize by folder, graph or git changes;
  - git overlay.
- ⌘-click a lane to travel to its far end; hovering with ⌘ held highlights the lane and previews it.
- Back and forward history of views (project, camera, folder view, selection).

### Code understanding

- AST metrics through tree-sitter (TS/JS, Python, PHP, Rust, Lua, Bash): functions with complexity, imports, calls.
- Dependency trees: imports and calls between files, aggregated to folders and districts.
- Domain knowledge (context cells) parsed from:
  - `context-cells/`;
  - jarvis `docs/domain/contexts/<Name>/<Name>.md`, or the flat `contexts/<name>.md` plus a shared `glossary.md`.

  Code is linked to cells by glossary terms, with document-frequency limits to avoid noise.
- Git: status, numstat, and an overlay of changed files. A branch and worktree switcher; worktrees open as their own
  planets.
- Embedded Neovim:
  - **opens only on explicit request** (Enter, `i`, double-click, an edit action, opening a manifest), never just by
    zooming;
  - auto-saves, so git sees changes;
  - can pop out into its own window, several at once, while navigating;
  - a full-screen toggle keeps the node header and a margin otherwise.

### Agents (servitors)

- Every Claude Code session on the machine appears as a servitor, found by tailing `~/.claude/projects/**.jsonl`.
- Animations:
  - movement to the files being read or written;
  - construction sites for files being created, in the style of Warcraft 3;
  - blocked or approval alerts.
- An agent list in a corner and a servitor card.
- Real Claude Code CLI sessions in detachable hosts. Create, attach, detach; answer approvals inline.

### Other project types

- **Jira:** Cellworks is its own OAuth client of Atlassian's remote MCP server (dynamic registration plus PKCE).
  - Map: components are districts, epics are factories, tickets are machines coloured by status, sub-tasks are crates,
    Jira links are lanes.
  - Done tickets older than 30 days are hidden.
  - Tickets are cached, and ⌘R resyncs.
- **Infrastructure (OTC):** Terraform blocks per environment, plus live workloads from kubeconfigs (read-only `kubectl
  get`).
  - Coloured by health.
  - Wired by `*_SERVICE_HOST` environment variables; ingress controller to services; services to databases.
  - Workloads link to their code ("show code") and their Kubernetes manifest ("open manifest").
- **Data structures:**
  - Postgres tables, by replaying Knex migrations, with foreign keys as lanes.
  - Mongo collections.
  - gRPC proto messages and services, with field-type lanes.
  - DTOs, with field-type lanes.
  - Services and the tables, collections and contracts they use. The main database's tables are grouped by the service
    using them, or "shared", or "not found in services".

### Navigation: layers of detail and the keyboard (frozen prototype feature set)

**Layers.** Each layer is a level of detail; the layer follows from the selection and the zoom.

| Layer | Holds | Interest |
|---|---|---|
| Universe | projects | managing projects |
| Project | top-level directories (districts) | project-wide concerns |
| Directory | folders, districts or modules; folder views live inside this layer | architecture, connections, bounded contexts |
| File | one file | health, connections, history, editing |

**Movement.**
- h/j/k/l and the arrow keys move between nodes of the current layer.
- Two switchable modes (`m`):
  - **spatial:** the nearest node in that direction;
  - **list order:** j/k next and previous, h/l out and in.
- The camera pans only when the cursor leaves the middle of the view.

**Enter (one layer in).**

| On | Does |
|---|---|
| a project | opens and locks it |
| a district | opens its folder view |
| a folder | frames the folder with all its files, centred, with a margin (about 70% of the view) |
| a file | frames it close enough to read its health, functions and connections (about 55%), without opening the editor |
| a selected file, again | opens nvim and gives it the keyboard |

**Esc (one layer out).**
- Order: editor → file → folder → leaving the folder view → district → project → universe (all projects framed).
- The Esc that leaves nvim does not also act on the map.

**Selection.** The selected node always shows a pulsing outline at every layer.

**Space or right-click: item menu.**
- A pop-up on the item itself, listing only that item's connections (imports, used by, calls, context, ring) and
  actions.
- Full-size; busy kinds go into submenus that open to the right: git (also branches, worktrees, changed files), view,
  four or more connections, files or functions of one kind.
- Typing filters across everything as a flat list.

**⌘K.** Searches everything (projects, folders, files, sessions), and lists every action once you type.

**`v` (or the View button): view options menu.** Each entry shows its state; it stays open for several changes in a row.

**E: tree (explorer, like nvim snacks explorer).** Tree contents per layer:

| Layer | Tree |
|---|---|
| Universe | projects by type |
| Project | districts → sub-districts → folders |
| Directory | folders → files |
| File | the folder's files → functions |

Tree behaviour:
- A root row for the level; everything expanded; sized to its content up to the window height.
- j/k move, and the map frames the node under the cursor:
  - a project's whole circle;
  - a folder with its files;
  - a file at inspection distance.
- l opens a row, or on a leaf goes there on the map. The tree then re-roots on the layer the map is now on.
- h closes, or goes to the parent, or to the root. On the root it moves the tree and the map out one layer; the
  Universe root frames all projects.
- gg / G go to the top / bottom, Z collapses everything, `/` filters, q, Esc or E close.

**Other keys.**

| Key | Does |
|---|---|
| `i` | edit |
| `p` | pop out the editor |
| Ctrl-O / Ctrl-I | back / forward |
| `?` | help |

The layer badge shows e.g. `FILE · spatial`.

### Operating Cellworks

- Projects are discovered automatically (git repositories under `~/Projects`, folders recent Claude sessions worked in).
  Any folder can be added.
- Settings are persisted.
- Rescan with ⌘R, and automatically after agents write files.
- Rebuild and restart from inside Cellworks; sessions keep running.
- Native macOS app (Rust, eframe/egui). Performance matters: personal daily use on large repositories.

## Edge cases that surfaced

- **Letter case in paths.** On macOS, paths match regardless of case. Matching projects and files by string must not
  assume one spelling: a project added as `~/projects/...` broke "show code".
- **Cross-project jumps.** A jump into a project whose map is still loading must apply after that project's first fit,
  or the camera is pulled back to the overview.
- **Folder views on project switch.** A folder view belongs to its project; switching projects closes it.
- **Jumps into hidden folders.** A jump to a node hidden by the current folder view opens the folder view that contains
  it.
- **Esc in nvim** must not also clear the map selection.
- **The keyboard** belongs to whatever has focus (text fields, editor, terminals, menus); map keys act only otherwise.
- **The Jira MCP** answers in two shapes (plain REST `{issues, isLast, nextPageToken}` or `{issues: {nodes,
  pageInfo}}`). OAuth callback parameters arrive URL-encoded and must be decoded once.
- **Sub-district layout.** Pushing sub-districts apart as hard boxes made large maps string out (one test project grew
  to 800k units). Homes per sub-district inside the district worked instead.

## Rejected along the way

- An HTML / localhost web version: superseded by a native app. A localhost HTTP server that spawns shells is off the
  table.
- A chat panel or stream-json "servitor" protocol: replaced by real Claude Code CLI sessions in detachable hosts.
- Relaying Jira tickets through a headless Claude session: too expensive, about 1M tokens per sync.
- The editor opening automatically when zooming close.
- The item menu as a mode of the ⌘K palette: it must be a pop-up on the item.
- A scrolling, truncated tree; the tree sizes to its content.
- Separate menus that show every global action for every item: item-specific actions belong in the item menu, and
  global ones in ⌘K and the view menu.
- Hard-box separation of sub-districts (see the edge cases above).

## Deferred (named by the product owner, not built)

- **Universe layer:** move projects, link projects together, rename projects, filter by type (code, Jira, infra, data).
- **Project layer:** run builds, navigate releases, open Jira or GitHub, an overview of a project, external system
  dependencies.
- **Directory layer:** visualise bounded contexts and spot badly drawn boundaries.
- **File layer:** git history of a file; why a file is the way it is.
- **Jira:** link tickets to the code changed in commits mentioning them; filters (mine, open, sprint).
- **ADRs and arc42 chapters** as context cells.
