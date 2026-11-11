# Prototype: STORY-025 Comment on a line and give a verdict

Issue #47, epic EPIC-012. Built on STORY-021 to STORY-024 (#43 to #46), all frozen; the prototype carries their
prototype commits. Driven headlessly against the cellworks repository's pull request #117, rendered to PNGs through
egui_kittest with wgpu; no window was opened. Sending was a dry run: nothing was posted to GitHub.

## Confirmed behavior

- Inside a tower during a review, `c` opens a one-line text field directly under the selected floor (the floors below
  move down to make room), with the hint "Comment on this line · Enter leaves it · Esc cancels". While it is open no
  other key acts on the city.
- Enter leaves the comment on that floor (the file line) and closes the field; Esc closes it without a comment. Text
  that is empty or only whitespace adds no comment.
- Inside the tower, each comment shows under its floor as "comment: <text>" with a blue mark (`#4aa8ff`). The inside
  view's hint line names `c comment`.
- In the city, a commented floor carries a blue flag off the tower's front corner, at that floor's height.
- The review panel shows `COMMENTS · n` after the critical cards and before the changed towers, one row per comment
  (`<tower>:<line>` and the text); clicking a row moves the view to that tower. With none, it says "none yet" and the
  hint "C ON A FLOOR INSIDE A TOWER".
- Inside a tower during a review, Esc steps out of the tower and the review stays open; in the city Esc leaves the
  review. This changes STORY-022's frozen requirement 8 ("Esc leaves the review first, also from inside a tower").
- `REQUEST CHANGES` appears in the footer once the review holds a comment; `APPROVE` (STORY-024) stays. Both verdicts
  are sent to GitHub as one review: `gh api repos/{owner}/{repo}/pulls/<n>/reviews --method POST --input -` with
  `commit_id` (the head), `event` (`REQUEST_CHANGES` or `APPROVE`), `body` and `comments`.
- A comment on a line the pull request adds becomes a line comment (`path` = the file, `line`, `side: RIGHT`). A
  comment on any other line goes into the body under "Comments on lines this pull request does not change:" as
  `- <file>:<line>: <text>`, because GitHub takes line comments only on lines of the diff.
- After sending, the review shows `CHANGES REQUESTED` (or `APPROVED`) and the verdict buttons are gone. In the
  prototype a "DRY RUN · <event> · NOT SENT" card showed the exact call and body instead of posting.
- Observed on #117: "why round here?" on `typescript.rs::items` line 109 (added) became a line comment; "is this still
  needed?" on `lib.rs::load` line 46 (unchanged) went into the body; Esc inside the tower kept the review open.

## Feedback log

None.

## Rejected

- Posting to a throwaway pull request from the prototype: declined; the prototype is a dry run and tests pin the call
  with a fake forge.
- Recording approval only locally: declined; both verdicts are sent.
- A comment box in the review panel instead of under the floor: declined.
- Handing the comments to an agent session when its pull request gets changes requested: not in this story (it belongs
  with the agent-sessions epic).
- Another key than Esc to step out of a tower during a review: declined.
- Allowing comments only on lines the pull request changes: declined; other lines go into the body.

## Edge cases

- Empty or whitespace-only comment: not added (scenario).
- Several comments on one floor: all are kept and shown under it.
- A comment on a line in a file the pull request does not change at all: it goes into the body with its path.
- Request changes with no comment: the button is not offered.
- After a verdict is sent the review takes no second verdict; leaving and reopening the review starts fresh, and the
  comments of the left review are gone (they were sent or dropped).
- The comments are kept per tower path relative to the checkout, so they stay in place when switching to before and
  back (STORY-024).
- The prototype's review header crowds when `CHANGES REQUESTED` and the before/after chip are both shown; the build
  wraps or shortens it.

## Decisions

- Q: How does the prototype prove sending? → A: A dry run that shows the payload; tests pin the real call with a fake
  forge.
- Q: Does approving also send an APPROVE review? → A: Yes, both verdicts are sent.
- Q: Where is a comment typed? → A: Inline under the floor.
- Q: Hand comments to an agent session? → A: Not in this story.
- Q: How does Esc work inside a tower during a review? → A: Out of the tower first; in the city it leaves the review.
- Q: Comments on lines the pull request does not change? → A: Into the review body.
- Q: Is STORY-025 the behaviour wanted? → A: Yes, freeze it.

## Evidence

- Branch `proto/review-comments` off `origin/test` (86f8a51), worktree `../cellworks-proto-comments`; it carries the
  STORY-021 to STORY-024 prototype commits.
- Files: `crates/map/src/ui.rs` (`inside_with_comments`, the comment rows and the input row, `ReviewOverlay::comments`
  and the city flag), `src/review.rs` (`Active::comment`, `review_call`, `send`, the comments section, verdict buttons,
  dry-run card), `src/app.rs` (`c`, the text field area, keys muted while typing, Esc order, `step_inside` helper),
  `tests/proto_comment_screens.rs` (headless PNGs).
- Run: `REVIEW_PR=117 REVIEW_OUT=<dir> cargo test --test proto_comment_screens -- --ignored --nocapture`.
