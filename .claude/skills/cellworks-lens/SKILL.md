---
name: cellworks-lens
description: Create or refine a Cellworks project lens — a set of rules that decides how a project's files are grouped, what lies underground, and which relations are drawn on the Cellworks map. Use when asked for a new lens, or when given feedback on one.
---

# Cellworks project lenses

A lens answers one question about the project ("what tests what?", "where does a request flow?") by arranging the
project's files on the Cellworks map. You write the lens's **definition**; Cellworks builds its **data** and draws it.
The developer watches the map redraw as you work, so build early and often.

## Files

- `.cellworks/lenses/<name>.lens.json`: the definition. You write it.
- `.cellworks/lenses/<name>.data.json`: the data. Only `cellworks lens build` writes it. Never edit it by hand.

## Tools

The path of the `cellworks` binary is given in the prompt. Run it with Bash:

- `<cellworks> facts <repo> files`: every file, its folder district, its kind, and how many items and test items it
  holds.
- `<cellworks> facts <repo> items`: every function, type and section, with its line range.
- `<cellworks> facts <repo> calls|imports|cochange`: file-to-file links, weighted. `calls` and `imports` are known
  for Rust only. `cochange` comes from git and works for every language.
- `<cellworks> lens build <repo> <name>`: applies the definition, writes the data, and prints a report. The report
  gives the districts, relation counts, and `unmatched`.

The facts output is large. Pipe it through `python3 -c` or `jq` rather than printing it whole.

## Definition format

```json
{
  "name": "tests",
  "question": "Which tests cover which class?",
  "cut": { "split_files": false },
  "districts": [ { "match": "^backend/src/(main|test)/kotlin/com/acme/(\\w+)/", "as": "$2" } ],
  "blocks":    [ { "match": "/(domain|application|infrastructure)/", "as": "$1" } ],
  "underground": [ "^backend/src/test/", "\\.spec\\.tsx?$" ],
  "relations": [
    { "kind": "tests", "viz": "cellar", "pairs": [
        { "from": "^(.*)/src/test/(.*)Test\\.kt$", "to": "$1/src/main/${2}.kt" },
        { "from": "^(.*)\\.spec\\.(tsx?)$",        "to": "$1.$2" } ] },
    { "kind": "changes-with", "viz": "cluster", "source": "cochange", "min_weight": 0.5 }
  ],
  "overrides": {
    "place": { "backend/src/main/kotlin/com/acme/App.kt": { "district": "app" } },
    "links": [ { "kind": "tests", "from": "backend/src/test/kotlin/com/acme/a/FooNamesTest.kt",
                 "to": "backend/src/main/kotlin/com/acme/a/Foo.kt" } ],
    "underground": { "backend/src/test/kotlin/com/acme/TestFixtures.kt": true }
  }
}
```

- Every pattern is a Rust regex (no look-around: `(?!…)` fails; the build lists it under `unmatched.bad_rules`) over the file's path relative to the repo root. Templates use `$1`, `${2}` or
  `$name`. Write `${2}.kt`, not `$2.kt`, when a capture is followed by a letter, a digit or `_`.
- A pair's `to` may hold `**/`: `src/main/**/${2}.kt` names the shortest file under `src/main/` that ends in
  `/<Name>.kt`. Use it when tests sit in another package or folder than their class.
- A relation with a `source` may set `only_from` (a regex) to keep only the links from matching files, for example
  `"source": "calls", "only_from": "^tests/"` to hang integration tests under the code they call most.
- `hide`: regexes over the path; matching files are left off the map. Hide whatever does not help answer the question
  (docs, specs, infrastructure, generated files). A lens that shows everything answers nothing.
- `districts` and `blocks`: the first rule that matches wins. A file no district rule matches stays in its folder
  district, and is listed under `unmatched.unplaced`.
- `underground`: these files lie below ground. A `cellar` relation then hangs each one beneath the file it names.
- `relations[].viz` sets how the relation is drawn:
  - `cellar`: the `from` file hangs underground beneath the `to` file.
  - `cluster`: districts linked by the relation pull together.
  - `orbit`: the most-linked district goes in the middle, the others round it.
  - `roads`: a labelled road between districts.
  - `lanes`: an arrow between the two files.
  - `none`: not drawn.

  The developer can switch `viz` in the app. Suggest the one that answers the question best.
- `cut.split_files`: set `true` for one tower per function or type, or `false` for one tower per file. Use `false`
  when the question is about files or classes, not functions.
- `overrides`: the edge cases no sensible rule covers, settled once by hand. An override link of a kind replaces that
  kind's rule-made links for its `from` file.

## How to work

1. **Understand the question.** If the question is vague, choose the most useful reading and state it in
   `question`. Do not ask.
2. **Learn the stack.** Run `facts files` and look at the folder layout, the languages, and the test conventions
   (`src/test/` vs `src/main/`, `*.spec.ts`, `test_*.py`, `*Test.kt`, `steps/`, `fixtures/`).
3. **Write rules.** Cover the general case with as few rules as you can. Prefer one rule per convention.
4. **Build:** `lens build`. Then read `unmatched`.
5. **Settle the edge cases.**
   - If a group of misses shares a pattern, add or fix a rule.
   - If a miss is a true one-off, add an override. To find its subject, read the file: its imports, the class
     under test, what it calls.
   - Leave a miss unmatched if it has no subject, such as an architecture test or a fixture. Say so in your summary.
6. **Rebuild** until `unmatched` holds only the misses you left on purpose.
7. **Report** in at most eight lines: what each district is, how tests pair, what you left unmatched and why, and
   the viz you chose.

## On feedback

The developer gives feedback in the app ("put the steps under the use case they drive", "one tower per file",
"group by feature, not by layer"). Change the definition, rebuild, and report what changed. Keep everything that the
feedback did not touch.

## Rules

- Never edit app code or the data file. Change only `.cellworks/lenses/<name>.lens.json`.
- Every lens file is valid JSON. Unknown fields are rejected, so a typo fails the build. Fix it when it does.
