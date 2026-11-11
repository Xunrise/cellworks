# ADR-0008: Per-line provenance from git blame, joined to pull requests and stories by commit message

## Status
Accepted

## Date
2026-10-19

## Context
The city explains why it was built (EPIC-011): a tower lists the stories behind its floors, a replay builds the city
commit by commit, and a review marks the floors a pull request wrote (EPIC-012). All of them need, for every line of
the opened project, the commit that last wrote it, and for that commit its pull request and stories. git is already
run as a child process (ADR-0003) and there is no async runtime. Pull requests on this repository are squash-merged,
so one commit on the branch stands for one pull request, and its subject ends in `(#N)`. Epic pull requests squash
several stories into one commit and name them all in the message.

## Decision
We will read per-line provenance locally with the git CLI and join it to pull requests and stories by the commit
message alone. While a project loads, `git log --reverse` lists its commits oldest first and `git blame --porcelain`
gives the commit that last wrote each line of every visible file, spread over a few threads. A commit's pull request
is the last `(#N)` of its subject; its stories are every `STORY-NNN` its message names, in any case. A line carries
all the stories of its commit. A line not yet committed has no commit. Nothing is fetched from GitHub.

## Consequences

### Positive
- Works offline and on any clone; no credentials, no rate limits.
- One-story pull requests, the Factory flow, credit exactly one story.
- The map carries the commits in replay order, so replay needs no second read.

### Negative
- An epic pull request credits all of its stories on every line it wrote; shares across them are an estimate.
- Commits merged without `(#N)` or without a story id in the message have no pull request or story.
- Blame cost grows with the number of files (about 1 s for 220 files on 8 threads).

### Neutral
- Pull request titles, bodies and URLs come from the pull-request source (STORY-021), not from here.
- Blame follows git's default rename detection; lines moved between files show the commit that moved them.

## Alternatives considered
- **Ask GitHub for each pull request's own commits**: one story per line even for epic pull requests, but needs the
  network, is slower, and overlaps the pull-request source of EPIC-012.
- **Credit only the first story a commit names**: simple, and wrong for epic pull requests.
- **Fetch pull request titles and URLs with `gh` here**: belongs to the pull-request source (STORY-021).

## References
- Discovery: [the living city](../discovery/2026-10-10-living-city-design.md), "Why it is built this way"
- Prototype: `proto/line-history`, `sdlc/prototype.md` (STORY-018)
- Domain: [CodeAnalysis](../domain/contexts/CodeAnalysis/CodeAnalysis.md)
- Related: ADR-0003
