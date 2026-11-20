---
name: ticket-case
description: The schema of the case file Cellworks renders for one ticket, field by field, and who writes which part across ingest, plan, approve and fix. Read by the ticket-ingest, ticket-plan and ticket-fix skills, and by the Factory and Jarvis planning sessions Cellworks starts.
---

# The Cellworks case file

One JSON file per ticket at `~/.cellworks/cases/<KEY>.json` (or
`$CELLWORKS_CASES/<KEY>.json`). Cellworks watches it and redraws the case as
soon as it changes.

## Rules for every writer

- **Jira is read-only.** Never comment, transition, edit, link or log work.
- Never put secrets into the case file: no passwords, tokens, connection
  strings with credentials. Name a secret by its path and key, never its value.
- The file must parse. Unknown fields are rejected, so write exactly the
  fields below. Read the file, change only your part, write it to a
  temporary name in the same folder and rename it into place, so Cellworks
  never reads half a file. Check it with `python3 -m json.tool`.
- Keep `history`: append, never rewrite. Cellworks appends its own entries
  when the user presses PLAN, APPROVE and FIX.

## The lifecycle

| status | written by | means |
|---|---|---|
| `ingested` | ticket-ingest | the problem side is filled: `problem`, `open_questions`, `repos` (root only) |
| `planning` | Cellworks, on PLAN | a plan session runs in `repos[0].worktree` |
| `planned` | the plan session | `approach`, `steps`, `systems`, `lanes` are filled |
| `approved` | Cellworks, on APPROVE | the user approved the plan; `note` holds what they added |
| `fixing` | Cellworks, on FIX | a fix session runs in the worktree |
| `fixed` | the fix session | the fix is committed on `branch`, not pushed |

## The schema, by screen

Cellworks has three screens per ticket. The Jira ticket itself (summary,
description, criteria, people) comes from Jira, not from this file. The file
carries what the investigation found.

### Top level

| field | type | used for |
|---|---|---|
| `key` | string | must equal the issue key |
| `repos` | array | the repositories the fix touches; Cellworks opens the map of one at a time |
| `written` | string | "from case by Claude, <written>" under the stage 1 heading; use the date, `YYYY-MM-DD` |
| `problem` | object | stage 1 |
| `status` | string | where the case stands: `ingested`, `planning`, `planned`, `approved`, `fixing`, `fixed` |
| `history` | array of `{status, at, note?}` | every status the case went through, oldest first; `at` is `YYYY-MM-DD HH:MM` |
| `open_questions` | array of strings | what the ticket and the code leave open; shown as Q1, Q2... under the problem cards |
| `approach` | string | CLAUDE'S FIX: the fix in two to four sentences, written by plan. Empty until planned |
| `systems` | array | the outside systems on the map |
| `steps` | array | stage 2's step list and stage 3's tabs; empty until planned |
| `lanes` | array | the connections drawn on the map, today and after |

### The repositories (`repos`)

`repos[]`: `{name, root, worktree?, branch?, base_branch?}`. `name` is a short
handle file spots use (the folder name is a good one); `root` the repository's
root folder (`~` is allowed); `worktree` the folder of the worktree the fix is
made in; `branch` the fix branch checked out there; `base_branch` the branch it
starts from. Cellworks writes `worktree`, `branch` and `base_branch` of the
first repository when the user presses PLAN; the plan session creates that
worktree. Cellworks opens the map at `worktree` once it exists, else at `root`.
Once the case is `fixing`, stage 3 shows `git diff <base_branch>...<branch>`
for each step's files instead of the planned excerpts. List only repositories with code the fix
changes; folders read for evidence (docs, notes) stay outside files.

### Stage 1: the problem (`problem`)

| field | type | shown as |
|---|---|---|
| `headline` | string | the big heading. One sentence, plain words, no ticket jargon. "Two services log in to the shared database as root." |
| `plain` | string | the paragraph under it: why it matters, 2 to 4 sentences |
| `problems` | array of `{id, title, where}` | the P1, P2... cards. `id` "P1"; `title` the finding; `where` a short mono line naming where it is (services, environments) |
| `today_chain` | array of `{kind, label, danger?}` | the "TODAY" strip, left to right: how the thing flows today. `kind` picks the glyph (see kinds); `danger: true` paints that link red. 3 to 5 links |
| `criteria_notes` | array of strings | one plain-words line under each Jira acceptance criterion, **in the same order as the criteria in the ticket**. When the ticket has no criteria, these lines are the criteria |

### Stage 2: the fix in place (`approach`, `steps`, `systems`, `lanes`)

Stage 2 shows the fix on the map of the code: the files the steps touch are
highlighted where they live, outside systems (secrets, roles, databases,
workloads) sit in a diagram beside the map, and lanes connect them.

`steps[]`:

