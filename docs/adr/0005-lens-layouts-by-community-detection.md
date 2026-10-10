# ADR-0005: Lens layouts by community detection

## Status
Accepted

## Date
2026-10-12

## Context
The relationship lenses (imports, calls, co-change) rebuild blocks and districts from a graph of connections rather
than from folders (EPIC-010). The grouping must be deterministic, so the same project always opens to the same city,
fast enough to run on the UI thread when the lens changes (about 600 items and 200 files for cellworks, up to a few
thousand items for the larger projects in scope), and it must say something true: the product owner rejected blocks
that merely "belong together loosely".

## Decision
We will cluster with modularity local moves (the first phase of Louvain), in a fixed node order with ties to the
lowest label, once on the units of a lens (items for calls, files for co-change) to form blocks, and once on the
blocks to form districts. In the imports lens a block is instead a strongly connected set of files (Tarjan): one file,
or the files of an import cycle; districts are still found by modularity. Districts are then placed by a layout of
the lens's own: a staircase by net import direction, a force-directed network, an orbit around the most coupled
district. All of it is pure code in the Map crate, without a graph library.

## Consequences

### Positive
- Deterministic, dependency-free, and fast for the sizes in scope.
- A multi-file block in the imports lens means exactly an import cycle, which is worth marking.

### Negative
- Local moves without Louvain's aggregation phase find smaller communities on large graphs; the calls lens is
  fine-grained (which the product owner chose to keep).
- Label propagation, tried first, collapsed into one giant cluster on imports and was dropped.

### Neutral
- The force-directed and orbit layouts are heuristics with fixed iteration counts; their parameters live with the
  layout code.

## Alternatives considered
- **Label propagation**: simpler, but collapsed into one giant cluster in the prototype.
- **Full Louvain or Leiden**: better communities on large graphs; more code than the sizes need today.
- **A graph crate (petgraph)**: SCC and layout helpers, but a dependency for a few dozen lines.

## References
- Discovery: [lenses and lanes](../discovery/2026-10-12-lenses.md)
- arc42: [05 Building blocks](../arc42/05-building-blocks.md)
- Domain: [Map](../domain/contexts/Map/Map.md)
