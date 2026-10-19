# CodeAnalysis — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| AST metrics | Functions with complexity, imports and calls of a file, through tree-sitter. | prototype-findings L95 |  |
| Co-change | Files changed in the same commits, weighted by how many files each commit changed. | lenses prototype 2026-10-12; EPIC-010; STORY-015 |  |
| Declaration | What an item declares, as its loader reads it: an enum and which variants carry data, a constant or an entry point; whether it is public; the item it is part of; the interfaces it implements; the items its fields hold. The map chooses the building from it. | architecture prototype 2026-10-13; STORY-064; ADR-0007 |  |
| Dependency tree | Imports and calls between files, aggregated to folders and districts. | prototype-findings L96 |  |
| Entry point | Where a program starts: in Rust, `main` at the root of a crate. | architecture prototype 2026-10-13; STORY-064 | shown on the map with a spire |
| File | A source file of a code project. | prototype-findings L46 | shown on the map as a Node |
| Floor provenance | The commit that last wrote a floor's line, with that commit's pull request and stories; none for a line not yet committed. | STORY-018 prototype 2026-10-19; ADR-0008 |  |
| Folder | A directory of a code project. | prototype-findings L45 | shown on the map as a Group |
| Function | A function in a source file, with its complexity. | prototype-findings L47, L95 | shown on the map as an Item |
| Git overlay | Changed files highlighted on the map. | prototype-findings L89, L102 |  |
| Node kind | What kind of file a file is, told by its name: code, document, configuration, specification, infrastructure, build, contract, secrets, an example of secrets, generated, schema or image. The map stands each kind as a building of its own. | architecture prototype 2026-10-13; STORY-067 | shown on the map as the building of the kind |
| Part | An item a file that is not code is read into: a section of a document, a top-level key of a configuration or a level nested under it, a feature file's header or a scenario, a top-level Terraform block. A nested level is part of its key, a scenario part of the header. | architecture prototype 2026-10-13; STORY-067 | shown on the map as a bay, cabinet, column or chimney |
| Replay | Rebuilding the city commit by commit, oldest first, up to today. | living-city design 2026-10-10; EPIC-011 | not Navigation's *History* |
| Text file | A file holding text that no parser reads; it is read line by line in the parts its kind has, or as one item. A file without text, too large or unreadable, and an image, stays a bare block; the lines of secrets are never held, only how wide each is. | nested-folders prototype 2026-10-12; STORY-057; STORY-067 |  |
| Visibility | Whether code outside an item's module may use it: any `pub` in Rust, and every method of an `impl Trait for Type` block. | architecture prototype 2026-10-13; STORY-064 |  |
| Worktree | A git worktree of a code project; opens as its own project. | prototype-findings L103 |  |
