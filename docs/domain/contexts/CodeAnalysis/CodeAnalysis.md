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
