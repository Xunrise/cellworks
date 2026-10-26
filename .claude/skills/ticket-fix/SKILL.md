---
name: ticket-fix
description: Implement the approved plan of a Cellworks case in its worktree, commit locally, and mark the case fixed. Never pushes. Use when Cellworks' FIX button starts a session, or for "/ticket-fix <KEY>".
allowed-tools: Read, Grep, Glob, Bash, Edit, Write
---

# Fix a Cellworks case

Input: the issue key and the case file, status `fixing`, whose plan the user
approved (the `approved` entry in `history`, with any note they added).
Output: commits on `repos[].branch` in the worktree, and the case file set to
`fixed`, as the `ticket-case` skill says. Read that skill first.

## Rules

- Work only in the worktree and on the branch the case file names.
- Commit locally. **Never push**, never open a pull request, never touch Jira.
- Follow the plan. When the code proves a step wrong, stop, write what you
  found into `open_questions`, leave the status at `fixing` and say so.
- Never commit secrets. A step that needs a secret value (Vault, a password)
  is done by a person: name it in the reply instead.

## Procedure

1. Read the case file: `approach`, the steps in order, their risks, and the
   approval note in `history`.
2. For each step, make the change in the worktree, run the repository's own
   checks if it has any, and commit it on its own with the step id and the
   ticket key in the message (`SHOP-42 S1: ...`).
3. Leave `excerpts` as they are: once the case is `fixing`, Cellworks shows
   `git diff <base_branch>...<branch>` for each step's files.
4. Set `status` to `fixed`, append `{"status": "fixed", "at": "<now>",
   "note": "<commits, and any manual steps left>"}` to `history`, write the
   file and check it parses.
5. Reply with the commits and the manual steps left. Pushing is the user's.
