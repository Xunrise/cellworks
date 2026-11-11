> Status: draft

# 4. Solution Strategy

Choices are justified against the quality goals in [chapter 1](01-introduction.md) (Usability, Performance,
Extensibility) and the constraints in [chapter 2](02-constraints.md).

## 4.1 Technology

| Choice | Why |
|---|---|
| Rust, native desktop app | Performance on large repositories; constraint in chapter 2. |
| eframe/egui with a custom-painted map canvas ([ADR-0001](../adr/0001-egui-as-ui-framework.md)) | Fastest of the measured options at overview and with dense text; stable releases. |
| tree-sitter for code analysis ([ADR-0004](../adr/0004-tree-sitter-for-code-analysis.md)) | Fast in-process parsing for all supported languages; LSP can enrich later. |
| External tools as child processes: git, kubectl, nvim, curl ([ADR-0003](../adr/0003-external-tools-as-child-processes.md)) | Reuses the user's configuration and credentials; read-only access stays auditable. |
| Embedded Neovim as the editor | A confirmed requirement: the editor the user already uses, inside the map. |

## 4.2 Structure

- **One crate per bounded context** in a Cargo workspace, plus a thin binary
  ([ADR-0002](../adr/0002-cargo-workspace-crate-per-bounded-context.md)). Crate dependencies follow the
  [context map](../domain/context-map.md).
- **The Map owns one generic graph** of districts, groups, nodes, items and lanes. Every loader (CodeAnalysis,
  DomainKnowledge, Sources) produces that graph; navigation, search, lanes and rendering work on the graph only. This
  is how Extensibility is reached: a new project type is a new loader.
- **The district strategy is a seam** in the Map: directories for now, swappable later.

## 4.3 Achieving the quality goals

| Goal | Approach |
|---|---|
| Usability | Keyboard-first input routed by focus (text field, editor, terminal, menu, map); layers of detail with Enter/Esc; the selection always drawn; one navigation model over the generic graph. |
| Performance | Scans and loaders run off the UI thread and hand finished graphs to the Map; level of detail and viewport culling are mandatory; layout is incremental and must not inflate on interaction; derived state is cached between frames. |
| Extensibility | The generic graph and the crate-per-context structure: new project types and district strategies plug in without touching navigation or rendering. |

## 4.4 Driving Claude Code sessions

Sessions are Claude Code background sessions ([ADR-0010](../adr/0010-claude-code-background-sessions.md)). Cellworks starts them with `claude --bg`, follows them through
their transcripts and `claude agents --json`, and types into them or answers their approvals through `claude attach` in
an embedded pseudo-terminal. Claude Code hosts the sessions, so they survive restarts of Cellworks and can be attached
from any terminal.
