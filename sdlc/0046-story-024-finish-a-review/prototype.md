# Prototype: STORY-024 Finish a review

Issue #46, epic EPIC-012. Built on STORY-021 (#43), STORY-022 (#44) and STORY-023 (#45), all frozen; the prototype
carries their prototype commits. Driven headlessly against the cellworks repository's pull requests #118 and #117,
rendered to PNGs through egui_kittest with wgpu; no window was opened. The look follows the review canvas pages v7
("a review you can finish") and v13.

## Confirmed behavior

- In a review, `n` steps to the next changed tower not yet visited and moves the view to it; the order is critical
  towers first, each group by most lines changed (added plus removed). When every tower is visited, `n` starts again
  at the first.
- A changed tower counts as visited when `n` stepped to it or its row in the review panel was clicked.
- The panel shows `VISITED k / N` (N = changed towers) with a thin progress bar and the hint "N NEXT · CRITICAL
  FIRST"; a visited row carries a check mark and dims; a critical row carries a gold flag.
- `b` (or the `B · AFTER` / `B · BEFORE` chip in the panel header) switches between after and before. Before is the
  city at the pull request's merge base, checked out into its own temporary detached worktree on first use and opened
  as the city, named `<project> · review #<n> · before`. In it the floors the pull request removes are drawn in the
  removed colour (`#ff4d6d`); towers the pull request adds do not exist. Switching back opens the head's city again.
  The view goes to the picked tower in the other city when it exists there.
- `APPROVE` in the panel footer: when every critical changed tower has been visited, the review is approved at once
  and shows an `APPROVED` chip. When critical changed towers are unvisited, a confirmation card names them
  (`<n> CRITICAL TOWERS NOT VISITED`, up to six paths and "+ n more") with `APPROVE ANYWAY` and `KEEP REVIEWING`.
- Approving is recorded in the review in Cellworks only; nothing is sent to GitHub (STORY-025 sends the verdict).
- Visited towers and the approval last while the review is open; leaving and reopening starts fresh.
- Observed on #118: order started with the critical towers `Commit`, `LineHistory`, `ProjectMap` (published language of
  Map); approving at once named those three; after three `n` presses approving needed no confirmation. On #117 the
  before city showed 13 towers with removed floors (`parse.rs::Reader` the largest).

## Feedback log

None.

## Rejected

- Posting the approval to GitHub (`gh pr review --approve`) in this story: declined; STORY-025 sends verdicts.
- Before as the head city with written floors hidden and removed lines drawn back: declined; towers the pull request
  deletes could not be shown.
- Stepping critical towers first and then in file order: declined in favour of most lines first.
- Keeping visited towers per pull request and head after leaving: declined; only while the review is open.

## Edge cases

- A pull request with no critical changed tower: approving needs no confirmation (#117).
- `n` with every tower visited wraps to the first in the order.
- The picked tower does not exist in the other city (a tower the pull request adds, in before): the view stays where
  it is.
- A pull request that only adds lines: the before city has no red floors.
- The confirmation card must fit above the footer; the changed-tower list gives up the room (a prototype clipping bug,
  fixed).
- Switching to before the first time checks out the merge base (seconds); later switches reuse it.

## Decisions

- Q: What does approving do? → A: Record it in the review; STORY-025 sends verdict and comments.
- Q: How does `b` show before? → A: The city at the merge base, its own checkout.
- Q: In what order does `n` step? → A: Critical first, then most lines.
- Q: Is visited remembered after leaving? → A: Only while the review is open.
- Q: Is STORY-024 the behaviour wanted? → A: Yes, freeze it.

## Evidence

- Branch `proto/finish-review` off `origin/test` (86f8a51), worktree `../cellworks-proto-finish`; it carries the
  STORY-021 to STORY-023 prototype commits.
- Files: `src/review.rs` (`Active::order`, `next`, `unvisited_critical`, `approve`, `overlay_before`,
  `base_checkout`, the panel's progress, chips, confirmation and approve button), `src/app.rs` (`n`, `b`, the
  pending jump into the other city), `crates/map/src/ui.rs` (`ReviewOverlay::removed_floors` drawn red),
  `tests/proto_finish_screens.rs` (headless PNGs).
- Run: `REVIEW_PR=118 REVIEW_OUT=<dir> cargo test --test proto_finish_screens -- --ignored --nocapture` (and 117).
