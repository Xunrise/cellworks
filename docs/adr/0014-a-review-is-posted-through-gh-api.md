# ADR-0014: A review is posted through gh api

## Status
Accepted

## Date
2026-10-20

## Context
STORY-025 sends a review's verdict and line comments to the pull request's author. ADR-0009 allowed read-only `gh`
commands only, and ADR-0013 kept approval local, so nothing Cellworks did in a review reached GitHub. A verdict the
author never sees has no effect on the pull request, and line comments need a call that `gh pr review` does not offer.

## Decision
We will post a verdict as one GitHub review through the `gh` CLI.
- A verdict (`REQUEST_CHANGES` or `APPROVE`) is posted once per review with
  `gh api repos/{owner}/{repo}/pulls/<n>/reviews --method POST --input -`. The JSON goes on stdin and never in
  arguments; it holds `commit_id` (the pull request's head), `event`, `body` and `comments`.
- A comment on a line the pull request adds is a line comment (`side` RIGHT). Any other comment goes into the body with
  its file and line.
- The body of a `REQUEST_CHANGES` review starts with "Changes requested."
- The post sits behind the Sources forge adapter and runs on a background thread, as ADR-0003 asks.
- Approval counts once the post has succeeded.
- This is the only write Cellworks makes to the forge.

## Consequences

### Positive
- The author receives the verdict and every comment in one place, as one notification.
- The post uses the engineer's own `gh` authentication; Cellworks handles no token.

### Negative
- Cellworks writes to GitHub under the engineer's login.
- GitHub refuses a verdict on one's own pull request (422); the refusal is shown as the failure of the post.
- A posted review cannot be withdrawn from Cellworks.

### Neutral
- ADR-0009's read-only rule for `gh` now has this one exception.

## Alternatives considered
- **`gh pr review`**: rejected because it takes no line comments.
- **A REST client or curl**: rejected because Cellworks would have to handle the token (ADR-0003).
- **One post per comment**: rejected because it sends many notifications and can fail partway.
- **Keeping the verdict local**: rejected because the author never learns it.

## References
- [ADR-0003](0003-external-tools-as-child-processes.md)
- [ADR-0009](0009-pull-requests-through-gh-and-git.md)
- [ADR-0013](0013-the-before-side-of-a-review-is-the-merge-bases-own-city.md)
- arc42: [03 Context](../arc42/03-context.md), [05 Building blocks](../arc42/05-building-blocks.md)
