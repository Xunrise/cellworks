# ADR-0004: Use tree-sitter for code analysis, with LSP as a later enrichment

## Status
Accepted

## Date
2026-10-08

## Context
CodeAnalysis needs functions with complexity, imports and calls for every file of projects with up to about 20,000
files, across TS/JS, Python, PHP, Rust, Lua and Bash. Scans must not block the UI and must finish fast enough to
rescan after sessions write files. Precise cross-file references would improve call lanes, but require per-language
tooling.

## Decision
We will analyse code with tree-sitter grammars in-process. Analysis sits behind an interface in the CodeAnalysis
context so that an LSP-based enrichment can add precise references later without changing the Map.

## Consequences

### Positive
- Fast, in-process parsing with no per-project setup and no external servers.
- One mechanism for all supported languages; a new language is a new grammar and a query.

### Negative
- Calls and imports are resolved by name heuristics, not types; some call lanes will be wrong or missing.
- Each grammar is a compiled dependency that adds build time and binary size.

### Neutral
- An LSP enrichment, when added, is a separate decision.

## Alternatives considered
- **LSP servers only**: precise references. Rejected for now because they need language servers installed per
  language and are slow to index 20,000 files.
- **Regular expressions**: rejected because complexity and nesting cannot be measured reliably.

## References
- arc42: [04 Solution strategy](../arc42/04-solution-strategy.md)
- Domain: [CodeAnalysis](../domain/contexts/CodeAnalysis/CodeAnalysis.md)
