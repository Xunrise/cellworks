# ADR-0016: Tickets share one hub (Cockpit) and one work folder (.work/<KEY>/)

## Status
Accepted

## Date
2026-11-20

## Context
Three tools of the same owner track tickets, each its own way:

- **Cellworks** lists tickets from Jira, Cockpit, GitHub and Linear (`crates/sources/src/tickets/`) and keeps a case per
  ticket outside the repository, in `~/.cellworks/cases/<KEY>.json` (problem, findings P1…, systems, steps, rounds).
- **wishlang** takes the ticket key from the branch name (`feature|fix|chore/<KEY>-…`, `proto/<KEY>`), reads notes from
  `<main checkout>/.work/<KEY>/` (`intent.md`, `spec.md`), distils `problems.json` (P1…) and tags every generated item
  with the problems it serves (journal `[P1,P3]: why`, commit trailer `Ticket: <KEY>`).
- **Cockpit** is a local tracker with a daemon (HTTP on 127.0.0.1:4747, SQLite): its own tickets (`TIC-2`), Claude
  sessions that claim them through hooks, links, and `ticket.for_change` (ticket of a branch or session).

They overlap: two problem lists in two formats, two places for a ticket's notes, three ways to find a branch's ticket,
two trackers of which session works on what. The three are meant to become one product. Cellworks must stay useful on
its own, without Cockpit installed.

## Decision
We will give a ticket one identity, one work folder in the repository, and one optional hub.

### 1. Identity
- A ticket is `<source>:<key>`, `source` one of `jira`, `github`, `linear`, `cockpit` (Cellworks' `TicketRef`). A bare
  key means Jira, as today.
- Its **folder key** is the key with every character outside `[A-Za-z0-9-]` replaced by `-` (`owner/repo#12` →
  `owner-repo-12`). Branches, `.work/` folders and commit trailers use the folder key.
- Every tool finds a branch's ticket the same way: `<type>/<FOLDERKEY>-<slug>` or `proto/<FOLDERKEY>`, then the
  `Ticket:` trailer of the branch's commits, then (with Cockpit) the session's claim.

### 2. The work folder: `<main checkout>/.work/<FOLDERKEY>/`
The notes of a ticket live with the code, on the branch, in one format every tool reads:

| File | Owner | Content |
|---|---|---|
| `ticket.json` | whoever opens the ticket first | `{ "ref": "jira:SHOP-42", "title", "url", "status", "category", "updated", "repos": [...] }`: a snapshot so every tool works offline |
| `intent.md`, `spec.md` | the engineer (wishlang reads) | why and what, as wishlang has them today |
| `problems.json` | shared | `[{ "id": "P1", "text": "…", "where": ["src/cart/totals.ts"], "scenarios": ["…"] }]`. wishlang's `{id, text, scenarios}` plus Cellworks' `where`; Cellworks' `title` becomes `text` |
| `case.json` | Cellworks | the rest of today's case file: headline, plain words, systems, today chain, steps, risks, rounds, status history |
| `plan.md` | the planning session | the approved plan, as Markdown |

Tools ignore files they do not know. Existing Cellworks cases in `~/.cellworks/cases/` are migrated on
first open (moved into the repo's `.work/` when the case names a repository, kept where they are otherwise).

### 3. The hub: Cockpit when present, never required
- **With Cockpit** (its daemon answers, or the `cockpit` binary is on PATH): Cockpit is the record of every ticket.
  It keeps its own tickets and mirrors Jira, GitHub and Linear tickets as linked rows (`source`, `external key`,
  `url`). It also records which session claims which ticket and the links to branches, commits, cases and wishlang
  journals, and it indexes the `.work/` folders of its registered repositories. Cellworks reads and writes tickets
  through Cockpit's API, and so does wishlang (problems, coverage). External trackers are reached through Cockpit.
- **Without Cockpit**: Cellworks keeps its direct Jira, GitHub and Linear adapters and the `.work/` folder, so tickets,
  cases and problems work. Session claims and the cross-repo view are the features Cockpit adds.
- **Onboarding**: the Cellworks settings window detects Cockpit. When it is missing it offers one step to install and
  start it, and then hands over: the direct adapters' sign-ins and caches are imported into Cockpit, and Cellworks
  switches to the hub.

### 4. API Cockpit adds
- `ticket.list` (filters: source, status, project, repo, assigned to me)
- `ticket.mirror` (upsert an external ticket)
- `problems.get` / `problems.set` (a ticket's `problems.json`)
- `work.index` (the `.work/` folders of a repository)
- `ticket.coverage` (wishlang: items per problem)

These go through the existing tool registry and HTTP API (`POST /api/tools/<name>`, header `x-tk: 1`), with the
`cockpit tool NAME '{json}'` fallback.

## Consequences

### Positive
- One problem list per ticket, versioned with the code. Cellworks shows wishlang's coverage on the map ("which items
  serve P2"), and wishlang tags against the problems a Cellworks case found.
- One place answers "which session works on which ticket", for Cellworks' units and wishlang's hooks.
- Cellworks still installs and works alone; Cockpit is an upgrade, not a dependency.

### Negative
- Two code paths in Cellworks (hub and direct) until the direct adapters can be retired.
- `.work/` folders appear in repositories; teams decide per repository whether to commit them or add `.work/` to
  `.gitignore` (Cellworks never commits them by itself).
- Cockpit grows external-tracker code (OAuth for Jira, API keys for Linear) that today lives in Cellworks.

### Order of work
1. Cockpit: the five tools above, mirroring, `.work/` indexing.
2. wishlang: `problems.json` with the optional `where`; report coverage to Cockpit when it is there.
3. Cellworks: case file → `.work/<FOLDERKEY>/` (with migration), hub-first ticket source with direct fallback, the
   onboarding in settings. After the running editor and agent-unit branches are merged.
