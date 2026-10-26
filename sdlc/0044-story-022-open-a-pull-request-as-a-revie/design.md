---
stage: design
issue: 44
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-022 Open a pull request as a review in the city

The contract is `issue.md` in this directory: its scenarios are what the change must do. The confirmed behaviour,
feedback, rejected options and decisions are in `prototype.md`; the panel follows canvas design v13 ("Review panel
(v13, STORY-022)"). This builds on STORY-021 (#43), whose pull-request source must have merged first: read its real
signatures before starting; the names below follow its frozen design and may differ in detail.

## Open questions

None.

## Requirements

1. `p` with a project open toggles the review panel on the right; it lists open pull requests and the last 30 merged
   in two tabs (Open, Merged); closed-unmerged pull requests are not listed. The list reloads each time the panel opens.
2. A list row shows number, story chip (or "NO STORY"), lines added and removed, file count, author and one chip per
   critical-change flag. Size, count and paths come from the listing; no diff is read for the list. `gh` returns at
   most 100 file paths per pull request, so a row's flags may miss files beyond that; the review's flags are complete.
3. Choosing a row opens a review: in the background the head is fetched, diffed and checked out into a temporary
   detached worktree; the head is then opened as an ordinary project named `<project> · review #<n>` with the home
   project's district rule. The panel says "Checking out #n…" and then "Mapping the head…" meanwhile.
4. Once the head's map has arrived, the overlay is computed: written floors and removed lines per tower from
   STORY-021's placement, impacted towers (any tower with a line whose resolved calls name a changed tower, tests
   included, never a changed tower itself).
5. Drawing (Map): untouched towers at 18% light; written floors in `#5cffb0`; a strike in `#ff4d6d` at the floor each
   removed line was removed before, and one debris piece per removed line (at most 24) at the foot; impacted outline
   `#ff9f1c`; a mild pulse (about 3 s) around changed and impacted towers in the colour of written, else removed, else
   impacted. Repaints are requested only while a pulsing tower is in view.
6. The panel follows v13: back control and "Esc leaves"; `REVIEW #n`, state chip, story chip, title, author, branch
   and short head; story card (title, As a, I want, So that) or "The story's issue was not found."; size with a
   proportional bar and file count; three counts doubling as the legend; one card per flag with its files; changed
   towers grouped by file, most lines first, file name first and folder dimmed, four towers per file then
   "+ N more towers" (click unfolds), "no towers: lines outside any tower" for such files; impacted towers; "Leave
   review". Theme words appear only in UI strings.
7. Clicking a tower row (changed or impacted) marks the row and moves the view to that tower.
8. Esc leaves the review before any other step, also from inside a tower; so do the back control and the button.
   Leaving drops the head checkout on a background thread, clears the overlay and reopens the home project.
9. Story text is a public Sources API (promised to EPIC-011 for STORY-019): `cellworks_sources::stories` with
   `StoryText { id, issue, title, as_a, i_want, so_that }`, `read(repository, id) -> Result<Option<StoryText>,
   ForgeUnavailable>`, `start_read(repository, ids) -> Receiver<StoryEvent>`, `StoryEvent::{Read { id, story },
   ReadFailed { id, reason }}`, plus `read_with`/`start_read_with` taking a `Forge`. The story's issue is the one whose
   title starts with `<id>:`, found with read-only `gh issue list --state all --search "<id> in:title"`. Found stories
   are cached per repository and id for the session; misses and failures are not.
10. Nothing slow runs on the UI thread: list, fetch, diff, checkout, story and worktree removal run on `std::thread`s
    and report as events; placement and the impacted walk run on the UI thread once (pure CPU over the mapped head).
11. A checkout event for a pull request that is no longer the one being opened is ignored and its guard dropped off
    the UI thread.

## Decisions

