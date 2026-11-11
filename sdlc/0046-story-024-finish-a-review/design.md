---
stage: design
issue: 46
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-024 Finish a review

The contract is `issue.md` in this directory. The confirmed behaviour, rejected options and decisions are in
`prototype.md`. This builds on STORY-021 (#43), STORY-022 (#44) and STORY-023 (#45), which must have merged first:
confirm `HeadCheckout`, `start_check_out`, `PullRequestEvent`, `ReviewOverlay`, `TowerReview`, `ReviewMode`,
`Critical` and the review panel at their real `path:line` before starting, and record any departure here.

## Open questions

None.

## Requirements

1. Review progress is part of Navigation's review mode: the visited towers, the side (after or before) and the
   approval (open, asking with the unvisited critical towers, approved). Paths are absolute in the head city; the
   binary translates them onto the before city by swapping the checkout root.
2. `n` (plain, only in a review) steps to the next changed tower not yet visited that exists in the open city and
   moves the view to it, marking it visited. Order: critical towers first, then the rest, each by lines changed
   (added plus removed), ties by path. With every tower visited it starts again at the first.
3. Choosing a changed tower's row in the panel marks it visited.
4. The panel shows `VISITED k / N` (N = changed towers) with a progress bar and "N NEXT · CRITICAL FIRST"; visited
   rows carry a check mark and dim; critical rows carry the gold flag.
5. `b` (plain, only in a review) and the header chip `B · AFTER` / `B · BEFORE` switch sides. Before is the city at
   `git merge-base <base> <head>`, checked out on first use into its own temporary detached worktree (a second
   checkout guard, removed off the UI thread when the review is left) and opened as the city named
   `<project> · review #<n> · before`; the floors the pull request removes are drawn in `#ff4d6d`. After reopens the
   head's city; the head overlay and file groups are kept, not recomputed. Progress survives switching.
6. After switching, the view goes to the picked tower in the other city when it exists there; otherwise it stays.
7. `APPROVE` approves at once when every critical changed tower is visited and shows the `APPROVED` chip; otherwise a
   card names the unvisited critical towers (`<n> CRITICAL TOWERS NOT VISITED`, six paths then "+ n more") with
   `APPROVE ANYWAY` and `KEEP REVIEWING`.
8. Approval is recorded in Cellworks only; no `gh` write happens.
9. Progress and approval last while the review is open; leaving (Esc also with the card open) clears them.
10. A failed merge-base checkout keeps the after city and says so in the panel; it never fails the review.

## Decisions

- **Progress lives in Navigation's `ReviewMode`** (ADR-0011 "Review mode opens the pull request head as the city",
  recorded by STORY-022 (#44); it is not in this branch's base — see Departures), extended with
  `visited`, `side: Side { After, Before }` and `approval: Approval { Open, Asking(Vec<PathBuf>), Approved }`, and the
  operations `review_next`, `review_approve`, `review_approve_anyway`, `review_keep_reviewing`, `switch_review_side`.
  Rejected: keeping progress in the binary's `ReviewSession` (splits review mode over two places).
- **Switching sides reopens a project** and `opened()` clears review mode, so `switch_review_side()` sets a one-shot
  latch that keeps the mode across the next `opened()`. Rejected: `opened()` never clearing review mode (opening an
  unrelated project must end a review).
- **Before is the merge base's own city**
  (`docs/adr/0013-the-before-side-of-a-review-is-the-merge-bases-own-city.md`): `Checkout::merge_base`,
  `start_check_out_base`,
  `PullRequestEvent::BaseCheckedOut { number, checkout }` / `BaseFailed { number, reason }`, reusing `HeadCheckout`.
  Rejected: the listed base commit (it moves as the base branch moves); the head city with removed lines drawn back
  (operator; deleted towers could not show); a new checkout type.
- **`ReviewOverlay::before(map, removed_old_lines)`** fills `removed_floors`; `TowerReview::removed_floors` exposes
  them to tests.
- **`n` skips towers the open city lacks**: a tower the pull request adds stays unvisited in before. Rejected: marking
  it visited unseen; `n` switching to after on its own.
- **The step order is a pure function in the binary** (`step_order`), fed to Navigation, because it needs the
  `Critical` value and the file groups.
- **Prototype leftovers are deleted:** `tests/proto_finish_screens.rs` and the prototype's `Active` fields.

## Order of work

1. Sources, merge-base checkout: `crates/sources/src/pull_requests/forge.rs` (`Checkout::merge_base`, `GitCli` with a
   fixed argument vector), `crates/sources/src/pull_requests/mod.rs` (`start_check_out_base`, `BaseCheckedOut`,
   `BaseFailed`), `crates/sources/src/lib.rs`; `crates/sources/tests/git_cli_against_real_git.rs` (merge base and base
   worktree on a two-commit temporary repository); unit tests.
2. Map, removed floors: `crates/map/src/review.rs` (`ReviewOverlay::before`, `removed_floors`),
   `crates/map/src/ui.rs` (draw them, `TowerReview::removed_floors`); unit tests.
3. Navigation, review progress: `crates/navigation/src/lib.rs` (`Side`, `Approval`, extended `ReviewMode`, the
   operations, the latch, visited on `pick_in_review`); unit tests.
4. Binary: `src/review.rs` (`step_order`, the base state machine and event draining, cached head overlay and groups,
   the before overlay, the panel's progress, chips, confirmation and approve button), `src/app.rs` (`n`, `b`, the
   chip, root translation, pending jump, leaving drops both checkouts); delete `tests/proto_finish_screens.rs`.
5. Acceptance tests and docs: `tests/story_024_finish_a_review.rs`, fixtures in `tests/common/mod.rs` (the "shop"
   #42 changing the critical `Order::cancel` and `receipt`; `FakeCheckout` serving a head and a base tree, recording
   adds and removals); the Docs impact lines the build writes.

## Risks

- The STORY-021 to STORY-023 code may differ from their designs (PRs #122 and #123 held only `sdlc/` documents when
  this was planned); confirm the names first and record departures.
- STORY-022's checkout fake and leave path may need a refactor to hold a second checkout.
- Critical pennants in the before city need STORY-023's home snapshot rebased onto the base root; if that is awkward
  the before city shows none, and the build says so.
- The riskiest step is 4 (two cities, three event streams); rollback: revert the squash commit.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: pass,
  no warnings, no window, no network, no real `gh` or `git` outside the real-git test.
- `tests/story_024_finish_a_review.rs`, one test per scenario, named after it:
  - `stepping_through_changes_visits_critical_towers_first` — `n`: selection `Order::cancel`, `VISITED 1 / 2`.
  - `choosing_a_change_in_the_review_counts_as_visiting_it` — clicking `receipt`'s row marks it, not `Order::cancel`.
  - `the_review_shows_how_much_has_been_looked_at` — after stepping to `Order::cancel`: `VISITED 1 / 2`.
  - `before_and_after_can_be_compared` — `b`: breadcrumb ends `review #42 · before`, `Order::cancel`'s
    `review.removed_floors` holds the two removed floors; `b`: back to `shop · review #42`, no `removed_floors`, one
    base worktree added, visited kept.
  - `approving_with_unvisited_critical_towers_asks_first` — the card names `Order::cancel`; no `APPROVED`;
    `KEEP REVIEWING` closes it.
  - `approving_after_visiting_every_critical_tower_needs_no_confirmation` — `n`, approve: `APPROVED`, no card, no forge
    write recorded.
  - `what_was_visited_lasts_only_while_the_review_is_open` — `n`, Esc (both worktrees removed), reopen #42:
    `VISITED 0 / 2`, not approved.
- Unit tests: Navigation progress (order and wrap, picking, approval with and without unvisited, latch kept across a
  side switch and cleared by a plain `opened`, leave then enter is fresh); `step_order`; `ReviewOverlay::before`;
  base checkout (guard on drop and failed add, `BaseFailed`, stale events); the session's base state machine.
- Manual smoke, not in CI: `n` and `b` on `squer-solutions/cellworks` #118 and #117 as recorded in `prototype.md`.

This is the proof the story needs, not proof it has: none of it was produced on this branch. See Departures.

## Docs impact

Each item carries its status on this branch. **Owed** means the item is still part of STORY-024 and has not been
cancelled: it is deferred to the change that writes the code, because its text does not apply to this base. Nothing
here is dropped. See Departures for the evidence.

- **Business decisions:** `docs/domain/contexts/Navigation/Navigation.md`: what counts as visited, the step order,
  `n` and `b`, the approval rule, progress lasts only while the review is open, nothing is sent to GitHub.
  — **owed**; `## Business decisions` on this base (`Navigation.md:24-43`) holds no review bullet at all, so the
  STORY-022 line these six extend ("At most one review is open, and opening any project ends it") is not there.
- **Tactical model:** `Navigation.md` `### Aggregates`, Navigation: review progress lives as long as review mode;
  approval needs every critical changed tower visited unless confirmed.
  — **owed**; `Navigation.md` on this base is 43 lines and has no `## Tactical model` or `### Aggregates` section at
  all, so there is no Navigation aggregate sentence to extend.
- **Glossary:** Navigation: Visited tower, Review progress, Approval, Before city (the merge base's city), After city.
  Map: Removed floor, if not yet defined. Counts in `docs/domain/glossary.md`.
  — **owed**; Navigation's glossary has no Review mode term to sit beside, Map's has no Review overlay or Removal
  strike term, and the counts differ from the prototype's by whole stories (Map 11 here against 58 there, Navigation
  7 against 14), so the count edits cannot be written to a right number yet.
- **Guidelines:** none. — **n/a**.
- **arc42:** `05-building-blocks.md`: Navigation (review progress), Sources (merge-base checkout). `03-context.md`: a
  review may hold a second temporary worktree, at the merge base.
  — **owed**; `05-building-blocks.md` does not mention a review at all on this base, and `03-context.md`'s git row
  does not yet mention the review's first temporary worktree that this item adds a second to.
  `09-decisions.md`: the ADR-0013 line. — **written** (`docs/arc42/09-decisions.md`), with a note on the numbers
  reserved for #43, #44 and #45.
- **ADR needed:** yes, ADR-0013 "The before side of a review is the merge base's own city", decided at freeze with the
  operator and written to `docs/adr/0013-the-before-side-of-a-review-is-the-merge-bases-own-city.md` verbatim from the
  frozen prototype (`prototype.sha` `ad5e564`), with its `09-decisions.md` index line. Its references to ADR-0011 and
  ADR-0012 resolve once #44 and #45 merge. — **written**; it is a new file recording a decision taken at freeze, so
  it stands on this base as written.

## Departures

Recorded as the design's opening paragraph asks. **The precondition is unmet and the Order of work was not executed.**

- STORY-021 (#43), STORY-022 (#44) and STORY-023 (#45) have not merged. `HeadCheckout`, `start_check_out`,
  `PullRequestEvent`, `ReviewOverlay`, `TowerReview`, `ReviewMode`, `Critical` and the review panel have no
  `path:line` in this branch's base: they appear nowhere in `crates/` or `src/`. `crates/sources/src/` holds only
  `lib.rs`; there is no `pull_requests/` module and no `src/review.rs`. #43 carries STORY-021's implementation on its
  own branch, unmerged; #44 and #45 carry only `sdlc/` documents, so the first risk above has materialised twice over.
- Every one of steps 1 to 5 extends a type that does not exist here, so the step order cannot be executed as written.
  Building it on this branch would mean implementing #43, #44 and #45 inside #46 — three other stories, each with its
  own issue, design, ADR and pull request, and #43's would collide with the work already on `feature/43-…`. That is
  outside this design, so it was not done.
- The Docs impact for Navigation (`Navigation.md` business decisions and `### Aggregates`), the glossary terms and the
  arc42 `05-building-blocks.md` / `03-context.md` lines are **owed, not cancelled**, and are not written here. Every
  one of them is marked **owed** in `## Docs impact` above with the reason, so the next person can tell the two apart
  from the artifact alone. Two reasons, both checkable:
  - They describe review progress, the step order, the approval rule and a second merge-base worktree, none of which
    the product does on this branch. Writing them now would record behaviour that does not exist.
  - They are not transplantable. The frozen prototype `ad5e564` touched eight files; the two written here (ADR-0013
    and its `09-decisions.md` index line) are **additions**, while the other six are **edits to text STORY-021 to
    STORY-023 put there**, and that text is absent from this base. `docs/arc42/05-building-blocks.md` contains no
    occurrence of "review" at all; `docs/domain/contexts/Navigation/Navigation.md` is 43 lines with no review bullet
    and no `### Aggregates` section; `docs/domain/contexts/Map/glossary.md` has neither "Review overlay" nor
    "Removal strike"; `docs/arc42/03-context.md` does not yet mention a temporary detached worktree; and
    `docs/domain/glossary.md`'s counts are Map 11 and Navigation 7 here against the prototype's 58 and 14, so the
    prototype's `58 → 59` and `14 → 19` edits would write counts that are wrong by whole stories. The split was
    therefore decided by what applies to this base, not by preference.
  ADR-0013 is written for both reasons in reverse: it is a new file that applies to any base, and it records a
  decision taken at freeze rather than shipped behaviour. Its `## Context` does describe the head-checkout review
  city that STORY-022 has not yet delivered — an ADR's context is the situation the decision was taken in, and the
  decision was taken at freeze, so this is the one place where naming the not-yet-built is correct. The owed six
  would instead assert the behaviour as the product's current truth, which it is not.
- **Proof is unmet.** The cargo checks below were re-run on this branch and pass, but they prove only the untouched
  baseline: no acceptance test named in Proof exists, and `tests/story_024_finish_a_review.rs` was never written.
- **A person has to sequence this.** #46 must wait behind #43, #44 and #45; it must not merge into `test` ahead of
  them as a markdown-only change that implements none of its seven scenarios.
