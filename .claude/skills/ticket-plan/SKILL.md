---
name: ticket-plan
description: Plan the fix for a ticket Cellworks has ingested - create the fix worktree and branch, investigate there, and write the approach, steps, systems and lanes into the case file. Changes no code. Use when Cellworks' PLAN button starts a session, or for "/ticket-plan <KEY>".
allowed-tools: Read, Grep, Glob, Bash, Write, mcp__atlassian__getJiraIssue, mcp__plugin_productivity_atlassian__getJiraIssue
---

# Plan the fix for a Cellworks case

Input: the issue key, the case file, and the worktree, branch and base branch
Cellworks chose (also in `repos[0]` of the case file, status `planning`).
Output: the same case file, planned, as the `ticket-case` skill says. Read
that skill first.

## Rules

- Jira is read-only.
- Create exactly one worktree: `git worktree add <worktree> -b <branch>
  <base_branch>` in `repos[0].root`. If it exists already, reuse it. Never
  push, never change another branch.
- Change no code. The plan is written into the case file only.

## Procedure

1. Read the case file: the problem, the findings and the open questions are
   what the plan must answer.
2. Create the worktree, then read the code there. Line numbers in the plan
   are of the files in that worktree.
3. Design the fix as 2 to 6 ordered steps, each one change a person can
   review (a script, a secret, a manifest, a revocation). For each step fill
   `id`, `title`, `summary`, `where`, `spots`, `changes`, `excerpts`, `risks`
   as `ticket-case` describes.
4. Write `approach`: the fix in two to four plain sentences, what changes and
   in which order. It is the first thing the user reads before approving.
5. Add the `systems` and the `lanes` that tell the story: the dangerous path
   today, the clean path after.
6. Answer the open questions you can; leave the rest in `open_questions` and
   turn each that affects a step into a risk on it.
7. Set `status` to `planned`, append `{"status": "planned", "at": "<now>"}`
   to `history`, write the file and check it parses.
8. Reply with the path and the approach. The user approves in Cellworks.
