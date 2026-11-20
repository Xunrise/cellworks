---
stage: design
issue: 138
status: frozen
author: claude (freeze)
date: 2026-10-27
---

# Design: Freeze a prototype and review the change (STORY-087)

The contract is `issue.md` in this directory: its acceptance scenarios are what
the change must do. The confirmed behaviour, the operator's feedback and what was
rejected are in `prototype.md`. This note holds only what they do not pin.

## Open questions

None.

## Requirements

1. FREEZE (stage 3) shows, over the prototype's well, each closed round (`R<n>`, its notes joined by " · " or "No note",
   its files) or "No round closed yet.", the whole `.cellworks/feedback.log` with 1-based line numbers or "Nothing sent
   yet.", and in the third column (x 1216, w 360) the eyebrow HANDS OFF with three plates: PROTOTYPE.MD, PLAN, BUILD on a
   Factory project, else PLAN, APPROVE, REVIEW. The log is read once on entering FREEZE; a missing file reads as empty.
2. FREEZE (the CTA, Enter with no widget focused, and the lifecycle's PLAN button all return `Run(Act::Plan)`) is
   enabled on `prototyping` and `prototyped`. It writes `prototyping → prototyped` (history note "frozen"), types the
   planning request into the session named `case.session` with `SessionsView::type_into`, and only when the typing
   succeeded writes `prototyped → planning` (a refusal there is ignored: the session may already have written
   `planned`). Then the case goes to PLAN. When the session is not running the case stays `prototyped` and the notice
   says to press PLAN once it is.
3. The request follows `PlanFlavour::of(root)`: `Factory` when `.github/workflows/sdlc.yml` is a file; else `Jarvis`
   when the project's own `.claude/settings.json` has an `enabledPlugins` key starting `jarvis-execute@` set to `true`
   (any read or parse failure means not Jarvis); else `Claude`. Factory types `/factory:freeze`; Jarvis types
   `/jarvis-execute:plan-story <KEY> -- plan only, …`; Claude types the ticket-plan request. Both non-Factory requests
   name `git diff <base>`, `.cellworks/feedback.log`, the case file, the instructions file, "set its status to planned"
   and "commit nothing".
4. The ticket-plan instructions ship in the binary (`crates/sources/assets/ticket-plan.md`, `include_str!`) and are
   written to `<cases dir>/ticket-plan.md` before each Jarvis or Claude request. They describe the plan fields of
   `CaseFile` and the write rule: read, change only the plan part, temp file and rename, `history` append-only.
5. The base is `repos[0].base_branch`, else `main`; a base starting with `-` is refused with the reason shown.
6. PLAN opens when the case has steps or its status is at least `prototyped`. Without steps it shows "The session is
   writing the plan into the case file." (Factory: "The Factory plans it on the issue."). The open case file is
   re-read when its modified time changes, checked at most once a second (no watch thread for one small file).
7. REVIEW opens whenever the case's prototype is held; it never waits for the build. STORY-084's `Standing::Blocked`
   goes; `builder(status, by_pipeline)` gives "the pipeline builds it" on a Factory project from `prototyped` on, "the
   session builds it" from `approved` until `fixed`, else nothing.
8. REVIEWED is enabled on every status from `prototyping` to `fixed` and writes `reviewed`; it is refused on `ingested`
   and `reviewed`.
9. Locally, REVIEW reads the change every 2 s while on screen, on a background thread, writing nothing to the
   repository: `merge-base <base> HEAD`, `diff --no-color --no-ext-diff -U0 <mb>`, `ls-files --others
   --exclude-standard` (untracked files count as all added), `log --format=%h%x09%s <mb>..HEAD`.
10. On a Factory project REVIEW runs `fetch -q --prune origin +refs/heads/feature/*:refs/remotes/origin/feature/*`
    every 30 s, finds the pipeline's branch once among the newest 20 `origin/feature/*` refs as the first whose
    `sdlc/*/prototype.sha` contains the proto branch's head (`git grep -l -F`), caches it and reads `merge-base`,
    `diff -U0` and `log` against that ref. Until found the column says "Waiting for the pipeline's branch (<branch> at
    <sha7>)". A failed fetch shows git's message and the read continues.