- **Crate flow per arc42 05, no new dependency.** Sources produces pull requests, diffs, placements and story text;
  Map owns `ReviewOverlay` (published language, with `ReviewOverlay::new` working out impacted towers, since Map owns
  the calls) and draws it from `ViewOptions::review`; Navigation owns `ReviewMode { number, picked }`; the binary
  wires them and holds the egui panel in `src/review.rs` (it shows Sources types). Rejected: Map or Navigation
  depending on Sources.
- **The head is kept as a checkout and opened as a project** (ADR-0011). Sources gains `HeadCheckout` (drop guard),
  blocking `check_out` and `start_check_out`, and `PullRequestEvent::CheckedOut { number, head, diff, size, flags }`;
  failure reuses `OpenFailed`; `place` becomes public. Rejected: STORY-021's `open`, which maps the head and removes
  the worktree (the city would be mapped twice and the editor could not open files at the head); an in-memory map
  without a folder.
- **STORY-021 types grow here:** `PullRequest` gains `additions`, `deletions`, `changed_files`, `files`, `flags`, and
  `list` fills `flags` once; the type carrying a tower's own lines (`placement::TowerPlacement`, not `TowerLines` as
  this decision assumed before STORY-021's real signatures were known) carries removals as the `Removal` STORY-021's
  `diff` module already defines — `Removal { old_line, before_new_line }`, not `Removal { old, before }` as first
  written here — so strikes land on the right floor. Rejected: computing flags per frame (the prototype read
  `critical.yaml` every frame); a size-less list.
- **Esc leaves first.** Rejected: stepping out of a tower first and needing a second Esc.
- **Leaving reopens the home project** (a re-read). Rejected: keeping the home `OpenedProject` alive while the review
  shows (two projects' threads and receivers).
- **Story cache is process-wide** (`Mutex<HashMap>`) and keeps only found stories. Rejected: caching misses (a later
  issue would never show); a cache object threaded through the app.
- **Tests assert drawn facts, not pixels:** `DrawnTower` gains `review: Option<TowerReview { written, removed,
  impacted, pulsing }>`; tests read it, `light`, panel labels and the breadcrumb. The egui_kittest `wgpu`/`snapshot`
  features of the prototype are removed. While a review is open the pulse repaints, so tests use `run_steps`.
  Rejected: pixel snapshots (GPU on CI, brittle).
- **Prototype leftovers are deleted:** `tests/proto_review_screens.rs`, `examples/pr_review.rs` (neither existed on this
  branch when the build ran — STORY-021 never added them under those names — so there was nothing to delete).
- **Departure:** the review fixture's impacted-caller tower lives in `orders/src/lib.rs` (`checkout`, `ship`), not a
  separate `orders/checkout.rs`, and "catalog" needed its own `src/lib.rs` (`pub use pricing::{discount, total};`)
  beside `pricing.rs` so `orders`'s `use catalog::discount;` resolves across files at all — `cellworks_code_analysis`
  indexes a crate's items by each file's own module path, so a bare `catalog::discount` only resolves through a
  crate-root re-export, exactly the mechanism `crates/code-analysis/src/rust/resolve.rs`'s own doc comment already
  describes for `cellworks_map::ProjectMap`/`graph::ProjectMap`. Without this the "impacted" scenario could not be
  demonstrated in a fixture with no real `gh`/`git`, which the Risks section anticipated as a possible gap.
- **Departure:** `cellworks_map`'s pulse keeps requesting an immediate repaint while any pulsing tower is in view (as
  designed — a live animation needs it), so an acceptance test driving a review must advance the headless harness with
  bounded `Harness::run_steps(n)` rather than `Harness::run()` (which waits for the frame to go idle and never does
  while pulsing) — already noted above ("tests use `run_steps`"); this turned out to apply to every helper in
  `tests/story_022_open_a_pull_request_as_a_review_in_the_city.rs` that advances the harness once a review may be
  active, not only to a dedicated pulse test.
