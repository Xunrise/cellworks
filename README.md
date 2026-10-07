# Cellworks

Cellworks shows your projects as a zoomable map and lets you run Claude Code sessions on top of it. It is a native
Rust app (eframe/egui).

- **Code map.** Each project is a planet. Top-level folders are districts, folders are factories, and files are machines.
  - Lanes between them show imports, function calls and links to domain knowledge, with dots flowing from upstream to
    downstream.
  - Zoom far enough into a file and a real Neovim opens in place.
- **Claude sessions as servitors.** Every Claude Code session on the machine appears as a servo-skull that flies to the
  files it reads and writes. Files it is writing show up as construction sites.
  - Start new sessions from inside Cellworks; they keep running when you close their window.
  - Answer permission prompts from the agent list.
- **Other views on the same map**:
  - Jira tickets (components → epics → tickets).
  - Infrastructure (Terraform plus live Kubernetes).
  - Data structures (Postgres tables, Mongo collections, gRPC contracts, DTOs).

Status: a working prototype that is used daily, built quickly. Expect rough edges, and treat `src/main.rs` as one large
file. It runs on macOS; other platforms are untested.

## Quick start

```sh
git clone git@github.com:squer-solutions/cellworks.git
cd cellworks
./scripts/setup.sh          # checks the tools, builds a release binary
./target/release/cellworks  # or: ./target/release/cellworks ~/path/to/project
```

`scripts/setup.sh` tells you what is missing. You need:

| Tool | Needed for |
|---|---|
| Rust (stable, edition 2024) | building |
| `git` | git state, branches, worktrees |
| `nvim` | the embedded editor |
| `claude` (Claude Code CLI) | servitor sessions |
| `kubectl` (optional) | live workloads in infrastructure views |
| `openssl`, `curl` (macOS default) | the Atlassian sign-in for Jira views |
| `herdr` (optional) | opening terminals as Herdr tabs |

Projects are discovered automatically:
- git repositories in `~/Projects/*` and `~/Projects/*/*`;
- folders that recent Claude Code sessions worked in.

Add anything else with **⌘K** and a path (`~/somewhere/repo`), or with the field at the top of the left panel.

## Getting around

| Input | Does |
|---|---|
| scroll, pinch | zoom; zooming onto something selects it |
| drag | pan; drag a factory or a district to move it |
| click / double-click | select / open (double-clicking a folder opens its folder view) |
| **Esc** | step back out: editor → folder view → selection → project |
| **⌘K** | search everything (projects, folders, files, sessions, commands), or add a project |
| **/** | filter the map |
| **⌘← / ⌘→** (or mouse back/forward) | previous / next view |
| **⌘-click a lane** | follow a connection to its other end (hold ⌘ to highlight and preview) |
| **⌘R** | rescan the project (and resync Jira) |
| **⌘F** | editor full screen |
| **⌘[ / ⌘]** | left / right panel |
| **F** | follow the selected servitor |

- **Folder views.** Double-click a district or folder to see only that folder, laid out tidily. Everything it connects
  to sits on a ring around it, and lanes show what flows in and out. Double-click deeper; press Esc to go back up.
- **Locking into a project.** Double-click a planet to lock into it, which hides the other planets. Esc unlocks.
- **View bar** (bottom centre) controls what is drawn:
  - imports, calls, context links and cell references;
  - connections between files, folders or districts;
  - minimum strength, and "around selection" only;
  - layout by folder, graph or git changes;
  - the git overlay.
- **Editing.** Zoom into a file and Neovim opens in place. Changes save automatically, so the git overlay sees them.
  - **pop out** moves the editor into its own window, so you can keep navigating or edit several files.
  - Esc in normal mode hands the keyboard back to the map.
- **Git.**
  - The `⎇ branch` menu (top right) switches branches, creates branches and opens worktrees as their own planets.
  - The Git tab stages, commits, pulls and pushes.

## Claude sessions (servitors)

- **Start a session:** ⌘K → "New servitor here", or use the Servitors tab.
  - Each session runs the real `claude` CLI in a small host process (`cellworks --host`).
  - Closing its window only detaches; reopen it from the agent list.
  - Sessions survive restarts of Cellworks.
- **Watching sessions:** sessions started elsewhere (in a terminal, in an IDE) show up too. Cellworks tails
  `~/.claude/projects/**.jsonl`.
- **Approvals:** when a session asks for permission, its servitor shows a bubble and the agent list offers Yes / No.

## Project types

| Add with ⌘K | Shows |
|---|---|
| a folder path | the code map: AST metrics (tree-sitter: TS/JS, Python, PHP, Rust, Lua, Bash), imports, calls; domain knowledge from `context-cells/` or a jarvis `docs/domain/contexts/` becomes cells linked to the code that uses their terms |
| `jira:<site>.atlassian.net/<KEY>` or any ticket link | Jira: components are districts, epics are folders, tickets are machines coloured by status, sub-tasks are their crates, Jira links are lanes |
| `otc:<folder>` | infrastructure: Terraform under `<folder>/infrastructure-v2/cloud_infrastructure/<env>` plus live workloads from every `*kubeconfig.yaml` in `<folder>` (read-only `kubectl get`), wired by their `*_SERVICE_HOST` variables; workloads link to their code and their Kubernetes manifest |
| `data:<repo>` | data structures: Postgres tables replayed from Knex migrations in `<repo>/migration/migrations*`, Mongo collections, proto messages and services in `<repo>/contract/proto`, DTOs per service, and which service uses what |

The infrastructure and data views follow the layout of the repositories they were built for. Point them at a similar
layout, or adapt `src/otc.rs` and `src/data.rs`.

**Jira** connects to Atlassian's remote MCP server as its own OAuth client. The first sync opens a browser sign-in.
Tokens are stored in `~/.config/cellworks/atlassian.json` (mode 600) and tickets are cached in
`~/.cache/cellworks/jira/`.

## Files on disk

| Path | What |
|---|---|
| `~/.config/cellworks/settings.json` | settings (Settings window, top right), added projects |
| `~/.config/cellworks/atlassian.json` | Atlassian tokens |
| `~/.cache/cellworks/sessions/` | session host sockets and metadata |
| `~/.cache/cellworks/jira/` | Jira ticket cache |

## Working on Cellworks with Claude

Open the repository in your own Claude Code session. `CLAUDE.md` gives it the architecture and the conventions. Good
first prompts:

- "Walk me through how Cellworks is put together, file by file."
- "How does a project become a map? Trace it from `scan` to what is drawn."
- "I want to add a new project type for X. Which seams do I use?"

You can also iterate from inside Cellworks: add this repository as a project, start a servitor on it, and run
**⌘K → Rebuild & restart Cellworks** to load your changes. Running sessions keep going across the restart.

Checks:

```sh
cargo build --release
cargo test --release                       # fast tests
cargo test --release -- --include-ignored  # also the tests that need local checkouts (see CLAUDE.md)
```
