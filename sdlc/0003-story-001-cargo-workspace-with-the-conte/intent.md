---
stage: intent
issue: 3
status: draft
author: Xunrise
date: 2026-10-08
---

# Intent: Cargo workspace with the context crates and an empty window

## Problem

The Cargo workspace described in [ADR-0002](../../docs/adr/0002-cargo-workspace-crate-per-bounded-context.md) does
not exist yet: there is no root `Cargo.toml` and no `crates/` directory, so none of the eight context crates
(`workspace`, `map`, `navigation`, `code-analysis`, `domain-knowledge`, `sources`, `agent-sessions`, `editing`) exist.
Every later story needs a crate to add its code to, and `.github/workflows/release.yml` already stamps a version
into `Cargo.toml` and runs `cargo build --release --target <target>` against it — with no `Cargo.toml`, that workflow
has nothing to build. Doing this scaffolding as its own piece of work keeps the first behavioural stories about
behaviour rather than setup.

## Proposed outcome

A root `cellworks` binary package declares the workspace, and the eight context crates exist under `crates/` with
the dependencies arc42 chapter 5 describes. Running the binary opens an empty eframe/egui window titled "Cellworks".
Running it with `--version` prints the version instead of opening a window. `cargo build --release`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` and `cargo test --workspace` all
succeed with no warnings, both locally and in the release workflow, on both Linux and macOS.

## Affected users and systems

No persona sees a behavioural change from this story — it is the foundation the later feature stories build on. It
affects: engineers implementing the next stories, who need a crate to put code in; the release workflow
(`.github/workflows/release.yml`), which stamps a released version into `Cargo.toml` and packages the `cellworks`
binary per target, and currently has no `Cargo.toml` to act on; and the code architect persona, who relies on the
crate dependency graph matching the context map.

## Constraints

- [ADR-0002](../../docs/adr/0002-cargo-workspace-crate-per-bounded-context.md) decides one crate per bounded
  context, plus the binary.
- [docs/arc42/05-building-blocks.md](../../docs/arc42/05-building-blocks.md) §5.2 names the eight crates and their
  dependency arrows; they must follow the [context map](../../docs/domain/context-map.md), with no cycles. Two
  context-map relationships (CodeAnalysis → Workspace, AgentSessions → Workspace) are not crate dependencies — the
  binary wires them instead, to avoid a cycle through `map → workspace` — and Sources → Navigation needs no crate
  dependency either.
- [ADR-0001](../../docs/adr/0001-egui-as-ui-framework.md) commits the window to eframe/egui 0.36 with the wgpu
  backend.
- Rust edition 2024 on the stable toolchain, and no async runtime — background work uses `std::thread` with results
  over `std::sync::mpsc` — per the project's documented tech stack.
- `.github/workflows/release.yml` stamps its computed `VERSION` into `Cargo.toml`'s `version` field before building,
  then runs `cargo build --release --target <target>` and packages the resulting `cellworks` binary; its build
  matrix already targets `aarch64-apple-darwin` (macos-14) and `x86_64-unknown-linux-gnu` (ubuntu-latest), and
  `cargo test --release` runs only on the Linux target.
- `.github/workflows/sdlc-build.yml`'s verify-command
  (`cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`)
  must keep passing, and must stay identical to the equivalent commands documented for contributors.

## Open questions

None.
