# Glossary

## Shared / Cross-context

Terms with one identical meaning in two or more contexts.

| Term | Definition | Source | Contexts | Notes |
|------|------------|--------|----------|-------|
| Commit | A change git recorded on the project: its id, time and subject, the pull request it was merged by (the last `(#N)` of the subject) and every story its message names. | STORY-018 prototype 2026-10-19; ADR-0008 | CodeAnalysis, Map |  |
| Lane | A connection drawn between two nodes, groups or districts, with dots flowing from upstream to downstream; it has a lane kind. | prototype-findings L49, L90 | Map, Navigation | theme: belt, pipe |
| Project | A top-level thing on the map, identified by its path, or by a `jira:`, `otc:` or `data:` prefix for the other project types, or by `host:path` for a remote project, as written and not canonicalized. | prototype-findings L42, L219 | Map, Navigation, Sources, Workspace | theme: planet, continent |

## Same word, different meaning

The same word used for different things in different contexts. Both definitions
are correct in their own context — this table exists so that reading only one of
them is not a silent trap.

| Term | Context | Means | vs. Context | Means |
|------|---------|-------|-------------|-------|

## Index

Every context's own glossary.

| Context | Terms |
|---|---|
| [AgentSessions](contexts/AgentSessions/glossary.md) | 5 |
| [CodeAnalysis](contexts/CodeAnalysis/glossary.md) | 7 |
| [DomainKnowledge](contexts/DomainKnowledge/glossary.md) | 2 |
| [Editing](contexts/Editing/glossary.md) | 2 |
| [Map](contexts/Map/glossary.md) | 15 |
| [Navigation](contexts/Navigation/glossary.md) | 8 |
| [Sources](contexts/Sources/glossary.md) | 9 |
| [Workspace](contexts/Workspace/glossary.md) | 2 |
