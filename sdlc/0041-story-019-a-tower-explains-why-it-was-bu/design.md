---
stage: design
issue: 41
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-019 A tower explains why it was built

The contract is `issue.md` in this directory: its scenarios are what the change must do. The confirmed prototype is
`prototype.md` beside this note; its Decisions and Rejected sections bind this design.

**Prerequisite:** STORY-018 (#40, branch `proto/line-history`) must be merged into `test` first. This design uses its
`ProjectMap.history: LineHistory`, `Commit { sha, time, summary, pull_request, stories }` and
`LineHistory::of_line(file, Floor.number)` as its design states them, and ADR-0008.

## Open questions

None.

## Requirements

1. Selecting a mapped tower opens a why panel in a fixed right column (`egui::Panel::right("why")`, 420 px), between
   the tree panel and the central panel; nothing or a district selected shows no panel.
2. Context section: the bounded context the tower's file belongs to and the context's description (the first
   non-empty line after the `# Context:` heading of `docs/domain/contexts/<Name>/<Name>.md`). A tower in no context
   says "No bounded context". The file-to-context rule is the cells lens's rule, moved to
   `BoundedContexts::context_of_file` and used by both.
3. Built-by section: one entry per pull request that last wrote floors of the tower, with its share of the tower's
   floors, largest first, ties by pull request number ascending. Under each pull request: every story its commits
   name, with the story's title and "so that" where known, else the id alone. Floors from commits without a pull
   request form one group "Commits without a pull request" with their subjects; uncommitted floors one group "Not
   committed yet". Stories carry no share of their own.
4. A tower whose floors all come from commits naming no story says "Built by commits without a story" and lists those
   commits' subjects.
5. Decisions section: every Accepted ADR whose scope in `docs/arc42/09-decisions.md` names the tower's bounded context,
   plus every Accepted ADR scoped `cross-cutting` or without a scope; each with its id, title and decision line (the
   first sentence of its `## Decision`). The scope is the optional `· <scope>` suffix of the ADR's index line (Jarvis
   `adr-writing` format), a comma list of bounded context names. A tower in no context shows only the cross-cutting
   ones.
6. The selected tower's floors are coloured by their pull request (a fixed palette in pull-request order of requirement
   3); every other tower is dimmed with the existing `light` factor. Nothing changes when no tower is selected.
7. Inside a tower, each floor row shows, after the code and before the calls column, the label `#<pr> STORY-NNN` (plus
   ` +k` for further stories) of the commit that last wrote it; nothing for an uncommitted floor or a commit without a
   pull request. Selecting a floor shows its commit subject and each of its stories' "so that" in the panel.
8. Story title and "so that" come only from local `sdlc/*/issue.md` files, keyed by the normalised id (upper case, at
   least three digits, as ADR-0008). No network and no `gh` in this story. When EPIC-012's story source exists (see
   Decisions), the panel takes its text from there.
9. All reading happens in DomainKnowledge's existing `read()` (off the UI thread, `ContextsRead` event); `why_of` runs
   on the UI thread only when the selected path changes, cached in the app. A missing ADR index or story folder gives
   empty lists and never makes `read()` fail.
10. Labels are plain egui labels readable by the accessibility tree, like `ui/explain.rs`; colours come from the
    existing palette consts. No skin switch is built (skins are STORY-036).
11. Nothing changes in `crates/code-analysis` (STORY-058 is in flight there) or in `crates/navigation`.

## Decisions

