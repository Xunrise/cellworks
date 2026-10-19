# Discovery: the living city

**Date:** 2026-10-10
**Status:** design direction confirmed by the product owner through interactive prototypes (v3 to v9); not yet built.
**Prototypes:** Claude Design canvas "Cellworks mind palace mockups", one page per version, latest first
(<https://claude.ai/artifact/5H97XvynwMnLeW5neamMJ5>). Design systems: "Cellworks" (clean, business theme,
<https://claude.ai/artifact/7eqPqvt2Bfe3xgcfKFivqY>) and "Cellworks Netrunner"
(<https://claude.ai/artifact/FQBGWeM9B6HhfaeTzwBMrT>).

## Summary

Cellworks renders code as a city that can be touched and that is alive. A line of code is a point of light; a
function is a tower built from its lines, one floor per line; a file is a block of towers; a crate is a district;
the repository is the city. The arrangement of blocks and districts follows a **lens** — the definition of what
counts as a connection: folders (default), imports, calls, co-change, bounded contexts ("cells"), or the user's own
arrangement. Inhabitants work in the city: Claude Code sessions walk to towers and add floors as they write; test
runs travel along the real call paths. The city reacts to state: compile errors burn on their exact floor, failing
tests go dark, uncommitted work stands in scaffolding, open branches are ghost districts. The engineer reviews pull
requests in the city — what a change builds, removes and impacts, and whether it touches critical buildings — and
reads code health as weathering: complex, long, untested or cyclic code crumbles. The interaction borrows from
real-time strategy games: select an agent, right-click a tower to give it work.

This replaces the box map of the earlier prototype as the target visual model. It keeps the existing navigation
grammar (layers, Enter/Esc, selection, the tree) and the quality goals in `docs/arc42/10-quality.md`.

## How we got here: prototype evidence

| Version | What it tried | Verdict |
|---|---|---|
| v1 | Machine-wide treemap of boxes, HUD, palette, hints, scan overlay; 13 screens in two skins | Rejected: reads like a UI, not like matter |
| v2 | "Living map": decorative point clouds, constellations, organism | Rejected: points must mean code, not decoration |
| v3 | Lines as points, functions as towers, files as blocks, crates as districts (static) | Kept: overview, file size and districts are clear |
| v4 | Agents, a city that reacts, construction verbs, time-lapse from git history | Kept: "it feels like an RTS" |
| v5 | Rotation, x-ray, line-level picking, stepping into a tower and editing a line; lens-derived districts | Kept |
| v6 | Pull-request review in the city, critical buildings from the Jarvis docs, "why" panel, cells and mine lenses | Kept |
| v7 | Review you can finish: before/after, progress with a critical gate, line comments, proof, inline diff | Kept |
| v8 | An agent's PR as ghost towers under a crane; approve merges, request changes sends the agent back | Kept |
| v9 | Code health as weathering: decay, cracks, lean, health bars, import-cycle loops, repair by agents | Kept |

Data in the prototypes is real where it can be (cellworks `origin/test`: 44 files, 585 functions and types, 6,783
lines, calls, imports, co-change, per-line commits, 21 PRs, stories, ADRs, bounded contexts) and invented where the
product does not produce it yet (agents, compile errors, failing tests, scaffolding, the agent PR).

## Research behind it

- **Cyberpunk's Ihara-Grubb transformation algorithms** render the Net as a physical environment so it can be
  navigated by intuition; Cyberpunk 2077 builds its cyberspace from sparse point clouds of real geometry
  (CD Projekt Red, SIGGRAPH 2021: <https://blog.siggraph.org/2021/08/cyberspaces-and-braindances-in-cyberpunk-2077.html/>).
  Cellworks takes the idea literally: the geometry is the code.
- **Spatial memory needs stable positions** (method of loci; Kuhn's software cartography; Robertson's Data Mountain).
  Tower shapes never change with the lens; only placement does, and the folders lens is the stable home.
- **3D placement hurts retrieval** (Cockburn and McKenzie, CHI 2002); code cities work because they are 2D layouts with
  height as a metric (Wettel and Lanza, ICSE 2011). The city is a 2.5D projection of a 2D layout; no free flight.
- **Games for overview of complex systems**: real-time strategy (select a unit, right-click a target, health bars,
  minimap, control groups), Factorio's alt-mode overlays, RimWorld's alert stack, EVE's linked overview table,
  Outer Wilds' rumour map. These carry the interaction; avatars and first-person movement do not.
- **High-performance HMI (ISA-101)**: quiet when healthy, colour and motion only for what needs attention. Neon in the
  Netrunner skin is reserved for abnormal or active states.

## The city model

| Code | City | Rule |
|---|---|---|
| Token | point | Up close, a line opens into one point per token |
| Line | floor | One floor per line in source order; the floor starts at the line's indentation, so a tower's silhouette is the code's shape |
| Function, type, `use` block | tower | Height is the number of lines; width the longest line; tests are grey towers |
| File | block | Towers in rows on one plot (folders lens) |
| Crate or folder | district | Blocks around a plaza (folders lens) |
| Repository | city | |
| Machine | the Net | Out of scope for this pass; see the earlier machine-territory direction |

Colour always encodes the tower's folder (crate), in every lens. Ground plots and district rings take the lens colour
outside the folders lens.

## Lenses

A lens decides which blocks and districts exist and where they stand. Towers keep their shape in every lens.

| Lens | Blocks | Districts | Roads |
|---|---|---|---|
| Folders (default) | files | crates | avenues from plaza to block |
| Imports | clusters of files that import each other (community detection on the file graph) | clusters of blocks | use edges between blocks |
| Calls | clusters of functions that call each other (community detection on the call graph; files dissolve) | clusters of blocks | call density between blocks |
| Co-change | clusters of files changed in the same commits | clusters of blocks | pulsing shared-change arcs |
| Cells | files | bounded contexts from `docs/domain/`, placed by the context map | the context-map relationships, labelled PL / CF / OHS |
| Mine | files | crates, arranged by the user by dragging blocks and districts; remembered | as folders |

In every lens except folders and mine, tests form their own district (one block per test file) and code the lens
does not connect forms an outskirts district at the edge. Switching lens animates every tower to its new place.

## Navigation and interaction

- Camera: zoom with the wheel or `+`/`-`, pan by dragging the ground or with the arrows, rotate by 90° with `Q`/`E`,
  fit with `f`. A minimap shows the city, agents and trouble spots, and the camera frame; clicking it jumps.
- Reaching towers: towers in front of the hovered or selected tower turn to glass (x-ray); picking hits the line under
  the pointer, not a bounding box; `Tab` cycles towers of the same block.
- Stepping inside: `Enter` or a double-click on a tower opens it face-on: one row per floor with the line's text and its
  token points, calls out marked on the right, and the PR and story that last wrote each line. `j`/`k` move between
  floors; `Enter` or `i` edits the line in place; the edited floor is marked uncommitted and the tower gets scaffolding.
- Construction: drag a tower to another block to move the function (a card shows how calls across district borders
  change before anything happens); select a tower, press `L` and click another to draw a new call (the card checks it
  against the context map); right-click empty ground to mark a plot for an agent.
- Time: a scrubber replays git history commit by commit; floors appear when the commit that wrote them lands.

## The living city

- **Agents** (Claude Code sessions) walk the streets, climb the tower they work on and add floors while they write.
  Selecting an agent (click or `F1`–`F3`) shows its command card with abilities: write, review, test, repair.
  Right-clicking a tower dispatches the selected agent with a prefilled, editable task; right-clicking ground has it
  build a draft there. An agent waiting for approval pulses.
- **Test runs** send traffic from the tests district along the real call paths; reached towers flash, green or red.
- **Reactions**: a compile error burns on its exact floor; failing tests stand dark; uncommitted files carry
  scaffolding; an open branch is a ghost district with a crane.

## Reviewing pull requests

- `p` lists pull requests; opening one puts the city into review: everything not involved dims; floors the PR wrote
  glow; lines it removed fall as debris; callers it affects get a ring; critical buildings it touches raise beacons.
- The review panel shows the story the PR implements (its "As a / I want / So that" and its intent), the
  `sdlc/critical.yaml` flags its files hit, the critical buildings touched (grouped by reason), the health of the towers
  it changes, the story's acceptance tests as proof with their scenarios, and the list of changed towers.
- `n` steps through changed towers, critical ones first, and ticks them off; approving with critical towers unvisited
  asks for confirmation and names them. `b` toggles before and after. Inside a tower, removed lines appear inline,
  struck through, and `c` leaves a comment on a line, shown as a flag on that floor in the city.
- **Agent PRs**: an agent's open PR stands as ghost towers under a crane. Approving merges it and the towers become
  solid; requesting changes sends the agent back to work on the comments, and a new revision returns to the queue.
- **Critical buildings** come from the Jarvis docs: aggregates named in the contexts' tactical models (`ProjectMap`,
  `Navigation`), the Map context's published language (read by CodeAnalysis, Sources and DomainKnowledge per the context
  map), and the biggest hotspots; the engineer can mark any tower critical.
- **Why it is built this way**: selecting any tower shows its bounded context and description, the PRs and stories that
  built its floors with the story's why, the ADRs that bind its crate, and why it is critical.

## Code health

- Health starts at 100% and loses points for cyclomatic complexity (bands from the earlier prototype: ≤5 simple,
  6–10 moderate, 11–20 complex, >20 untestable), length, being a hotspot, not being reached by any test within three
  calls, and sitting in a file that is part of an import cycle.
- The health view (`h`) weathers the city: low-health towers lose and shift points, stain, leave rubble at their foot
  and, below 40%, lean; cracks run across the floors that make decisions in complex functions; health bars stand above
  weak towers and every district; import cycles are red pulsing loops.
- Repair: dispatching an agent to a crumbling tower prefills the task from its health findings; the result arrives as a
  pull request to review.

## Requirements confirmed through the prototypes

1. Points are code: one point per line, one per token up close; no decorative geometry.
2. A function is a tower whose silhouette follows its indentation; a file is a block; a crate is a district.
3. Tower shapes are independent of the lens; folders is the default lens and the stable home.
4. Lenses rebuild blocks and districts from the relationship they name; districts exist as folders only in the folders
   lens; colour always shows the folder.
5. Tests form their own district in the relationship lenses.
6. Any tower can be reached: x-ray, rotation, line-level picking, cycling within a block.
7. Stepping into a tower shows its lines readably and allows editing a line.
8. Agents are units that can be selected and given work by right-click, with visible progress.
9. The city reacts visibly to compile errors, test results, uncommitted work and open branches.
10. A pull request can be reviewed in the city: changed, removed and impacted code, critical buildings, story and proof,
    before/after, progress, comments, approve or request changes.
11. Critical buildings are derived from the Jarvis docs and can be marked by hand.
12. For any tower, the city explains why it exists: context, stories, PRs, ADRs.
13. Code health is visible as weathering, with complexity, size, test reach, hotspots and import cycles.
14. Two skins share one model: Netrunner (default) and Clean.

**Rejected on the way:** boxes and panels as the primary form; decorative point clouds; random placement; free 3D or
first-person flight; districts that persist across lenses; call cycles from name-based matching (false positives —
needs resolved calls).

## Architecture impact

- **Map** gains the city geometry (tower, block, district placement), lenses and their layouts, health overlays and
  review overlays as published language; rendering stays egui immediate-mode painting (points, lines, text) per
  ADR-0001. The cellworks repository needs about 7k points per frame, well inside the budget; large repositories need
  level of detail (a line point per floor far out, tokens only near; cached tower sprites at the far band).
- **CodeAnalysis** needs tree-sitter (ADR-0004) for functions, tokens, resolved calls and complexity; git for per-line
  history and co-change.
- **DomainKnowledge** reads the Jarvis docs: bounded contexts, context map, tactical aggregates, ADRs, sdlc artifacts.
- **Sources** gains the pull-request and story source (GitHub through the work-tracking façade's backend).
- **AgentSessions** places sessions as units and dispatches work; how sessions are driven is still open (arc42 4.4).
- **Navigation** gains the camera (rotate, x-ray, fit), stepping into towers, agent selection and the review mode.
- **Editing** opens a floor in the embedded editor; the in-place line edit of the prototype is a stand-in.

Decisions to record as ADRs when their first story is planned: the city rendering model; lens layouts with community
detection; per-line provenance from git; pull-request data source; health metrics; the review mode's state.

## Glossary gaps

Terms these stories need that the glossaries do not have yet (to settle with the domain artifacts before or with the
first story that uses them):

| Proposed term | Context | Relation to existing terms |
|---|---|---|
| Lens | Map | Extends or replaces *View options*: the definition of a connection that decides blocks and districts |
| Tower | Map | The rendering of an *Item* (function, type, `use` block) |
| Floor | Map / CodeAnalysis | One line of code; a new level below *Item* |
| Block | Map | A group of towers on one plot; a file in the folders lens, a cluster otherwise |
| Health | CodeAnalysis | Score from *AST metrics* (complexity), size, test reach, churn and cycles; today "health" exists only for workloads in Sources |
| Import cycle | CodeAnalysis | Files that import each other in a loop; not in the *Dependency tree* yet |
| Critical building | DomainKnowledge | A tower the docs mark as critical (aggregate, published language, hotspot) or the engineer marks |
| Review | Sources / Navigation | A pull request opened in the city |
| Dispatch | AgentSessions | Giving a *Session* work by pointing at a tower or plot |
| Replay | CodeAnalysis | Rebuilding the city commit by commit; *History* stays the back/forward of views |
| Skin | Workspace | The display theme set (Netrunner, Clean, 40k); identifiers stay neutral |

## Backlog

All stories are `status:draft` until approved; component `cellworks`.

| Epic | Stories |
|---|---|
| EPIC-009 The city | STORY-010 (chore: tree-sitter lines and tokens), STORY-011 A project opens as a city, STORY-012 Fly the camera, STORY-013 Reach any tower, STORY-014 Step inside a tower |
| EPIC-010 Lenses | STORY-015 Relationship lenses, STORY-016 Cells lens, STORY-017 Arrange my own city |
| EPIC-011 Why it was built | STORY-018 (chore: per-line history), STORY-019 A tower explains why it was built, STORY-020 Replay the city history |
| EPIC-012 Reviewing pull requests in the city | STORY-021 (chore: pull-request source), STORY-022 Open a review, STORY-023 Critical buildings, STORY-024 Finish a review, STORY-025 Comment and verdict, STORY-026 Proof |
| EPIC-013 Code health | STORY-027 Health as weathering, STORY-028 Import cycles as loops |
| EPIC-014 Agents in the city | STORY-029 (spike and ADR: driving sessions), STORY-030 Sessions walk to their towers, STORY-031 Dispatch an agent, STORY-032 The city reacts, STORY-033 An agent's pull request, STORY-034 Test runs travel, STORY-035 Construct in the city |
| EPIC-015 Skins | STORY-036 (chore: Netrunner and Clean) |

Suggested order: EPIC-009 first (STORY-010 unblocks it), then EPIC-011's history chore and EPIC-012 (the review is
the daily use), EPIC-013, EPIC-010, and EPIC-014 once STORY-029 has decided how sessions are driven. This supersedes the
earlier E4–E13 roadmap sketched during planning.

## Open questions

- How sessions are driven (arc42 4.4) — blocks agent dispatch, repair and agent PRs.
- Whether the in-place line edit replaces or front-runs the embedded Neovim for small edits.
- How much of the review (comments, verdicts) is written back to GitHub, and when.
- Level-of-detail thresholds for repositories two orders of magnitude larger than cellworks (P1).
- Whether the machine-wide territory (whole filesystem) returns on top of the city, as the earlier direction proposed.
