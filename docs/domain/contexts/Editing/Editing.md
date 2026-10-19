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
