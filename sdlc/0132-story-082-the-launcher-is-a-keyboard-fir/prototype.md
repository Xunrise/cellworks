# Prototype: STORY-082 — the launcher is a keyboard-first palette

Issue: #132 (EPIC-025, redesign the projects picker). The prototype is a
Claude Design canvas on the Cellworks design system, not code:
https://claude.ai/artifact/PY7i4CJwQ4zRXEfGy26XEi, page v1 ("v1 — palette
launcher"), boards `Main.dc.html` (default state) and `Search.dc.html`
(searching, offline host selected). It restyles the STORY-078 launcher; the
data model from STORY-078 stays.

## Confirmed behavior

- Header: "Cellworks", the project and host count ("14 projects · 2 hosts"), and
  an "Add project ⌘N" button (still inert, as in STORY-078).
- One centred palette card replaces the three-column layout.
- Search row on top of the card, focused when the launcher opens; while
  searching it shows the match count ("3 matches") and an "Esc clear" hint.
- A row of location chips replaces the left locations column: "All", "This
  Mac", and one chip per remote host with its state glyph and word
  (● connected, ◌ checking, ○ offline), each with its project count. The picked
  chip is highlighted.
- Tab moves to the next location chip.
- The list shows Recent first, then one group per location. Each row shows
  the project name, path and branch ("folder" when the project has none).
- While searching, the matched fragment of each name is underlined.
- A preview pane on the right of the list shows the selected project: name,
  kind and location, path, branch and number of files, an "Open · Enter"
  button and a disabled Sync button.
- A remote project on an offline host can be selected, but Open is disabled
  and reads "Open · connect the host first"; Enter does nothing.
- Footer with key hints: "↑ ↓ move", "Enter open", "Tab scope", "Esc clear
  search".
- Esc clears the search text.
- Notices (for example a skipped host) appear below the card.

## Feedback log

- "looks great, freeze and implement it" → v1 accepted as drawn; no prototype
  change.

## Rejected

- Building every element of the v1 mockup in one story: the operator chose
  "Layout first". Last-opened time, changed-file count, worktree count,
  project sessions in the preview and the ⌘R host recheck need new data or
  new probes and go to later EPIC-025 stories. This story does not build them,
  and does not show the ⌘R hint or the "Check again" button.
- Esc returning to the map: the operator chose "Clear search only".

## Edge cases

- Offline host: its projects stay listed and selectable; opening is refused
  in the preview and on Enter.
- Checking host: shown with ◌ and the word "checking"; its projects are
  listed as before (STORY-078 behaviour).
- No match: the STORY-078 empty state ("no project matches …", offer to add a
  project) stays, inside the card.
- Tab on the last location wraps to "All".

## Decisions

- Q: How should the mockup be split into stories? → A: Layout first
  (Recommended): this story is the palette layout over today's data; the data
  extras are later stories in EPIC-025.
- Q: What does Esc do in the launcher? → A: Clear search only (Recommended).

## Evidence

- Branch: `proto/projects-picker` (off `origin/test`); no code changed.
- Canvas: https://claude.ai/artifact/PY7i4CJwQ4zRXEfGy26XEi, page v1, boards
  `Main.dc.html` and `Search.dc.html` at 1280×800, on the Cellworks design
  system (https://claude.ai/artifact/7eqPqvt2Bfe3xgcfKFivqY).
- Code read for the mockup: `crates/workspace/src/launcher.rs`,
  `crates/workspace/src/ui.rs`, `src/app.rs`.
- Reviewed by the operator in the Claude Design canvas viewer.
