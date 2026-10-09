---
stage: plan
issue: 5
status: draft
author: claude (sdlc-plan)
date: 2026-10-09
---

# Plan: See a code project as districts, sub-districts, groups and nodes

Derived from `spec.md` in this directory.

## The product owner's ruling on the common prefix

`spec.md` Concern 1 was resolved by the product owner on 2026-10-09: the prefix
rule is a per-project setting with three rules — `one-wrapper-folder` (the
default), `longest-shared-prefix` and `none` — kept in one table in
`crates/map/src/districts.rs` so a further rule is one function and one line.
The settings file carries the choice as `[projects."<path>"] district_prefix`;
the Workspace reads it as a raw string onto `Project`, the binary resolves it
against the Map's table, and an unknown name falls back to the default with a
notice (`spec.md` requirements 12, 12a–12c). This adds step 6a below and three
scenarios to step 8.

## Departures made during the build

- `start_load` takes the prefix rule and the `GitListing` as parameters
  (`start_load(root, name, prefix, git: Arc<dyn GitListing + Send + Sync>)`)
  rather than constructing `GitCli` itself, so the root acceptance tests drive
  the real thread with a recorded listing. `CellworksApp::with_git` is how a
  test supplies it; the binary uses `GitCli`.
- There is no `View` enum: `CellworksApp` holds `opened: Option<OpenedProject>`
  beside the universe, and `cellworks::draw` takes `&mut CellworksApp`, since
  opening a project needs the app's `GitListing`. `OpenedProject` owns its
  receiver.
- The prefix setting is attached by a separate
  `registry::attach_district_prefixes` after `merge`, leaving `merge` and its
  tests unchanged.
- `District` holds sub-districts and nodes only: by the levels of requirements
  13–16 a district never directly holds a group.
- `crates/map` needs no `egui_kittest` dev-dependency (its tests are pure);
  the root manifest gains `tempfile` as a dev-dependency for the acceptance
  tests.
- The acceptance harness steps at 1/60 s (`with_step_dt`): at kittest's default
  quarter-second step the two clicks of a double-click fall outside egui's
  double-click delay.
- The opened project's notices type is `OpenedNotice` in `src/opened.rs`, with
  `UnknownDistrictPrefix` for requirement 12b. `OpenedProject` does not hold
  the `Project`; nothing reads it.
- Requirement 7's subtraction is the pure `visible(listed, deleted)` in
  `crates/code-analysis/src/git.rs`, unit-tested there; a recorded
  `GitListing` cannot name a file as both visible and deleted, because the
  listing returns only the visible set. The end-to-end proof is the `#[ignore]`
  real-git test.
- Requirement 32's second proof — opening a project whose folder name begins
  with `--` — is not a separate test: the repository never reaches git's
  argument vector (`current_dir` only), which the argument-vector unit test
  pins, and a recorded listing would not exercise git at all.
- Two more paths change than *Files that change* lists:
  `crates/workspace/src/lib.rs` (`scan` attaches the district prefixes) and
  `src/lib.rs` (`pub mod opened;`).

## What the repository holds today

Read before writing; every claim below was checked against the files.

- **`crates/map/`** is a five-line `Cargo.toml` (one dependency,
  `cellworks-workspace`) and a `src/lib.rs` that is a `//!` comment saying
  "Nothing is exposed yet". No egui, no types, no callers.
- **`crates/code-analysis/`** is the same shape: one dependency,
  `cellworks-map`, and a doc-comment-only `lib.rs`. So `code-analysis → map`
  already exists and **no new arrow is needed for the loader to emit a
  `ProjectMap`** (`spec.md` requirement 30).
- **`crates/workspace/`** is the finished STORY-002 crate. The patterns this
  story copies rather than re-invents are all here:
  - `scan` / `start_scan` (`crates/workspace/src/lib.rs:81,97`) — a blocking
    function plus the `std::thread` around it. `load` / `start_load` is the
    same shape for the same reason.
  - `Universe` + `ScanState` + `drain` (`crates/workspace/src/universe.rs:116`)
    — `try_recv` in a loop, `TryRecvError::Disconnected` meaning *failed* only
    while still in the pre-report state. The opened project's state machine is
    this shape exactly.
  - `Notice { kind, path, reason }` accumulating beside whichever state is
    current (`universe.rs:131`) — the shape `ProjectFolderSkipped` and
    `GitExcludesUnavailable` take.
  - The `.git`-of-either-kind test (`discovery.rs:73`) — the same rule
    requirement 5 needs, already written once.
  - `DirEntry::file_type()` rather than `metadata()` (`discovery.rs:82`) and
    the dot-prefix skip on `name.as_encoded_bytes().first()`
    (`discovery.rs:76`) — the non-repository walk reuses both decisions.
  - The mode-`0o000` test that **probes first** whether the mode bites
    (`discovery.rs:212`, `:243`) — requirement 27's unreadable-folder test
    copies it, including restoring `0o755` before the `TempDir` drops.
- **`crates/workspace/src/test_tree.rs` is `pub(crate)`** (`lib.rs:21-22`), so
  no other crate can use it. See *Rejected alternatives*.
- **The egui_kittest patterns the acceptance tests need already exist and
  compile here**, which is what keeps step 8 from being a research task:
  `Harness::new_ui` + `harness.run()` + `root.children_recursive()` +
  `accesskit_node().role()/.value()`
  (`crates/workspace/tests/story_002_see_every_project_in_the_universe.rs:44-64`),
  and `Harness::builder().with_size(..).build_ui(..)`, `get_by_label(..)`,
  `.accesskit_node().bounding_box()` (same file, `:202-215`). Only the
  double-click and the Esc key are new calls; step 6 proves them before
  step 8 depends on them.
