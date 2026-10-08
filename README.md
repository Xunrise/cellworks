# Cellworks

A native desktop map that makes an engineer's projects, code, AI agent sessions, tickets, infrastructure and data
navigable in one place.

Cellworks shows the projects you work on — code, Jira, infrastructure, data — as one zoomable map, and your Claude
Code sessions as units moving across it. It runs no server: it reads your machine and the systems you already use,
and draws what it finds.

## Status

Early. The workspace, the eight bounded-context crates and an empty window exist; the map itself is being built
story by story.

## Build and run

Rust 1.85 or later, stable toolchain.

```sh
cargo build --release      # target/release/cellworks
cargo run                  # opens the window
cellworks --version        # prints the version and exits
```

The checks a change has to pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Tests never open a window; the UI is driven headlessly.

## Layout

- `src/` — the `cellworks` binary: the window, the frame loop, the command line
- `crates/<name>/` — one library crate per bounded context: `workspace`, `map`, `navigation`, `code-analysis`,
  `domain-knowledge`, `sources`, `agent-sessions`, `editing`

## Documentation

- `docs/domain/` — bounded contexts, glossary, context map
- `docs/arc42/` — architecture documentation
- `docs/adr/` — architecture decision records
- `docs/services/cellworks/` — the component's conventions and decisions
- `CLAUDE.md` — how to work in this repository
