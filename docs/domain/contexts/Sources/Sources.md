# Context: Sources

Loaders for project types that are not code: Jira, infrastructure and data structures, and the pull requests of a project. Each turns an external system into nodes and lanes for the map.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Ticket and ops persona
- Data persona
- Atlassian remote MCP server
- GitHub through the `gh` CLI (read-only), git
- Terraform, Kubernetes (read-only)
- Knex migrations, Mongo, proto, DTOs

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Jira: Cellworks is its own OAuth client of Atlassian's remote MCP server (dynamic registration plus PKCE). Components are districts, epics groups, tickets nodes coloured by status, sub-tasks items, Jira links lanes. Done tickets older than 30 days are hidden; tickets are cached and ⌘R resyncs (L123-127). The MCP answers in two page shapes; OAuth callback parameters are decoded once (L237-238). Relaying through a headless Claude session was rejected as too expensive (L247).
- Infrastructure (OTC): Terraform blocks per environment plus live workloads via read-only `kubectl get`, coloured by health, wired by `*_SERVICE_HOST`, ingress → services → databases; workloads link to their code and manifest (L128-132).
- Data structures: Postgres tables by replaying Knex migrations with foreign keys as lanes; Mongo collections; proto messages and services and DTOs with field-type lanes; service usage groups the main database's tables by service, "shared" or "not found in services" (L133-139).
- Pull requests: a pull request implements at most one story, found as `STORY-NNN` in its title, else in its head branch. The list holds every open pull request and the last 30 merged ones; closed pull requests that were not merged are not listed.
- A pull request listing's size, file count and critical-change flags come from the listing itself, not a diff read for every row (STORY-022).
- A story's text — its title, As a, I want and So that — comes from the issue of the same repository whose title starts with the story's id (STORY-022).
