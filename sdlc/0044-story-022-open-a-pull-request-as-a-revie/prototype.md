# Prototype: STORY-022 Open a pull request as a review in the city

Issue #44, epic EPIC-012. Built on the pull-request source of STORY-021 (#43, frozen on `proto/pr-source`): the
prototype carries that source's prototype commit. Driven headlessly against the cellworks repository's own pull
requests, rendered to PNGs through egui_kittest with wgpu; no window was opened. The side panel follows the canvas
design "Review panel (v13, STORY-022)" on the Cellworks mind palace mockups canvas
(https://claude.ai/artifact/5H97XvynwMnLeW5neamMJ5), which builds on the review pages v6 to v8.

## Confirmed behavior

- `p` with a project open toggles the pull-request panel on the right.
- The list has two tabs, Open and Merged. Open lists every open pull request; Merged the last 30 merged ones. Closed
  pull requests that were not merged are not listed.
- Each list row shows the number, a story chip (`STORY-NNN`, or "NO STORY" when none), lines added and removed, the
  number of files, the author and a chip per critical-change flag the pull request's files raise. Size, file count and
  file paths come with the listing (`gh pr list` fields `additions`, `deletions`, `changedFiles`, `files`), so no diff
  is read to fill the list.
- Clicking a row opens the pull request as a review: its head is checked out into a temporary detached worktree and
  opened as the city, named `<project> · review #<n>` in the breadcrumb, with the project's district rule.
- In the review, towers the pull request does not touch recede (drawn at 18% light); towers it changes keep their
  light.
- The floors (lines) a pull request writes are drawn in the review's written colour (`#5cffb0`).
- Where lines were removed, a red strike (`#ff4d6d`) crosses the tower at the floor they were removed before, and one
  piece of red debris per removed line (at most 24) lies at the tower's foot.
- A tower that calls a changed tower (any line whose resolved calls name a changed tower, tests included) is impacted
  and gets an orange outline (`#ff9f1c`).
- Changed and impacted towers carry a mild pulse: their outline brightens and fades about every three seconds, in the
  colour of what happened to them (written, else removed, else impacted). It replaced beacons, which were too bright.
- The review panel shows: back to the list and "Esc leaves"; `REVIEW #n`, state chip, story chip; title; author,
  branch and short head; the story's title and its As a / I want / So that, read from the issue whose title starts
  with the story id; lines added and removed with a proportional bar and the file count; three counts that double as
  the legend (towers written, towers with removals, impacted callers) in the towers' colours; one card per critical
  flag naming the files that raised it; changed towers grouped by file, most lines first, file name first with its
  folder dimmed, the top four towers per file and the rest folded behind "+ N more towers" (click unfolds); the
  impacted towers; a "Leave review" button.
- Clicking a changed or impacted tower's row selects that row and flies the camera to the tower.
- Esc, the back control or "Leave review" leaves the review: the overlay is gone and the project's own city is
  reopened. No tower recedes or stands out any more.
- Observed on #117 (open, STORY-058): 17 files, +1374/-103, flag `dependencies` (`Cargo.lock`,
  `crates/code-analysis/Cargo.toml`), 92 towers written, 9 with removals, 2 impacted (two STORY-057 tests calling
  changed code-analysis towers); the story card read "As a code architect / I want the functions, classes and imports
  of TypeScript and JavaScript files to stand as towers / So that a TypeScript project reads like a Rust one in the
  city" from issue #83.

## Feedback log

- "Stronger far out" (to: keep the look as shown?) → beacons added above changed towers.
- "Show project name + #117" → the breadcrumb reads `<project> · review #<n>`; leaving reopens the project's own city.
- "Yes, tests too" → test towers count as impacted callers.
- "the side panel is kinda fucked, create a design for that" → the panel was designed on the canvas as page v13 (list,
  review, panel over the real #117 shot).
- "The towers are too glowy now with the beacons, so maybe just a mild pulsating around the changed towers" → beacons
  removed; a mild pulsing outline around changed and impacted towers.
- "The v13 panel design works, build it into the egui prototype (with the story's As a / I want / So that)" → the
  panel was rebuilt to v13 in egui, with the story card read from GitHub issues.

## Rejected

- Beacons above changed towers: too bright, the city became too glowy; replaced by the mild pulse.
- The first panel: a flat list of long wrapped item paths with no grouping, story text or legend; replaced by the v13
  design.
- Keeping the worktree's folder name (`cellworks-review-117-7f4584af`) in the breadcrumb.
- Leaving test towers out of the impacted callers.
- Listing closed, unmerged pull requests (the first prototype listed them with `--state all`).

## Edge cases

- A pull request without a story: the chip reads "NO STORY" and no story card is shown.
- A story id whose issue is not found: the panel says the story's issue was not found.
- A file whose lines no tower holds (`Cargo.lock`, files under `.github/`): it is listed with "no towers: lines outside
  any tower" and counts in the size, not in the tower counts.
- A removed line at the boundary of two towers is attributed to one tower only.
- A changed file with more than four changed towers folds the rest; clicking "+ N more towers" unfolds them.
- Opening a review takes seconds (fetch, worktree, mapping); the panel shows "Mapping the head…" until the city has
  arrived. In the prototype the diff and story are read on the UI thread; the build reads them in the background.
- The pulse needs frames: the review asks for repaints while it is open.

## Decisions

- Q: Is canvas v6 to v8 the settled look, or new canvas pages first? → A: Settled; prototype in egui.
- Q: How is the egui prototype shown? → A: Headless screenshots; no window.
- Q: Keep the look, or stronger far out? → A: Stronger far out (then corrected to the mild pulse).
- Q: Do test towers count as impacted? → A: Yes, tests too.
- Q: The worktree name in the breadcrumb? → A: Show the project name and `review #<n>`; leaving returns to the
  project's own city.
- Q: Does the v13 panel design work? → A: Yes; build it into egui with the story's As a / I want / So that.
- Q: Who provides story text by id (asked by EPIC-011 for STORY-019)? → A: This story, as a public Sources API usable
  by id from other contexts (operator's decision relayed by the EPIC-011 session).
- Q: Is STORY-022 the behaviour wanted? → A: Yes, freeze it.

## Evidence

- Branch `proto/review-city` off `origin/test` (86f8a51), worktree `../cellworks-proto-review-city`; it carries the
  STORY-021 prototype commit.
- Files: `crates/map/src/ui.rs` (`ReviewOverlay`, `ViewOptions::review`, receding, written floors, removal strikes and
  debris, impacted outline, pulse), `src/review.rs` (review state, head checkout, overlay and file groups, the v13
  panel), `src/app.rs` (`p`, the right panel, Esc, jumps, reopen on leave), `crates/sources/src/pull_requests.rs`
  (size, file count and paths in the listing), `crates/sources/src/stories.rs` (story text by id through
  `gh issue list --search "<id> in:title"`), `tests/proto_review_screens.rs` (headless PNGs), `Cargo.toml`
  (egui_kittest `wgpu` and `snapshot` features for the screenshots).
- Run: `REVIEW_PR=117 REVIEW_OUT=<dir> cargo test --test proto_review_screens -- --ignored --nocapture`.
- Design: canvas page "Review panel (v13, STORY-022)", artboards PrList13, ReviewPanel13, ReviewContext13.
