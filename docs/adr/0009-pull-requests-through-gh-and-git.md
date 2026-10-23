# ADR-0009: Read pull requests through gh and git

## Status
Accepted

## Date
2026-10-19

## Context
Review mode (EPIC-012) needs a project's pull requests, their diffs placed on the city, and the critical-change flags
they raise. GitHub is the forge, and ADR-0003 applies: tools run as child processes behind a small adapter, with the
user's own configuration and authentication. The prototype tried `gh pr diff` for the diffs and it failed on a real
promotion pull request: the forge refuses diffs above 20,000 lines (HTTP 406 on #113), and one call per pull request
took 22 s instead of 2 s for 15 pull requests.

## Decision
We will list pull requests through read-only `gh pr list` and read their diffs through git.
- Listing runs `gh pr list` twice, for the open pull requests and for the last 30 merged ones, with a tab-separated
  `--template`; no JSON dependency is added and lines that do not parse are ignored.
- A diff is read with `git fetch origin pull/<n>/head`, only when the commit is absent, then
  `git diff -U0 <base>...<head>`.
- Lines are placed on the city at the pull request's head: the head is checked out into a temporary detached worktree,
  mapped by CodeAnalysis, and the worktree is removed afterwards.
- Critical-change flags are read from the project's `sdlc/critical.yaml`.

## Consequences

### Positive
- No diff size limit; one fetch and one `git diff` per opened pull request.
- Line numbers on the city are exact, because the city is built at the commit the diff describes.
- No token handling in Cellworks; `gh` uses the user's own login.

### Negative
- `gh` must be installed and logged in, and `origin` must be a GitHub repository; otherwise only this source fails and
  says why.
- Fetching writes objects and `FETCH_HEAD` into the user's repository. It creates no branch and moves no ref the user
  owns.
- Each opened pull request costs one temporary worktree, created and removed on a background thread.
- The flags stay in step with the Factory's `critical.sh` only as long as `critical.yaml` keeps its simple shape (one
  flag per line, one quoted glob per list entry).

### Neutral
- Diffs and worktrees are the git adapter's job; the forge adapter only lists.

## Alternatives considered
- **`gh pr diff`**: rejected because the forge refuses diffs above 20,000 lines (HTTP 406 on a real promotion pull
  request) and it needs one call per pull request (22 s versus 2 s for 15 pull requests).
- **GitHub REST through `curl` or a client crate**: rejected because it makes Cellworks handle a token, against
  ADR-0003, where `gh` already holds the user's login.
- **Mapping onto the open checkout's city**: rejected because line numbers drift wherever the checkout differs from the
  pull request's head.

## References
- [ADR-0003](0003-external-tools-as-child-processes.md)
- arc42: [03 Context](../arc42/03-context.md), [05 Building blocks](../arc42/05-building-blocks.md)