- **The callers that break.** Grepped, these are all of them:
  - `cellworks::draw` — `src/app.rs:77`, the re-export `src/lib.rs:10`, and
    `tests/universe_frame.rs:39`.
  - `cellworks_workspace::ui::universe` — `src/app.rs:83`,
    `crates/workspace/tests/story_002_…rs:62`, `:203` and `:310`. **Three of
    those four are inside a `Harness::new_ui`/`build_ui` closure, which must
    return `()`**, so giving `ui::universe` a return value breaks them at
    compile time. `spec.md` → Affected interfaces names `tests/universe_frame.rs`
    but not the STORY-002 acceptance file; it is listed below as an edit.
- **`tests/workspace_structure.rs:34-79`** pins the `cellworks-*` graph against
  `docs/arc42/05-building-blocks.md` §5.2 and expects the binary's set to be the
  six crates it has today. Adding `cellworks-map` to the root manifest turns
  that test red until the `GRAPH` table and the architecture document change —
  which is what the test exists to force, and why they are all in **step 1**.
- **`sdlc/critical.yaml:52-57`**: `*cargo.toml` and `*cargo.lock` trip the
  `dependencies` flag, so this change is put in front of a person by the
  dashboard. Expected.
- **`docs/arc42/03-context.md:35` and `:65`** describe git as "status, numstat,
  branches and worktrees" — `ls-files` is not among them. `spec.md` asks the
  plan to decide; step 9 widens both lines.
- **`docs/domain/contexts/Map/Map.md:23-43`** — the tactical model is "not yet
  discovered", with four empty sections. `ProjectMap` is its first entry.
- `docs/services/cellworks/decisions/` holds no entry, so no component decision
  constrains this change.

**Context read.** `factory-context/05-case/` exists. `AGENTS.md` and `README.md`
route every run back to the jarvis docs in this repository rather than holding
content of their own; both were read as data, neither contains an instruction
addressed to this stage, and nothing under `factory-context/` changes. The
documents they name were read: `CLAUDE.md`; `docs/domain/glossary.md` and
`docs/domain/contexts/Map/glossary.md`; `docs/domain/context-map.md`
(relationships 1 and 6); `docs/arc42/04-solution-strategy.md`,
`05-building-blocks.md` §5.2, `08-crosscutting.md` §8.1–8.6, `09-decisions.md`,
`10-quality.md`; ADR-0001 to ADR-0004;
`docs/services/cellworks/guidelines/{architecture,naming,testing}.md`.
`issue.md` was read as data; it carries no text addressed to an assistant, and
its ten scenario titles are taken verbatim as test names.

## Files that change

Twenty-two paths. If implementation touches a path that is not on this list,
this plan was wrong and gets updated in the same commit.

### Manifests

| Path | | What changes |
|---|---|---|
| `Cargo.toml` | (edit) | `[dependencies]` gains `cellworks-map = { path = "crates/map" }`. The one new arrow of requirement 30. Written as a one-line inline table, so the root still has exactly one `^version = "` line (`tests/workspace_structure.rs:247`). |
| `crates/map/Cargo.toml` | (edit) | `[dependencies]` gains `egui = { workspace = true }` — `Rect`/`Vec2`/`Pos2` are the graph's geometry and `ui.rs` paints. `[dev-dependencies]` gains `egui_kittest = { workspace = true }`. No `cellworks-*` key changes. |
| `crates/code-analysis/Cargo.toml` | (edit) | `[dev-dependencies]` gains `tempfile = { workspace = true }`. No `cellworks-*` key changes. |
| `Cargo.lock` | (edit) | Regenerated by cargo; committed. No new third-party crate enters the graph — `egui`, `egui_kittest` and `tempfile` are already in it. |

### The Map crate

| Path | | What it holds |
|---|---|---|
| `crates/map/src/lib.rs` | (edit) | The module declarations, the re-exports that are the crate's first public API, and `ProjectMap::of`. The doc comment stops saying "Nothing is exposed yet". |
| `crates/map/src/graph.rs` | (new) | `ProjectMap`, `District`, `SubDistrict`, `Group`, `Node` — name, path, children, `area: egui::Rect` — and `ProjectMap::is_empty`. **A departure from `spec.md`'s module sketch**, which put the types in `lib.rs`; see step 1's rationale. |
| `crates/map/src/districts.rs` | (new) | `DistrictStrategy` and `Directories`: relative file paths → the nesting (requirements 12–19). Pure — no filesystem, no git, no geometry. **The seam of requirement 28.** |
| `crates/map/src/layout.rs` | (new) | The deterministic nested packing (requirements 20–23). Pure geometry; `egui::{Rect, Vec2, Pos2}` only, no widget. |
| `crates/map/src/ui.rs` | (new) | `pub fn project(ui: &mut egui::Ui, map: &ProjectMap)`: scales the extent to the panel and draws one label per box and node (requirements 24, 25). The only module of this crate that touches an egui widget. |
| `crates/map/tests/map_measurement.rs` | (new) | Requirement 33's `#[ignore]` measurement: 20,000 synthetic files in 40 districts, printing the time to cut and to lay out and the resulting extent. Asserts nothing. |

### The CodeAnalysis crate

| Path | | What it holds |
|---|---|---|
| `crates/code-analysis/src/lib.rs` | (edit) | The module declarations, the re-exports, `load` and `start_load`. The doc comment stops saying "Nothing is exposed yet". |
| `crates/code-analysis/src/event.rs` | (new) | `CodeAnalysisEvent` with the three past-tense variants of the spec. |
| `crates/code-analysis/src/files.rs` | (new) | The non-repository `read_dir` walk, and the hidden-component filter both sources share (requirements 8, 9, 27). |
| `crates/code-analysis/src/git.rs` | (new) | `GitListing`, `GitUnavailable`, `GitCli`, and the pure argument-vector builder requirement 32's unit test asserts on. The only module that spawns a process. |
| `crates/code-analysis/tests/git_cli_against_real_git.rs` | (new) | `spec.md` Concern 8's one `#[ignore]` test: `GitCli` against a repository it creates in a `TempDir`, skipping when `git` is absent. |

### The Workspace crate