11. The REVIEW frame reuses ITERATE's `IterateFrame` (wells, column, swap with V and PLAN | CITY). The plan view marks
    the changed files as STORY-086's scaffolding and keeps the session marker; the city draws the Review's lines
    (`ViewOptions::changes`: changed towers outlined, added floors in `OK`, a `DANGER` crown where lines were removed,
    other towers dimmed) and AgentSessions' sessions over it. Entering REVIEW re-reads the worktree's project once.
12. The REVIEW column: "REVIEW · SINCE <BASE>", the builder line, the size, one `DANGER` chip per raised flag, up to
    six "<id>  <subject>" commits then "+<k> more" (not clickable), one "~ <path>" button per file, the status line and
    REVIEWED. A file opens STORY-086's code panel with `diff -U3 <mb> [<ref>] -- <rel>`, or whole as `UNCHANGED`.
13. STORY-085's resume rule widens from `prototyping` to every status from `prototyping` to `reviewed` whose worktree
    exists, so FREEZE and REVIEW open after a restart.
14. Every plate, chip, button and CTA is reported to accesskit. Case-file values reach `git` and `claude` only as
    separate argv entries, never through a shell; paths follow `--`.
15. Seams assumed from STORY-086 (reconcile names at build, and update this note in the same commit):
    `Prototype::rounds() -> &[Round]` with `Round { n, notes, files /* relative */, closed }`;
    `<worktree>/.cellworks/feedback.log`, one line per note; `PlanMarks::scaffolding` and `DrawnPlate::scaffolding` in
    `crates/map/src/ui/plan.rs`; plan-view file clicks reaching the binary; the code panel taking a label and
    `CodeLine { no, mark, text }` built from `git diff -U3` text by one parser, reused here, never a second one.

## Decisions

