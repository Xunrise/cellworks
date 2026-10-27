# Sources — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| Base branch | The branch a case's prototype was cut from (`main` unless the case file names another); a case's Review compares against its merge base. | STORY-087 freeze |  |
| Critical-change flag | A name in the project's `sdlc/critical.yaml` that a changed path raises by matching one of its globs. | living-city-design |  |
| Freeze | Ending a case's prototyping: the case is marked prototyped and the prototype's session is asked to plan from what it changed and the feedback log. | STORY-087 freeze | on a Factory project it runs `/factory:freeze`, on a Jarvis project `/jarvis-execute:plan-story` |
| Hand-off | What follows a freeze, shown on the FREEZE stage: prototype.md, plan, build on a Factory project; plan, approve, review elsewhere. | STORY-087 freeze |  |
| Pull request | An open or merged request to merge a branch; implements at most one story, found as `STORY-NNN` in its title or branch. | living-city-design |  |
| Review | A change judged against its base, a pull request at its head or a case's worktree since its base branch, with its size, the flags it raises and its added and removed lines placed on towers and floors of the city. | living-city-design | redefined at STORY-087 freeze: a case's REVIEW stage is a Review, watched while the change is built |
| Service usage | Which services use which tables, collections and contracts; groups tables by service, "shared" or "not found in services". | prototype-findings L138-139 |  |
| Sync | Fetching and caching Jira tickets; ⌘R resyncs. | prototype-findings L127 |  |
| Ticket | A Jira issue, coloured by status; done tickets older than 30 days are hidden. | prototype-findings L124-126 |  |
| Wiring | Infrastructure lanes derived from `*_SERVICE_HOST` variables: ingress controller to services, services to databases. | prototype-findings L131 |  |
| Workload | A live Kubernetes object, coloured by health, linked to its code and manifest. | prototype-findings L128-132 |  |
