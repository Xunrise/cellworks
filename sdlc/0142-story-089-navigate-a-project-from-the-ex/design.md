---
stage: design
issue: 142
status: frozen
author: claude (freeze)
date: 2026-10-27
---

# Design: STORY-089, navigate a project from the Explorer

The contract is `issue.md` in this directory. Confirmed behaviour, feedback, rejections, edge cases and the
operator's answers are in [prototype.md](prototype.md); the look is the "Tree navigation" Claude Design canvas
(https://claude.ai/artifact/C9KPGojj9mG4Adc1iBygTR). Evidence only: `src/explorer.rs` and the diff on
`proto/file-tree-navigation`. Build from `test`.

No-flag: a desktop app released per merge with no remote configuration; the Explorer replaces the Tree and the lens
bar, so there is no old behaviour to keep beside it.

## Open questions

> ASSUMED (Q-1): Should the Explorer's camera flight write into `Navigation`'s own `camera` field (as a first draft of
> `crates/navigation/src/explorer.rs` did, adding a `pub(crate) fn fly_to`)? — default: no; `beside`/`flown` compute a
> display-only `Framing` that `src/app.rs`'s `project()` builds locally and feeds straight to
> `cellworks_map::ui::project`, and `Navigation::framing` itself always stays the unshifted, instant truth. Because
> the acceptance tests read `navigation.framing(&map).area` right after a key press, expecting the row's own,
> unshifted rectangle; if the flight set `self.camera`, `framing()`'s early-return branch would report the
> shifted, mid-flight rectangle instead, and Esc/F would also need to know to clear a camera they didn't set. Found
> and corrected while verifying `tests/story_089_navigate_a_project_from_the_explorer.rs`; `fly_to` and the
> `Explorer::drive` method that called it were removed in the same pass.
>
> ASSUMED (Q-2): How tall should the Explorer's rows list be when other panels (a session pane, say) share the
> screen with it? — default: the rows' `ScrollArea` shrinks to its own content (`auto_shrink([false, true])`,
> capped by a reserved `FOOTER_HEIGHT`), and a final non-interactive `ui.allocate_space` pads the Frame back out
> to the full forced panel height for its opaque background only. Because an `auto_shrink([false, false])`
> `ScrollArea` filling the whole forced height was found, while verifying `tests/story_081_steer_a_session_from_its_session_pane.rs`
> against this change, to swallow pointer clicks aimed at the session pane's "Close (detach)" button through the
> blank space below the last row, even though that point sits outside the Explorer's painted rows — the padding
> keeps the panel's opaque look without claiming interaction there.

## Requirements

1. The Explorer is open by default whenever a project is open; Cmd+B hides and shows it, and E stays an alias.
   Cmd+B with no project open does nothing. Opening another project resets folds and filter, not open/hidden.
