---
name: ticket-ingest
description: Ingest one Jira ticket into a Cellworks case file - the problem side only (headline, findings, today chain, criteria in plain words, open questions, the repositories involved). Plans nothing. Use for "/ticket-ingest <KEY>", or when Cellworks' INGEST button starts a session.
allowed-tools: Read, Grep, Glob, Bash, Write, mcp__atlassian__getJiraIssue, mcp__atlassian__searchJiraIssuesUsingJql, mcp__plugin_productivity_atlassian__getJiraIssue, mcp__plugin_productivity_atlassian__searchJiraIssuesUsingJql
---

# Ingest a ticket for Cellworks

Input: an issue key and the case-file path to write. Output: a new case file
with `status: "ingested"`, written as the `ticket-case` skill says. Read that
skill first; it holds the schema and the rules every writer follows.

Ingest states the problem. It does not design the fix: no `approach`, no
`steps`, no `systems`, no `lanes`. Those belong to the plan, which the user
starts separately.

The session starts in the workspace root: the folder that holds all the
projects Cellworks knows, not one repository.

## Rules

- Jira is read-only: `getJiraIssue` and `searchJiraIssuesUsingJql` only.
- Change nothing in any repository. Read, grep, `git log`, `git show`.
- No worktrees, no branches.

## Procedure

1. Read the ticket: summary, description, acceptance criteria, comments,
   linked tickets that explain it.
2. Find the code it is about: follow names from the ticket (services,
   secrets, tables, roles) with `grep` across the repositories below the
   workspace root. Decide which repositories a fix would touch; list them in
   `repos` with `name` and `root` only.
3. Establish **today**: what runs, how it reaches what, and why that is the
   problem. Every claim points at a file and line you have read. Fill
   `problem.headline`, `plain`, `problems`, `today_chain`, `criteria_notes`.
4. Collect what is still open: questions the ticket, its comments or the code
   leave unanswered go into `open_questions`, one plain sentence each.
5. Write the file with `key`, `repos`, `written` (today's date), `status:
   "ingested"`, `history: [{"status": "ingested", "at": "<now>"}]`, `problem`
   and `open_questions`. Check it parses.
6. Reply with the path and a two-line summary of the problem. Do not paste
   the file.
