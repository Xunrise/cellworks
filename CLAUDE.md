# Cellworks: notes for Claude

A native Rust (edition 2024) desktop app: eframe/egui 0.36. Read `README.md` for what it does from a user's point of
view. This file is about the code.

## Layout

| File | Responsibility |
|---|---|
| `src/main.rs` | The app: the `App` state, input, all drawing (`draw_world`, `draw_machine`), panels, palette (⌘K), folder views, navigation history, session windows, editor windows, CLI modes, and the UI tests at the bottom. Large; find things by function name. |
| `src/world.rs` | The world model and layout: `scan` (code projects), `parse_cells` / `parse_jarvis` (domain knowledge), `jira_world`, `graph_world` (generic node/link maps), districts and sub-districts, physics (`step`, `separate*`), and layout tests. |
| `src/ast.rs` | tree-sitter analysis per file: functions with complexity, imports, calls. |
| `src/backend.rs` | Everything outside the map: Claude transcript tailing, project discovery, git, Herdr. |
| `src/term.rs` | Detachable Claude Code sessions: `cellworks --host` owns the pty and a Unix socket; the app attaches and renders with vt100. |
| `src/nvim.rs` | Embedded Neovim client (`--embed`, ext_linegrid). |
| `src/jira.rs` | Atlassian remote MCP client: OAuth (dynamic registration + PKCE), JSON-RPC over HTTP via `curl`, ticket sync and cache. |
| `src/otc.rs` | Infrastructure loader: Terraform blocks plus live `kubectl get`; also `Node`, the generic map node. |
| `src/data.rs` | Data-structure loader: Knex migration replay, Mongo, proto, DTOs, service usage. |
| `src/settings.rs` | Persisted settings. |
| `src/shapes.rs` | Machine outlines per file type. |

## How a project becomes a map

1. A project is identified by its path.
   - Plain folders are code projects.
   - `jira:`, `otc:` and `data:` prefixes select the other loaders; see the top of `world::scan`.
2. `scan` builds a `World`: factories (`Fac`, a folder) hold sections (`Section`, a file, ticket, table, …), and those
   hold items (`Item`, a function, column, sub-task, …).
   - `deps`, `dependents`, `calls_out`, `calls_in` and `links` between sections become lanes.
   - `finish` derives the belts, assigns districts and sub-districts from factory ids (`district/...`), and lays the map
     out.
3. `World::step` runs the physics.
4. `App::draw_world` renders it with level-of-detail.

New kinds of maps should go through `graph_world`: produce `otc::Node`s (district, folder, title, body, state, path,
line, items, code) plus links, and the map, folder views, ⌘K search, lanes and navigation come for free.

## Navigation seams

- `fit_to`, `goto_section`, `navigate_to`, `enter_scope` / `leave_scope`, and `reveal` (opens the folder view that shows
  a factory).
- `record` / `restore` drive back and forward history.
- `show_code` flies to another project and applies `pending_dir` once that project's map has loaded and been fitted.
- Prefer these over setting `goal` / `goal_zoom` by hand, so that history, the editor hold and folder views stay
  consistent.

## Conventions

- Keep it small. Reuse what is here before adding a helper; add a dependency only when a few lines cannot do it. For
  example, HTTP goes through `curl` in `jira.rs`.
- Comments explain why, not what. No commented-out code.
- Secrets never go on a command line: `jira.rs` passes headers through curl's stdin config and bodies through private
  temp files.
- Cluster access stays read-only (`kubectl get` only).
- Commit messages: conventional commits, e.g. `feat(map): …`, `fix(jira): …`.

## Checks

```sh
cargo build --release   # must stay warning-free
cargo test --release    # fast tests
cargo test --release -- --include-ignored --nocapture
```

The ignored tests exercise real checkouts: `~/Projects/Accounts/acme`, `~/Projects/TheFactory/The-Factory`, and a
reachable cluster for `otc`. They print what they measured, such as layout size or overlap counts. Run them when you
touch layout, physics or the loaders, and skip the ones you have no checkout for.

Smoke modes, when the window is not needed:

| Command | Checks |
|---|---|
| `cellworks --scan <project path>` | scan summary (also `jira:…`, `otc:…`, `data:…`) |
| `cellworks --jira-sync jira:<site>/<KEY>` | sign in to Atlassian if needed, sync, summarise |
| `cellworks --nvim-smoke <file>` | the embedded Neovim |
| `cellworks --term-smoke`, `--claude-smoke <dir>` | session hosts |

Do not open test windows over the user's screen without asking; prefer the smoke modes and tests.
