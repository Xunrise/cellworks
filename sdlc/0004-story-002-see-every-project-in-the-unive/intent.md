---
stage: intent
issue: 4
status: draft
author: Xunrise
date: 2026-10-08
---

# Intent: See every project in the universe

## Problem

Cellworks has no project discovery yet — the universe (the all-projects view) currently shows nothing, because
nothing scans a projects folder or lets a folder be added. An engineer who opens Cellworks today cannot reach any
of their projects from the map at all: there is no automatic discovery of the git repositories they work in, and
no way to bring in a project that lives outside the usual folder. Until projects appear, none of the rest of the
map — districts, code understanding, Jira, infrastructure, data — has anything to show.

## Proposed outcome

When Cellworks starts, the universe shows one project for every git repository found under the projects folder,
plus one project for every folder the user has separately added — including a folder outside the projects folder.
A folder under the projects folder that is not a git repository does not appear as a project. The same folder
reached two different ways (e.g. discovered under the projects folder and also added explicitly, or discovered
twice under different letter-case spellings) still shows as exactly one project. When neither the projects folder
nor the added folders contain anything to show, the universe is empty and tells the engineer how to add a project,
rather than showing a blank, unexplained screen.

## Affected users and systems

Every persona who navigates from the universe to their own work (code architect, AI engineer / operator, ticket
and ops persona, data persona) depends on their projects actually appearing here first. The onboarding colleague
depends on it too: their first session is their own project, and it must show up the same way. This is Workspace
context behaviour; it feeds the Map context, which has nothing to lay out until Workspace hands it projects.

## Constraints

- [docs/domain/contexts/Workspace/Workspace.md](../../docs/domain/contexts/Workspace/Workspace.md) and
  [the prototype findings](../../docs/discovery/2026-10-08-prototype-findings.md) L219-220 are the source of truth
  for discovery: git repositories under the projects folder are discovered automatically, and any folder can be
  added regardless of location.
- The prototype findings' edge case on letter case (L228-229) is the source of truth for de-duplication: on macOS,
  paths match regardless of letter case, so a discovered and an added spelling of the same folder must resolve to
  one project. The design stage applies the project identity rule from the domain glossary's `Project` entry
  ([docs/domain/glossary.md](../../docs/domain/glossary.md)), which identifies a project by its path.
- `docs/domain/contexts/Workspace/Workspace.md` and the Workspace glossary are this story's scope boundary:
  rescanning and rebuild/restart are separate, already-documented Workspace behaviours and are not part of this
  story. Added folders do persist across restarts: they are kept, by canonical path, in a settings file in the
  application support directory (`docs/arc42/08-crosscutting.md` §8.3), read when Cellworks starts. Product owner,
  2026-10-08. The interface for adding a folder comes with EPIC-008; until then the settings file is how a folder is
  added.
- CLAUDE.md commits the project to macOS first, with the release workflow also building Linux; any design-system
  or platform-guardrail cell under the business context repository governs the empty-state message's wording and
  the exact affordance for adding a folder.

## Open questions

Resolved by the product owner on 2026-10-08:

- Letter case follows the filesystem. Projects are matched by the
  filesystem's canonical path, so on macOS (case-insensitive by default)
  "Alpha" and "alpha" are one project, and on Linux, where they are two
  folders, they stay two projects.
- No text in the issue was addressed to an AI assistant; there is nothing to
  resolve.
