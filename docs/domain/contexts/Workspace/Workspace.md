# Context: Workspace

Which projects Cellworks knows and how it runs: automatic project discovery, adding folders, persisted settings, rescans, and rebuild and restart from inside the app.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Every persona
- Onboarding colleague — starts with their own session

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Projects are discovered automatically (git repositories under `~/Projects`, folders recent sessions worked in); any folder can be added (L219-220).
- Settings are persisted (L221).
- Rescan with ⌘R, and automatically after sessions write files (L222).
- Rebuild and restart from inside Cellworks; sessions keep running (L223).
- Paths match regardless of letter case on macOS (L228-229).
