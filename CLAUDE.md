# Cellworks

A native desktop map that makes an engineer's projects, code, AI agent sessions, tickets, infrastructure and data navigable in one place.

## Tech stack

- Rust, edition 2024, stable toolchain; a Cargo workspace with one crate per bounded context
  ([ADR-0002](docs/adr/0002-cargo-workspace-crate-per-bounded-context.md))
- eframe/egui 0.36 with the wgpu backend ([ADR-0001](docs/adr/0001-egui-as-ui-framework.md))
- tree-sitter for code analysis ([ADR-0004](docs/adr/0004-tree-sitter-for-code-analysis.md))
- git, kubectl, nvim and curl as child processes ([ADR-0003](docs/adr/0003-external-tools-as-child-processes.md))
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

- Claude Code — sessions observed through transcripts in `~/.claude/projects`; how sessions are driven is still open
- git — status, numstat, branches, worktrees, via the CLI
- Neovim — embedded with `nvim --embed`
- Atlassian remote MCP — Jira sync over HTTPS, Cellworks as its own OAuth client
- Kubernetes — read-only `kubectl get`
- Terraform — files read from infrastructure repositories

## Cross-context communication

- In-process only. A downstream crate calls the upstream crate's public API; crate dependencies follow the
  context map, and the binary wires the flows that would otherwise form a cycle (`docs/arc42/05-building-blocks.md`).
- Background results reach the UI thread as `<Context>Event` enums with past-tense variants (e.g.
  `SessionEvent::FileWritten`), sent over `std::sync::mpsc` and drained once per frame.
- Identifiers use the glossary's neutral terms (District, Group, Node, Item, Lane, Session). The Warhammer 40k
  words are display theme only and appear only in UI strings.

## Per-component conventions

Each component's docs live in its own folder, fronted by a `README.md` that states the component's
purpose and indexes the folder's documents. Before working on a story, read the `README.md` of the
component(s) it touches and open only the documents its index points you at:
- `docs/services/cellworks/README.md` — the desktop app: all crates of the workspace

## Conventions

- **Every source code change must be motivated by a story.** Use `gherkin-story-authoring` for
  behavioral changes (new features, rule changes) and `chore-story` for non-behavioral changes
  (layout tweaks, refactorings, dependency updates, tooling).
- Story IDs: `STORY-NNN` / Epic IDs: `EPIC-NNN` / ADR IDs: `NNNN`
- Branch names: `STORY-NNN-kebab-case-title`
- Commit messages: Conventional Commits with `Refs: STORY-NNN` in footer
- One change request per story, draft while in progress, squash on merge
- Default branch: `main`