- **Fix (counter-review):** `CellworksApp::open` now clears `self.review` first — the build's first pass left it
  untouched, so a review still listing, mid-checkout or mapping (never one already shown as the city, which already
  went through `Navigation`'s own review-clearing `esc`) survived opening a different project, and its background
  checkout could complete afterwards and silently reopen the pull request's head in place of the project the engineer
  had just opened. This directly contradicted the requirement 11 and Navigation docs line this same change adds.
  Opening the review's own head goes through the new private `CellworksApp::open_review_head` instead, which shares
  `open`'s own body (now factored into `open_reading`) but does not clear the session it is itself completing.
  Covered by `tests/story_022_open_a_pull_request_as_a_review_in_the_city.rs`'s
  `opening_a_different_project_ends_a_review_session_in_progress`.
- **Fix (counter-review):** the Decisions section above named the real `placement::TowerPlacement` type `TowerLines`
  and `Removal`'s fields `old`/`before`, following this design's own pre-freeze assumption rather than STORY-021's
  real, merged names (`TowerPlacement`, `Removal { old_line, before_new_line }`) — corrected above, in the same bullet
  the Risks section already flagged this kind of drift for.
- **Fix (review):** Esc that closes the project clears `CellworksApp::review` too, so a listing, checkout or
  mapping still in flight cannot reopen the head after the project was closed (the twin of the `open` fix above).
- **Fix (review):** `groups` lists only towers the pull request added or removed lines of, and keeps a file's
  lines outside every tower as `outside` whether or not the file has towers (requirements 6 and 8).

## Order of work

1. [x] Sources, story text and listing: `crates/sources/src/stories.rs`, `crates/sources/src/pull_requests/forge.rs`
   (`Forge::story_issues`, listing template and parser fields), `crates/sources/src/pull_requests/mod.rs`
   (`PullRequest` fields and flags, `HeadCheckout`, `check_out`, `start_check_out`, `CheckedOut`, public `place`),
   `crates/sources/src/pull_requests/placement.rs` (`Removal`), `crates/sources/src/lib.rs`; fixtures
   `crates/sources/tests/fixtures/gh_pr_list_open.txt`, `gh_pr_list_merged.txt`, `gh_issue_list.txt`; unit tests.
2. [x] Map, the overlay: `crates/map/src/review.rs` (`ReviewOverlay`, `ReviewOverlay::new`, `TowerReview`),
   `crates/map/src/lib.rs`, `crates/map/src/ui.rs` (`ViewOptions::review`, drawing per requirement 5,
   `DrawnTower::review`, the existing `ViewOptions` literals in its tests); unit tests.
3. [x] Navigation, review mode: `crates/navigation/src/lib.rs` (`ReviewMode`, `enter_review`, `review`, `pick_in_review`,
   `leave_review`, `Moved::LeftReview`, `esc` leaves first, `opened` clears it); unit tests.
4. [x] Binary, wiring and panel: `src/review.rs` (`ReviewSession`, event draining, `overlay_from`, `groups`, the v13
   panel), `src/app.rs` (`p`, the right panel, `with_forge`, `with_checkout`, opening the head, leaving, the overlay in
   `ViewOptions`), `src/lib.rs`; `Cargo.toml` and `Cargo.lock` without the kittest `wgpu`/`snapshot` features; delete
   `tests/proto_review_screens.rs` and `examples/pr_review.rs`.
5. [x] Acceptance tests and docs: `tests/story_022_open_a_pull_request_as_a_review_in_the_city.rs`, fixtures in
   `tests/common/mod.rs` (`FakeForge`, `FakeCheckout`, the "shop" home and head trees with `orders/checkout.rs` and
   `sdlc/critical.yaml`); the Docs impact lines the build writes.

## Risks

- STORY-021's real signatures may differ from its frozen design; adapt the Sources step, keep the behaviour, and
  record the departure here.
- "Impacted" relies on CodeAnalysis resolving calls across files (`checkout` calling `discount` in another file). If
  it does not, the fixture calls within one file and the build says so; CodeAnalysis is not changed here.
- The head worktree lives for the whole review under the OS temp directory; a crash leaves it behind
  (`git worktree prune` clears it).
- About 20 files across four crates; the riskiest step is 4 (app wiring with three event streams). Step 1 can land
  on its own if the change must be split.
- Rollback: revert the squash commit; the review mode is reachable only through `p`.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: pass,
  no warnings, no window, no network, no real `gh` or `git` in tests.
- `tests/story_022_open_a_pull_request_as_a_review_in_the_city.rs`, one test per scenario, named after it:
  - `open_and_merged_pull_requests_are_listed_with_their_story_size_and_flags` — `p`; #41 with `STORY-012`, `+3`,
    `−2`, `2 files`, `dependencies`; the Merged tab holds the merged one.
  - `a_pull_request_opened_from_the_list_is_shown_as_a_review` — breadcrumb `shop · review #41`, the opened folder is
    the head's, the story card shows title, As a, I want, So that.
  - `the_floors_a_pull_request_writes_stand_out` — `discount`'s `review.written` is its two new floors and it pulses;
    untouched towers' `light` is far below.
  - `removed_lines_are_shown_where_they_were_removed` — `total`'s `review.removed` is 2, at the floor they were
    removed before.
  - `callers_of_changed_towers_are_marked_as_impacted` — `checkout`'s `review.impacted`.
  - `the_review_lists_the_changed_towers_by_file` — `pricing.rs` group with `discount` (+2) and `total` (−2);
    choosing `discount` selects it and frames it.
  - `leaving_the_review_restores_the_city` — Esc (and the button): breadcrumb `shop`, no tower with `review` or reduced
    light, the fake checkout recorded the worktree removal.
- Unit tests: `ReviewOverlay::new` (tests count, changed towers never impacted, empty diff); painter marks over a small
  map; Navigation review mode (Esc first, also inside a tower; `opened` clears it; `pick_in_review`); stories (title
  prefix, not a mention; `Ok(None)`; forge failure; cache hit; `start_read` events); listing parser with the new
  fields, flags from files, malformed line; `HeadCheckout` removes on drop and when `check_out` fails after adding;
  `groups` and `overlay_from` (order, fold after four, file without towers).
- Manual smoke, not in CI: `p` against `squer-solutions/cellworks` #117, as observed in `prototype.md`.

## Docs impact

- **Business decisions:** `docs/domain/contexts/Navigation/Navigation.md`: Esc leaves a review first; leaving reopens
  the project's own city; the review city is named `<project> · review #<n>`; at most one review is open and opening
  any project ends it. `docs/domain/contexts/Map/Map.md`: in a review untouched towers recede and changed, removed and
  impacted towers are marked. `docs/domain/contexts/Sources/Sources.md`: a story's text comes from the issue whose
  title starts with its id; the list shows size and flags from the listing.
- **Glossary:** Navigation: Review mode. Map: Review overlay, Impacted tower, Written floor, Removal strike. Sources:
  Story text. The per-context counts in `docs/domain/glossary.md`.
- **Guidelines:** `docs/services/cellworks/guidelines/architecture.md`, Threads and events: a guard that runs a process
  when dropped (a checkout, a worktree) is dropped on a background thread, never on the UI thread.
- **arc42:** `05-building-blocks.md`: interfaces of `map` (review overlay), `navigation` (review mode), `sources`
  (story text, head checkout), and that the binary wires Sources to Map and Navigation for reviews. `03-context.md`:
  the GitHub entry also covers read-only `gh issue list`. `09-decisions.md`: the ADR-0011 line.
- **ADR needed:** yes, ADR-0011 "Review mode opens the pull request head as the city", recorded at freeze with the
  operator; ADR-0009 gains one cross-reference line.