- Credit per pull request with its stories nested; rejected a 1/n split per story and full credit per story (operator).
- Right column; rejected a floating card and a bottom drawer (operator).
- ADR-to-tower link through the bounded context: the ADR's scope on its `docs/arc42/09-decisions.md` index line, the
  Jarvis format; rejected a `## Binds` section of crate names in every ADR, a separate mapping file, and guessing
  from crate names in the ADR text (operator: an ADR's reach follows the context cell it belongs to).
- Story text local only now; rejected a `gh` lookup in this story, which would duplicate EPIC-012's story source
  (operator). Follow-up: EPIC-012 provides story title and As a / I want / So that by story id through a public
  Sources API; when it lands, `why_panel` reads from it.
- Join and shares in a pure `cellworks_map::why::why_of(map, item) -> Option<Why>`; rejected computing in the binary
  or the panel, so the shares are unit-testable.
- Description, decisions and stories extend `BoundedContexts` and DomainKnowledge's existing `read()`; rejected a new
  event, thread or crate dependency.
- Reuse the cells-lens file-to-context rule; rejected a second copy of it.

### Settled while building steps 3 to 5

- A story each pull request names is a `why::Story { id, text: Option<StoryText> }`, so the panel writes the id alone
  where no local `sdlc/*/issue.md` knows it; `Why::without_story` and `Why::stories_of` answer requirements 4 and 7
  from the same gathered lists, so nothing is joined twice.
- The acceptance tests are split. The scenarios that turn on per-line history hand-build the `ProjectMap` the loaders
  would deliver, as the Proof says, and drive the Map context's own `why_panel`, `inside` and `project`; the scenarios
  that turn only on the Jarvis docs — the bounded context and the decisions — run through the whole binary over a real
  folder tree, which also pins the `src/app.rs` wiring. Rejected recording a git log in the binary's test harness: it
  would mean exporting `LoggedCommit` from `crates/code-analysis`, which requirement 11 leaves alone.
- `src/opened.rs` changes too, which the Order of work did not foresee. `ContextsRead` filled in the contexts of the
  map it keeps for the lenses but not of the map on screen, so a tower said "No bounded context" whenever the code read
  finished first — a race, and requirement 2 fails on the wrong side of it. The drain now fills both and reports it
  with `take_learned`, which is also how the app knows to work `why_of` out again for a cached selection.
- `CellworksApp::panel()` reports the space the map was drawn in, beside `view()`. The why panel takes a fixed column
  out of the window (requirement 1), so the middle of the map is no longer the middle of the window;
  `tests/story_045_the_camera_turns_to_face_a_facade.rs` asserted against a hard-coded 600 px and now asks the app
  where it drew. The behaviour STORY-045 pins — a faced facade settles in the middle of the view — is unchanged.

## Order of work

1. Map types and the context rule: `crates/map/src/contexts.rs` gains `BoundedContext.description`,
   `Decision { id, title, line, scope: Scope }` with `Scope::{CrossCutting, Contexts(Vec<String>)}`,
   `StoryText { title, so_that }`, `BoundedContexts.decisions` and `.stories` (empty by default), and
   `BoundedContexts::context_of_file`; `crates/map/src/lens/cells.rs` uses it; fix every `BoundedContext { .. }`
   literal. Existing cells tests stay green.
2. DomainKnowledge reading: `crates/domain-knowledge/src/lib.rs` `read()` fills the description, `read_decisions`
   (index lines of `docs/arc42/09-decisions.md` with their scope, Accepted only, decision line from each ADR file) and
   `read_stories` (`sdlc/*/issue.md`, parsed as plain text); doc comments updated. Unit tests with the existing
   tempdir helper.
3. The why model: `crates/map/src/why.rs` (new) with `Why`, `PrShare`, `PrKey::{Number, NoPullRequest, NotCommitted}`,
   `why_of`, `floor_commit`, pull-request order for the tint; re-export in `crates/map/src/lib.rs`. Unit tests.
4. The panel and the tower: `crates/map/src/ui/why.rs` (new) `why_panel`; `crates/map/src/ui.rs` `ViewOptions.tint`
   and dimming, and the floor label in `inside()`. If threading the tint through the painter grows large, land the
   panel first and the tint in a second commit.
5. Wiring and acceptance: `src/app.rs` shows the panel for a selected mapped item, caches `why_of` per selected path,
   passes the tint and the inside floor; `tests/story_019_a_tower_explains_why_it_was_built.rs`.

## Risks

- STORY-018 not merged or its field names shifted: the build stops at step 3; the acceptance tests look floors up
  from the loaded item's own path, so a path-key mismatch fails loudly.
- The painter in `crates/map/src/ui.rs` is large; the tint is the riskiest step and is separable from the panel.
- Rollback: revert the squash commit; the new fields default to empty and the panel only appears on selection.

## Proof

- `cargo test --workspace` passes, including the acceptance tests in
  `tests/story_019_a_tower_explains_why_it_was_built.rs`, one per scenario and named after it:
  `the_selected_tower_names_its_bounded_context`,
  `the_pull_requests_that_built_the_tower_are_listed_with_their_share_of_floors`,
  `a_pull_request_that_implemented_several_stories_lists_all_of_them`, `decisions_that_bind_the_tower_are_listed`,
  `inside_the_tower_each_floor_names_the_pull_request_and_story_that_last_wrote_it`,
  `a_tower_with_no_story_behind_it_says_so`; plus `no_panel_without_a_selected_tower` and
  `other_towers_are_dimmed_while_one_is_selected`. Hand-built `ProjectMap` with `history` and `contexts`; no real
  git, no network.
- Unit: `why.rs` (`shares_are_floors_per_pull_request_largest_first` 7/9 and 2/9,
  `a_pull_request_lists_every_story_its_commits_name`, `floors_of_commits_without_a_pull_request_are_one_group`,
  `uncommitted_floors_are_counted_as_not_committed_yet`, `a_tower_in_no_context_has_none`,
  `decisions_follow_the_context_or_are_cross_cutting`); DomainKnowledge
  (`a_context_has_the_first_line_after_its_heading_as_description`,
  `adr_scopes_are_read_from_the_decisions_index`, `an_adr_without_scope_is_cross_cutting`,
  `only_accepted_adrs_are_read`, `stories_are_read_from_issue_files_by_normalised_id`,
  `a_project_without_adrs_or_stories_reads_empty_lists`).
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`,
  `SHELLCHECK_OPTS=--exclude=SC2016 actionlint` clean.

## Docs impact

- **Business decisions** — None.
- **Glossary** — DomainKnowledge: "Why panel"; "ADR scope" (the bounded contexts an ADR binds, from its index line).
  Map: "Pull request share" (the floors of a tower last written by one pull request, as a share of its floors).
  Written at freeze.
- **Guidelines** — None.
- **arc42** — `09-decisions.md`: a scope on the index lines of ADR-0001 to ADR-0007, written at freeze; the build adds
  `· CodeAnalysis, Map` to the ADR-0008 line that STORY-018 brings.
- **ADR needed** — no.
