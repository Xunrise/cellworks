# Discovery: the folder hierarchy in the folders lens

**Date:** 2026-10-12
**Status:** confirmed by the product owner through a throwaway prototype on the lenses as built (EPIC-010); built as
STORY-056, STORY-057 and STORY-062.

## Summary

On a large project whose code sits under one top folder (`services/` holding forty services), the folders lens
showed one district with every file spiralled around its plaza: the file hierarchy, the thing the folders lens is
for, could not be read. Every folder is now a ring inside the ring of the folder holding it, a chain of folders
holding only one folder is one ring, and every file holding text stands as a tower even when no parser reads its
language. Far out, towers are boxes.

## How we got here

The prototype ran in a worktree off the lens performance fixes and was iterated with the product owner on the running
app, with a multi-service project of 3,555 files (538 of them Rust, 1,763 TypeScript) as the project. Ground plans of
the layout were checked as images before each run.

| Tried | Verdict |
|---|---|
| Rings packed along the spiral in folder order | Rejected: a large folder placed late leaves wide empty bands |
| Rings packed largest first | Kept |
| A ring centred on the middle of what it holds | Rejected: a few loose files beside one large folder pull the centre off and inflate the ring |
| A ring centred on its middle or its largest, whichever needs the smaller ring | Kept |
| Avenues from a district's plaza to every block | Rejected: with nesting they fan out across every ring |
| Test-only blocks moved to the district's cellar | Rejected: their folders' rings stood empty |
| A ring for every folder of a chain (`src`, `main`, `kotlin`) | Rejected: rings inside rings with nothing between them |
| Files no parser reads as bare blocks | Rejected: most rings of a TypeScript project looked empty |
| Every line of every tower drawn far out | Rejected: 28 ms per frame far out once every file had a tower; boxes bring it to 15 ms |

## Confirmed behaviour

- Every folder is a dashed ring, in its district's colour, inside the ring of the folder holding it. Inside a folder
  its rings and blocks are packed largest first, along a spiral out from the middle.
- A folder holding only one folder and no file is one ring named after both: `src/main/kotlin`. The tree, the
  breadcrumb and the folder view name it the same way.
- A folder's name is written on the front of its ring once the ring is wide enough on screen to carry it; until then
  it is clickable only for the accessibility tree. Clicking a written name, or anywhere on the ring's open ground,
  selects the folder; a selected folder's ring is outlined. A district's open ground outside every ring stays empty
  ground (STORY-013).
- In the folders lens no avenue runs from a district's plaza to its blocks: the rings say where a block belongs.
- Tests stay in the folder holding them.
- A file holding text that no parser reads stands as one tower, a floor per line, at most 400. A file larger than
  512 KB, not UTF-8, holding a NUL byte or empty stays a bare block.
- Below a zoom of 0.5 a tower is drawn as its box with a lit front, and picked by its middle.

## Settled while building

- A district's own blocks of only tests still lie in its cellar under the plaza (STORY-048); only deeper ones stay
  in their folder's ring. The relationship lenses, whose districts hold only blocks, keep their cellar as before.
- Rings are drawn only in the folders lens. The custom lens moves blocks without moving the folders' rings, so its
  folders keep no ground of their own.
- Ring names are clickable over the ground only once written; an unwritten name would otherwise take clicks meant
  for the towers standing over it.

## Open

- Text towers in the relationship lenses stand in the outskirts: they import and call nothing.
- Classes as compound buildings of their methods (EPIC-021).
