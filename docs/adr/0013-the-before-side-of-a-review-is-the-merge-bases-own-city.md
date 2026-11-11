# ADR-0013: The before side of a review is the merge base's own city

## Status
Accepted

## Date
2026-10-19

## Context
STORY-024 lets the engineer compare a pull request before and after. ADR-0011 made the review city the head's
checkout, which is the after side. The before side must show what the pull request deletes: towers that no longer
exist at the head, and the lines removed from towers that remain.

## Decision
We will show the before side as the city at the pull request's merge base, opened like any other project.
- Before is the city at `git merge-base <base> <head>`. It is checked out on first use into its own temporary
  detached worktree, a second checkout guard that is removed off the UI thread when the review is left.
- It is opened as the city `<project> · review #<n> · before`, with the floors the pull request removes marked.
- Switching back reopens the head's city with its review overlay kept, not recomputed.
- Review progress (the visited towers, the side and the approval) is Navigation's review mode and survives the
  switch.
- Approval is recorded in Cellworks only; nothing is sent to GitHub.

## Consequences

### Positive
- Deleted towers and removed lines can be seen, because the before side is a real city of the old code.
- Both sides use the same city machinery, so no second way of drawing a review is needed.
- Progress does not depend on which side is open.

### Negative
- A review holds up to two temporary worktrees.
- The first switch to before takes seconds, while the merge base is checked out.
- The head's city is read again when switching back.

### Neutral
- A failed merge-base checkout keeps the after side and says so in the panel; it never fails the review.

## Alternatives considered
- **The listed base commit**: rejected because it moves as the base branch moves, so it would show changes the pull
  request did not make.
- **The head city with removed lines drawn back**: rejected because towers the pull request deletes could not show.
- **Both cities open at once**: rejected because ADR-0011 rejected keeping two projects alive.
- **Review progress in the binary**: rejected because it would split review mode over two places.

## References
- [ADR-0011](0011-review-mode-opens-the-pull-request-head-as-the-city.md)
- [ADR-0012](0012-critical-buildings-from-docs-crate-and-marks.md)
- arc42: [03 Context](../arc42/03-context.md), [05 Building blocks](../arc42/05-building-blocks.md)
