# Discovery: code stands as the building its kind is

**Date:** 2026-10-13
**Status:** confirmed by the product owner through a throwaway prototype on the folder hierarchy as built (EPIC-021);
built as STORY-064 to STORY-069 (EPIC-024).

## Summary

Every item stood as the same plain tower, so a class, an enum, a constant and an entry point could not be told apart
from afar, and a type's methods stood scattered beside it. Code now stands as the building its kind is: a type with
methods is a stepped skyscraper of its methods crowned by the traits it implements, an enum a terrace of row houses,
a constant a monument, an entry point a landmark with a spire. Buildings show how their code is used (doors,
forecourts, masts, skybridges), files that are not code stand as buildings of their own kind, and blocks, folders and
districts grow like towns, outlined round what they hold. The facts come from the loaders, the buildings from the map
(ADR-0007).

## How we got here

The prototype ran in a worktree off the folder hierarchy as built and was iterated with the product owner on the
running app, with cellworks itself and a multi-service project of about 3,500 files as the projects. Each round was
checked against the "Cellworks Architecture Vocabulary" design canvas.

| Tried | Verdict |
|---|---|
| A type's methods as tiers stepping back above it, the largest lowest | Kept |
| A type's methods on a podium beside it | Rejected: it said the methods belong together, not what the class is; not intuitive |
| A crown on top for each trait implemented, one colour per trait | Kept |
| An enum as a terrace of row houses, an annex for each variant carrying data | Kept |
| A constant as a monument, an entry point with a spire and a beacon | Kept |
| Methods written in another file standing on their own file's block | Rejected: a type showed only part of itself |
| Buildings in straight rows on their block | Rejected: read like a circuit board; buildings that belong together stood apart |
| Blocks and folders in circuit-board rows or rings | Rejected: the folders lens read like a circuit board, not a town |
| Buildings placed beside what they are most bound to, the skyline kept | Kept |
| Cables from every mast drawn over everything | Rejected: cables and bridges showed through the buildings in front of them |
| M toggling the masts themselves | Rejected: masts are part of the building; only their cables are optional, and the call lanes already say where code reaches |
| M toggling the masts' cables, masts always shown | Kept |
| Masts of tests above and below the ground at once | Rejected: the cellar's masts belong to the underground view, hanging upside down |
| Masts of tests hanging below, shown only underground | Kept |
| Callers, doors, masts and outlines worked out every frame | Rejected: about 34 ms per frame far out against 16 ms |
| Worked out once per map with the lanes, details left out far away | Kept |

## Confirmed behaviour

- A type with methods stands as a skyscraper: the type is its base, each method a tier stepping back a tenth of the
  base's shorter side on every side, the largest lowest, never past 0.42 of it in all. Each tier is a box of its own
  with its floors.
- A type wears a crown on top for each trait it implements, two steps high, in the trait's colour: every type
  implementing the same trait wears the same crown.
- A type gathers its methods and traits from the whole project: a method written in another file stands as a tier on
  its type's skyscraper, on its type's block. Selection, the tree, lanes and navigation follow it there; a file
  holding only `impl` blocks of types written elsewhere stays an empty block.
- An enum without methods is a terrace: a house per variant, 3.4 wide, with an annex behind for each variant that
  carries data.
- A constant is a monument on 6 by 6 ground: a plinth, an obelisk and its cap. A terrace and a monument show their
  lines only close enough to read them.
- An entry point is a tower with a spire: a plinth, a shaft a third as tall as the building and at least 14, and a
  haloed beacon that grows with the zoom, so it stays in sight from far out.
- Anything else stays a tower. Tests hang in the cellar, each a tower of its own.
- The arrows walk from building to building; a tier walks as its building. Tab still visits every item.

## Settled while building

- A skybridge joins buildings standing on the same block only; one reaching into another block or district was rejected on review.
- Any `pub`, `pub(crate)` and `pub(super)` included, counts as public; so does every method of an `impl Trait for
  Type` block.
- Associated constants stay part of their `impl` block; only constants and statics of a module are monuments.
- Building features are worked out with the lanes, whether lanes are shown or not.

## Open

- Doors, forecourts, masts and skybridges (STORY-065), cables (STORY-066), files that are not code (STORY-067), the
  organic layout and outlines (STORY-068) and working features out off the frame loop (STORY-069) are built in their
  own stories.