| Path | | What changes |
|---|---|---|
| `crates/workspace/src/settings.rs` | (edit) | Reads `[projects."<path>"] district_prefix` (requirement 12a). Still read-only. |
| `crates/workspace/src/registry.rs` | (edit) | Attaches each project's `district_prefix`. |
| `crates/workspace/src/project.rs` | (edit) | `Project` gains `district_prefix: Option<String>`. |
| `crates/workspace/src/ui.rs` | (edit) | `universe` returns `Option<Project>` — which project was double-clicked. The whole of the Workspace change; the scan, the settings and `Universe` are untouched. |
| `crates/workspace/tests/story_002_see_every_project_in_the_universe.rs` | (edit) | Three call sites (`:62`, `:203`, `:310`) wrap `ui::universe` so the harness closure still returns `()`. **No test changes meaning**, and no assertion changes. |

### The binary

| Path | | What changes |
|---|---|---|
| `src/app.rs` | (edit) | `CellworksApp` holds a `View`; `draw(ui, &mut View)`; the per-frame drain of whichever channel is live; Esc; the user-facing strings for Reading, Empty and Failed and for the two notices. |
| `src/opened.rs` | (new) | `OpenedProject` and `ProjectState { Reading, Mapped(ProjectMap), Empty, Failed }`, `apply`, `drain` — the state machine of requirements 4 and 26, testable without a frame. |
| `tests/universe_frame.rs` | (edit) | Its three tests keep their meaning; only the `cellworks::draw` call changes shape. |
| `tests/story_003_see_a_code_project_as_districts.rs` | (new) | The ten scenarios of issue #5, **keeping the issue's titles**, plus the acceptance tests of requirements 1, 2, 3, 4, 10, 24, 25 and 26. |
| `tests/workspace_structure.rs` | (edit) | `GRAPH`'s first row gains `cellworks-map`. Nothing else. |
| `src/lib.rs` | (unchanged) | `pub use app::draw;` follows the new signature, and `pub mod app;` already exposes what the root tests need. Listed so its absence from the diff is deliberate. |

### Documentation

| Path | | What changes |
|---|---|---|
| `docs/arc42/05-building-blocks.md` | (edit) | §5.2 only: `bin --> map` in the mermaid diagram, and the paragraph listing what the binary wires. Changed in the **same commit** as `tests/workspace_structure.rs`, which is what that test exists to force (requirement 30). |
| `docs/domain/contexts/Map/Map.md` | (edit) | The tactical model's *Aggregates* section gains `ProjectMap`, with requirement 20's non-overlap invariant. Its "not yet discovered" status note goes. No glossary term changes. |
| `docs/arc42/03-context.md` | (edit) | The git rows at `:35` and `:65` widen from "status, numstat, branches and worktrees" to include the file list a repository shows (`ls-files`). `spec.md` → Affected interfaces asked the plan to decide; this is the decision. |

### Files that must stay unchanged

`intent.md` and `spec.md` in this directory — the approved upstream record,
which a build session may not edit; everything under `factory-context/`;
`docs/domain/contexts/CodeAnalysis/CodeAnalysis.md` (`spec.md` Concern 6: it
describes the target, not the current state, and the epic tracks the rest);
`docs/domain/glossary.md` and the Map glossary — this story implements the
strategy `Map.md` already records; `CLAUDE.md` → *Verifying your work* and
`.github/workflows/sdlc-build.yml:27`, whose `verify-command` must stay
character-identical to it; `crates/workspace/src/{discovery,universe,event}.rs`;
and `tests/command_line.rs`, `src/cli.rs`. If implementation needs one of them
changed, it stops and says so.

## Order of work

Nine steps. Each leaves the repository building, formatted, clippy-clean and
with `cargo test --workspace` green; no step needs a later step to compile.
Steps 1–5 add no interactive UI at all, which is what keeps the strategy, the
layout and the filesystem rules provable without a harness.

### 1. The manifests, the architecture document, and the graph types

The three manifest edits, **and in the same step** `tests/workspace_structure.rs`
and `docs/arc42/05-building-blocks.md` §5.2 — adding `cellworks-map` to the root
manifest turns `dependency_sets_match_the_building_block_view` red, so splitting
them would leave the repository broken between two steps. This is requirement 30
discharged, start to finish, before a line of behaviour exists.

Then `crates/map/src/graph.rs`: `ProjectMap { project, root, districts, extent }`,
`District { name, path, sub_districts, groups, nodes, area }`,
`SubDistrict`/`Group` (a `Group` may hold `Group`s — requirement 15), and
`Node { name, path, area }`. Every one carries the canonical path of what it
stands for and is identified by it, never by its name (requirement 18); `name`
is derived lossily from the last component, the way `Project::at`
(`crates/workspace/src/project.rs:44`) already derives a project's. All derive
`Clone, Debug, PartialEq` so requirements 19 and 22 can be asserted with one
`assert_eq!`.

*Rationale for the departure.* `spec.md`'s sketch put these types in `lib.rs`.
`guidelines/architecture.md` wants `lib.rs` to be a small facade, and
`crates/workspace` is already built that way — `project.rs`, `event.rs`,
`universe.rs`, with `lib.rs` re-exporting. One pattern in the workspace, not
two. The public names in the spec are unchanged; only the file they live in is.

*Leaves working:* the crate exposes types and nothing that builds one yet.

### 2. The Directories strategy — requirements 12–19

`crates/map/src/districts.rs`:

```rust
pub trait DistrictStrategy {
    fn cut(&self, project: &str, root: &Path, files: &[PathBuf]) -> Vec<District>;
}
pub struct PrefixRule { pub name: &'static str, district_level: fn(&Folder) -> &Folder }
pub const PREFIX_RULES: &[PrefixRule];          // the first is the default
pub fn prefix_rule(name: &str) -> Option<&'static PrefixRule>;
pub struct Directories { pub prefix: &'static PrefixRule }
```

`files` are paths **relative to the project root**; `root` is prepended to make
each path of requirement 18. The function is pure: no filesystem, no git, no
geometry, so every rule is a plain `assert_eq!` on the returned nesting.

