---
stage: intent
issue: 5
status: draft
author: Xunrise
date: 2026-10-08
---

# Intent: See a code project as districts, sub-districts, groups and nodes

## Problem

Cellworks shows a code project as a single entry in the universe (STORY-002), but opening one shows nothing of what
is inside it: a project's folders and files are not yet laid out as anything on the map. The code architect — whose
job is reading a codebase's structure, modules and dependencies — opens a project today and finds no district,
sub-district, group or node to read it from; the map has nothing to show below the project itself.

## Proposed outcome

When the code architect opens a code project, its folder structure appears as districts, sub-districts, groups and
nodes, arranged so that none of them overlap: the top folders below the project's common prefix become districts, the next folder level becomes sub-districts, and folders deeper than that become groups holding files as
nodes; a file sitting directly in a district is a node of that district. A project whose files do not all share a
folder — loose files at the very top, alongside at least one top-level folder — shows those loose files as nodes of
a district named after the project itself, next to a district for each top-level folder. Hidden folders, and, inside
a git repository, folders git ignores, do not appear; an added folder that is not a git repository leaves out only
the hidden ones. A project with no files still opens, and tells the code architect it has no files rather than
showing an unexplained blank. This is the simple, deterministic version of the layout named in the issue's own
scope note: what EPIC-002 replaces later is the arrangement (today's is not physics-based), not what counts as a
district, sub-district, group or node.

## Affected users and systems

The code architect is the persona named in the report and the one who needs this to read a project's structure at a
glance. Every other persona and the onboarding colleague eventually opens a project too and depends on the map
having something to show once inside it. This is Map and CodeAnalysis context behaviour: it builds on STORY-002
(Workspace discovers and lists projects in the universe) and on the existing "open a project" gesture — double-click
in the universe, which locks into the project and hides the others. Enter doing the same is STORY-004's concern, not
this story's.

## Constraints

- `docs/domain/contexts/Map/Map.md` and its glossary are the source of truth for District, Sub-district, Group and
  Node as map levels, and record that districts are cut by directories as "the current district strategy"
  (product owner, 2026-10-08), with the strategy becoming swappable later. This story implements the directories
  strategy; it does not change what the terms mean.
- `docs/domain/contexts/CodeAnalysis/CodeAnalysis.md` and its glossary already map a Folder to a Group and a File to
  a Node on the map. The design stage reads that alongside `Map.md`'s District/Sub-district/Group levels, since this
  story is what gives folders their District and Sub-district levels as well.
- The issue's own scope note is this story's boundary: the layout is "a simple, deterministic arrangement that keeps
  every district, sub-district and group apart"; the physics layout — repulsion, forces only within a group,
  affinity clustering (`Map.md`; prototype findings L65-69) — is EPIC-002's, not this story's.
- `docs/discovery/2026-10-08-prototype-findings.md` L71 ("clear margins between districts and sub-districts so the
  folder hierarchy reads at a glance") and the sub-district edge case at L239-240 and L253 (pushing sub-districts
  apart as hard boxes strung large maps out; homes per sub-district inside the district worked instead) were
  recorded against the physics layout, but "keeps every district, sub-district and group apart" is this story's own
  version of the same requirement, so the design stage should weigh them.
- "Opens the project" is the existing double-click-in-the-universe gesture that locks into a project and hides the
  others (prototype findings L64; `docs/domain/contexts/Navigation/Navigation.md` L159-167, Enter goes one layer
  in). This story is about what appears once that happens, not the gesture itself.
- Performance matters for personal daily use on large repositories generally (prototype findings L224). The issue
  sets no specific scale target for this deterministic layout, unlike the physics layout's 20k-file check
  (prototype findings L70), which belongs to EPIC-002.

## Open questions

Resolved by the product owner on 2026-10-09: which common prefix is stripped
before the districts is a per-project setting (`district_prefix` in the
settings file) with three rules — `one-wrapper-folder`, the default, which
strips one top folder every file shares; `longest-shared-prefix`; and `none`.
The rules are built so a further one is easy to add later. An unknown value
falls back to the default and says so.

Resolved by the product owner on 2026-10-09: anything git ignores is left
out, individual files as well as folders, together with hidden folders. A
control to bring ignored files back into view is its own story, STORY-005
(#16).

No text in the issue was addressed to an AI assistant; there is nothing to flag on that front.
