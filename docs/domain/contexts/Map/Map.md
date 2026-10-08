# Context: Map

The universe of projects as one zoomable map. Each project is laid out by physics into districts, sub-districts, groups, nodes and items joined by lanes, and rendered with level of detail so labels stay readable at every zoom. The map is source-agnostic: every loader writes into its graph of nodes and lanes.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Code architect — reads structure, dependencies and health
- Every other persona — sees their project type on the same map

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Projects sit in a universe; zooming out of one and into the next is seamless. Double-clicking a project locks into it and hides the others; Esc unlocks (L63-64).
- Physics layout: repulsion within a capped range, forces only within a group, districts anchored apart, sub-districts with their own home inside their district, nodes inside a group clustered by affinity (shared users, shared dependencies, name tokens) (L65-69).
- Clicking around must not inflate the map, verified on about 20k files (L70). Hard-box separation of sub-districts was rejected: it strung large maps out (L239-240, L253).
- Clear margins between districts and sub-districts (L71).
- Level of detail: readable labels at every zoom and during transitions, names only at overview, shapes per node kind, rounded corners (L72-76).
- Districts, groups and group boxes can be dragged; middle-drag always pans (L82).
- View options: lanes between nodes, groups or districts; lane kinds on/off; minimum strength; only around the selection; organize by folder, graph or git changes; git overlay (L83-89).
- Districts are cut by directories for now; the district strategy is to become swappable (domain context, module, type, language) (product owner, 2026-10-08).

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
