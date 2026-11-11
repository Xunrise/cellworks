# Prototype: Explorer as the main navigation

Branch `proto/file-tree-navigation`, off `test` at 7fa999f. Design mockup: the "Tree navigation" Claude Design
canvas (https://claude.ai/artifact/C9KPGojj9mG4Adc1iBygTR), on the Cellworks Netrunner design system. Discovery:
`docs/discovery/2026-10-23-file-tree-navigation.md`.

## Confirmed behavior

- An Explorer panel overlays the left edge of the map when a project is open. It is open by default; Cmd+B hides
  and shows it.
- The panel is opaque (panel colour, line border); the map does not show through it.
- The header reads "EXPLORER" on the left and "<N> files" (the project's file count) on the right.
- Under the header, one row of equal-width lens chips: FOLDERS, IMPORTS, CALLS, CO-CHG, CELLS, CUSTOM. The active
  chip has a flow-cyan border and text on a faint cyan fill; inactive chips are dim text on a line border. Hovering
  a chip shows the lens's explanation. These chips replace the map's lens bar.
- The tree regroups by the active lens the way the map groups districts: folders in the folders lens, the lens's
  groups ("around <file>", "changes with <file>", …) in relationship lenses. A file appears only under its lens group.
- Under the chips, a filter field: a "/" prefix and the placeholder "filter files and symbols". Pressing "/" while
  no text field has focus focuses the filter (as in Vim) without typing the "/". Typing narrows the rows to names
  that match.
- Rows: folders and groups have a chevron, a square swatch in their district's map colour and, right-aligned, their
  file count. Files have an extension badge, their name and, right-aligned, their floor count. Under an expanded
  file, its tree-sitter symbols appear as leaves with a fn / ty / use badge.
- Git state: a file with uncommitted changes shows an "M" marker; a folder holding changed files shows a dot.
- j/k move the cursor one row. The cursor is the selection: moving it selects the row and flies the camera
  (0.6 s) to frame the folder's district or the file's building.
- l unfolds and h folds the row under the cursor; clicking a folder row toggles it. Shift+L moves to the next lens.
  Esc returns to the whole map.
- While the Explorer is open, framing a district or building centres it in the part of the map to the right of the
  panel, not under it. The whole-project overview is not shifted.
- Selecting a building or district on the map selects its row; the tree expands to it.
- A footer in terrain ink lists the keys: "j/k move · h/l fold · click toggle · / filter" and
  "Shift+L next lens · Esc whole map · Cmd+B hide".

## Feedback log

- "Lets create tree-sitter type menu on the left side of the screen, similar to how it works in nvim or zed editor,
  as an overlay on top of the map view." → the Explorer overlay on the left of the map, keyboard-first.
- "When i select a folder in the tree view, select and center around the district in the map view." → selecting a
  folder row frames its district (sub-districts for subfolders).
- "My goal is basically to navigate the way im used to: like in intellij, vscode, nvim via the file tree, and then
  instead of having the file preview open (in intellij for example) in the center panel, it shows me the
  building/district." → selecting a file frames its building; the map acts as the preview pane.
- "The lenses will correlate to the tree-view." → the tree regroups by the active lens.
- "Keep it: moving the cursor selects and flies the camera, like a preview pane." → j/k select and fly; there is no
  separate confirm step.
- "Fix the overlay overlap on the project overview, the hint text bleeding through and the Cmd+B glyph, then show me
  again." → the overview is no longer shifted, the panel is opaque, and the key hint reads "Cmd+B" in text.
- "what about the "modified" labels and things like that from the mockup?" → git M markers, change dots, extension
  badges, file counts and floor counts on rows; "EXPLORER" header with the file count.
- "what about the colorful squares that correlate to the district colors on the folder levels?" → district-colour
  swatches on folder rows, taken from the same palette the map uses.
- "just like in VIM "/" should focus the filter, and the label should be the same as in the mockup. The Folders/
  Imports/ Co-chg/ Cells buttons should also look similarly to the mockup with the clean design system" → "/"
  focuses the filter; the field shows the "/" prefix and the mockup's placeholder; the lens chips take the mockup's
  equal-width outlined style.

## Rejected

- Keeping the map's lens bar beside the Explorer's chips: two places to switch lenses.
- Giving a file a second, folder-ordered home in relationship lenses: the tree would stop matching the map.
- Hiding the Explorer until called: it is the main way to navigate.
- Flying the camera only on Enter: the cursor should preview, as in an editor.
- Shifting the whole-project overview to the right of the panel: it pushed the city off-centre (corrected after the
  first run).
- A translucent panel: the map's hint text bled through it (corrected).
- The "⌘" glyph and "▼" glyph: the app's font renders them as boxes; "Cmd+B" text and drawn chevrons instead.

## Edge cases

- Overview framing (the whole project fills the view): not shifted beside the panel. Only district and building
  framing are.
- A map narrower than twice the panel width: framing is not shifted.
- "/" typed while the filter, the session search or another text field already has focus is ordinary text.
- In a relationship lens, a filter that matches several files of the same name lists each under its group; the
  rows show only the file name. Known rough edge, accepted for this slice.
- A project with no uncommitted changes shows no markers; the git state refreshes about every 5 s.
- Cmd+B with no project open does nothing.

## Decisions

- Q: Should the tree replace the map's lens bar? → A: Yes, the Explorer's chips replace it.
- Q: In a relationship lens, does a file keep a second home under its folder? → A: No, a file appears only under
  its lens group.
- Q: Is the Explorer always open or toggled? → A: Open by default; Cmd+B hides and shows it.
- Q: Does moving the cursor select and fly, or only Enter? → A: Moving the cursor selects and flies the camera.
- Q: When the filter matches, does the Explorer list only the matches or also the folders leading to them? → A:
  Matches plus the folders leading to them, as an editor's file tree does.
- Q: With the Explorer open, what do the arrow keys do? → A: They drive the Explorer like j/k/l/h; arrows pan the
  map only while the Explorer is hidden.
- Q: Split the git change marks into a follow-up story to keep this one smaller? → A: No, keep one story.

## Evidence

- Branch `proto/file-tree-navigation`, worktree `cellworks-proto-file-tree-navigation`.
- Files: `src/explorer.rs` (new: rows, chips, filter, drawing, framing beside the panel, git and count stats),
  `src/app.rs` (keys, Cmd+B, camera flight, map-click reveal, lens bar removal), `src/lib.rs`,
  `crates/navigation/src/lib.rs` (`frame_on`), `crates/map/src/ui.rs` (`district_colour_at`).
- Run with `cargo run --release` against the cellworks repository; driven by AppleScript key events and the in-app
  F12 screenshot (`CELLWORKS_SHOT`), screenshots checked after each change.
