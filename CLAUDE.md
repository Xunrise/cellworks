# Cellworks

A native desktop map that makes an engineer's projects, code, AI agent sessions, tickets, infrastructure and data navigable in one place.

## Tech stack

- Rust, edition 2024, stable toolchain; a Cargo workspace with one crate per bounded context
  ([ADR-0002](docs/adr/0002-cargo-workspace-crate-per-bounded-context.md))
- eframe/egui 0.36 with the wgpu backend ([ADR-0001](docs/adr/0001-egui-as-ui-framework.md))
- tree-sitter for code analysis ([ADR-0004](docs/adr/0004-tree-sitter-for-code-analysis.md))
- git, kubectl, nvim, curl and ssh as child processes ([ADR-0003](docs/adr/0003-external-tools-as-child-processes.md))
- No async runtime: background work on `std::thread`, results over `std::sync::mpsc`
- macOS first; the release workflow also builds Linux

## How to run things

```
cargo build --release                          # must stay warning-free
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo test --workspace                         # unit and acceptance tests
cargo test --workspace --release -- --ignored --nocapture   # measurement tests (chapter 10 scenarios)
```

Tests never open windows; the UI is driven headlessly. Do not open app windows over the user's screen without asking.

## Domain at a glance

Cellworks shows an engineer's projects (code, Jira, infrastructure, data) on one zoomable map, and their Claude Code
sessions as units moving across it. The people who use it are the code architect, the AI engineer / operator, the
ticket and ops persona, the data persona and an onboarding colleague. Contexts: **Workspace** (projects, settings,
rescan), **Map** (the source-agnostic graph of districts, groups, nodes, items and lanes, its layout and rendering),
**CodeAnalysis**, **DomainKnowledge**, **Sources** (Jira, infrastructure, data loaders), **AgentSessions**,
**Navigation** and **Editing**. Flow: Workspace identifies a project → a loader (CodeAnalysis, Sources,
DomainKnowledge) writes nodes and lanes into the Map → Navigation moves the selection through layers →
Editing opens a node on request, while AgentSessions place live sessions on the map. Neutral terms are canonical;
the Warhammer 40k words are display theme only. Full model: `docs/domain/`.

## Where things live

- `docs/domain/` — bounded contexts, glossary, context map
- `docs/arc42/` — architecture documentation
- `docs/adr/` — architecture decision records
- `docs/services/<component>/` — per-component docs; start at its `README.md` (purpose + index
  of that component's documents)
- `docs/discovery/` — discovery evidence: the prototype findings and spikes
- `Cargo.toml` — the `cellworks` binary package and the workspace; context crates in `crates/<name>/`

## Foreign systems

Detail in `docs/arc42/03-context.md`.

- Claude Code — sessions started as background sessions with `claude --bg` and followed through their transcripts in
  `~/.claude/projects` and `claude agents --json` ([ADR-0010](docs/adr/0010-claude-code-background-sessions.md))
- git — status, numstat, branches, worktrees, via the CLI
- ssh — reachability probe for a remote host (`ssh ... true`)
- Neovim — embedded with `nvim --embed`
- Atlassian remote MCP — Jira sync over HTTPS, Cellworks as its own OAuth client
- Cockpit — local ticket tracker, HTTP on 127.0.0.1:4747 through `curl`, else the `cockpit` binary
- Linear — tickets over GraphQL through `curl`, personal API key
- GitHub — pull requests and issues through `gh`
- Kubernetes — read-only `kubectl get`
- Terraform — files read from infrastructure repositories

## Cross-context communication

- In-process only. A downstream crate calls the upstream crate's public API; crate dependencies follow the
  context map, and the binary wires the flows that would otherwise form a cycle (`docs/arc42/05-building-blocks.md`).
- Background results reach the UI thread as `<Context>Event` enums with past-tense variants (e.g.
  `SessionEvent::FileWritten`), sent over `std::sync::mpsc` and drained once per frame.
- Identifiers use the glossary's neutral terms (District, Group, Node, Item, Lane, Session). The Warhammer 40k
  words are display theme only and appear only in UI strings.

## Verifying your work

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
actionlint                                     # zero findings
```

Run all of them before reporting any task complete, and paste the output. If a check fails, fix the code, not the
check. Keep the cargo commands the same as the `rust` job in `.github/workflows/ci.yml`.

## Things Claude gets wrong

The rule: **when Claude makes the same mistake twice, the correction goes here.** Nothing recorded yet.

## Per-component conventions

Each component's docs live in its own folder, fronted by a `README.md` that states the component's
purpose and indexes the folder's documents. Before working on a story, read the `README.md` of the
component(s) it touches and open only the documents its index points you at:
- `docs/services/cellworks/README.md` — the desktop app: all crates of the workspace

## Conventions

- Story IDs: `STORY-NNN` / Epic IDs: `EPIC-NNN` / ADR IDs: `NNNN`. Older commits and docs reference them.
- Branch names: `<type>/<slug>` (`feat/`, `fix/`, `chore/`)
- Commit messages: Conventional Commits
- Pull requests into `main`, squash on merge
- Default branch: `main`; releases are cut by pushing a `vX.Y.Z` tag
