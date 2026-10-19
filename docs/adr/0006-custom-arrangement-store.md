# ADR-0006: Keep the custom arrangement in Cellworks' own data directory

## Status
Accepted

## Date
2026-10-12

## Context
The custom lens lets the engineer move blocks and districts; the arrangement must be there the next time the project
opens, and moving blocks must not change any file of the project (STORY-017). The settings file is the user's own,
read-only by construction, and hand-edited; Cellworks must not write it.

## Decision
We will keep one arrangement file per project in a folder `arrangements/` beside the settings file, written by
Workspace when a drag ends and read when the project opens. A file holds, per district or node path relative to the
project, how far it was moved, in the map's unturned ground coordinates. The folder is passed in, so tests use a
temporary one.

## Consequences

### Positive
- No file of the project changes; the settings file stays read-only.
- Moves in unturned coordinates stay right when the map is turned.

### Negative
- An arrangement does not travel with the repository; another machine starts from the folder city.
- A renamed file loses its move.

### Neutral
- The format is Cellworks' own and may change without migration while the feature is young.

## Alternatives considered
- **A file inside the project** (`.cellworks/`): travels with the repository, but changes the project.
- **The settings file**: one place for everything, but it is the user's and read-only by construction.

## References
- Discovery: [lenses and lanes](../discovery/2026-10-12-lenses.md)
- Domain: [Workspace](../domain/contexts/Workspace/Workspace.md)
