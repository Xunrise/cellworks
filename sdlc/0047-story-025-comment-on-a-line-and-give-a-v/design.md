---
stage: design
issue: 47
status: frozen
author: claude (freeze)
date: 2026-10-20
---

# Design: STORY-025 Comment on a line and give a verdict

The contract is `issue.md` in this directory. The confirmed behaviour, rejected options and decisions are in
`prototype.md`. This builds on STORY-021 to STORY-024 (#43 to #46), which must have merged first: confirm `Forge`,
`GhCli`, `PullRequestEvent`, `ReviewMode`, `Approval`, `ReviewOverlay`, `TowerReview`, the inside view and the review
panel at their real `path:line` before starting, and record any departure here.

## Open questions

None.

## Requirements

1. Inside a tower in a review, on the after side, `c` opens a one-line text field under the selected floor (the floors
   below move down one row). On the before side `c` does nothing (old-side line numbers cannot be posted as `RIGHT`).
2. While the field is open no city key acts, Esc included. Enter leaves the comment; Esc or a click elsewhere cancels;
   empty or whitespace-only text adds nothing.
3. Comments are review-mode state in Navigation: file, tower and line, relative to the checkout; several per line
   allowed; they survive switching sides and are dropped when the review is left.
4. Inside the tower each comment shows under its floor as "comment: <text>" with a mark in `#4aa8ff`; the hint line
   names `c comment`. In the city a commented floor carries a blue flag at its height (`TowerReview::commented`).
5. The panel shows `COMMENTS · n` after the critical cards: one row per comment (`<tower>:<line>` and the text), a
   click moves the view to the tower; "none yet" and "C ON A FLOOR INSIDE A TOWER" when empty.
6. Esc inside a tower steps out of it and keeps the review; Esc in the city leaves the review. This changes STORY-022's
   requirement 8.
7. `REQUEST CHANGES` is offered once the review holds a comment; `APPROVE` follows STORY-024's rule. Either posts one
   review through the forge adapter: `gh api repos/{owner}/{repo}/pulls/<n>/reviews --method POST --input -`, the JSON
   on stdin, never in arguments: `commit_id` (the head), `event` (`REQUEST_CHANGES` or `APPROVE`), `body`, `comments`.
8. A comment on a line the pull request adds becomes a line comment (`path` the file, `line`, `side: RIGHT`); any other
   comment goes into the body under "Comments on lines this pull request does not change:" as `- <file>:<line>:
   <text>`. A `REQUEST_CHANGES` body always starts with "Changes requested." (GitHub needs a non-empty body).
9. Posting runs on a background thread and reports `PullRequestEvent::ReviewPosted { number, event }` or `ReviewFailed
   { number, reason }`. The verdict buttons hide while posting. On success the review shows `CHANGES REQUESTED` or
   `APPROVED` and takes no second verdict; on failure the comments stay, the buttons return and the panel shows the
   forge's message (for example GitHub refusing a verdict on one's own pull request).
10. `Approval::Approved` (STORY-024) is set when the APPROVE post succeeds, not on the click.
11. Leaving a review while a post is running drops its result.

## Decisions

- **The first write to the forge goes behind the adapter** (ADR-0014): `Forge::post_review(repository, number,
  &ReviewPost)`, `GhCli` piping the JSON on stdin with a fixed argument vector; a pure `compose(diff, comments, event,
  head) -> ReviewPost` splits line comments from body lines. Rejected: `gh pr review` (no line comments); a REST
  client or curl (token handling, ADR-0003); posting each comment separately (several notifications, partial failure).