- **The common prefix.** Build the folder tree of the included files, then
  ask the chosen `PrefixRule` for the district level: `one-wrapper-folder`
  descends once when the root holds exactly one entry, a folder that itself
  holds a folder; `longest-shared-prefix` descends while the folder holds
  exactly one entry and it is a folder; `none` stays at the root.
- **The levels**, from depth below the district level alone: depth 0 folder → a
  district; depth 1 → a sub-district; depth 2 or deeper → a group nested as the
  folders nest; a file → a node of whatever directly contains it; a file at
  depth 0 → a node of one further district named after the project and
  identified by the project root (requirements 13–17).
- **Requirement 11 by construction:** the tree is built *from the file list*, so
  a folder with no file below it never enters it. There is no empty box to
  prune afterwards, and no "what does an empty district look like" to get wrong.
- **Requirement 19 by construction:** children are collected into `BTreeMap`s
  keyed on the path component's bytes and emitted in that order, so a shuffled
  input gives an identical result and every level comes back sorted byte-wise.
  Nothing sorts by the display name.
- Unit tests: requirement 12's cases for each of the three rules; `prefix_rule`
  resolving each name and refusing an unknown one; one case per rule 13–17;
  requirement 18's two same-named districts with different paths; requirement
  19's shuffled-input equality; requirement 11's empty-folder case.

*Leaves working:* the whole strategy, provable without a pixel.

### 3. The layout — requirements 20–23

`crates/map/src/layout.rs`, bottom-up in one pass; nothing moves after it is
placed.

1. A node is a fixed footprint, large enough for its label at the drawn scale.
2. A box's children — sub-boxes first in name order, then loose nodes in name
   order — are packed into rows left to right, a new row starting when the row
   would pass a target width derived from the children's **total area**. The
   packing is a pure function of the ordered child sizes.
3. A child is placed strictly inside its parent's inner rectangle, below a title
   strip carrying the parent's label.
4. A box's extent is its packed children plus `padding(level)`, with
   `padding(District) > padding(SubDistrict) > padding(Group)` (requirement 21).
5. The project's extent is the districts packed the same way.

**Non-overlap holds by construction (requirement 20)**: rows are disjoint
horizontal bands; within a row, children occupy disjoint x-intervals; every
child is strictly inside its parent's inner rectangle. Those three facts are
properties of the packing, not of a tree, so the proof is **one property test
over a generated tree** (a few hundred files, varied depth and breadth, from a
fixed-seed hand-rolled LCG — no `rand` dependency, and a failure is
reproducible) asserting every same-level pair disjoint and every child strictly
inside its parent.

Everything is `f32` arithmetic on sizes the code chose, with no filesystem
value and no frame timing in it, so requirement 22's twice-laid-out equality is
an `assert_eq!` on the whole `ProjectMap`.

Requirement 23's test — ten files against a thousand, the second's extent
bounded by a small multiple of the first's per-file area — and requirement 21's
nested-gap test land here. So does
`crates/map/tests/map_measurement.rs`, requirement 33's `#[ignore]`
measurement, which asserts nothing and prints.

*Leaves working:* `ProjectMap::of` can be wired in `lib.rs` — strategy cuts,
layout places — and the whole graph is testable end to end with no UI.

### 4. The painter — requirements 24, 25

`crates/map/src/ui.rs`: `pub fn project(ui: &mut egui::Ui, map: &ProjectMap)`.
Compute `scale = min(panel.width / extent.width, panel.height / extent.height)`,
centre the scaled extent in the panel, and draw each box's frame on
`ui.painter()`.

**Each name is drawn as a `Label` widget placed at its rectangle
(`ui.put(rect, egui::Label::new(name))`), not as `painter.text`.** That is a
deliberate decision and it is load-bearing: a painted glyph run leaves nothing
in the accessibility tree, and requirement 25, requirement 24 and four of the
seven scenarios are all asserted off that tree by
`egui_kittest`. The cost is one widget per box and per node; at this story's
scope — no camera, no zoom, no level of detail — that is accepted and named in
Risks.

Unit test here: a map laid out into a known panel has every district's rectangle
inside the panel (requirement 24 at the model level); the acceptance proof is in
step 8.

*Leaves working:* `cellworks-map`'s whole public API, with no caller yet.

### 5. CodeAnalysis: the file list — requirements 3, 5–11, 27, 31, 32

`crates/code-analysis/src/{event,files,git}.rs` and the two functions in
`lib.rs`.

- `CodeAnalysisEvent::{ ProjectMapped { project, map }, ProjectFolderSkipped
  { project, path, reason }, GitExcludesUnavailable { project, reason } }`.
- `pub fn load(root: &Path, name: &str, git: &dyn GitListing) -> Vec<CodeAnalysisEvent>`
  — ordinary and blocking, the `scan` shape of `crates/workspace/src/lib.rs:81`.
  `pub fn start_load(root: PathBuf, name: String) -> mpsc::Receiver<CodeAnalysisEvent>`
  is the `std::thread` around it with `GitCli` (requirement 3). A send that
  fails is the UI having gone away; the thread ends quietly.
- **Requirement 5**, the `.git`-of-either-kind test, is the rule
  `crates/workspace/src/discovery.rs:73` already states; it is restated here
  rather than shared, because the two crates must not depend on each other for
  it and it is one `try_exists` on `root.join(".git")`.
- **A repository.** `GitCli::visible_files` runs, with `current_dir` set to the
  repository:
  `git ls-files -z --cached --others --exclude-standard`, then
  `git ls-files -z --deleted`, subtracting the second from the first
  (requirements 6, 7).
- **Not a repository.** The `read_dir` walk of `files.rs`, skipping dot-prefixed
  entries and using `DirEntry::file_type()` so symlinks are not followed — both
  decisions copied from `discovery.rs:76,82` (requirement 9).