| field | type | shown as |
|---|---|---|
| `id` | string | "S1", "S2"... The tab label in stage 3 |
| `title` | string | the step card's title |
| `summary` | string | the card's text, 1 to 2 sentences |
| `where` | string | the card's mono footer: files and environments, short |
| `spots` | array of spots | what lights up on the map when the step is hovered or open |
| `changes` | array of `{what, before, after}` | stage 3 "WHAT CHANGES": one card per change, `before` in red, `after` in green. Short; code, not prose |
| `excerpts` | array | stage 3 code panel (see below) |
| `risks` | array of `{level, tag, text}` | stage 3 "RISKS AND OPEN QUESTIONS" |
| `open_cmd_line` | number, optional | the line OPEN IN NVIM jumps to; the first hunk's start otherwise |

A **spot** is one of:

```json
{"file": {"repo": "backend", "path": "services/a/deployment.yaml", "lines": [28, 28]}}
{"file": {"repo_root": "~/Projects/x/Docs", "path": "notes/roles.sql"}}
{"system": "k_pc"}
```

`repo` names a `repos[].name`; `path` is relative to that repository's
worktree (or root) and may name a folder; `lines` is optional. A file outside
every listed repository gives its folder as `repo_root` instead. A file spot in
a listed repository lights its real building on that repository's map; an
outside file is drawn in the outside-systems diagram. A system spot names a
`systems[].id`.

`systems[]`: `{id, kind, label, group?}`. `id` is a short stable handle used
by spots and lanes. `kind` is one of `vault`, `key` (a secret path), `workload`
(a pod, job, init container), `service`, `queue`, `role` (a database or cloud
role), `database`, `doc`. `label` is what the diagram prints: the real name
(`shop/payments`, `db-orders`). `group` is free text for
grouping, e.g. `vault`.

`lanes[]`: `{from, to, when, kind}`. `from` and `to` are spots. `when` is
`today`, `after` or `both`: the TODAY / AFTER THE FIX toggle shows `today` +
`both`, or `after` + `both`. `kind`:

- `root`: logs in as the administrator (drawn red),
- `own_role`: logs in as a role of its own (drawn cyan),
- `defined_by`: is defined by a script or manifest (dashed violet),
- `reads`: reads a secret or config (dashed grey).

Draw the lanes that tell the story, not every edge: the dangerous path today,
the clean path after, and what defines each part.

### Stage 3: the evidence (`excerpts`, `changes`, `risks`)

`excerpts[]`: `{repo | repo_root, path, hunks, inserted}`, the file named as
in a file spot. Cellworks reads the lines
from disk itself; the file only says which lines.

- `hunks[]`: `{start, end, marks}`, inclusive 1-based line numbers. `marks`
  maps a line number (as a string) to `hit` (the line that matters, yellow),
  `del` (the fix removes it, red), `add` (the fix adds it, green) or `ctx`.
  Unmarked lines are context. Cellworks puts "..." between hunks.
- `inserted[]`: `{after_line, text, mark}`: lines the fix adds that the file
  does not have yet, shown after `after_line` (0: before the first hunk).
  `mark` defaults to `add`; give the full line with its indentation.
- The first excerpt's file is the heading of the code panel; more excerpts
  follow under their own file headings.

### Picking spots and excerpts

- Open the file and **read the exact lines** before writing a number. Line
  numbers are of the file as it is on disk now, on the branch checked out.
- A hunk is the smallest window that shows the change in context: the line
  that changes plus 3 to 8 lines that make it readable (the enclosing block's
  name, the key above it). Two short hunks beat one long one.
- Mark the line that changes `del` and add its replacement as an `inserted`
  line right after it. Mark a line `hit` when it is evidence but does not
  change (the place where root is used, the grant that is missing).
- When a step changes the same thing in several files (dev and prod), excerpt
  one and name the others in a `changes` card and a `warn` risk; spot all of
  them.
- When the fix is a new file (a SQL script), point the excerpt at the file in
  the repository that will hold it if it exists already, or at a draft in the
  docs repository; otherwise show it with `inserted` lines after line 0.

### Deriving risks

Every open question from the ticket, its comments or your investigation that
affects a step becomes a risk on that step:

- `danger`: doing the step in the wrong order or without the answer breaks
  something (a pod that never starts, a service that loses access). Put the
  question's id in the tag when it has one: `"OQ8 · MERGE ORDER"`.
- `warn`: easy to get half right (two manifests, two environments, a manual
  step).
- `info`: what deliberately does not change, and why.

Write the risk as what happens and what to do: "Merge only after S2 has added
the key in Vault. Without it the init container fails and the pod never
starts." Tags are short upper-case labels.

## Example

`example-SHOP-42.json` next to this file is a planned case: three steps,
systems for the checkout pod, the orders database, the payment secret and the
payments API, and lanes for today and after. Use it as the shape to follow, not as
content.
