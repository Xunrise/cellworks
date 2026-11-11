---
stage: design
issue: 132
status: frozen
author: claude (freeze)
date: 2026-10-22
---

# Design: STORY-082 — the launcher is a keyboard-first palette

The contract is `issue.md` in this directory: its acceptance scenarios are what
the change must do. `prototype.md` beside it records the confirmed palette
layout (Claude Design canvas, page v1) and every answered question; its Rejected
items are not built. The STORY-078 data model (locations, remote hosts, recent,
groups, search) is unchanged; this story changes the layout, the key routing and
adds pure view-model helpers.

## Open questions

> ASSUMED (Q-1): Requirement 3 gives a chip's text as "<glyph> <host> <state
> word> <n>", but the issue's own scenarios quote the chips verbatim as "All
> 4", "This Mac 2", "wsl connected 1" and "devbox offline 1" — no glyph
> character in the text a test can read back — default: a chip's own label is
> `"{host} {word} {n}"` (the state's word alone, e.g. "wsl connected 1"), and
> the glyph is drawn as a small painted shape beside it rather than embedded
> in the label text, because the issue's scenarios are the contract this story
> must satisfy verbatim and a kittest can only assert what the accessibility
> tree actually contains.

> ASSUMED (Q-3): Implementing requirement 7 (the preview pane also shows the
> selected project's own name) means a project's name can appear twice in the
> accessibility tree once it is selected at the universe layer — its row, and
> the pane. This isn't only this story's concern: `tests/story_004_move_one_
> layer_in_and_out.rs`'s `rect_of` helper used a single `get_by_label`, which
> now panics on "shop" once it is selected (opened, then closed again, which
> that story's own tests do) — default: fixed `rect_of` to take the *last*
> matching label (the row; the pane is drawn first, see `ui.rs`'s `launcher`
> — the right panel is shown before the central panel), since every other
> name that helper is asked for (a folder, a node) is never also a selected
> project's name at the universe layer, because `src/app.rs` only draws the
> launcher (and so the pane) when no project is open. Not touching STORY-004's
> own test would leave a story this change didn't list as a finding instead,
> but the fix is a one-line query disambiguation, not a behaviour change, and
> leaving the whole suite red over it serves nobody.

> ASSUMED (Q-2): Neither the issue nor the design states what order the host
> chips (and `cycle_pick`) visit when there is more than one remote host.
> Alphabetical order (as `Launcher::hosts` already sorts, for STORY-078's
> locations column) would visit "devbox" before "wsl" in the acceptance
> fixture, but the issue's scenario requires "wsl" (connected) to be reached
> on the second Tab, before "devbox" (offline) — and the design's own
> unit-test prose for `location_chips` also lists "wsl" before "devbox".
> — default: chips and `cycle_pick` order remote hosts by state (connected,
> then checking, then offline) and by name within a state, because that is
> the only ordering consistent with both the acceptance scenario and the
> design's own example, and it matches the product sense of surfacing reachable
> hosts first. `Launcher::hosts` (STORY-078's locations column, now unused by
> any view) and `groups`'s host ordering are unchanged.

## Requirements

1. When no project is open, the launcher is one centred card (max width 920 px)
   instead of three columns. Above it a header: "Cellworks", "<n> projects · <m>
   hosts" and a disabled "Add project ⌘N" button (hover "Coming soon").
2. Search row at the top of the card. The search field has the focus on the
   first frame the launcher is drawn after it opens; ⌘K keeps focusing it.
   While the query is non-empty the row shows "<k> matches" (distinct projects
   in the filtered list) and an "Esc clear" hint.
3. A row of location chips under the search row replaces the left column:
   "All <n>", "This Mac <n>", and one chip per remote host
   "<glyph> <host> <state word> <n>" using the existing `HostState` display
   (● connected, ◌ checking, ○ offline). Counts are totals per location,
   independent of the query and the picked location. The picked chip is
   highlighted; clicking a chip picks it.
4. Tab moves the picked location to the next chip, wrapping from the last host
   to "All". Tab is consumed, so egui focus does not move. If the selection is
   no longer visible after the pick changes, it is dropped.
5. The list shows a "Recent" group first, then one group per location (as in
   STORY-078; recent projects also stay in their location group, and arrow
   order keeps de-duplication). A row shows the name, the path and the branch,
   or "folder" when the project has no branch.
6. While searching, the matched fragment of the project name is underlined.
   Search still matches name or path (STORY-078); a path-only hit has no
   underline. Matching is case-insensitive; when lowercasing changes a name's
   byte length, no underline is drawn.
7. A preview pane of 300 px to the right of the list shows the selected
   project: name, "<kind> · <location>", path, branch, number of files, an Open
   button and a disabled "Sync · coming soon" button.
8. Open is enabled only for local projects. Its label is "Open · Enter". For a
   remote project on a host that is not connected the button is disabled and
   reads "Open · connect the host first"; for a remote project on a connected
   host it is disabled with the hover "Remote projects open in a later story".
   Enter follows the same rule: it opens nothing for a remote project.
9. Footer of the card: "↑ ↓ move", "Enter open", "Tab scope", "Esc clear search".
10. Esc in the launcher clears the search text and nothing else; it does not
    reach the map handling.
11. Notices (for example a skipped host) are drawn below the card. The
    scanning, failed, empty and no-match states of STORY-078 keep their texts
    and are drawn inside the card.
12. Colours come from the visuals `ui.rs` already uses; no new token system.

## Decisions

1. Open is disabled for every remote project, because remote opening is not
   built (STORY-078 requirement: remote projects are never opened as local
   folders). Rejected: enabling Open on a connected host, which would open a
   `host:path` id as a local folder.
2. The underline marks the name only; search keeps matching name or path.
   Rejected: restricting search to the name, which changes STORY-078 behaviour.
3. Chip counts are unfiltered totals; only "<k> matches" follows the query.
   Rejected: counts that change while typing (the mockup shows static counts).
4. Recent stays a list group and its projects stay in their location groups.
   Rejected: removing them from the groups, which changes the group counts.
5. Tab and Esc are routed in the key block of `src/app.rs`, next to Enter and
   the arrows. Rejected: a second key site in `ui.rs`.
6. The "+ Connect host…" button is dropped; the chip row has no place for it,
   and hosts are still added in the settings file. Rejected: a trailing chip,
   which suggests a flow that does not exist.
7. View-model logic (match range, match count, chips, cycling, open rule,
   branch label, clear, focus request) lives in `launcher.rs` as pure functions;
   `ui.rs` stays the only egui module and only formats. The underline uses
   `egui::text::LayoutJob` with `TextFormat::underline`.
8. The ⌘R hint and "Check again" button from the mockup are not drawn; the host
   recheck is a later EPIC-025 story.
9. `Launcher::recent_row` is now narrowed by the current pick and search, like
   `groups`: requirement 4 ("only 'factory-evals' is listed" once "wsl" is
   picked) would otherwise leave a recent project from elsewhere still shown
   under Recent. Rejected: a recent row unaffected by the pick, which is what
   STORY-078 built (never exercised against more than one location at a time)
   but STORY-082's Tab scoping now contradicts.
10. The centred card's width cap (`CARD_WIDTH`, 920) is clamped to
    `ui.available_width()` before `set_max_width`: on a window narrower than
    920 + the 300 px preview pane, `vertical_centered` would otherwise centre
    on a negative offset, drawing the card (and the list inside it) partly off
    the left edge — caught by `tests/story_004_move_one_layer_in_and_out.rs`,
    whose 1200 px harness predates this story and sits right in that gap.
    Rejected: widening that test's harness instead, which papers over a real
    "narrower than the mockup" case the shipped window can still hit.
11. `Launcher::groups` groups local projects by *location* rather than by
    parent folder: one "This Mac" group holding every local project, then one
    group per host. Requirement 5's "one group per location" and the issue's
    group names ("This Mac", "wsl", "devbox") cannot both hold while
    STORY-078 keeps a group per parent folder — two local folders would draw
    two groups both headed "This Mac". The folder a project lives in is still
    named, on its own row, by its path; inside the group the projects keep
    STORY-078's order (parent folder, then name). STORY-078's unit test
    `groups_are_parent_folders_then_hosts` becomes
    `groups_are_this_mac_then_hosts`, `home_is_shown_as_a_tilde` asserts the
    `~` on a row's path (where it is still shown), and its acceptance test
    `projects_are_grouped_by_location_or_remote_host` pins that the fixture's
    two local folders draw one heading. Rejected: rewriting the heading in
    `ui.rs` while `groups` still splits by folder, which is what produced the
    duplicate headings.
12. `Launcher::branch_label` returns `Option<String>`: the branch once it is
    known, `Some("folder")` for a project whose details came back without
    one, and `None` while nothing is known — a local project not yet
    selected, or a remote one, which never has details. The row and the pane
    then show no branch rather than calling every repository a folder before
    it is selected. Requirement 5 and the issue's scenario 1 both make
    "folder" mean *has no branch*. Rejected: reading the branch of every
    project found, which STORY-078's
    `git_reads_only_the_selected_project_and_only_once` forbids.
13. The preview pane's location line uses the new `Launcher::location_of`
    ("This Mac", or the host's name), not the host's connection state.
    Requirement 7 asks for "`<kind> · <location>`"; a state is not a
    location, and the host's chip already carries it. `Launcher::state_of`,
    which STORY-078 drew on every row of the three-column layout this story
    replaces, is gone with its only call site. Rejected: keeping the state on
    that line, which never names the host the project is on.
14. ASSUMED (Q-1) is implemented as written: `HostState::word` is the word
    alone ("connected"), and `ui::state_glyph` paints the glyph as a shape
    beside it — a filled dot, a ringed dot, an empty ring. `impl Display for
    HostState`, which wrote "● connected", is removed with its last caller,
    so no label can embed a glyph again. The STORY-078 acceptance test's
    "● connected"/"○ offline" assertions become the words, and STORY-082's
    chip test additionally pins that no label contains a glyph.
15. Requirement 10's "does not reach the map handling" is implemented for the
    *letters* as well as for Esc: `src/app.rs`'s `typing` flag, which already
    suppressed the map's letter shortcuts while the sessions sidebar's search
    field had focus, now also counts the launcher's search field. Requirement
    2 focuses that field on the first frame, so typing a project name is the
    launcher's default path, and `u`, `m` and `z` (underground, cables, lock)
    are the three shortcuts that act whether a project is open or not — a
    search for "surface-kit" would otherwise flip underground behind the
    launcher. The field's id becomes the module constant `ui::SEARCH_ID` with
    a `ui::search_focused` reader, the shape `SessionsView::search_focused`
    already uses, because the key block runs before `ui::launcher` draws the
    field and so cannot let the `TextEdit` swallow the key first. Rejected:
    gating the three actions on `app.opened.is_some()`, which fixes those
    three and leaves the next unguarded shortcut to be found the same way.
16. The search field keeps its focus through every key the launcher routes
    itself, which Decision 5 and requirement 4 assumed but did not secure.
    Consuming Tab in the key block is not enough: egui reads the focus
    direction from the raw input in `Focus::begin_pass`, hands the focus to
    the next interested widget (the first chip's name label) and sets its own
    `give_to_next` flag, which a later `request_focus` cannot outvote — so
    the field is built with `lock_focus(true)`, which takes Tab out of that
    pass. A single-line field inserts no tab character, and the key block has
    consumed the key before the field reads its events, so the flag only
    stops the focus from moving. Esc and Enter *can* be answered with
    `Launcher::request_focus`, which the key block now calls for both: egui
    clears the focus on an unmodified Escape, and a single-line `TextEdit`
    surrenders it on Enter — including on the frames where the launcher opens
    nothing (requirement 8's remote project). A click on a project row takes
    the focus the same way, so `LauncherAction::Selected` asks for it back
    too. Without this, requirement 10's "and nothing else" fails and the next
    letter typed reaches the map's "u", "m" and "z" instead of the field.
    Rejected: re-requesting the focus for Tab as well, which loses to
    `give_to_next` in the same frame; and leaving the row click alone, which
    keeps the same defect behind a mouse.

## Order of work

1. [x] View model. In `crates/workspace/src/launcher.rs` add `match_range(name,
   query) -> Option<Range<usize>>`, `Launcher::match_count`, `Chip { pick,
   label, count, state }` and `Launcher::location_chips`, `Launcher::cycle_pick`,
   `Launcher::can_open`, `open_label`, `branch_label`, `Launcher::clear_search`
   and a focus-request flag with `take_focus_request`. Unit tests first.
   Files: `crates/workspace/src/launcher.rs`.
2. [x] Palette view. Rewrite `ui::launcher` as the centred card: header, search row,
   chip row, list with underlined names and branch column, preview pane, footer,
   notices below the card; keep the scanning, failed, empty and no-match states.
   Files: `crates/workspace/src/ui.rs`.
3. [x] Key routing. In the launcher branch of the key block: Tab → `cycle_pick`
   (consumed), Esc → `clear_search` (not passed on), Enter only when `can_open`;
   request search focus on open. Files: `src/app.rs`.
4. [x] Acceptance tests. Add
   `tests/story_082_the_launcher_is_a_keyboard_first_palette.rs` (one kittest per
   scenario) and update the STORY-078 acceptance tests for the changed strings
   ("All projects", "Open (Enter)", the recent row labels). Files: `tests/`.

## Risks

- egui uses Tab for focus navigation; the key must be consumed with
  `input.consume_key` before widgets run, *and* the field must be built with
  `lock_focus(true)`, or focus jumps to the first chip (Decision 16). A
  kittest pins it.
- Enter while the search field has focus: the single-line field drops focus on
  Enter; keys are read before the widget, as `app.rs` already does, and the
  launcher asks for the focus back (Decision 16).
- The STORY-078 tests assert old strings and must be updated in the same change,
  not deleted.
- The kittest harness must be at least 1000 px wide for the 920 px card.
- Out of scope (later EPIC-025 stories): last-opened time, changed-file count,
  worktree count, sessions in the preview, ⌘R host recheck, Esc back to the map.

## Proof

Commands (all must pass, as in `CLAUDE.md` § Verifying your work):

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    SHELLCHECK_OPTS=--exclude=SC2016 actionlint

Unit tests in `launcher.rs`:
- `match_range`: ASCII, case-insensitive, no match gives `None`, a Unicode name
  whose lowercase changes length gives `None` without panicking.
- `location_chips`: "All 4", "This Mac 2", "wsl" connected 1, "devbox" offline 1;
  unchanged by a query.
- `cycle_pick`: twice from All gives `Host("wsl")`; after the last host it wraps
  to All; a selection that is no longer visible is dropped.
- `match_count` for "fac" is 3.
- `clear_search` empties the query and lists every project.
- `can_open` and `open_label`: local opens with "Open · Enter"; remote on an
  offline host does not open and reads "Open · connect the host first".
- `branch_label` is `None` until the details arrive and for a remote project,
  then the branch, or "folder" for details that came back without one
  (Decision 12).
- `location_of` is "This Mac" or the host's name, never its state
  (Decision 13); `HostState::word` carries no glyph (Decision 14).
- `groups` is "This Mac" then the hosts, two local folders in one group
  (Decision 11).

Acceptance tests, one per scenario, headless `egui_kittest` through
`cellworks::draw`, harness width 1280, in
`tests/story_082_the_launcher_is_a_keyboard_first_palette.rs`. Fixture: tempdir
home with `Projects/cellworks` and `Projects/The-Factory` (both with `.git`),
`settings.toml` with hosts `wsl` (projects `~/src/factory-evals`) and `devbox`
(projects `/srv/surface-kit`), probe results connected and offline, recent file
naming `cellworks`.
- Recent first, then groups "This Mac", "wsl", "devbox"; search field focused;
  no row claims "folder" before its branch is read, and the selected row shows
  its branch (Decision 12).
- Chips read "All 4", "This Mac 2", "wsl connected 1", "devbox offline 1", and
  no label embeds a state glyph (Decision 14).
- Tab twice picks "wsl" and only "factory-evals" is listed; the search field
  still has the focus after each Tab (Decision 16).
- Query "fac" lists three projects and shows "3 matches".
- Esc after "fac" empties the field, lists every project and leaves the focus
  where it was (Decision 16).
- "u", "m" and "z" pressed into the focused search field leave the navigation's
  underground, cables and lock alone (Decision 15).
- Selecting "cellworks" shows kind and location, path, branch, files; Enter
  opens it.
- Selecting "surface-kit" shows the disabled "Open · connect the host first";
  Enter opens nothing and the search field still has the focus afterwards
  (Decision 16).

The STORY-078 acceptance tests pass with their updated strings.

## Docs impact

Written at freeze in the commit `docs: ingest STORY-082 prototype decisions`;
the build changes no other doc.

- **Business decisions** — None.
- **Glossary** — `docs/domain/contexts/Workspace/glossary.md`: the Location
  row's UI note changes from "sources" (left column) to the location chips; the
  Remote host row's UI note drops "HOSTS".
- **Guidelines** — None.
- **arc42** — None.
- **ADR needed** — No.
