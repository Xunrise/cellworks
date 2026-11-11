> Status: usable

# 9. Architecture Decisions

Index of the architecture decision records in [`docs/adr/`](../adr/README.md).

- [ADR-0001: Use eframe/egui as the UI framework](../adr/0001-egui-as-ui-framework.md) — Accepted — technology · Map, Navigation, Workspace
- [ADR-0002: One crate per bounded context in a Cargo workspace](../adr/0002-cargo-workspace-crate-per-bounded-context.md) — Accepted — structure · cross-cutting
- [ADR-0003: Integrate external tools as child processes](../adr/0003-external-tools-as-child-processes.md) — Accepted — technology · CodeAnalysis, Sources, Editing
- [ADR-0004: Use tree-sitter for code analysis, with LSP as a later enrichment](../adr/0004-tree-sitter-for-code-analysis.md) — Accepted — technology · CodeAnalysis
- [ADR-0005: Lens layouts by community detection](../adr/0005-lens-layouts-by-community-detection.md) — Accepted — technology · Map
- [ADR-0006: Keep the custom arrangement in Cellworks' own data directory](../adr/0006-custom-arrangement-store.md) — Accepted — structure · Workspace
- [ADR-0007: Loaders state declarations and file kinds; the map chooses the building](../adr/0007-loaders-state-declarations-map-chooses-building.md) — Accepted — structure · CodeAnalysis, Map
- [ADR-0008: Per-line provenance from git blame, joined to pull requests and stories by commit message](../adr/0008-per-line-provenance-from-git-blame.md) — Accepted — technology · CodeAnalysis, Map
- [ADR-0009: Read pull requests through gh and git](../adr/0009-pull-requests-through-gh-and-git.md) — Accepted — technology · Sources
- [ADR-0010: Drive Claude Code sessions as Claude Code background sessions](../adr/0010-claude-code-background-sessions.md) — Accepted — integration · AgentSessions
- [ADR-0011: Review mode opens the pull request head as the city](../adr/0011-review-mode-opens-the-pull-request-head-as-the-city.md) — Accepted — structure · Map, Navigation, Sources
- [ADR-0013: The before side of a review is the merge base's own city](../adr/0013-the-before-side-of-a-review-is-the-merge-bases-own-city.md) — Accepted — structure
- [ADR-0014: A review is posted through gh api](../adr/0014-a-review-is-posted-through-gh-api.md) — Accepted — technology

ADR-0012 is recorded by STORY-022 (#44) and enters the index when that merges.
