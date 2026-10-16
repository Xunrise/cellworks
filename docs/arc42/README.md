# Architecture (arc42)

Architecture documentation for Cellworks, following the
[arc42](https://arc42.org/) template. Each chapter is a file in this section; the
sidebar lists them in order.

Chapters are written as the project firms up; each starts with its status (draft, usable, stable).

## Chapters

| Chapter | Status | What it covers |
|---------|--------|----------------|
| [01-introduction.md](01-introduction.md) | draft | Requirements overview, quality goals and stakeholders |
| [02-constraints.md](02-constraints.md) | draft | Technical and organizational constraints, conventions |
| [03-context.md](03-context.md) | draft | Context view and the foreign systems: Claude Code, git, Neovim, Atlassian remote MCP, Kubernetes, Terraform files |
| [04-solution-strategy.md](04-solution-strategy.md) | draft | Technology, structure, how the quality goals are reached, open questions |
| [05-building-blocks.md](05-building-blocks.md) | draft | Cellworks and its neighbours, the crates, level 3 |
| [06-runtime.md](06-runtime.md) | draft | Opening a project, moving through layers with the keyboard, a session working in a project |
| [08-crosscutting.md](08-crosscutting.md) | draft | Concurrency, error handling, persistence, security, paths, test strategy |
| [09-decisions.md](09-decisions.md) | usable | Index of the architecture decision records in `docs/adr/` |
| [10-quality.md](10-quality.md) | draft | Quality scenarios for usability, performance, extensibility and the lower-priority goals |
