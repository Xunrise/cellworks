> Status: draft

# 1. Introduction and Goals

Cellworks makes the work an engineer does with AI agents across many projects readable and navigable in one place: a
native desktop map on which code projects, Jira projects, infrastructure and data structures are laid out with the same
mechanics, and on which Claude Code sessions appear as units that can be created and steered. It is meant to replace
using the Claude Code CLI directly.

The domain model is in [`docs/domain/`](../domain/README.md); the discovery input is
[the prototype findings](../discovery/2026-10-08-prototype-findings.md).

## 1.1 Requirements overview

- All projects in one view (Universe), each project laid out as districts, groups, nodes and items joined by lanes.
- Claude Code sessions visible on the map, moving to the files they touch; created, attached, detached and answered
  from Cellworks.
- One navigation model for every project type: layers of detail, keyboard-first movement, item menu, palette, tree,
  history.
- Code understanding: AST metrics, dependency trees, git state, domain knowledge as context cells.
- Other project types: Jira, infrastructure (Terraform plus read-only Kubernetes), data structures.
- An embedded editor that opens only on explicit request.

## 1.2 Quality goals

Top priority, in order:

| # | Goal | Meaning |
|---|---|---|
| 1 | Usability | Keyboard-first; readable at every zoom; the selection is always visible; the same navigation works on every project type; the editor and terminals never steal keys they were not given. |
| 2 | Performance | The map stays interactive on large repositories (about 20k files); scans and rescans never block the UI; clicking around never inflates the layout. |
| 3 | Extensibility | A new project type or district strategy plugs into the Map's graph and gets navigation, search, lanes and rendering for free. |

Lower priority:

| Goal | Meaning |
|---|---|
| Reliability | Sessions survive restarts of Cellworks; edits auto-save; a failing loader degrades its own project only. |
| Security | Secrets never on a command line or in logs; cluster access read-only; no local network server. |

## 1.3 Stakeholders

| Role | Expectation |
|---|---|
| Product owner (a software engineer at SQUER, the primary daily user) | A fast, keyboard-driven replacement for working with the Claude Code CLI across many projects. |
| Code architect | Understand a codebase's structure, dependencies, health and bounded-context boundaries. |
| AI engineer / operator | Run several Claude Code sessions, watch what they touch, answer approvals. |
| Ticket and ops user | Navigate Jira and live infrastructure like code. |
| Data user | Navigate tables, collections, contracts, DTOs and their users. |
| Onboarding colleague | Get started quickly with their own session (README, CLAUDE.md, setup script). |