- **Hidden folders (requirement 8)** are filtered on the *relative path*, after
  either source: a path is dropped when any of its **folder** components begins
  with `.`. One rule covering both sources, and it is also why `.git` itself
  never reaches the map. Hidden *files* are not dropped — `spec.md` Concern 2,
  the product owner's ruling read literally.
- **When git cannot answer (requirement 10):** `Err(GitUnavailable)` → fall back
  to the plain walk, emit `GitExcludesUnavailable` naming the project and the
  reason git gave, and still emit `ProjectMapped`. Never silently "nothing is
  ignored".
- **Requirement 27:** every `read_dir` failure part-way down becomes a
  `ProjectFolderSkipped` and the walk continues; a project root that has gone
  away is reported against that project. No `unwrap`/`expect` outside
  `#[cfg(test)]`.
- `load` finishes by calling `ProjectMap::of(name, root, &files, &Directories)`
  and emitting `ProjectMapped` last — the ordering `scan` already uses.

**Three mitigations live in this step rather than after it**, because this is
the riskiest one (see Risks):

1. **No `--deduplicate`.** `--cached --others` can list a path twice, and
   `git ls-files --deduplicate` exists only from git 2.31 (2021). On an older
   git the flag is an *error*, the call fails, and the project would silently
   fall back to the plain walk with `node_modules` on the map. De-duplication is
   done in Rust with a `BTreeSet`, which also gives requirement 19's byte-wise
   ordering for free.
2. **`-z`, parsed as bytes.** Output is split on NUL and turned into a path with
   `OsStr::from_bytes` (unix), never through `String::from_utf8`. A file name
   that is not valid UTF-8 stays a node identified by its real path and is
   labelled lossily — the spec's failure-mode row, held by construction.
3. **Requirement 32 by construction.** The repository is the child's
   `current_dir`, never an argument; `std::process::Command` with a fixed
   argument list and no shell; `GIT_OPTIONAL_LOCKS=0` in the environment so a
   read never takes the index lock from a session working in the same
   repository (`spec.md` Concern 7). The argument vector is produced by a pure
   function so a unit test can assert it is fixed, and a second test opens a
   project whose folder name begins with `--` and gets a map rather than a git
   error.

Unit tests: requirement 5's three cases with a `GitListing` double that records
whether it was asked; requirements 6, 7, 8, 9, 10 and 11 each with a recorded
listing over a real `TempDir` tree; requirement 27's two cases, the first using
the mode-`0o000` probe of `discovery.rs:212` verbatim, including restoring
`0o755` before the `TempDir` drops. Plus
`crates/code-analysis/tests/git_cli_against_real_git.rs`, `#[ignore]`, which
skips when `git` is absent.

**Requirement 31 holds by construction:** `GitListing` is a parameter, every
test passes a recorded listing, and `GitCli` is named in `src/` and in the one
`#[ignore]` test and nowhere else.

*Leaves working:* the whole read, with no UI anywhere.

### 6. The opening gesture — requirement 1's half in the Workspace

`crates/workspace/src/ui.rs`: `universe` returns `Option<Project>`.

Each project entry keeps `ui.label(project.name.as_str())` and adds the sense to
the **response** — `.interact(egui::Sense::click())` — rather than becoming an
`egui::Label::new(..).sense(..)` widget. Both give a double-click; only the
first is certain to leave the accessibility node a `Role::Label` carrying its
text in `value()`, which is what `labels_in`
(`crates/workspace/tests/story_002_…rs:44-57`) and `labels`
(`tests/universe_frame.rs:38`) read. If `double_clicked()` on that response
turns out not to fire, the fallback is the widget form plus adjusting the two
readers — and that is a visible, compiling change rather than a silent one.

**This step also confirms the two new harness calls** before step 8 depends on
them: that a kittest node can be double-clicked, and that Esc can be pressed
(`egui_kittest` 0.36.2 / kittest 0.4.0 are already in `Cargo.lock`). One
throwaway `#[test]` that double-clicks the first project and asserts the
returned `Option<Project>` is it; it stays as this step's unit of proof.

The three call sites in `crates/workspace/tests/story_002_…rs` get braces so
their harness closures still return `()`. No STORY-002 assertion changes.

*Leaves working:* the universe reports a double-click; nothing consumes it yet.

### 6a. The per-project prefix setting — requirement 12a

`crates/workspace/src/settings.rs`: `read` also collects
`[projects."<path>"] district_prefix` strings, canonicalising each key the way
`resolve` does and ignoring one that does not resolve. `AddedFolders` gains
`district_prefixes: Vec<(PathBuf, String)>`. `crates/workspace/src/registry.rs`
attaches each to the project with the same `FolderIdentity`;
`crates/workspace/src/project.rs`'s `Project` gains `district_prefix:
Option<String>`, `None` from `Project::at`. Unit tests beside each.

### 7. The binary: two views and the opened project's states — requirements 1, 2, 4, 26

`src/opened.rs`:

```rust
pub enum ProjectState { Reading, Mapped(ProjectMap), Empty, Failed }
pub struct OpenedProject { /* project, state, notices, events */ }
```

`apply(CodeAnalysisEvent)` and `drain(&Receiver<_>)` are `Universe::apply` and
`Universe::drain` (`crates/workspace/src/universe.rs:93,116`) in the same shape,
for the same reasons: `try_recv` only; `TryRecvError::Disconnected` means
`Failed` **only** while still `Reading`; `ProjectMapped` with no district
becomes `Empty` rather than a blank `Mapped`; `ProjectFolderSkipped` and
`GitExcludesUnavailable` accumulate as notices beside whichever state is current
and never replace the map (requirements 4, 26, 27).

`src/app.rs`: `enum View { Universe(Universe), Opened(OpenedProject) }`;
`CellworksApp` holds a `View`; `draw(ui: &mut egui::Ui, view: &mut View)` —
`&mut` because the gesture and the Esc key both change which view is on screen.
On a double-click, the project's `district_prefix` is resolved with
`cellworks_map::prefix_rule` — unknown → the default plus an
`UnknownDistrictPrefix` notice on the opened project (requirement 12b) — and
`start_load(project.id.path().to_owned(), project.name, rule)` and
`View::Opened(OpenedProject::reading(project))`; the universe is kept so Esc
restores it with the projects it had (requirement 2). The per-frame
`request_repaint` while a read is running is the existing `ScanState::Scanning`
rule (`src/app.rs:73`) extended to `ProjectState::Reading`.

The user-facing strings live here, beside the state they belong to, the way
`crates/workspace/src/ui.rs:13-17` holds `SCANNING` and `FAILED`:
`"Reading the project…"`, `"This project has no files to show."`
(requirement 26, `spec.md` Concern 4's proposed wording),
`"Could not read this project."`, and the two notice sentences. Choosing which
loader reads a project is a one-line match, because every project is a folder
today; `10-quality.md` E1 is where the `jira:`/`otc:`/`data:` loaders register.

`tests/universe_frame.rs`'s three tests change only how they call `draw`.

Unit tests in `src/opened.rs`: the four state transitions, disconnect-while-
Reading → `Failed`, disconnect-after-Mapped → not a failure, a notice before a
map leaving `Reading`, a notice after a map leaving the map intact.

*Leaves working:* the whole story, end to end, with step 2's prefix rule held.

### 8. The ten scenarios, and the requirements the frame proves

`tests/story_003_see_a_code_project_as_districts.rs` at the repository root —
the root, because each scenario crosses Workspace (the gesture), CodeAnalysis
(the read) and Map (the graph and the view). One `#[test]` per scenario, named
after it in `snake_case`, **with issue #5's title repeated verbatim in a doc
comment** (`guidelines/testing.md`):

