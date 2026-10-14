# CodeAnalysis — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| AST metrics | Functions with complexity, imports and calls of a file, through tree-sitter. | prototype-findings L95 |  |
| Dependency tree | Imports and calls between files, aggregated to folders and districts. | prototype-findings L96 |  |
| File | A source file of a code project. | prototype-findings L46 | shown on the map as a Node |
| Folder | A directory of a code project. | prototype-findings L45 | shown on the map as a Group |
| Function | A function in a source file, with its complexity. | prototype-findings L47, L95 | shown on the map as an Item |
| Git overlay | Changed files highlighted on the map. | prototype-findings L89, L102 |  |
| Text file | A file holding text that no parser reads; it is read line by line as one item. A file without text, too large or unreadable stays a bare block. | nested-folders prototype 2026-10-12; STORY-057 |  |
| Worktree | A git worktree of a code project; opens as its own project. | prototype-findings L103 |  |
| Co-change | Files changed in the same commits, weighted by how many files each commit changed. | lenses prototype 2026-10-12; EPIC-010; STORY-015 |  |
