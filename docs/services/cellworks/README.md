# cellworks

The Cellworks desktop application: one binary built from a Cargo workspace with one crate per bounded context. It
owns the whole product, from project discovery and loaders to the map, navigation, sessions and the embedded editor.
It runs no server and writes nothing to clusters.

- **Bounded context(s):** Workspace, Map, Navigation, CodeAnalysis, DomainKnowledge, Sources, AgentSessions, Editing
- **Building block:** all crates in `docs/arc42/05-building-blocks.md` (level 2)
- **Code lives in:** this repository — `src/` (the binary) and `crates/<name>/`
- **Tech:** Rust 2024, eframe/egui 0.36 (wgpu), tree-sitter, external tools as child processes
- **Build & test:** `cargo test --workspace`
- **Talks to:** git, Claude Code transcripts, Neovim, Atlassian remote MCP, kubectl, Terraform files

## Documents in this folder

Read this index first and then open **only** the documents your task needs.

| Document | What it contains | Read it when |
| --- | --- | --- |
| `guidelines/` | Architecture (crates, threads, errors, external tools), naming, testing. `guidelines/README.md` indexes them. | Writing or reviewing production or test code. |
| `decisions/` | Component-scoped mini-ADRs — decisions binding only this component. `decisions/README.md` indexes them all. | Planning or reviewing a change: scan the index, open the entries that bear on it, and check the change does not contradict one. |
