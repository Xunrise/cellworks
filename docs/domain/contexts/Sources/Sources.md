# Context: Sources

Loaders for project types that are not code: Jira, infrastructure and data structures. Each turns an external system into nodes and lanes for the map.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Ticket and ops persona
- Data persona
- Atlassian remote MCP server
- Terraform, Kubernetes (read-only)
- Knex migrations, Mongo, proto, DTOs

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Jira: Cellworks is its own OAuth client of Atlassian's remote MCP server (dynamic registration plus PKCE). Components are districts, epics groups, tickets nodes coloured by status, sub-tasks items, Jira links lanes. Done tickets older than 30 days are hidden; tickets are cached and ⌘R resyncs (L123-127). The MCP answers in two page shapes; OAuth callback parameters are decoded once (L237-238). Relaying through a headless Claude session was rejected as too expensive (L247).
- Infrastructure (OTC): Terraform blocks per environment plus live workloads via read-only `kubectl get`, coloured by health, wired by `*_SERVICE_HOST`, ingress → services → databases; workloads link to their code and manifest (L128-132).
- Data structures: Postgres tables by replaying Knex migrations with foreign keys as lanes; Mongo collections; proto messages and services and DTOs with field-type lanes; service usage groups the main database's tables by service, "shared" or "not found in services" (L133-139).

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