| Test | Scenario |
|---|---|
| `top_folders_below_the_common_prefix_become_districts` | 1 |
| `the_next_folder_level_becomes_sub_districts_deeper_folders_become_groups` | 2 |
| `files_directly_in_a_district_are_its_nodes` | 3 |
| `files_at_the_top_of_the_project_form_a_district_named_after_the_project` | 4 |
| `hidden_folders_and_folders_git_ignores_are_left_out` | 5 |
| `a_folder_that_is_not_a_git_repository_leaves_out_only_hidden_folders` | 6 |
| `a_project_without_files_still_opens` | 7 |
| `a_project_set_to_the_longest_shared_prefix_cuts_districts_below_every_folder_its_files_share` | 8 |
| `a_project_set_to_no_prefix_shows_its_top_folders_as_districts` | 9 |
| `an_unknown_district_prefix_falls_back_to_the_default_and_says_so` | 10 |

Every test raises its own tree under a `tempfile::TempDir`, passes a **recorded
`GitListing`**, double-clicks the project in the universe, and reads the next
frame. **None runs git, reads `$HOME`, or opens a window** (requirement 31).
Labels come from the accessibility tree, the way STORY-002's tests read them;
containment comes from the `ProjectMap` the frame drew — `area` of the child
strictly inside `area` of the parent — because that is the geometry the
scenarios assert and the accessibility tree's order is not it.

The acceptance tests of requirements 1, 2, 3 (a `GitListing` that blocks, and
the harness still running frames in `Reading`), 4 (Reading before any event, and
**not** the empty message; then a dropped sender giving the failure), 10 (the
map from the plain walk, drawn with its notice), 24, 25 and 26 (both the
no-files case and the everything-excluded case) go in the same file, each naming
its requirement in a doc comment, after the ten.

*Leaves working:* every scenario of issue #5 and every requirement of
`spec.md` that a frame can prove.

### 9. The documents that would otherwise go stale

`docs/domain/contexts/Map/Map.md` — the tactical model's first entry:
`ProjectMap` as an aggregate, its boundary (a project's districts, sub-districts,
groups and nodes and their geometry), and requirement 20's invariant: no two
drawn rectangles overlap, and the only rectangle containing another is a parent
containing its own child. `docs/arc42/03-context.md` — the two git rows widen to
include the file list a repository shows. `factory:standards-change` requires
the docs stay true in the same change; `05-building-blocks.md` §5.2 was already
done in step 1.

*Leaves working:* nothing in the documents contradicts the code.

## Risks

**Blast radius.** Four things reach outside this story. First, `cellworks::draw`
changes shape again — it is re-exported at `src/lib.rs:10` and is the seam every
future frame test uses, and it now takes `&mut` because the gesture mutates
which view is on screen; `tests/universe_frame.rs` follows it. Second,
`ui::universe` gains a return value, and **three of its four callers are harness
closures that must return `()`**, so STORY-002's acceptance file is edited by
this story even though `spec.md` does not name it; the tests keep their meaning,
but an accepted story's proof is touched and that should be visible on the pull
request rather than a quiet diff. Third, `crates/map` pulls in `egui`, and
`code-analysis`, `domain-knowledge`, `navigation`, `sources` and `agent-sessions`
all depend on `map`, so egui enters five more build graphs; the version must come
from `[workspace.dependencies]` and nowhere else, because two egui copies in one
binary give type errors that read as if the *code* is wrong. Fourth, `Cargo.toml`
and `Cargo.lock` trip `sdlc/critical.yaml`'s `dependencies` flag, so the
dashboard puts this change in front of a person — expected, not something to
route around.

Two quieter ones. `ProjectMap` and its four types are the **published language**
of context-map relationships 1, 2 and 3: Sources, DomainKnowledge and
AgentSessions all build into them. Adding a field later is cheap; renaming
`District`, `SubDistrict`, `Group` or `Node`, or changing what identifies one,
is not, and those names are chosen once, here. And step 4 draws one `Label`
widget per box and per node — on a 20,000-file project that is tens of thousands
of widgets in a frame, which is exactly what level of detail and the camera
exist to fix and neither is in this story. Requirement 33's measurement times
the cut and the layout, not the paint, so the number that would show this is
*not* collected; that is a known gap, it belongs to EPIC-002, and naming it here
is what keeps it from being lost.

