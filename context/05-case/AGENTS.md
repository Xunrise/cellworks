# Always-on context

Status: 2026-10-08.

Cellworks keeps its business context in the jarvis documentation under `docs/`
in this repository, not as context cells. Every design run loads the files
below, whatever the work item says.

## The files

- `CLAUDE.md`: stack, commands, cross-context rules, naming, and the domain at
  a glance.
- `docs/domain/glossary.md` and the glossary of each context the work item
  names (`docs/domain/contexts/<Context>/glossary.md`): the ubiquitous
  language; neutral terms are canonical, theme words are display only.
- `docs/domain/context-map.md`: the bounded contexts and their relationships.
- `docs/arc42/04-solution-strategy.md` and `docs/arc42/05-building-blocks.md`:
  the structure a change must respect.
- `docs/arc42/09-decisions.md`: the decisions already taken; open the ADRs that
  bear on the work.
- `docs/services/cellworks/README.md`: the component's conventions; open the
  guidelines it points to.

## What stays in the work item

The Gherkin scenarios of the story, its context and its epic.

## Open, and the plan for each

None.
