# ADR-0011: Review mode opens the pull request head as the city

## Status
Accepted

## Date
2026-10-19

## Context
STORY-022 opens a pull request as a review. The city must show the code of the pull request's head, and the editor must
open files at the head, not at the checkout the engineer has open. ADR-0009 (STORY-021) places lines at the head by
checking it out into a temporary detached worktree, mapping it with CodeAnalysis and removing the worktree afterwards,
which leaves no folder for the editor to open.

## Decision
We will keep the head's checkout for the whole review and open it as an ordinary project.
- The checkout is held by a drop guard in Sources. It is removed on a background thread when the review is left.
- The head is opened as an ordinary project named `<project> · review #<n>`, so mapping, layout, rendering and the
  editor work on it unchanged.
- The overlay is a value of the Map's published language, `ReviewOverlay`, passed in `ViewOptions`. Map works out the
  impacted towers, because Map owns the calls.
- Review mode (the open pull request and the picked row) is Navigation state.
- The binary wires Sources to Map and Navigation: it receives the checkout and the pull request's lines from Sources,
  builds the overlay and hands it to Map, and tells Navigation to enter review mode.
- Leaving a review reopens the home project. Esc leaves a review first, before any other step out.

## Consequences

### Positive
- The head is mapped once, and the editor opens files at the head.
- One mapping path serves reviews and ordinary projects.
- Crate dependencies keep to the context map.

### Negative
- A worktree lives under the OS temp directory for the whole review; a crash leaves it behind, and
  `git worktree prune` clears it.
- Opening takes seconds on large repositories (fetch, checkout, map); all of it runs on background threads.
- The home city is read again on leaving.

### Neutral
- ADR-0009's description of a worktree removed after mapping applies to reading a pull request's lines, not to a
  review.

## Alternatives considered
- **STORY-021's map-and-remove `open`**: rejected because the head would be mapped twice and the editor could not open
  files at the head.
- **An in-memory map without a folder**: rejected because the editor has the same problem and it adds a second mapping
  path.
- **Keeping the home project open alongside the review**: rejected because two projects would run their threads and
  receivers at once.
- **Map or Navigation depending on Sources**: rejected because it is against the context map.

## References
- [ADR-0003](0003-external-tools-as-child-processes.md)
- [ADR-0009](0009-pull-requests-through-gh-and-git.md)
- arc42: [05 Building blocks](../arc42/05-building-blocks.md)
