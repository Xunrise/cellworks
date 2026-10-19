# ADR-0007: Loaders state declarations and file kinds; the map chooses the building

## Status
Accepted

## Date
2026-10-13

## Context
Code is to stand as the building its kind is: a type with its methods as a stepped skyscraper crowned by the traits it
implements, an enum as a terrace, a constant as a monument, an entry point with a spire, and files that are not code
as buildings of their own kind (EPIC-024). The map is source-agnostic (ADR-0002, the Map's graph types are the
language every loader speaks), and loaders must not reach into layout or rendering. Something has to say what an item
is, and something has to decide what it looks like. The prototype read both from the first line of an item's text in
the map, which tied the map to Rust syntax and broke for anything written differently.

## Decision
We will have loaders state facts and let the map choose the building. Every item carries a `Declaration`: what it
declares (`Plain`, `Enum` with which variants carry data, `Constant`, `EntryPoint`), whether it is public, the item it
is part of, the interfaces it implements by name, and the items its fields hold. Every node carries the kind of file
it is. A loader fills these from its own parse, resolving names across the files of the project; the map groups a
block's items into buildings from these facts alone and lays out and draws them. A part written in another file than
the item it is part of stands on that item's building, on that item's block: every block holds its buildings.

## Consequences

### Positive
- The map knows no language: a second parser fills the same facts and gets the same buildings.
- The building vocabulary changes in one place, the map, without touching any loader.
- Tests state facts directly when they build a graph by hand.

### Negative
- An item is not always on its file's block; code that walks a file's items must find them by path, not by node.
- A fact a loader cannot tell (a method on a type it cannot resolve) leaves the item a plain tower.

### Neutral
- The facts are few on purpose; a new building form adds a fact only when none of the existing ones tells it.

## Alternatives considered
- **The map reads the item's first line**: what the prototype did; ties the map to one language's syntax.
- **Loaders choose the building**: puts layout and rendering vocabulary into every loader, against the context map.
- **Parts stay on their file's block**: a type's methods written in another file would stand apart from it, and a
  type would show only part of what it is.

## References
- Discovery: [architecture](../discovery/2026-10-13-architecture.md)
- Domain: [Map](../domain/contexts/Map/Map.md), [CodeAnalysis](../domain/contexts/CodeAnalysis/CodeAnalysis.md)