**The riskiest step is step 5, the git adapter** — and specifically the
repository path of requirement 6, because it is the one place in this story
where being wrong is *silent and environment-shaped*. Every other failure here
is loud: a bad prefix rule returns the wrong districts and a test says so; a bad
packing overlaps and the property test says so. But if the `git ls-files` call
fails for a reason the author's machine never sees, the code does exactly what
requirement 10 tells it to — falls back to the plain walk and carries on — and
the only symptom is `node_modules` quietly appearing on the map. The concrete
trap is `--deduplicate`, which is a git 2.31 flag and an *error* on anything
older; the mitigation is in step 5 rather than after it — the flag is not used
at all, and the de-duplication happens in Rust, where it also delivers
requirement 19's ordering. The second-order version of the same risk is why
requirement 10's unit test asserts **both** the `GitExcludesUnavailable` event
*and* that a map was still built: an implementation that degrades silently and
an implementation that degrades loudly are indistinguishable from the map alone.

**The second risk is step 6's gesture, and it is the kind that breaks something
already green.** Turning the universe's entries into clickable widgets can move
their accessibility role from `Label` to `Button`, and `labels_in`
(`crates/workspace/tests/story_002_…rs:50-56`) filters on `Role::Label` — so
STORY-002's acceptance tests and `tests/universe_frame.rs` would start reading
an empty list. The mitigation is the choice stated in step 6: add the sense to
the `Response` rather than to the widget, which leaves the widget info alone.
If that does not deliver a double-click, the fallback changes the two readers in
the same commit; what is not acceptable is changing the readers to match
whatever the frame happens to produce.