- **ADR-0009's "read-only `gh`" is superseded for this one call**; ADR-0009, ADR-0011 (Esc order) and ADR-0013
  (approval local only) get one-line status notes pointing to ADR-0014. Those three records are not in this branch's
  base — they are recorded by STORY-021 (#43), STORY-022 (#44) and STORY-024 (#46) — so the status notes are owed when
  those merge; see Departures. STORY-022's and STORY-024's frozen design notes stay untouched; their tests that pin the
  old Esc order and "no forge write" change here.
- **Comments live in Navigation's `ReviewMode`.** Rejected: the binary's session (review mode would again be split).
- **Comments only on the after side.** Rejected: `side: LEFT` comments from the before city (a second coordinate
  system for one comment list).
- **The flag is part of the overlay** (`ReviewOverlay::set_comments`, `TowerReview::commented`). Rejected: a new
  `ViewOptions` field.
- **`inside` changes in place** (comments, typing flag; returns the clicked floor and the input row). Rejected: a
  second inside function.
- **One post per review.** Rejected: allowing a second verdict (GitHub keeps every review; the panel would need a
  history).

## Order of work

1. Sources, the review post: `crates/sources/src/pull_requests/review.rs` (`ReviewPost`, `LineComment`, `compose`, the
   JSON writer), `crates/sources/src/pull_requests/forge.rs` (`Forge::post_review`, `GhCli` with stdin),
   `crates/sources/src/pull_requests/mod.rs` (`start_post_review`, `ReviewPosted`, `ReviewFailed`),
   `crates/sources/src/lib.rs`; unit tests (escaping, added versus unchanged lines, empty comments, the request-changes
   body).
2. Navigation, comments and Esc: `crates/navigation/src/lib.rs` (`ReviewComment`, `comment`, `comments`, posting state
   on `ReviewMode`, `esc` steps out of a tower first in a review, `Approval::Approved` on success); unit tests.
3. Map, comments drawn: `crates/map/src/review.rs` (`set_comments`, `TowerReview::commented`), `crates/map/src/ui.rs`
   (the flag; `inside` with comments, the reserved row and the hint); unit tests.
4. Binary: `src/review.rs` (comments section, `REQUEST CHANGES`, posting state, failure line, verdict chips),
   `src/app.rs` (`c`, the `TextEdit` area, keys muted while typing, starting the post and draining its events,
   dropping a result after leaving); update STORY-022's Esc test and STORY-024's no-write test; delete
   `tests/proto_comment_screens.rs`.
5. Acceptance tests and docs: `tests/story_025_comment_on_a_line_and_give_a_verdict.rs`, fixtures in
   `tests/common/mod.rs` (`FakeForge` records posted reviews and can fail; "shop" #41 adds the fourth floor of
   `discount`); the Docs impact lines the build writes.

## Risks

- STORY-021 to STORY-024 are not merged (their pull requests held only `sdlc/` documents when this was planned); every
  name here follows their frozen designs and must be confirmed first.
- GitHub refuses APPROVE and REQUEST_CHANGES on one's own pull request (422); the panel shows the message, no work
  around. The body, `line` and `commit_id` rules are proven only by one manual post to a throwaway pull request (manual
  smoke).
- The file a tower belongs to is taken from the diff's paths; when it is ambiguous the comment goes into the body, never
  to a wrong file.
- The riskiest step is 4 (a text field over the inside view, three event streams); rollback: revert the squash commit.
  A posted review cannot be rolled back from Cellworks.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: pass,
  no warnings, no window, no network; no test posts to GitHub.
- `tests/story_025_comment_on_a_line_and_give_a_verdict.rs`, one test per scenario, named after it:
  - `a_comment_is_left_on_the_selected_line` — `c`, type, Enter: the fourth floor carries it; the panel lists it.
  - `an_empty_comment_is_not_left` — `c`, Enter: no comment.
  - `stepping_out_of_a_tower_keeps_the_review_open` — Esc inside: outside the tower, still reviewing #41.
  - `comments_are_visible_in_the_city` — after stepping out, `discount`'s `review.commented` holds its fourth floor.
  - `requesting_changes_sends_the_comments_to_the_author` — the fake forge records one REQUEST_CHANGES post with the
    line comment on `discount`'s file and line; the panel shows `CHANGES REQUESTED`.
  - `a_comment_on_a_line_the_pull_request_does_not_change_reaches_the_author_too` — the body holds `- <file>:<line>:
    still needed?`.
  - `approving_sends_the_approval` — the fake records APPROVE with the comments; `APPROVED` after `ReviewPosted`.
- Edge tests: a failed post keeps the comments and shows the message; `c` in before does nothing; keys are muted while
  typing; leaving during a post drops the result; several comments on one floor.
- Unit tests: `compose` and the JSON writer; Navigation comments and Esc order; the overlay's `commented`.
- Manual smoke, not in CI: one REQUEST_CHANGES with a line comment and a body comment on a throwaway pull request.

This is the proof the story needs, not proof it has: none of it was produced on this branch. See Departures.

## Docs impact

- **Business decisions:** `docs/domain/contexts/Navigation/Navigation.md`: comments in a review (after side only,
  dropped on leaving), Esc steps out of a tower before it leaves a review, one verdict per review, approval counts once
  posted. `docs/domain/contexts/Sources/Sources.md`: a verdict is posted as one GitHub review; comments on unchanged
  lines go into its body.
- **Glossary:** Navigation: Review comment, Verdict. Sources: Review post. Map: Comment flag. Counts in
  `docs/domain/glossary.md`.
- **Guidelines:** `docs/services/cellworks/guidelines/architecture.md`, External tools: `gh` runs read-only commands
  except posting a review (`gh api … /reviews`, JSON on stdin).
- **arc42:** `03-context.md`: GitHub is written to once per review (the posted review). `05-building-blocks.md`:
  Sources posts reviews; Navigation keeps comments. `09-decisions.md`: the ADR-0014 line.
- **ADR needed:** yes, ADR-0014 "A review is posted through gh api", decided at freeze and written to
  `docs/adr/0014-a-review-is-posted-through-gh-api.md` verbatim from the frozen prototype (`prototype.sha` `84ed17b`),
  with its `09-decisions.md` index line. Its references to ADR-0009 and ADR-0013 resolve once #43 and #46 merge. The
  status notes on ADR-0009, ADR-0011 and ADR-0013 cannot be written here — those files do not exist on this branch;
  see Departures.

## Departures

Recorded as the design's opening paragraph asks. **The precondition is unmet and the Order of work was not executed.**

- STORY-021 (#43), STORY-022 (#44), STORY-023 (#45) and STORY-024 (#46) have not merged, so the first risk above has
  materialised. `Forge`, `GhCli`, `PullRequestEvent`, `ReviewMode`, `Approval`, `ReviewOverlay`, `TowerReview`, the
  inside view's review panel — none of them has a `path:line` in this branch's base: they appear nowhere in `crates/`
  or `src/`. `crates/sources/src/` holds only `lib.rs`; there is no `pull_requests/` module, and neither
  `crates/map/src/review.rs` nor `src/review.rs` exists. #43 carries STORY-021's implementation on its own branch,
  unmerged; #44, #45 and #46 carry only `sdlc/` documents.
- Every one of steps 1 to 5 extends a type that does not exist here, so the step order cannot be executed as written.
  Building it on this branch would mean implementing #43 to #46 inside #47 — four other stories, each with its own
  issue, design, ADR and pull request. That is outside this design, so it was not done. No source file, no test and no
  `tests/story_025_comment_on_a_line_and_give_a_verdict.rs` exists on this branch.
- The Docs impact for Navigation (`Navigation.md`, its glossary), Sources (`Sources.md`, its glossary), Map's glossary,
  the `docs/domain/glossary.md` counts, the `gh` guideline line and the arc42 `03-context.md` / `05-building-blocks.md`
  lines are **not** written, for the same reason and because their base text is missing: this branch's arc42 and
  context documents do not describe a forge, review mode or a pull-request source at all, since those arrive with #43
  to #46. Writing them now would record behaviour the product does not have. They belong in the same change as the
  code. ADR-0014 is written, because it records a decision taken at freeze rather than shipped behaviour.
- **Proof is unmet.** The cargo checks were re-run on this branch and pass, but they prove only the untouched
  baseline: not one of the seven acceptance tests named in Proof exists, and no manual smoke was run.
- **No counter-review ran.** `roles.json` asks for the implementer, the test author and the backend security reviewer;
  none of them had anything to work on, and no `counter-review.md` was produced. One is owed with the build, not
  written after the fact, so none was added here.
- **A person has to sequence this.** #47 must wait behind #43 to #46; it must not merge into `test` ahead of them as a
  markdown-only change that implements none of its seven scenarios, and its title and description must say that it
  carries the design and ADR-0014 only.
- **ADR-0014 needs a correction this branch may not make.** The second review of `306a19e..d8aa526` is right that the
  record lands ahead of what it amends: it carries `Status: Accepted` and states at `:38` that "ADR-0009's read-only
  rule for `gh` now has this one exception", while ADR-0009 is not in `docs/adr/` here, and its References at `:48-49`
  to ADR-0009 and ADR-0013 are dead relative links in this tree. Merged into `test` ahead of #43 to #46, it lifts the
  read-only posture for external tools by amending a record that is not there. The correction it needs is a status
  that is not yet in force — the decision takes effect with the code, behind #43 to #46 — a Neutral consequence
  written in the future rather than the present, and references that do not pretend to resolve, with the matching
  status word on the `09-decisions.md` index line. This session did not make it: the factory plugin's
  `hooks/guard-paths.sh` denies every build-stage write under `docs/adr/`, because a decision record is a person's to
  take, and names reporting the finding as the route instead. By that same rule ADR-0014's arrival in `d8aa526` was
  already past what a build session may write, which is how it came to sit here in force; a person records both the
  record and its correction, with `adr-writing`.
- **The sequencing condition is still open after two reviews.** The pull request's title is still
  `feature: story-025-comment-on-a-line-and-give-a-v` and its description still claims that the build ran. Neither is
  in the working tree, and the two ways to close it — implementing #43 to #46 inside #47, or retitling the pull
  request — are outside this design and outside a build session. It stays what the bullet above calls it: a decision
  for a person, before this merges.
