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

## Business decisions
- An opened project's map is built only by cutting the project's included files with a district strategy and laying
  them out; its districts, sub-districts, groups and nodes are each identified by the canonical path of their folder
  or file (STORY-003).
- No two drawn rectangles overlap, and the only rectangle that contains another is a parent containing its own child
  (STORY-003).
- In the directories strategy, a named prefix rule finds a project's district level; it is chosen per project in the
  settings file, `one-wrapper-folder` by default (STORY-003).
- While a review is open, a tower the pull request does not touch recedes so the towers it marks stand out: a written
  floor is one of its lines the pull request added or changed, a removal strike marks the floor a removed line stood
  on, and an impacted tower is one the pull request did not itself change but that calls a tower it did, tests
  included; a changed tower is never also marked impacted (STORY-022).