- **Review is one concept** (operator's decision): the Sources term Review becomes a change judged against its base, a
  pull request at its head or a case's worktree. Rejected: a distinct "Case review", and renaming the pull-request one.
  This replaces the prototype's "REVIEW waits until built" and the scenario "Review waits for the build".
- **The Review computation is shared**: `pull_requests::review_of(diff_text, repo, map, root) -> Review`, which `open`
  now calls too. The minimal rendering (`ViewOptions::changes`, `DrawnTower::added`/`removed`) is built here for both.
  Rejected: waiting for STORY-022, which has no rendering yet and would leave the case's REVIEW without the line marks
  the operator asked for.
- **REVIEW opens right away and watches the build live** (operator's decision). Rejected: REVIEW blocked until `fixed`.
- **REVIEWED can be pressed any time** from `prototyping` to `fixed` (operator's decision). Rejected: only once built.
- **On a Factory project the pipeline's feature branch is fetched and found by `prototype.sha`** (operator's decision to
  fetch; the lookup is ours). Checked: `test`'s `sdlc/0043-…/prototype.sha` equals `proto/pr-source`'s head. Rejected:
  `feature/NNNN` from the issue number, which a case keyed by its ticket key does not know; having the session write the
  branch into the case file, which interleaves with the Factory's own skill; `gh pr list --head`, which needs the branch
  anyway.
- **On the Factory, lines are placed on the worktree's city**, not on the feature head's, so they can drift. Accepted
  for watching a build; the exact view is the pull request's Review.
- **Jarvis projects plan with `/jarvis-execute:plan-story`** (operator's decision), detected by the project's own
  `.claude/settings.json`; the Factory wins when both apply. Rejected: the user's `~/.claude/settings.json`, which
  enables Jarvis on every project; a substring match on "jarvis"; `.claude/work-tracking.conf`.
- **FREEZE is the PLAN act.** Rejected: a sixth `Act::Freeze` nobody asked for; writing `planning` before typing, which
  claims a plan session that may not exist.
- **The ticket-plan instructions ship with Cellworks.** Rejected: installing into `~/.claude/skills` (the user's Claude
  configuration); the schema inline in the typed line; the prototype's path into the source tree, absent beside a
  released binary.
- **The local read writes nothing.** Rejected: the prototype's snapshot commit per read, an object every 2 s.
- **Reading the change lives in Sources** (`case/review.rs`), since Review is Sources' concept; AgentSessions keeps the
  FREEZE overlay (rounds, feedback log). Git runs through `forge::command`/`run`, made `pub(crate)`, with the injected
  git program (STORY-085's `PrototypeTools::git`). Rejected: AgentSessions reading git for REVIEW; a second process
  helper.

## Order of work

1. Sources, the shared Review: `review_of` in `crates/sources/src/pull_requests/mod.rs` (`parse_diff`, `size`,
   `read_flags` + `flags_raised`, `place`), `open` calling it; `command` and `run` in
   `crates/sources/src/pull_requests/forge.rs` become `pub(crate)`.
2. Map, a Review's lines: `ChangedTower`, `ViewOptions::changes` (`None` at the existing construction sites), the
   painter branch beside `tint`, and `DrawnTower::added`/`removed` in `crates/map/src/ui.rs`.
3. Sources, case rules, writes and requests in `crates/sources/src/case.rs`: `Stage::opens(Plan)`, `Standing::Blocked`
   removed, `builder`, `Cases::freeze`/`planning`/`review` through `rewrite`, `base_of`, `PlanFlavour::of`,
   `plan_request`, `handoff`, `Cases::plan_instructions`; new `crates/sources/assets/ticket-plan.md`.
4. Sources, reading the change: new `crates/sources/src/case/review.rs` (`Commit`, `Change`, `ChangeEvent`,
   `parse_names`, `parse_log`, `added_whole`, `ChangeWatch` with `new`, `poll`, `drain`, `change`, `status_line`,
   `read_diff`, `take_diff`, `place`) and `pub mod review;` in `case.rs`.
5. Sources, the stage frames in `crates/sources/src/case/ui.rs`: `CaseView::refresh`, `FreezeFrame` in `CaseFrame` and
   `frame_of`, the FREEZE column with HANDS OFF and the CTA, the PLAN placeholder, the REVIEW layout, `ReviewAction` and
   `review_column`.
6. AgentSessions, the FREEZE overlay: `Prototype::read_feedback_log`/`feedback_log` in
   `crates/agent-sessions/src/prototype.rs`; `freeze_overlay` in `crates/agent-sessions/src/ui.rs`.
7. Binary wiring: `src/case.rs` (`freeze`, `review`, `refresh` each frame, FREEZE entry and overlay, REVIEW entry with
   `ChangeWatch`, plan view, city with changes and sessions, column, file diffs into the code panel, the widened resume
   rule) and `src/app.rs` (`reread_opened` extracted from `toggle_ignored`, `change_watch`, the accessors `case_status`
   and `review_line`).
8. Tests: `tests/story_087_freeze_a_prototype_and_review_the_change.rs` with its fake `git` and `claude`, and the unit
   tests of steps 1 to 5 (see Proof).

## Risks

- Blast radius: the case flow (Sources), the city painter (Map), the FREEZE overlay (AgentSessions) and the binary's case
  wiring; `pull_requests::open` changes shape but not behaviour, pinned by `review_of_matches_what_open_reported` and
  STORY-021's tests.
- Riskiest step, 7: typing into a live session that then writes the case file and later commits. The status guards in
  step 3 make a stale write a no-op; the planning write happens only after typing succeeded.
- STORY-086 is assumed, not merged: Requirement 15 names every seam; reconcile at build.
- The Factory lookup depends on `prototype.sha` holding the proto head as pushed. Verified on one merged story; if the
  Factory changes that, the column waits. Fallback: match the proto branch name in `sdlc/*/prototype.md`.
- The fetch writes objects and `refs/remotes/origin/feature/*` (with `--prune`) into the user's repository every 30 s
  while a Factory case's REVIEW is on screen; no local branch moves (ADR-0009).
- Parallel writers on the case file: the session writes `planned`/`fixed` while Cellworks may write `planning` or
  `reviewed`; the window is milliseconds.
- Rollback: revert the squashed commit. Case files keep the statuses written and the tolerant reader still reads them;
  fetched remote-tracking refs stay until the next prune.

## Proof

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`: all pass,
  with no warnings.
- Acceptance (`tests/story_087_freeze_a_prototype_and_review_the_change.rs`, egui_kittest, a `tempfile` cases folder,
  `common::Tree::repository`, a fake `git` and `claude` through `with_prototype_tools` and `with_pane_sources`, each
  case resumed from a case file in the named status; no test reads `$HOME`, runs real `git` or `claude`, or opens a
  window):
  - `the_freeze_stage_shows_what_the_rounds_confirmed` — R1 and R2 with notes and files, every `feedback.log` line
    numbered; a case with none shows "No round closed yet.".
  - `freezing_plans_from_the_prototype` — hand-off PLAN, APPROVE, REVIEW; after FREEZE the history holds `prototyped`,
    the status is `planning`, the pane text holds `git diff main`, `.cellworks/feedback.log`, the case file, `planned`
    and `commit nothing`, `<cases>/ticket-plan.md` exists, the stage is PLAN with the placeholder; a step the test
    writes then shows.
  - `freezing_on_a_factory_project_hands_the_prototype_to_the_factory` — hand-off PROTOTYPE.MD, PLAN, BUILD; the pane
    text holds `/factory:freeze`; the history holds `prototyped`.
  - `freezing_on_a_jarvis_project_plans_with_jarvis` — the pane text holds `/jarvis-execute:plan-story DEMO-1` and
    `feedback.log`; a project with both files gets `/factory:freeze`.
  - `review_shows_the_change_since_the_base` — on a `fixed` case the changed and untracked files carry scaffolding and an
    unchanged one does not; the column shows the commit, both files and the size; the git log holds `merge-base main
    HEAD` and no `commit-tree`; a file opens `SINCE MAIN` with `+`/`-` lines; the city's changed tower has `added > 0`.
  - `marking_the_change_reviewed` — REVIEWED on a `fixed` case writes `reviewed` and shows `✓ REVIEW`; an `approved`
    case also accepts it.
  - `review_shows_the_change_while_it_is_built` — on an `approved` case with session `demo-1` writing in the worktree,
    REVIEW shows "the session builds it" and draws `demo-1`; a file added to the fake diff appears in the column.
  - `review_follows_the_pipelines_feature_branch` (scenario 7 on a Factory project) — "the pipeline builds it", the
    fetch and `diff -U0 mb0000 origin/feature/0138-x` in the git log and that diff's files listed; before the match,
    "Waiting for the pipeline's branch".
  - Existing: STORY-084, 085, 086 and STORY-021's tests pass.
- Unit: `plan_opens_with_steps_or_once_prototyped`, `freeze_planning_and_review_write_only_from_their_statuses`,
  `the_flavour_follows_the_project`, `each_flavour_asks_for_its_plan`, `the_handoff_names_what_follows`,
  `the_builder_is_named_by_status_and_project` (`crates/sources/src/case.rs`);
  `change_parsers_read_names_commits_and_untracked_files` (`crates/sources/src/case/review.rs`);
  `review_of_matches_what_open_reported` (`crates/sources/src/pull_requests/mod.rs`);
  `changed_towers_carry_their_added_floors` (`crates/map/src/ui.rs`).
- Before merge, by hand: freeze real prototypes on a plain, a Jarvis and a Factory project with the real `claude` CLI and
  see each plan arrive and read in Cellworks; on one Factory run see REVIEW find the feature branch; compare FREEZE and
  REVIEW with the "Iterate loop" canvas.

## Docs impact

- **Business decisions** — the build writes them in `docs/domain/contexts/Sources/Sources.md`: a Review is also opened
  on a case's worktree; freezing types the planning request (Factory, Jarvis or ticket-plan); Cellworks writes
  `prototyped`, `planning` and `reviewed`, each only from the statuses it expects; REVIEW follows the change as it lands,
  on a Factory project from the pipeline's feature branch.
- **Glossary** — Sources: Review redefined (a pull request or a case's worktree against its base), the Stage row's note
  settled, Freeze, Hand-off and Base branch added. Done at freeze (commit "docs: ingest STORY-087 prototype decisions").
- **Guidelines** — None.
- **arc42** — `05-building-blocks.md` (sources: the Review of a pull request or a case's change; map: a Review's line
  marks); `03-context.md` (git: read-only reads of a case's change and the `feature/*` fetch; Claude Code: the planning
  request typed into a session's pane); `08-crosscutting.md` 8.3 (the cases folder holds `ticket-plan.md`). The build
  writes them.
- **ADR needed** — No: the fetch follows ADR-0009 (writes objects and remote-tracking refs, moves no ref the user owns).
