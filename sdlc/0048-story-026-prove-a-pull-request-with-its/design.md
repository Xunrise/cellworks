---
stage: design
issue: 48
status: frozen
author: claude (freeze)
date: 2026-10-20
---

# Design: STORY-026 Prove a pull request with its story's acceptance tests

The contract is `issue.md` in this directory. The confirmed behaviour, rejected options and decisions are in
`prototype.md`. This builds on STORY-021 to STORY-025 (#43 to #47), which must have merged first: confirm `Forge`,
`GhCli`, `StoryText`, `ReviewOverlay`, `TowerReview`, `ReviewSession`, the story card, `FakeForge` and `FakeCheckout`
at their real `path:line` before starting, and record any departure here.

## Open questions

None open for this design's own content. Build was stopped before starting the Order of work — see the finding
below.

**Finding (blocking, not a question to default past):** the design's own precondition — "This builds on STORY-021 to
STORY-025 (#43 to #47), which must have merged first: confirm `Forge`, `GhCli`, `StoryText`, `ReviewOverlay`,
`TowerReview`, `ReviewSession`, the story card, `FakeForge` and `FakeCheckout` at their real `path:line` before
starting" — does not hold. Checked 2026-10-20:

- `origin/feature/43-story-021-pull-request-source-with-diffs` (#43, STORY-021) is built (`build:
  story-021-pull-request-source-with-diffs`, refs #43): `Forge`, `GhCli` and the pull-request source live in
  `crates/sources/src/pull_requests/{forge,mod,diff,critical,placement}.rs`. Not yet merged to `main` or `test`.
- `origin/feature/44-story-022-open-a-pull-request-as-a-revie` (#44, STORY-022), `origin/feature/45-story-023-critical-buildings`
  (#45, STORY-023) and `origin/feature/46-story-024-finish-a-review` (#46, STORY-024) carry only a `design:` commit
  and review-fix commits on the design docs; diffed against their own merge-base each has zero changes under
  `crates/` or `src/`. `StoryText`, `ReviewOverlay`, `TowerReview`, `ReviewSession`, the story card, `FakeForge` and
  `FakeCheckout` do not exist on any branch.
- `origin/feature/47-story-025-comment-on-a-line-and-give-a-v` (#47, STORY-025) likewise carries only its design
  commit, no code.
- `crates/sources/src/lib.rs` on this branch is still the empty seam ("Nothing is exposed yet"); none of this
  design's Order of work can be grounded in real `path:line` references as required.

Stopping here rather than building STORY-022 to STORY-025 myself: they are separate stories with their own issues,
specs/designs and review chains not given to this session. Implementing them as a side effect of STORY-026 would
skip their own intake and review. This is reported as a finding, per instructions, rather than worked around.

**For a person to decide:** this pull request carries the design only. None of STORY-026's seven scenarios is
implemented, so whatever happens to the branch, #48 is not delivered by it: either it lands as a design-only merge and
#48 stays open behind #44 to #47, or the branch waits for them. The pull request's own description ("the full chain —
intent, spec, plan, build — ran automatically") does not describe this build and should not be read as delivery.

## Departures from the frozen design (review fixes, 2026-10-20)

Recorded here because the sections above were edited after the freeze, as the preamble asks:

- Requirement 4 now says apostrophes are dropped before the run of non-alphanumeric characters is collapsed. The frozen
  rule turned `The review lists the story's scenarios` into `the_review_lists_the_story_s_scenarios`, which matches
  neither the test names this design prescribes in `## Proof` nor the repository's existing test files; matching
  nothing, every such scenario would have rendered `NO TEST`.
- The proof target directory is keyed per pull request instead of shared by all reviews. Shared, a pull request's build
  script could leave an artifact or a fingerprint in the cache that `cargo` reuses during a review of an unrelated pull
  request — a trust boundary the per-run consent does not cover.
- ADR-0015 moves into the Order of work (step 6). The freeze commit `f4df818` recorded no ADR, so "recorded at freeze"
  pointed three readers at a file that does not exist, and a resuming build would not have created it.
- The real-cargo test becomes a measurement test (`#[ignore]`). `docs/services/cellworks/guidelines/testing.md` keeps
  real tools out of `cargo test --workspace`, which is the `verify-command` of `sdlc.yml`.

## Requirements

1. The review panel shows a `PROOF` card directly under the story card and before the flag and critical cards.
2. The story's scenarios are the `Scenario:` and `Scenario Outline:` titles of the story issue's Gherkin, in order,
   read through STORY-022's `StoryText` (new field `scenarios: Vec<String>`).
3. The acceptance test file is `tests/story_NNN_*.rs` in the head checkout, `NNN` the story number zero-padded to three
   digits (more digits allowed). When several files match, the first by name wins.
4. A scenario's test is the function in that file whose name is the scenario title in snake case: lowercase, then
   apostrophes (`'` and `’`) are dropped without leaving a separator, then every run of the remaining non-alphanumeric
   characters becomes one `_`, trimmed. `The review lists the story's scenarios` is
   `the_review_lists_the_storys_scenarios` — the convention the repository's own tests already follow
   (`tests/story_053_follow_a_buildings_lanes.rs` for "Follow a building's lanes"). Module-nested tests match on the
   last path segment.
5. Row states: `NOT RUN` (test found, no run yet), `PASSED`, `FAILED`, `NO TEST` (the file holds no test of that name;
   never counted as passed). A test in the file that matches no scenario is not shown. Each row carries the state word
   as text and a disc: grey, green, red, or an amber ring for `NO TEST`; a failing row's title is red.
6. The card names the file. Its top line shows the pull request's CI: `CI PASSES`, `CI FAILS · <failing check names>`,
   `CI RUNNING` (a check pending, none failing), `CI UNKNOWN` (checks cannot be read, or there are none). CI is read
   once when the review opens, in the background, and never blocks the review.
7. `RUN PROOF` (only when the file exists) runs the file's tests at the head on a background thread. While it runs the
   card says "RUNNING THE ACCEPTANCE TESTS AT THE HEAD…" and the button is hidden. It never runs on opening, and never
   twice at once.
8. The result updates the rows: a passing test `PASSED`, a failing one `FAILED`, an `ignored` one or one absent from
   the output `NOT RUN`. When the run produces no test results (it does not compile), the card shows "The run failed:
   <last three lines of stderr>" and the rows keep their states.
9. In the city each acceptance-test tower of the file carries a proof disc at its foot from the moment the review
   opens (grey), turning green or red with its row. The before side (STORY-024) draws no proof discs.
10. A pull request with no story shows "No acceptance tests: #<n> implements no story."; a story whose file is
    missing, or whose issue cannot be read, shows "No acceptance tests found for STORY-NNN.".
11. Leaving the review cancels a running proof (the child process is killed) before the head checkout is removed, and
    drops its result.
12. The run uses its own target directory: never the engineer's build, and never one shared with another pull request.

## Decisions

- **Proof logic lives in Sources, its state in the binary.** A new module `cellworks_sources::proof` holds the pure
  parts (`snake`, `find_file`, `match_scenarios`, `parse_test_output`, `ci_status`, `CiStatus`), the adapters
  (`TestRunner`, `CargoTest`) and the events; the binary's `ReviewSession` holds the per-review `Proof` and the card.
  Rejected: Navigation's `ReviewMode` (Navigation may not depend on Sources, and run state is panel state, not
  navigation progress); a new crate.
- **Tests are found by the existing naming convention** (operator), the rule already in
  `docs/services/cellworks/guidelines/testing.md`. Rejected: a mapping file; searching test bodies for scenario
  titles.
- **Test towers come from the head city**, through `ProjectMap::items_under` of the file, so the marker and the row
  refer to the same thing and a test CodeAnalysis cannot see counts as `NO TEST`. Rejected: grepping `fn <name>` in
  the file (a second Rust parser that can disagree with the city).
- **Two seams.** `Forge::checks(repository, number) -> Result<Vec<Check>, ForgeUnavailable>` (`Check { name, bucket
  }`), `GhCli` running `gh pr checks <n> --json name,bucket --template …` (read-only) with a fixed argument vector; and
  `TestRunner::run(head, stem, target_dir, &CancelToken) -> Result<Vec<TestResult>, String>`, implemented by
  `CargoTest`. `FakeForge` and a new `FakeTestRunner` serve the tests. Rejected: calling `gh` and `cargo` from the
  binary as the prototype does.
- **Fixed command:** `cargo test --test <stem> --no-fail-fast`, `CARGO_TERM_COLOR=never`, `CARGO_TARGET_DIR` set,
  working directory the head checkout; the stem and the target directory's segments are the only values taken from the
  pull request — the stem is validated to `[a-z0-9_]+` and the segments are sanitised to `[a-z0-9_-]+` first, in
  `proof::target_dir`, so neither can steer the command or escape the temp directory. Stdout lines
  `test <name> ... ok|FAILED|ignored` are parsed; `ignored` is `NOT RUN`. Rejected:
  `--message-format json` for tests (unstable on the stable toolchain); running the whole workspace (CI covers it);
  the prototype counting every outcome other than `ok` as failed.
- **Own target directory in the OS temp directory, one per pull request:**
  `<temp>/cellworks-proof-target/<owner>_<repository>/pr-<n>`, each segment taken from the repository slug and the
  number and sanitised to `[a-z0-9_-]+`. A run writes its build cache where only runs of the same pull request read it,
  so the build script or proc macro of one pull request — which `cargo test` executes with the engineer's rights —
  cannot plant an artifact or a fingerprint that `cargo` reuses while the engineer reviews a different one; the second
  review's rows then report on the second pull request's own build. Running the same pull request again, including
  after the agent pushed a fix, still reuses its cache. Rejected: `<project>/target/review-proof` (the prototype;
  writes into the engineer's project); one directory shared by all reviews (the cache crosses the trust boundary
  between pull requests that are not equally trusted, and the engineer's per-run consent is about the pull request in
  front of them, not the one that filled the cache); a directory keyed by head commit (cold build on every push, while
  a new push is the same trust domain as the pull request it belongs to); a fresh directory per review (a full build
  every time).
- **The run is cancellable.** `CargoTest` spawns the child and polls `try_wait`; a `CancelToken` (`Arc<AtomicBool>`)
  kills it. Leaving the review sets the token before the head checkout guard is dropped. Rejected: letting the run
  finish and discarding the result (a build in a removed directory); a timeout (no good default).
- **Own events:** `ProofEvent::{ChecksRead { number, status }, Ran { number, results }, RunFailed { number, reason }}`
  over `std::sync::mpsc`, drained once per frame with the other review events; `start_checks` and `start_run` spawn
  the threads. A checks failure becomes `CiStatus::Unknown`, not an error event. Events for another pull request than
  the open review are ignored. Rejected: new `PullRequestEvent` variants (STORY-021's list and open vocabulary).
- **CI is shown beside the local run and may differ** (operator). `CI RUNNING` exists so pending checks never read as
  passing (the prototype counted only failing checks). CI is read once per open. Rejected: polling CI; refreshing CI on
  every run.
- **Map draws the marker, Navigation is untouched.** Map gains `ProofMark { NotRun, Passed, Failed }`,
  `ReviewOverlay::set_proof(BTreeMap<PathBuf, ProofMark>)` and `TowerReview::proof: Option<ProofMark>`; the disc is
  drawn at the tower's foot in the review colours (grey `#8a8f98`, green `#5cffb0`, red `#ff4d6d`). The binary maps its
  states to `ProofMark`; `NO TEST` has no tower and no mark. Rejected: the prototype's `u8` map; a `ViewOptions` field
  (STORY-025 put review decorations on the overlay).
- **Rows carry a state word as text** so acceptance tests can assert them, and because egui's monospace font lacks
  check and cross glyphs. Rejected: colour-only rows.
- **The scenarios are cached with the story** in STORY-022's cache; reopening costs no extra `gh` call. Rejected: a
  second cache.
- **ADR-0015 "Cellworks runs the project's tests as a child process"**, written by the build as step 6, not at freeze:
  on demand only, fixed arguments, a target directory of its own per pull request, killed on leave, and the run
  executes the pull request's code (build scripts and tests) with the engineer's rights — which is why it never starts
  by itself and why no two pull requests share a build cache. ADR-0003 gets a cross-reference. (The freeze commit added
  no file under `docs/adr/`; the design said "recorded at freeze" for a record that does not exist. See the departure
  note in Open questions.)
- **Prototype leftovers are deleted:** `src/proof.rs`, `tests/proto_proof_screens.rs` and the prototype's proof fields.

## Order of work

1. Sources, story scenarios and CI checks: `crates/sources/src/stories.rs` (`StoryText::scenarios`, the parser for
   `Scenario:` and `Scenario Outline:`), `crates/sources/src/pull_requests/forge.rs` (`Forge::checks`, `Check`,
   `GhCli::checks`), fixtures `crates/sources/tests/fixtures/gh_pr_checks.txt` and the issue fixture with scenarios;
   unit tests.
2. Sources, the proof module: `crates/sources/src/proof.rs` (`snake`, `find_file`, `match_scenarios`, `target_dir`,
   `Proof`, `ScenarioProof`, `TestState`, `CiStatus`, `ci_status`, `parse_test_output`, `TestRunner`, `CargoTest`,
   `CancelToken`, `ProofEvent`, `start_checks`, `start_run`, `Proof::apply`), `crates/sources/src/lib.rs`; unit tests
   and the measurement-level real-tool test `crates/sources/tests/cargo_test_against_real_cargo.rs` (`#[ignore]`, run
   with `cargo test --workspace --release -- --ignored --nocapture`).
3. Map, the marker: `crates/map/src/review.rs` (`ProofMark`, `ReviewOverlay::set_proof`, `TowerReview::proof`),
   `crates/map/src/ui.rs` (the disc at the foot; none on the before overlay), `crates/map/src/lib.rs`; unit tests.
4. Binary, the card and wiring: `src/review.rs` (`ReviewSession::proof`, `proof_card`, the CI line, `Asked::RunProof`,
   draining `ProofEvent`s, the overlay's proof marks), `src/app.rs` (`with_runner`, starting the checks when the head
   is checked out, starting the run, cancelling on leave); delete `src/proof.rs` and `tests/proto_proof_screens.rs`.
5. Acceptance tests and fixtures: `tests/story_026_prove_a_pull_request_with_its_storys_acceptance_tests.rs`,
   `tests/common/mod.rs` (the "shop" head tree gains `tests/story_012_loyalty_discount.rs` with two tests; `FakeForge`
   gains story scenarios and checks; `FakeTestRunner`; pull request #44 without a story).
6. Docs and the decision record: `docs/adr/0015-cellworks-runs-the-projects-tests-as-a-child-process.md` (confirm 0015
   is still the next free number once #43 to #47 have merged) with the cross-reference from ADR-0003, and the Docs
   impact lines below.

## Risks

- **Running a pull request's code:** `cargo test` at the head runs an agent's build scripts and tests with the
  engineer's rights. Mitigation: on demand only, never on opening (settled), fixed arguments, and a build cache no
  other pull request reads; ADR-0015 (step 6) records all three. No sandbox in this story.
- Only Rust acceptance tests are found and run; other repositories show "No acceptance tests found".
- The first run of each pull request builds the project cold in that pull request's target directory (minutes); the
  card shows the running line meanwhile. A crashed Cellworks leaves the child to finish alone. The directories stay in
  the OS temp directory until the OS clears it; Cellworks removes none.
- STORY-021 to STORY-025 are not merged; every name here follows their frozen designs and must be confirmed first.
- CI is read once per open and can go stale (`CI RUNNING` stays until the review is reopened).
- The riskiest step is 4 (a cancellable child process tied to the head checkout's lifetime); rollback: revert the
  squash commit.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: pass,
  no warnings, no window, no network, no real `gh`, `git` or `cargo` — the one test that uses a real `cargo` is
  `#[ignore]`d and runs only in the measurement command.
- `tests/story_026_prove_a_pull_request_with_its_storys_acceptance_tests.rs`, one test per scenario, named after it,
  asserting panel labels and `DrawnTower.review.proof`, never pixels:
  - `the_review_lists_the_storys_scenarios` — open #41: the card names the file and lists "Discount for loyal
    customers" and "No discount for guests" as `NOT RUN`; the fake runner was not called.
  - `the_acceptance_tests_are_marked_as_proof_in_the_city` — both test towers have `proof == Some(NotRun)`; other
    towers `None`.
  - `running_the_proof_shows_which_scenarios_pass` — `RUN PROOF`: the runner gets the file stem; the running line
    shows while the fake is held; "Discount for loyal customers" reads `PASSED` and its tower `Some(Passed)`.
  - `a_failing_acceptance_test_shows_the_proof_as_broken` — the fake fails "No discount for guests": `FAILED`, its
    tower `Some(Failed)`; the other stays `PASSED`.
  - `the_review_shows_the_pull_requests_ci` — the fake forge reports "rust" failing: `CI FAILS · rust`.
  - `a_scenario_without_a_test_is_not_proven` — the file lacks the second test: `NO TEST`, no tower, a run does not
    make it `PASSED`.
  - `a_pull_request_without_a_story_says_it_has_no_proof` — open #44: "No acceptance tests: #44 implements no story.",
    no run button.
- Unit tests: `snake` (punctuation, apostrophes — `The review lists the story's scenarios` is
  `the_review_lists_the_storys_scenarios` — digits, repeated separators, unicode); `target_dir` (two pull requests of
  one repository never share a directory; a slug with a slash, a dot-dot or an upper-case letter is sanitised and stays
  under the temp directory); `find_file` (padding, four
  digits, several matches, no tests folder); `parse_test_output` (ok, FAILED, ignored, module paths, empty);
  `match_scenarios` (nested module, absent); `ci_status` (fail, pending, pass, empty, error); `GhCli::checks` parsing
  from the fixture with a malformed line; the scenario parser (outline, none, indentation); `CancelToken` kills a long
  child; `Proof::apply` (compile failure keeps states, stale number ignored); Map `set_proof`; `proof_card` states;
  leaving during a run cancels it before the checkout is removed.
- Real-tool test, at the measurement level: `crates/sources/tests/cargo_test_against_real_cargo.rs` writes a
  dependency-free crate with `tests/story_001_x.rs` (one passing, one failing, one ignored test) to a temporary
  directory and runs `CargoTest` on it with its own target directory; skipped when `cargo` is missing. It is
  `#[ignore]`d and runs with `cargo test --workspace --release -- --ignored --nocapture`, because
  `docs/services/cellworks/guidelines/testing.md` keeps real tools out of `cargo test --workspace` — a nested `cargo`
  compiling a crate from cold would turn the fast suite into a multi-minute build and make it depend on a `cargo` on
  `PATH`.
- Manual smoke, not in CI: `RUN PROOF` on `squer-solutions/cellworks` #117 (STORY-058, eight scenarios) and the
  no-story message on #113, as recorded in `prototype.md`.

## Docs impact

- **Business decisions:** `docs/domain/contexts/Sources/Sources.md`: a story's acceptance tests are the file
  `tests/story_NNN_*.rs` with one test per scenario named after it; a scenario's proof is not run, passed, failed or no
  test; CI and the local run are shown side by side and may differ; the run starts only on the engineer's request.
- **Tactical model:** none; the proof is a value derived from a story, a file and run results.
- **Glossary:** Sources: Proof, Acceptance test, Proof state. Map: Proof marker. Counts in `docs/domain/glossary.md`.
- **Guidelines:** `docs/services/cellworks/guidelines/testing.md`: the acceptance-test naming is relied on by the
  review's proof, so renaming a test breaks it; spell out the snake-case rule of requirement 4, apostrophes included. `docs/services/cellworks/guidelines/architecture.md`, External tools:
  `cargo test` runs on the head checkout on request, with fixed arguments and a target directory of its own per pull
  request.
- **arc42:** `03-context.md`: GitHub read-only `gh pr checks`; the project's own `cargo test` run as a child process
  at the head checkout. `05-building-blocks.md`: Sources (proof and checks), Map (proof marker). `09-decisions.md`: the
  ADR-0015 line.
- **ADR needed:** yes, ADR-0015 "Cellworks runs the project's tests as a child process", written in step 6 of the
  Order of work (it was not recorded at freeze); cross-reference from ADR-0003.