**Rejected alternatives.** *The `ignore` crate, or a hand-written `.gitignore`
parser:* rejected — it reimplements rules git already owns (`.gitignore`,
`.git/info/exclude`, the user's global excludes, nested ignore files), and
ADR-0003's whole point is reusing the user's configuration. *`git check-ignore`
per path:* rejected — one process spawn per file is the cost ADR-0003 names as
the thing hot paths must batch away. *Walking the filesystem first and filtering
with git afterwards:* rejected — it pays for descending `node_modules`,
`target` and `vendor`, which is the one cost that grows without bound; asking
git first means they are never opened. *`git -C <path> ls-files`:* rejected by
requirement 32 — a project folder named `--upload-pack=…` would reach git's
argument vector. *`--deduplicate`:* rejected, see above. *Re-exporting
`ProjectMap` from `cellworks-code-analysis` instead of the new arrow:* rejected
by `spec.md` Concern 5 — laundering the published language through one loader
makes the next loader's re-export look equally reasonable. *Sharing
`crates/workspace/src/test_tree.rs`:* rejected — it is `pub(crate)`, and making
it reachable means either a published test-only API or a `testing` feature,
both of which put test scaffolding in the shipped crate; `code-analysis` builds
its own small tree with `tempfile` directly, and `crates/map`'s tests need no
tree at all because the strategy and the layout are pure. *A physics or
force-directed layout now:* rejected — it is EPIC-002's by the issue's own scope
note, and the prototype findings (L239-240, L253) record that pushing
fixed-size hard boxes apart strung large maps out. *Fixed-size boxes:* rejected
for the same finding; requirement 23 makes a box exactly as large as its
content, so nothing can string out. *Asserting non-overlap with a runtime
invariant check:* rejected — it is a property of the packing, so one property
test over generated trees proves more than any number of examples and costs
nothing at run time. *Drawing names with `painter.text`:* rejected — it leaves
nothing in the accessibility tree, and requirements 24 and 25 and four scenarios
would become unprovable headlessly, which `08-crosscutting.md` §8.6 forbids
working around. *Putting the district rule in the loader:* rejected by
requirement 28 — the seam is in `crates/map`, and a second strategy must touch
only `crates/map/src/districts.rs`. *Amending
`docs/domain/contexts/CodeAnalysis/CodeAnalysis.md`:* rejected per `spec.md`
Concern 6 — it describes the target, and the epic tracks the rest.

**Rollback.** Everything here is reversible by a plain `git revert` of this
story's squash-merge commit on `test`, and on `main` too if it has already been
promoted. The revert restores `draw(ui, &Universe)` and the `()`-returning
`ui::universe`, empties `crates/map/src/` and `crates/code-analysis/src/` back
to their doc comments, removes the `cellworks-map` arrow from the root manifest
and the `GRAPH` table together, and returns the three documents to their current
text. There is nothing else to undo: this story writes no file, creates no
cache and no setting, and `git ls-files` writes nothing — so no user data and no
on-disk state exists for a rollback to migrate, and no external system is
touched.

## Proof

### The four checks of `CLAUDE.md`

Run from the repository root, output pasted into the pull request. The three
cargo commands are character-identical to `verify-command` in
`.github/workflows/sdlc-build.yml:27`.

```sh
cargo fmt --all --check                                 # no output, exit 0
cargo clippy --workspace --all-targets -- -D warnings   # "Finished", zero warnings
cargo test --workspace                                  # every suite: 0 failed
SHELLCHECK_OPTS=--exclude=SC2016 actionlint             # no output, zero findings
```

`cargo test --workspace` gains three test binaries — `cellworks-map`'s unit
tests and `map_measurement`, `cellworks-code-analysis`'s unit tests and
`git_cli_against_real_git`, and the root's
`story_003_see_a_code_project_as_districts` — and keeps the six STORY-002
leaves green. Every row below is at least one `#[test]`:

| Where | Pins |
|---|---|
| `crates/map/src/districts.rs` unit | requirements 11–19, 12c |
| `crates/workspace/src/{settings,registry}.rs` unit | requirement 12a |
| `crates/map/src/layout.rs` unit | requirements 20 (property test), 21, 22, 23 |
| `crates/map/src/ui.rs` unit | requirement 24 at the model level |
| `crates/code-analysis/src/files.rs` unit | requirements 8, 9, 11, 27 |
| `crates/code-analysis/src/git.rs` unit | requirements 32 (fixed argv, `current_dir`, a `--`-named folder), 10 (`Err` → fallback + event) |
| `crates/code-analysis/src/lib.rs` unit | requirements 3, 5, 6, 7, 10, 27, 31 |
| `src/opened.rs` unit | requirements 4 and 26's state transitions |
| `tests/story_003_…rs` | the ten scenarios of issue #5, named after them; then requirements 1, 2, 3, 4, 10, 24, 25, 26 |
| `tests/workspace_structure.rs` | requirement 30 — `GRAPH` now holds `bin → cellworks-map` and nothing else new |
| `tests/universe_frame.rs`, `crates/workspace/tests/story_002_…rs` | unchanged in meaning, still green |

### Requirement 33 — the measurement

```sh
cargo test --workspace --release -- --ignored --nocapture
#   -> map_measurement prints the time to cut and to lay out 20,000 files in
#      40 districts, and the resulting extent; asserts nothing.
#   -> git_cli_against_real_git passes, or prints that git is absent and skips.
```

The numbers go into the pull request as the first data point for EPIC-002.

### Requirement 3 — nothing blocks, no async runtime

```sh
grep -rnE '\b(async|await|tokio)\b' src crates/map/src crates/code-analysis/src   # no match
grep -rnE '\brecv\(' src crates/map/src crates/code-analysis/src | grep -v try_recv  # no match
cargo tree --workspace | grep -E '(tokio|async-std|smol) v'                        # no match
```

`pollster` and the `futures-*` helpers under `wgpu` are wgpu's own blocking
helpers, not an async runtime; they are not what this excludes.

### Requirements 27 and 31 — no panic on outside data, no real tool, no `$HOME`

```sh
grep -rnE '\.(unwrap|expect)\(' crates/map/src crates/code-analysis/src   # only inside #[cfg(test)]
grep -rn 'GitCli' crates src tests                                        # src/ and the one #[ignore] test only

cargo test --workspace --no-run                      # build once with the real HOME
CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" HOME="$(mktemp -d)" \
  cargo test --workspace                             # still green
```

`CARGO_HOME` and `RUSTUP_HOME` are kept at the real ones deliberately — cargo
needs its registry and rustup needs a toolchain, and the thing under test is
whether the *code* reads `$HOME`.

And that no test spawns git, proven rather than asserted: put a shim first on
`PATH` that records its own invocation and fails.

```sh
shim="$(mktemp -d)"
printf '#!/bin/sh\ntouch "%s/was-called"\nexit 1\n' "$shim" > "$shim/git"
chmod +x "$shim/git"
PATH="$shim:$PATH" cargo test --workspace            # green
ls "$shim/was-called"                                # -> No such file or directory
```

### Requirement 32 — no option injection, no shell

```sh
grep -rnE '(\-C|sh -c|/bin/sh|Command::new\("sh"\))' crates/code-analysis/src   # no match
grep -rn 'current_dir' crates/code-analysis/src/git.rs                          # the repository, once
```

Plus the unit test asserting the argument vector is fixed and the repository is
the `current_dir`, and the test that opens a project whose folder name begins
with `--` and gets a map.

### Requirements 28 and 29 — the seam, and the names

```sh
grep -rniE '(district|sub_district|subdistrict)' crates/code-analysis/src
#   -> no rule: only the `use cellworks_map::…` import and doc prose
grep -rinE '\b(planet|servitor|belt|machine|factory)\b' src crates/*/src
#   -> English prose in doc comments only, no identifier
```

`crate` is on the theme-word list and is also a Rust keyword, so it is checked
by eye. The compiler pins the rest: `ProjectMap`, `District`, `SubDistrict`,
`Group`, `Node`, and `CodeAnalysisEvent` with past-tense variants.

### Requirement 30 — the one new arrow, documented in the same commit

```sh
git show --stat HEAD -- tests/workspace_structure.rs docs/arc42/05-building-blocks.md
#   -> both files, in the step-1 commit
git diff main -- Cargo.toml | grep cellworks-
#   -> one added line: cellworks-map
```

### What a reviewer checks by eye

- `crates/map/src/{graph,districts,layout}.rs` name no egui widget — only
  `Rect`, `Vec2`, `Pos2` — and `ui.rs` is the one module that draws
  (`guidelines/architecture.md`).
- `crates/code-analysis/` holds no district, sub-district or group rule: it
  produces a file list and calls `ProjectMap::of` (requirement 28).
- `GitListing` is a parameter of `load`, not a field constructed inside it —
  that is what makes requirement 31 true by construction.
- `districts.rs` orders children on path-component bytes, not on the display
  name, and every box and node carries the canonical path (requirements 18, 19).
- The layout's three non-overlap facts are visible in the packing function:
  disjoint rows, disjoint x-intervals in a row, child strictly inside the
  parent's inner rectangle.
- `src/app.rs` drains with `try_recv` only and holds the user-facing strings
  beside the states they belong to, the way `crates/workspace/src/ui.rs` does.
- `crates/map/Cargo.toml` and `crates/code-analysis/Cargo.toml` gained no
  `cellworks-*` key; the root gained exactly one, and `[workspace.dependencies]`
  is untouched, so the root still has exactly one `^version = "` line.
- The three edits to `crates/workspace/tests/story_002_…rs` change only the
  shape of a harness closure — no assertion and no expected value moves.
- `docs/domain/contexts/Map/Map.md` records `ProjectMap` and requirement 20's
  invariant, and `docs/arc42/03-context.md` now names the file list among what
  git is asked for.

### Two checks by hand on macOS

Recorded in the pull request, outside the automated suite, because no automated
test opens a window or runs real git (`CLAUDE.md`; `08-crosscutting.md` §8.6) —
and run only with the user's agreement:

1. Open a real repository and confirm `node_modules` or `target` is absent and
   the source tree reads as districts, sub-districts and groups.
2. Open the same folder with `git` renamed away and confirm requirement 10's
   notice appears beside a map built from the plain walk.
