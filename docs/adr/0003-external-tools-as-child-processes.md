# ADR-0003: Integrate external tools as child processes

## Status
Accepted

## Date
2026-10-08

## Context
Cellworks talks to git, kubectl, Neovim and Atlassian's remote MCP over HTTPS (see [chapter 3](../arc42/03-context.md)).
Cluster access must stay read-only and secrets must never appear on a command line
([chapter 2](../arc42/02-constraints.md)). Each tool already has the user's configuration and authentication
(git config, kubeconfigs, the installed `nvim`). The prototype drove all of them as CLIs and that worked.

## Decision
We will integrate git, kubectl, nvim and HTTP (through `curl`) as child processes rather than linking client libraries.
Secrets go to `curl` through stdin config or private temp files, never through arguments.

## Consequences

### Positive
- The user's own configuration, credentials and tool versions apply without reimplementing them.
- Few heavy dependencies; the binary stays small and builds fast.
- Read-only access is easy to audit: every cluster call is a visible `kubectl get`.

### Negative
- A process spawn per call costs milliseconds; hot paths (git status on 20k files) must batch calls and cache results.
- Output is parsed from text or JSON, so a tool's output changes can break a loader; parsing must be tolerant.
- The tools must be installed; a missing tool degrades only its feature and must say so.

### Neutral
- Each integration sits behind a small adapter in its context crate, so a library can replace a CLI later without
  touching callers.

## Alternatives considered
- **Client libraries** (gix or git2, kube-rs, reqwest): typed APIs without spawning. Rejected because they bring heavy
  dependency trees and handle auth and configuration in-process, which duplicates what the tools already do.
- **Mixed** (libraries only on hot paths): rejected for now; the adapter seam allows it later when measurement shows a
  CLI is too slow.

## References
- arc42: [02 Constraints](../arc42/02-constraints.md), [03 Context](../arc42/03-context.md),
  [04 Solution strategy](../arc42/04-solution-strategy.md)
