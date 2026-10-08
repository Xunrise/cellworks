# Context: CodeAnalysis

Understanding a code project: per-file AST metrics, imports and calls aggregated into dependency trees, and git state including branches and worktrees.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Code architect
- tree-sitter — parses TS/JS, Python, PHP, Rust, Lua, Bash
- git

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- AST metrics through tree-sitter: functions with complexity, imports, calls (L95).
- Dependency trees: imports and calls between files, aggregated to folders and districts (L96).
- Git: status, numstat, an overlay of changed files; a branch and worktree switcher; worktrees open as their own projects (L102-103).

## Tactical model

> Status: not yet discovered
> The tactical model emerges during implementation. A story adds an
> entry here only for a new aggregate, a changed aggregate boundary or
> a new invariant — the code is the rest of the model. Do not
> pre-populate from the discovery source: tactical choices are made at
> code time, not at analysis time.

### Aggregates
_No entries yet._

### Entities (non-aggregate roots)
_No entries yet._

### Value objects
_No entries yet._

### Domain services
_No entries yet._
