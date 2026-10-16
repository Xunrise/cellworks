# Context: DomainKnowledge

Domain knowledge as context cells: parsed from domain documents and linked to the code that uses their glossary terms.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Code architect

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Context cells are parsed from `context-cells/`, from jarvis `docs/domain/contexts/<Name>/<Name>.md`, or from flat `contexts/<name>.md` plus a shared `glossary.md` (L97-99).
- Code is linked to cells by glossary terms, with document-frequency limits to avoid noise (L100).