2. Look, per the canvas: opaque panel 320 px wide with a 10 px margin over the left of the map; header "EXPLORER"
   and "<N> files"; equal-width lens chips (active: flow border and text on a faint flow fill, inactive: dim text on
   a line border; hover shows the lens's explanation); the filter field with a "/" prefix and the placeholder
   "filter files and symbols"; a two-line key footer in terrain ink. Chevrons are drawn shapes, and key hints are
   text ("Cmd+B"), never glyphs the app font lacks.
3. "<N> files" counts the nodes of the folders-lens map, so it is the same in every lens.
4. Rows: folders and groups have a chevron, a district-colour swatch (the map's palette) and a file count; files
   have an extension badge and their highest floor number; symbol leaves (fn / ty / use / §) under an unfolded file,
   without test items.
5. Change marks: a file with uncommitted changes (modified or untracked) is marked modified; every folder holding
   one is marked as holding changes. Read in the background, refreshed 5 s after the last result.
6. Keys with the Explorer open, not typing, not walking and not inside a tower: j / ArrowDown and k / ArrowUp move
   the cursor; l / ArrowRight unfold; h / ArrowLeft fold, or move to the parent row when already folded; Shift+L
   next lens (plain L only while hidden); "/" focuses the filter. With the Explorer hidden, arrows pan as before.
7. "/" focuses the filter only when no text field has focus, and is not typed into it. While the filter has focus,
   Enter and Esc do nothing on the map.
8. A filter (case-insensitive substring) unfolds everything and lists the matching rows plus the folders leading
   to them.
9. Moving the cursor or clicking a row frames that row's folder (its folder view) or node (inspection distance) and
   selects it; the camera flies there in 0.6 s, centred in the part of the map right of the panel. Clicking a
   folder row also toggles it. Enter, Esc and F stay instant.
10. Framing is not shifted for the whole-project overview, a user-zoomed camera, or a map narrower than twice the
    panel width.
11. A selection made on the map unfolds the Explorer to its row and scrolls it into view, once per change.

## Decisions

- **The Explorer lives in the `navigation` crate** (`explorer.rs` model, drawing in `ui.rs`); the binary reads keys
  and applies results. Rejected: the prototype's binary module, which puts context behaviour outside its crate.
- **`Navigation::frame(map, path)` = select, then Enter**, so Esc keeps one step per layer. Rejected: `jump`, which
  frames the parent folder.
- **Rows come from one structural O(n) walk per frame** over the shown lens map, producing counts and marks too; a
  file appears once because the lens map holds it once. Rejected: the prototype's `find` per row (O(n²)) behind a
  global 5 s cache.
- **Git change marks follow ADR-0003**: `GitListing::changed` behind the hardened `command()`, run by
  `start_changes` on a `std::thread`, reported as `CodeAnalysisEvent::ChangesRead`, one read in flight. Navigation
  receives a plain `HashSet<PathBuf>`. Rejected: the prototype's synchronous git call on the UI thread.
- **Only Explorer-initiated framing flies.** Rejected: animating every framing, which changes STORY-004/007/012.
- **Shift+L cycles lenses; plain L only while hidden; arrows drive the Explorer while open** (operator's answer).
  Rejected: plain L always cycling (clashes with l), arrows always panning.
- **Filter keeps ancestor folders** (operator's answer). Rejected: a flat list of matches.
- **One story including the change marks** (operator's answer). Rejected: splitting the marks out.
- **Lens chips replace the lens bar**: `lens_bar` becomes `lens_help(lens)`; chips' accessible labels are the
  lowercase lens names so STORY-055's tests still find "calls". Rejected: keeping both.
- **The Tree (E) is removed** with its tests (`tests/story_008`) and rule; the Explorer supersedes it.
- **Accessibility**: one "Explorer" container node that `tests/common::labels()` skips; rows are toggled buttons
  labelled with their text, count and mark in the description (fallback: the row's value).
- The prototype's F12 screenshot hook and its debug print are not carried over.

## Order of work

1. [x] Map and CodeAnalysis groundwork: `district_colour_at` in `crates/map/src/ui.rs`; `lens_bar` → `lens_help` in
   `crates/map/src/ui/explain.rs`; `GitListing::changed` and its porcelain parser in
   `crates/code-analysis/src/git.rs`; `CodeAnalysisEvent::ChangesRead` in `crates/code-analysis/src/event.rs`;
   `start_changes` in `crates/code-analysis/src/lib.rs`. The binary keeps building (its `lens_bar` call switches
   to `lens_help` until step 3 removes it).
2. [x] Navigation model: `Navigation::frame` in `crates/navigation/src/lib.rs` (Tree functions deleted);
   `crates/navigation/src/explorer.rs` with `Explorer`, `Row`, `Badge`, `Change`, `rows`, `step`, `fold`,
   `follow`, `reveal`, `reset`, `beside`, `flown`, and its unit tests.
3. [x] Drawing and wiring: `ui::explorer` replacing `tree()` in `crates/navigation/src/ui.rs` (panel, chips, filter
   with "/" focus, rows, footer, accessibility); `files()` in `src/opened.rs`; `src/app.rs` state, keys, framing
   through `beside`/`flown`, `Picked` handling, background change reads; lens bar and left tree panel removed.
4. [x] Tests: `tests/story_089_navigate_a_project_from_the_explorer.rs`; remove `tests/story_008_*`; `labels()` skips
   the Explorer subtree; adapt input in existing tests (Shift+L, Cmd+B before arrow panning, `explorer_open()`,
   Explorer-excluding label lookups, hiding the Explorer where a framing-centre check does not concern it).
5. [x] Docs: Navigation context, Navigation glossary, arc42 building blocks (see Docs impact; the context map and
   domain README are not touched — the Docs impact section below names no change to either).

## Risks

- Test churn: about a dozen existing acceptance tests need input changes; the riskiest step is 3, because the
  shifted framing moves the view centre that facade turning (STORY-045) and walking (STORY-046) read. Zoomed
  cameras are exempt by design.
- Shift also shows the underground view while held; a Shift+L press shows it for one frame.
- `git status -z` rename entries: the parser tolerates either path order and skips entries outside the project.
- Frame budget: the Explorer is drawn every frame; the release measurement test checks it.
- Rollback: revert the squash-merge commit on `test`; no data or configuration migrates.

## Proof

```
cargo fmt --all --check                                  # no output
cargo clippy --workspace --all-targets -- -D warnings    # no warnings
cargo test --workspace                                   # all pass, story_089 included
cargo test --workspace --release -- --ignored --nocapture   # frame_measurement within budget
```

Acceptance tests in `tests/story_089_navigate_a_project_from_the_explorer.rs` (headless egui_kittest, recorded git):

- `the_explorer_is_open_beside_the_map_by_default` — The Explorer is open beside the map by default
- `the_explorer_can_be_hidden_and_shown_again` — The Explorer can be hidden and shown again
- `moving_the_cursor_to_a_folder_row_frames_its_district`, `…_subfolder_row_frames_its_sub_district`,
  `…_file_row_frames_its_building` — Moving the cursor selects the row and frames it on the map
- `selecting_on_the_map_selects_the_matching_row` — Selecting on the map selects the matching row
- `the_explorer_regroups_by_the_active_lens` — The Explorer regroups by the active lens
- `filtering_narrows_the_rows_by_name` — Filtering narrows the rows by name
- `rows_show_where_each_part_stands` — Rows show where each part stands
- `leaving_to_the_whole_map` — Leaving to the whole map

Unit tests: `rows_list_folders_first_and_count_files`, `a_filter_keeps_matches_and_the_folders_leading_to_them`,
`a_changed_file_marks_itself_and_every_folder_holding_it`, `stepping_stops_at_the_ends_and_frames_the_row`,
`following_a_selection_unfolds_its_ancestors_once`,
`beside_shifts_folder_and_node_framing_but_not_the_whole_project_nor_a_narrow_view`,
`flown_reaches_the_target_at_one` (`crates/navigation/src/explorer.rs`);
`framing_a_folder_shows_its_folder_view_and_a_node_its_building` (`crates/navigation/src/lib.rs`);
`the_colour_at_a_subfolder_is_its_districts` (`crates/map/src/ui.rs`);
`porcelain_entries_become_paths_under_the_project` (`crates/code-analysis/src/git.rs`); integration
`changed_lists_modified_and_untracked_files` (`crates/code-analysis/tests/git_cli_against_real_git.rs`, skips
without git).

## Docs impact

- **Business decisions** — `docs/domain/contexts/Navigation/Navigation.md`: the Tree rule becomes "The Explorer
  lists the whole open project as the current lens groups it, a file once; its cursor row is the selection, and
  moving it frames the row's folder or node beside the panel; a selection made on the map unfolds the Explorer to
  its row. (STORY-089)" Ingested at freeze.
- **Glossary** — Navigation glossary: add "Explorer" and "Lens chips", retire "Tree" and "Lens bar". Ingested at
  freeze.
- **Guidelines** — None.
- **arc42** — `docs/arc42/05-building-blocks.md`, the `navigation` row: "tree" → "Explorer". Ingested at freeze.
- **ADR needed** — no.
