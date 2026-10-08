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

## Tactical model

> Status: not yet discovered
> The tactical model emerges during implementation. A story adds an
> entry here only for a new aggregate, a changed aggregate boundary or
> a new invariant — the code is the rest of the model. Do not
> pre-populate from the discovery source: tactical choices are made at
> code time, not at analysis time.

### Aggregates
_No entries yet._

### Entities (non-aggregate roots)
_No entries yet._

### Value objects
_No entries yet._

### Domain services
_No entries yet._
