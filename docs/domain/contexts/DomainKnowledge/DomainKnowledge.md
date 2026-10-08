# Context: DomainKnowledge

Domain knowledge as context cells: parsed from domain documents and linked to the code that uses their glossary terms.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Code architect

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Context cells are parsed from `context-cells/`, from jarvis `docs/domain/contexts/<Name>/<Name>.md`, or from flat `contexts/<name>.md` plus a shared `glossary.md` (L97-99).
- Code is linked to cells by glossary terms, with document-frequency limits to avoid noise (L100).

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
