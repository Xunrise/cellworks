# Context: Editing

Editing files inside Cellworks with an embedded Neovim that opens only on explicit request, auto-saves, and can pop out into its own windows.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- AI engineer / operator
- Code architect
- Neovim

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Opens only on explicit request (Enter on a selected node, `i`, double-click, an edit action, opening a manifest), never by zooming (L105-106, L248).
- Auto-saves so git sees changes (L107).
- Pops out into its own window, several at once, while navigating (L108, L211); a full-screen toggle keeps the node header (L109).
- The Esc that leaves the editor does not also act on the map (L171, L235).

## As built

- Entering a file (double-click on a file or a tower, or Enter on a framed file) opens it in one editor over the
  map, the same in the city and the flat view; a tower opens its file at its first line.
- The editor floats over most of the map and can fill the map's panel (⌘F); popped out, it becomes a window of
  its own that stays open while the next file opens over the map.
- It runs the user's own Neovim (`nvim --embed`, their configuration and colour scheme); Settings › Editor can
  name another configuration, or another terminal editor (Helix, Vim, Micro, ...) run behind a pseudo-terminal.
  Without `nvim` the file is shown read only.
- While it has the keyboard the map's keys stand back; Esc in normal mode gives the keyboard back without moving
  the map.
