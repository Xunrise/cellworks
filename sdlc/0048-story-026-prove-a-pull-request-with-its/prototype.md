# Prototype: STORY-026 Prove a pull request with its story's acceptance tests

Issue #48, epic EPIC-012. Built on STORY-021 to STORY-025 (#43 to #47), all frozen; the prototype carries their
prototype commits. Driven headlessly against the cellworks repository's pull requests #117 (STORY-058) and #113 (no
story), rendered to PNGs through egui_kittest with wgpu; no window was opened.

## Confirmed behavior

- The review panel shows a `PROOF` card directly under the story card (As a / I want / So that) and before the
  critical cards.
- The story's acceptance tests are found by naming convention: the file `tests/story_NNN_*.rs` of the story's number
  (three digits) in the head checkout, and in it one test per scenario, named after the scenario title in snake case
  (`Discount for loyal customers` → `discount_for_loyal_customers`). The scenarios come from the story issue's Gherkin.
- The card shows the test file's path and one row per scenario with a disc: grey not run, green passed, red failed
  (the title in red too), and an amber ring when the file holds no test of that scenario's name.
- `RUN PROOF` runs the file's tests at the head on a background thread (`cargo test --test <stem> --no-fail-fast`,
  its own target directory so the engineer's build is not disturbed); while it runs the card says "RUNNING THE
  ACCEPTANCE TESTS AT THE HEAD…". The run is on demand only, never on opening the review.
- When the run cannot produce test results (it does not compile), the card shows "The run failed: <last lines of the
  error>".
- The card's top line shows the pull request's CI from its GitHub checks: `CI PASSES`, `CI FAILS · <failing check
  names>`, or `CI UNKNOWN` when the checks cannot be read. The local run and CI are shown side by side and may differ:
  CI covers the whole workspace, the local run only the story's acceptance tests.
- In the city, each acceptance-test tower carries a proof disc at its foot, coloured like its scenario row (grey, green,
  red).
- A pull request that names no story shows "No acceptance tests: #<n> implements no story."; a story without a test
  file shows "No acceptance tests found for STORY-NNN."
- Observed on #117 (STORY-058): all 8 scenarios matched by convention and passed locally in about 21 s while CI showed
  `CI FAILS · rust`; breaking one test in the temporary head checkout showed that scenario red in the card and on its
  tower. #113 showed the no-story message.

## Feedback log

None.

## Rejected

- Only a local run, or only CI: declined; both are shown.
- Running the tests automatically when the review opens: declined; on demand from the panel.
- Finding the tests by a mapping file or by searching for the scenario titles in test bodies: declined; naming
  convention.
- Check and cross glyphs for the states: egui's monospace font lacks them; discs are used.

## Edge cases

- A scenario without a test of its name: amber ring, never counted as passed.
- A test in the file that matches no scenario: not shown.
- The run fails to compile: the error line, the discs stay as they were.
- CI checks cannot be read (no `gh`, no access): `CI UNKNOWN`, the review still works.
- The before side (STORY-024) shows no proof discs; the proof belongs to the head.
- Leaving the review while a run is going: its result is dropped.
- The story issue cannot be read: no scenarios, "No acceptance tests found for STORY-NNN.".

## Decisions

- Q: Where does pass or fail come from? → A: Both a local run and CI.
- Q: How are a scenario's tests found? → A: By naming convention.
- Q: When do the tests run? → A: On demand from the panel.
- Q: Is STORY-026 the behaviour wanted? → A: Yes, freeze it.

## Evidence

- Branch `proto/review-proof` off `origin/test`, worktree `../cellworks-proto-proof`; it carries the STORY-021 to
  STORY-025 prototype commits.
- Files: `src/proof.rs` (`snake`, `find`, `ci`, `run`, `TestState`, `Proof::poll`), `src/review.rs` (the PROOF card,
  `Asked::RunProof`, the overlay's proof entries), `crates/sources/src/stories.rs` (scenario titles from the issue),
  `crates/map/src/ui.rs` (`ReviewOverlay::proof`, the disc at a tower's foot), `tests/proto_proof_screens.rs`
  (headless PNGs).
- Run: `REVIEW_PR=117 REVIEW_OUT=<dir> cargo test --test proto_proof_screens -- --ignored --nocapture` (and 113).
