---
stage: spec
issue: 5
status: draft
author: claude (sdlc-spec)
date: 2026-10-09
---

# Spec: See a code project as districts, sub-districts, groups and nodes

Derived from `intent.md` in this directory. Answers, or explicitly carries
forward, every open question it raised.

**The intent's open questions.** The intent carries no `> OPEN QUESTION:` line;
its *Open questions* section records two things, and both are carried into the
design rather than dropped:

1. *What is left out* — resolved by the product owner on 2026-10-09: anything
   git ignores is left out, individual files as well as folders, together with
   hidden folders; a control to bring ignored files back is its own story,
   STORY-005 (#16). Answered in [Design → What counts as a
   file](#what-counts-as-a-file) and requirements 5–10. Reading that ruling
   literally decides one further case the intent does not spell out — hidden
   *files* that git does not ignore — see [Concerns](#concerns), item 2.
2. *No text in the issue was addressed to an AI assistant* — confirmed for
   `issue.md` and for `intent.md` as well. Both were read as data. Neither
   contains an instruction to change behaviour, skip a policy, or act outside
   this stage. There is nothing to record as a prompt-injection finding.

**Policies applied:** `CLAUDE.md`; `factory-context/05-case/AGENTS.md` and
`README.md`, which route to the jarvis docs in this repository and were
followed rather than copied; `docs/domain/glossary.md`,
`docs/domain/contexts/Map/{Map.md,glossary.md}`,
`docs/domain/contexts/CodeAnalysis/{CodeAnalysis.md,glossary.md}`,
`docs/domain/contexts/Navigation/Navigation.md`,
`docs/domain/contexts/Workspace/Workspace.md`, `docs/domain/context-map.md`;
`docs/arc42/02-constraints.md`, `04-solution-strategy.md`,
`05-building-blocks.md`, `08-crosscutting.md` (§8.1, §8.2, §8.4, §8.5, §8.6),
`10-quality.md` (U3, P1–P3, E1, E2, R1);
`docs/services/cellworks/guidelines/{architecture,naming,testing}.md`;
`docs/discovery/2026-10-08-prototype-findings.md` L41–L47, L63–L76, L95–L96,
L224, L239–240, L253; ADR-0001, ADR-0002, ADR-0003, ADR-0004.
`docs/services/cellworks/decisions/` holds no entry yet. The one product policy
skill, `.claude/skills/secure-api-review`, was read and does not apply — see
[Concerns](#concerns), item 7, which records the one security question this
story *does* raise.

**Not in this story.** The physics layout (repulsion, forces within a group,
affinity clustering, the 20k-file inflation check) is EPIC-002's, by the
issue's own scope note. So are lanes and lane kinds, items (functions), level
of detail, zoom and camera, the selection and keyboard movement, folder views,
dragging, the view-options menu, the git overlay, the tree and the palette —
Map and Navigation stories, none of which this story needs to make its ten
scenarios true. Enter as the keyboard twin of the opening gesture is STORY-004.
A control to bring ignored files back into view is STORY-005 (#16).

## Requirements

Numbered, independently verifiable; the proof of each is in
[Verification](#verification). The terms are the glossary's: **District**,
**Sub-district**, **Group**, **Node**, **Project**. Theme words appear nowhere.

### Opening a project

1. **(must)** A double-click on a project in the universe opens it: the other
   projects are no longer drawn, and that project's map takes their place
   (`Map.md`, "double-clicking a project locks into it and hides the others";
   prototype findings L63-64).
2. **(must)** Esc leaves the opened project and shows the universe again, with
   the same projects it had before (same source). Without it the engineer is
   trapped in the first project they open — see [Concerns](#concerns), item 3.
3. **(must)** Reading the project runs on a `std::thread`; the UI thread never
   blocks on a filesystem walk or on a child process (`08-crosscutting.md`
   §8.1, `guidelines/architecture.md`). Results reach the UI thread as
   `CodeAnalysisEvent` values over a `std::sync::mpsc` channel, drained once per
   frame with `try_recv`. No async runtime is introduced.
4. **(must)** From the moment a project is opened until its first result
   arrives, the project shows that it is being read. An empty map never flashes
   on the way, and a read that ends without reporting leaves a stated failure
   rather than "reading" for ever.

### What counts as a file

5. **(must)** A project folder is a git repository exactly when it directly
   contains an entry named `.git`, of either kind — the same rule STORY-002
   uses for discovery. Nothing else decides it, and no git call is needed to
   answer it.
6. **(must)** In a git repository, a path git ignores does not appear on the
   map — a single file as much as a whole folder (product owner, 2026-10-09).
   A file that exists in the worktree and that git does not ignore appears,
   whether or not it is tracked, so a file written a second ago is on the map.
7. **(must)** A file that git still has in its index but that no longer exists
   in the worktree does not appear. The map never shows a node for a file that
   is not there.
8. **(must)** A folder whose name begins with `.` is left out, everywhere in the
   tree and whatever else it contains, in a repository and outside one.
9. **(must)** In a folder that is not a git repository, that hidden-folder rule
   is the *only* exclusion: a folder named `build` appears, because there is no
   git to ask whether it is ignored.
10. **(must)** When git cannot answer for a repository — not installed, the
    repository is broken, the call fails — the project still opens, read with
    the hidden-folder rule alone, and says that git could not be asked so
    ignored files may be on the map. It is never silently treated as "nothing
    is ignored" (`08-crosscutting.md` §8.2; ADR-0003: a missing tool degrades
    only its own feature and says so).
11. **(must)** A folder that holds no included file anywhere below it is not
    drawn at all. An empty box carries nothing to read, and this is what makes
    requirements 8–9 visible rather than leaving hollow outlines behind.

### The directories district strategy

The rule below is the **directories** strategy, which `Map.md` records as the
current one. It does not change what District, Sub-district, Group or Node
mean. Requirement 12 was settled by the product owner — see [Concerns](#concerns), item 1.

12. **(must)** The **district level** of a project is found once, at the
    project root, by the project's **prefix rule** (product owner, 2026-10-09).
    Three rules exist, each applied to the folder tree left after the
    exclusions of requirements 6–11:
    - `one-wrapper-folder`, **the default**: if the root holds exactly one
      entry, that entry is a folder, and that folder itself holds at least one
      folder, the district level is inside that folder. In every other case it
      is the project root. Taken at most once.
    - `longest-shared-prefix`: descend while the current folder holds exactly
      one entry and that entry is a folder; the district level is where that
      stops.
    - `none`: the district level is the project root.
12a. **(must)** The rule is chosen per project in the settings file, as
    `[projects."<absolute path>"] district_prefix = "<rule name>"`. The path
    is canonicalised the way `added_folders` entries are, and a key that does
    not resolve is ignored. A project with no entry uses the default.
12b. **(must)** A rule name the Map does not know falls back to the default,
    and the opened project says the name is unknown. It is never silently
    treated as a valid choice.
12c. **(must)** The rules are one table in the Map crate: adding a rule is one
    function and one entry in that table, and touches nothing else.
13. **(must)** Each folder at the district level is a **district**, named by its
    folder name.
14. **(must)** Each folder directly inside a district is a **sub-district** of
    it, named by its folder name.
15. **(must)** Each folder deeper than a sub-district is a **group**, named by
    its folder name, nested exactly as the folders nest — a group may hold
    groups.
16. **(must)** Each included file is a **node**, named by its file name, held by
    the district, sub-district or group whose folder directly contains it.
17. **(must)** Included files sitting directly at the district level are nodes
    of one further district named after the project itself.
18. **(must)** Every district, sub-district, group and node is identified by the
    canonical path of its folder or file, never by its name
    (`08-crosscutting.md` §8.5). The project-named district of requirement 17 is
    identified by the project root. Two names may therefore collide — a project
    `shop` that also has a top-level folder `shop` shows two districts labelled
    `shop` — and that is correct, because nothing keys on the label.
19. **(must)** The map is the same for the same filesystem: districts,
    sub-districts, groups and nodes are ordered by name, byte-wise, at every
    level, and two reads of an unchanged project produce an identical map.

### The layout

20. **(must)** No two drawn boxes or nodes overlap. The only rectangle that
    contains another is a parent containing its own child
    (issue scope note: "a simple, deterministic arrangement that keeps every
    district, sub-district and group apart").
21. **(must)** Districts are separated from each other by a larger margin than
    sub-districts are, and sub-districts by a larger margin than groups, so the
    folder hierarchy reads at a glance (prototype findings L71).
22. **(must)** The layout is deterministic: the same map laid out twice gives
    byte-identical geometry. Nothing is random, nothing is iterated to
    convergence, nothing depends on frame timing or on previous frames.
23. **(must)** A box's extent is derived from what it holds, never fixed in
    advance, so a project is exactly as large as its content. This is what keeps
    the rejected sub-district behaviour away: pushing *fixed-size* hard boxes
    apart is what strung large maps out (prototype findings L239-240, L253), and
    nothing here pushes anything.
24. **(must)** The whole opened project is visible at once: its extent is scaled
    uniformly to fit the panel and centred. There is no camera, no zoom and no
    scrolling in this story, so every district the project has is on screen.
25. **(must)** Every district, sub-district, group and node drawn carries its
    name as a label.

### Empty projects and failures

26. **(must)** A project with no included file — no files at all, or none left
    after requirements 6–11 — opens and says it has no files, rather than
    showing an unexplained blank. Proposed wording: *"This project has no files
    to show."* — see [Concerns](#concerns), item 4.
27. **(must)** A folder that cannot be read part-way through, or a project
    folder that has gone away since the scan, is reported against that project
    and does not take the rest of the read, or any other project, down
    (`08-crosscutting.md` §8.2, `10-quality.md` R1). No `unwrap` or `expect` on
    anything read from the filesystem or from git
    (`guidelines/architecture.md`).

### Structure, naming and checks

28. **(must)** The district strategy is a seam in the **Map** crate, not in the
    loader: adding a second strategy later touches only that seam
    (`04-solution-strategy.md` §4.2, `10-quality.md` E2). CodeAnalysis finds the
    files; the Map cuts them into districts and lays them out; CodeAnalysis
    emits the finished graph (context map relationship 1).
29. **(must)** Identifiers use `District`, `SubDistrict`, `Group`, `Node`,
    `ProjectMap`; the event enum is `CodeAnalysisEvent` with past-tense variants;
    no Warhammer 40k word appears in any identifier
    (`guidelines/naming.md`). `File` and `Folder` keep their CodeAnalysis
    meaning and are not used for a map node.
30. **(must)** Exactly one new `cellworks-*` dependency arrow is introduced —
    the binary on `cellworks-map` — and `docs/arc42/05-building-blocks.md` §5.2
    and `tests/workspace_structure.rs` are updated in the same commit, as that
    test's own doc comment requires. No other arrow changes. See
    [Concerns](#concerns), item 5.
31. **(must)** No unit or acceptance test runs real git, reads the user's home
    directory, or reads any checkout outside its own temporary tree
    (`guidelines/testing.md`, `08-crosscutting.md` §8.6). This holds by
    construction: what git says is a parameter of the loader, filled by the
    binary with the child-process adapter and by every test with a recorded
    listing.
32. **(must)** Secrets and paths never become git arguments that git could read
    as options: the repository is given to the child process as its working
    directory, not as an argument, and no project path is interpolated into a
    command line (`02-constraints.md` §2.1, ADR-0003). See
    [Concerns](#concerns), item 7.
33. **(should)** Opening a large project keeps the UI responsive: no frame
    exceeds 50 ms while the read runs (`10-quality.md` P2). The issue sets no
    scale target for this deterministic layout, so the measurement test records
    what it measured and asserts nothing about it.
34. **(must)** All four commands of `CLAUDE.md` → *Verifying your work* succeed
    with zero warnings and zero findings, on macOS and on Linux.

## Design

### Where the code lives

Three contexts have a share, and the split follows the context map rather than
convenience:

| Context | Crate | Its share |
|---|---|---|
| CodeAnalysis | `cellworks-code-analysis` | Finding the included files of a project: the git adapter, the filesystem walk, and the thread they run on. |
| Map | `cellworks-map` | The graph (`District`, `SubDistrict`, `Group`, `Node`), the **district strategy seam** with `Directories` as its first strategy, the deterministic layout, and the painting of a project map. |
| Workspace | `cellworks-workspace` | One change only: the universe view reports which project was double-clicked. |
| — | the binary | Which of the two views is on screen, the opened project's state, and starting and draining the read. |

```
crates/map/src/
  lib.rs        # ProjectMap, District, SubDistrict, Group, Node, DistrictStrategy
  districts.rs  # the Directories strategy: relative file paths -> the nesting
  layout.rs     # the deterministic nested packing; pure geometry, no egui widgets
  ui.rs         # painting one ProjectMap

crates/code-analysis/src/
  lib.rs        # load, start_load, CodeAnalysisEvent, GitListing
  git.rs        # the `git` child-process adapter (ADR-0003)
  files.rs      # the filesystem walk for a folder that is not a repository

crates/workspace/src/ui.rs   # the universe view now reports a double-click
src/app.rs                   # universe or opened project; starts and drains the read
```

`guidelines/architecture.md` keeps egui out of model modules: `districts.rs` and
`layout.rs` use only `egui::{Rect, Vec2, Pos2}` as geometry, so the whole
strategy and the whole layout are unit-testable without a frame.

### Public API, in sketch

The shapes, not the final signatures; the plan settles those.

```rust
// cellworks-map — the Map's published language, first instalment.
pub struct ProjectMap { /* project name, root, districts, extent */ }
pub struct District    { /* name, path, sub_districts, groups, nodes, area */ }
pub struct SubDistrict { /* name, path, groups, nodes, area */ }
pub struct Group       { /* name, path, groups, nodes, area */ }   // may hold groups
pub struct Node        { /* name, path, area */ }

/// The seam of 10-quality E2. `Directories` is the one strategy this story adds.
pub trait DistrictStrategy {
    /// Cuts a project's included files, relative to its root, into districts.
    fn cut(&self, project: &str, root: &Path, files: &[PathBuf]) -> Vec<District>;
}
pub struct Directories;

impl ProjectMap {
    /// The only constructor: the strategy cuts, then the layout places.
    pub fn of(project: &str, root: &Path, files: &[PathBuf],
              strategy: &dyn DistrictStrategy) -> ProjectMap;
    pub fn is_empty(&self) -> bool;     // requirement 26
}

pub mod ui { pub fn project(ui: &mut egui::Ui, map: &ProjectMap); }
```

```rust
// cellworks-code-analysis
/// What git says about a repository. The binary passes the child-process
/// adapter; every test passes a recorded listing — which is what makes
/// requirement 31 true by construction rather than by remembering.
pub trait GitListing {
    /// Paths relative to the repository root that git does not ignore and that
    /// exist in the worktree.
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable>;
}
pub struct GitCli;   // runs `git ls-files` as a child process

pub enum CodeAnalysisEvent {
    ProjectMapped        { project: PathBuf, map: ProjectMap },
    ProjectFolderSkipped { project: PathBuf, path: PathBuf, reason: String },
    GitExcludesUnavailable { project: PathBuf, reason: String },
}

/// Blocking: called on the background thread, and directly from a test.
pub fn load(root: &Path, name: &str, git: &dyn GitListing) -> Vec<CodeAnalysisEvent>;
/// The `std::thread` around it, with the real git adapter.
pub fn start_load(root: PathBuf, name: String) -> mpsc::Receiver<CodeAnalysisEvent>;
```

`load` being an ordinary blocking function separate from `start_load` is the
same shape STORY-002 used for `scan`/`start_scan`, and for the same reason:
every rule above is unit-testable without a thread, a channel or a frame.

### What counts as a file

Two paths, chosen by requirement 5's `.git` test, and the exclusion is applied
where it is cheapest:

**A git repository.** One batched child process gives the included set
directly:

```
git ls-files -z --cached --others --exclude-standard --deduplicate
```

That is exactly "every file git would show you": tracked files, plus untracked
files that no ignore rule covers, minus everything `.gitignore`,
`.git/info/exclude` and the user's global excludes drop (requirement 6). A
second, equally cheap call, `git ls-files -z --deleted`, names the files git
still has in the index but that are gone from the worktree; they are subtracted
(requirement 7).

Asking git rather than walking the filesystem is deliberate and is the
performance decision of this story: a repository's `node_modules`, `target` or
`vendor` are never descended into at all, because git never lists them. Walking
first and filtering afterwards would cost the walk, and that is the one cost
that grows without bound (ADR-0003: hot paths batch their calls; `10-quality.md`
P2).

The hidden-folder rule of requirement 8 is then applied on top of git's answer,
because git happily tracks `.github/workflows/ci.yml`.

**A folder that is not a git repository.** A plain `read_dir` walk, skipping
folders whose name begins with `.` and not following symbolic links —
`DirEntry::file_type()` rather than `metadata()`, the same single decision
STORY-002's discovery walk makes. Nothing else is excluded (requirement 9).

**Hidden folders, everywhere.** A path is dropped when *any* of its folder
components begins with `.`. Doing it on the path rather than at the walk makes
one rule cover both sources, and it is also why `.git` itself never reaches the
map.

**When git cannot answer.** `GitListing::visible_files` returns
`Err(GitUnavailable)`; the loader falls back to the plain walk, emits
`GitExcludesUnavailable`, and the project draws a notice beside its map
(requirement 10). It is a degraded read that says it is degraded, never a
silent one.

### The directories district strategy

The strategy takes the included files as paths relative to the project root and
builds the nesting. It is pure: no filesystem, no git, no geometry.

**The common prefix (requirement 12).** Build the folder tree of the included
files. If the root has exactly one child, that child is a folder, and that
folder has at least one folder of its own, descend once — that folder is the
common prefix, and the level below it is the district level. Otherwise the
project root is the district level. The guard matters twice:

- Without "descend once", `src/catalog/pricing/{price.rs, rules/discount.rs}`
  would be stripped to `pricing` and the scenario's district `catalog` would
  disappear.
- Without "that folder has at least one folder of its own", a project whose
  only top-level folder is `build`, holding files directly, would lose `build`
  and show its files under a district named after the project — and the
  issue's sixth scenario expects the district `build`.

This rule, and the fact that it needs a guard at all, is the subject of
[Concerns](#concerns), item 1.

**The levels.** With the district level fixed, every level below follows from
depth alone:

| Depth below the district level | Becomes |
|---|---|
| a folder at depth 0 | a district |
| a folder at depth 1 | a sub-district of that district |
| a folder at depth 2 or deeper | a group, nested as the folders nest |
| a file | a node of whatever directly contains it |
| a file at depth 0 | a node of the district named after the project |

Worked through the issue's scenarios:

| Files | District level | Result |
|---|---|---|
| `src/catalog/…`, `src/ordering/…` | inside `src` | districts `catalog`, `ordering` |
| `src/catalog/pricing/price.rs`, `src/catalog/pricing/rules/discount.rs` | inside `src` | district `catalog` → sub-district `pricing` → node `price.rs` and group `rules` → node `discount.rs` |
| `src/catalog/item.rs`, `src/ordering/order.rs` | inside `src` | district `catalog` → node `item.rs` |
| `README.md`, `src/catalog/item.rs` (project `shop`) | the root | district `shop` → node `README.md`; district `src` |
| `build/output.o` (not a repository, `.cache/` hidden) | the root | district `build` → node `output.o` |

A folder that survives the exclusions but holds no file below it never enters
the tree (requirement 11), so the question "what does an empty district look
like" has no answer to get wrong.

### The layout

A nested packing, computed bottom-up in one pass. Nothing moves after it is
placed.

1. A node is a fixed footprint, large enough for its label at the drawn scale.
2. A box's children — its sub-boxes first in name order, then its loose nodes
   in name order — are packed into rows left to right, a new row starting when
   the row would pass a target width derived from the children's total area.
   The packing is a pure function of the ordered child sizes.
3. A child is placed strictly inside its parent's inner rectangle, below a title
   strip that carries the parent's label.
4. A box's extent is its packed children plus the padding of its level, with
   `padding(District) > padding(SubDistrict) > padding(Group)` (requirement 21).
5. The project's extent is the districts packed the same way, and the view
   scales that extent uniformly to fit the panel (requirement 24).

**Non-overlap holds by construction, not by checking (requirement 20).** Rows
are disjoint horizontal bands; within a row, children occupy disjoint
x-intervals; every child is strictly inside its parent's inner rectangle. Those
three facts are the invariant, and they are properties of the packing rather
than of any particular tree — so the proof in [Verification](#verification) is
one property test over generated trees, not a collection of examples.

**Why this does not string the map out.** The sub-district finding
(prototype findings L239-240, L253) was about pushing *fixed-size* hard boxes
apart with forces until a test project reached 800k units. Here a box is as
large as its content and never larger (requirement 23), and no force acts on
anything, so the failure mode cannot occur. The physics layout that replaces
this one in EPIC-002 has to solve it again; this one does not have it.

### States of an opened project

| State | Entered when | Shown |
|---|---|---|
| Reading | the project is opened, before any event | that the project is being read |
| Mapped | `ProjectMapped` with at least one district | the map |
| Empty | `ProjectMapped` with no district | requirement 26's message |
| Failed | the channel closes before any `ProjectMapped` | that the project could not be read |

Reading is a state of its own rather than "empty until proven otherwise", for
the reason STORY-002 made Scanning one: a slow read must not show the engineer
an empty project that then fills in (requirement 4).
`ProjectFolderSkipped` and `GitExcludesUnavailable` accumulate as notices beside
whichever state is current and never replace the map (requirement 27,
`10-quality.md` R1) — the same shape as `Universe`'s notices, deliberately, so
there is one pattern in the binary and not two.

### Failure modes

| Situation | Behaviour |
|---|---|
| `git` not installed, or not on `PATH` | `GitExcludesUnavailable`; the project opens, read with the hidden-folder rule alone, and says so (requirement 10). |
| `git` exits non-zero, or its output is not valid | the same; the reason is the message git gave, truncated for display. |
| The project folder has gone away since the scan | `ProjectFolderSkipped` naming it; the project shows the failure; the universe and every other project are untouched. |
| A folder below the project cannot be read (non-repository walk) | skipped, `ProjectFolderSkipped`, the rest of the walk continues. |
| Every file is hidden or ignored | Empty — requirement 26's message, not a blank panel. |
| The project is a repository with no commits | `git ls-files --others --exclude-standard` still lists the untracked files; nothing special is needed. |
| The read thread panics or hangs | the sender drops and the state becomes Failed (requirement 4). A hang leaves Reading, with the UI still drawing at full rate; cancelling a read is a later story and is named as such. |
| A file name is not valid UTF-8 | it is labelled lossily, the way `Project::at` already labels a project, and is still a node identified by its real path. |

### What changes in the binary

`CellworksApp` gains one more piece of state: either the universe, or an opened
project with its own state and its own `Receiver<CodeAnalysisEvent>`. Each frame
it drains whichever channel is live — `try_recv`, never waiting — and draws
either `cellworks_workspace::ui::universe` or `cellworks_map::ui::project`.
`ui::universe` grows a return value saying which project was double-clicked;
that is the whole of the Workspace change, and the Workspace scan is untouched.

Choosing *which* loader reads a project is a one-line match on the project today,
because every project is a folder; when Sources lands its `jira:`, `otc:` and
`data:` projects, that match is where they register (`10-quality.md` E1).

## Affected interfaces

- **New: the public API of `cellworks-map`** — its first. `ProjectMap`,
  `District`, `SubDistrict`, `Group`, `Node`, `DistrictStrategy`, `Directories`,
  `ui::project`. This is the published language of context-map relationships 1,
  2 and 3; every later loader builds it. `Item` and `Lane` are deliberately
  absent until a story needs them. The crate's `lib.rs` doc comment stops saying
  "nothing is exposed yet".
- **New: the public API of `cellworks-code-analysis`** — its first.
  `CodeAnalysisEvent`, `load`, `start_load`, `GitListing`, `GitCli`. Added
  `CodeAnalysisEvent` variants are additive; renamed ones are not.
- **Changed: `cellworks_workspace::ui::universe`** gains a return value — which
  project the engineer opened. Its only caller is the binary.
- **Changed: `cellworks::draw`** takes the app's state rather than `&Universe`
  (`src/app.rs:82`, re-exported from `src/lib.rs`). Every caller changes with
  it, including `tests/universe_frame.rs:39`, whose three tests keep their
  meaning and change only how they call it.
- **New crate arrow: the binary on `cellworks-map`.** The binary holds the
  opened project's `ProjectMap` and calls the Map's painter, so it needs the
  type. `docs/arc42/05-building-blocks.md` §5.2 — both the mermaid diagram and
  the paragraph listing what the binary wires — and the `GRAPH` table of
  `tests/workspace_structure.rs:34` change in the same commit, which is what
  that test exists to force. No other arrow changes; `code-analysis → map`
  already exists, and the loader takes the project root as a `&Path`, so
  CodeAnalysis still does not depend on Workspace.
- **Changed manifests:** `crates/map/Cargo.toml` gains `egui` and
  `egui_kittest` as a dev-dependency; `crates/code-analysis/Cargo.toml` gains
  `tempfile` as a dev-dependency; the root `Cargo.toml` gains
  `cellworks-map`.
- **Process note:** touching any `Cargo.toml` trips the `dependencies` flag of
  `sdlc/critical.yaml`, so this change is put in front of a person by the
  dashboard. Expected, not a thing to route around.
- **New foreign-system call:** `git ls-files`, the first use of git in the
  product. `docs/arc42/03-context.md` already names git with "status, numstat,
  branches, worktrees, via the CLI"; `ls-files` is a read of the same kind and
  needs no new neighbour, but the plan should check whether that sentence wants
  widening.
- **Documents the plan should update in the same commit:**
  `docs/domain/contexts/Map/Map.md`'s tactical model, which has no entry yet and
  gains `ProjectMap` as an aggregate with the non-overlap invariant of
  requirement 20; and `docs/arc42/05-building-blocks.md` §5.2 as above. No
  glossary term changes — this story implements the strategy `Map.md` already
  records.
- **Changed: the Workspace settings file** gains the per-project
  `[projects."<path>"] district_prefix` key (requirement 12a), still read-only;
  `Project` gains `district_prefix: Option<String>`, the raw name, which only
  the Map interprets.
- **Unchanged:** the Workspace scan's discovery; the `navigation`,
  `sources`, `domain-knowledge`, `agent-sessions` and `editing` crates, all
  still empty; the command line of `cellworks`.

## Concerns

1. **Resolved by the product owner, 2026-10-09: the common prefix rule.** The
   intent's prose ("whatever path prefix every file shares", the longest
   prefix) and issue #5's second scenario (one wrapper folder) disagreed. The
   owner made the rule a per-project choice among `one-wrapper-folder` (the
   default, which keeps every original scenario true), `longest-shared-prefix`
   and `none`, built so further rules are easy to add; issue #5 gained three
   scenarios for it. The guard that leaves a sole top-level folder alone when
   it holds no folder of its own belongs to `one-wrapper-folder` and is what
   makes the sixth scenario's district `build` come out.

2. **Resolved during drafting, and visible: hidden *files* are not excluded.**
   The product owner's 2026-10-09 ruling reads "anything git ignores is left
   out, individual files as well as folders, together with hidden folders" — it
   says *files as well as folders* for git, and *folders* for hidden. Read
   literally, a hidden file git does not ignore is a node: `.gitignore`,
   `.gitattributes` and `.editorconfig` appear in the project-named district of
   most repositories, and in a folder that is not a repository, so does `.env`.
   I took the ruling literally rather than widening it, because widening it
   would also hide `.gitignore` from the engineer whose ignore rules are the
   thing shaping the map. Flagging because it is a visible product consequence
   and one sentence from the owner settles it; STORY-005's control over ignored
   files is the natural place for the answer to live.

3. **Resolved during drafting: the opening gesture has to be built here.** The
   intent says "This story is about what appears once that happens, not the
   gesture itself", and describes double-click as "the existing
   double-click-in-the-universe gesture". It does not exist: STORY-002 draws the
   universe with `ui.label`, which cannot be double-clicked
   (`crates/workspace/src/ui.rs:43`). Nothing can be verified without it, so
   requirement 1 builds it. Requirement 2 adds Esc with it, because a map the
   engineer cannot leave is worse than no map; `Map.md` pairs the two in one
   sentence, so this is not new behaviour, only behaviour arriving a story
   earlier than Navigation will formalise it. STORY-004 adds Enter as the
   keyboard twin and is unaffected.

4. **The empty-project wording.** Requirement 26 needs a sentence, and
   `factory-context/05-case/` names no document that owns UI copy. Unlike
   STORY-002's empty state, this one publishes no internal path and creates no
   contract, so I have proposed wording rather than holding the artifact for it:
   *"This project has no files to show."* If the product owner wants different
   copy, it is a one-line change to a constant and to one assertion.

5. **Resolved during drafting: the binary gains an arrow to `cellworks-map`.**
   The alternative was re-exporting `ProjectMap` from `cellworks-code-analysis`,
   which the binary already depends on. I rejected it: the Map's graph is the
   published language of three contexts (`04-solution-strategy.md` §4.2), and
   laundering it through one loader would make the next loader's re-export look
   equally reasonable. The explicit arrow is cheap — `tests/workspace_structure.rs`
   is built to be edited alongside the architecture document — and it is honest
   about the binary owning the frame loop and therefore the painter it calls.

6. **Scope note, not a design concern: `CodeAnalysis.md` is delivered by a
   sliver.** Its three behaviours are AST metrics, dependency trees and git
   state. This story delivers none of them: it delivers the file list that the
   map is built from, which the document does not mention because it is
   `Map.md`'s side of the same work. After this story `CodeAnalysis.md` will
   look untouched and the crate will not be. Recommendation for the plan: do
   not amend `CodeAnalysis.md` — it describes the target, not the current state
   — and let the epic track the rest. Flagging so the gap is on the record.

7. **`.claude/skills/secure-api-review` was read and does not apply — but this
   story does raise one security question, and I have answered it by
   construction.** The skill's trigger is an external-facing endpoint, a
   request or response schema, or an OpenAPI spec; this story adds none, and
   Cellworks runs no network server (`02-constraints.md` §2.1,
   `08-crosscutting.md` §8.4). There is no conflict to escalate. The real
   question is the one ADR-0003 raises: this is the first child process the
   product spawns, and the untrusted value it handles is a path the engineer
   chose. Two rules, both held by construction rather than by review attention
   (requirement 32):

   - The repository path is the child's **working directory**
     (`Command::current_dir`), never an argument. A project folder named
     `--upload-pack=…` therefore cannot be read by git as an option, because it
     never reaches git's argument vector at all. A `-C <path>` form would have
     left exactly that hole.
   - No shell is involved: `std::process::Command` with a fixed argument list,
     so no quoting rule has to be got right.

   For the record: no secret, token or credential is read, written or logged
   anywhere in this story, and `git ls-files` writes nothing. Running git inside
   the engineer's own folder does execute that folder's git configuration — that
   is inherent to ADR-0003's "reuse the user's configuration" and is accepted,
   not introduced here. The plan should set `GIT_OPTIONAL_LOCKS=0` on the call
   so a read never takes the index lock out from under a session working in the
   same repository.

8. **A note for the plan: the git scenarios need a recorded listing, not real
   git.** `08-crosscutting.md` §8.6 allows real tools only in smoke and
   measurement tests, and the fifth scenario is about what git ignores. That is
   why `GitListing` is a parameter (requirement 31): the acceptance tests pass a
   recorded listing over a real temporary folder tree, and one `#[ignore]` test
   runs the real `GitCli` against a repository it creates, skipping when git is
   absent. Without that seam the scenario would either run real git in the
   acceptance suite or not be tested at all.

## Verification

Per `08-crosscutting.md` §8.6 and `guidelines/testing.md`. Unit tests sit beside
the code in `crates/map` and `crates/code-analysis`. The ten scenarios of
issue #5 become ten acceptance tests, **keeping the issue's titles**, in
`tests/story_003_see_a_code_project_as_districts.rs` at the repository root —
the root, because each scenario crosses Workspace (the gesture), CodeAnalysis
(the read) and Map (the graph and the view). Every test builds its own tree
under a `tempfile::TempDir` and passes a recorded `GitListing`; none runs git,
reads `$HOME`, or opens a window.

### The scenarios

| Scenario (test name) | How it is proven |
|---|---|
| Top folders below the common prefix become districts | A tree with `src/catalog/item.rs` and `src/ordering/order.rs`; double-click the project in the universe; the frame labels the districts `catalog` and `ordering`. |
| The next folder level becomes sub-districts, deeper folders become groups | `src/catalog/pricing/price.rs` and `src/catalog/pricing/rules/discount.rs`; the frame shows district `catalog` containing sub-district `pricing`, containing group `rules` with node `discount.rs`, and node `price.rs`. Containment is asserted on the geometry (`area` of the child inside `area` of the parent), not on the accessibility tree order. |
| Files directly in a district are its nodes | `src/catalog/item.rs`, `src/ordering/order.rs`; node `item.rs` is inside district `catalog`. |
| Files at the top of the project form a district named after the project | Project `shop` with `README.md` and `src/catalog/item.rs`; two districts, `shop` holding node `README.md`, and `src`. |
| Hidden folders, and folders git ignores, are left out | A repository with `.cache/stale.bin`, `build/output.o` and `src/catalog/item.rs`, and a recorded listing that omits `build/`; nothing labelled `.cache` or `build` is drawn. |
| A folder that is not a git repository leaves out only hidden folders | The same tree with no `.git`, so no listing is asked for; nothing labelled `.cache`, **and** district `build`. The fixture keeps `build` holding `output.o` directly — the scenario's `Given` does not pin it, and the district the `Then` names is what fixes the choice ([Concerns](#concerns), item 1). |
| A project without files still opens | An empty folder; the project opens and the frame shows requirement 26's message and no district. |
| A project set to the longest shared prefix cuts districts below every folder its files share | `src/main/java/acme/{billing/Invoice.java,orders/Order.java}` with the project's `district_prefix` set; districts `billing` and `orders`. |
| A project set to no prefix shows its top folders as districts | `src/catalog/…`, `src/ordering/…` with `none`; district `src`. |
| An unknown district prefix falls back to the default and says so | The same tree with `sideways`; districts `catalog` and `ordering`, and a notice naming `sideways`. |

### The requirements

| Req | How it is proven |
|---|---|
| 1 | Acceptance: a harness over the universe with two projects; a double-click on the first, and the next frame labels that project's districts and neither project name. |
| 2 | Acceptance: from the opened project, Esc, and the next frame labels both projects again. |
| 3 | Unit: `load` returns its events with no thread. Acceptance: with a `GitListing` that blocks, the harness still runs frames and stays in Reading. Plus `grep` over `src/` and `crates/`: no `async`, no `tokio`, no `recv()` without `try_`. |
| 4 | Acceptance, in order: a harness built before any event shows Reading and **not** the empty message; then `ProjectMapped` is applied and the map appears. A second case drops the sender without reporting and the frame says the project could not be read. |
| 5 | Unit, three cases: `.git` as a directory, `.git` as a file (a linked worktree), and no `.git` — the first two ask the listing, the third does not. The test double records whether it was asked. |
| 6 | Unit: a recorded listing omitting `build/output.o` over a tree that contains it, and no node for it. |
| 7 | Unit: a recorded listing naming `gone.rs` as both visible and deleted, and no node for it. |
| 8 | Unit: `.cache/stale.bin` and `.github/workflows/ci.yml` are both in the recorded listing, and neither appears; nor does any district, sub-district or group named after a dotted folder. |
| 9 | Unit: a non-repository tree with `.cache/x` and `build/x`, and exactly one district, `build`. |
| 10 | Unit: a `GitListing` returning `Err`, and the result holds `GitExcludesUnavailable` naming the project **and** a map built from the plain walk. Acceptance: that map is drawn with a notice beside it. |
| 11 | Unit: a tree with an empty folder `docs/` beside `src/catalog/item.rs`, and no district, sub-district or group named `docs`. |
| 12 | Unit, per rule. `one-wrapper-folder`: one top-level folder holding folders (prefix taken); one top-level folder holding only files (not taken); two top-level folders (not taken); one top-level folder and a loose file (not taken); "at most once": root → `src` → `catalog` → `pricing` gives district `catalog`. `longest-shared-prefix`: `src/main/java/acme/{billing,orders}` gives `billing`, `orders`. `none`: `src/…` gives `src`. Acceptance: the two new scenarios. |
| 12a | Unit in the Workspace: a settings file naming a project's path gives that project its `district_prefix`; an unresolved key is ignored. |
| 12b | Acceptance: the scenario "An unknown district prefix falls back to the default and says so". |
| 12c | By inspection: the rules are one `const` table in `crates/map/src/districts.rs`. |
| 13–17 | Unit on `Directories`, one case per rule, asserted on the nesting the strategy returns rather than on the drawn frame: district naming, sub-district naming, a group inside a group three levels down, a node under each of the three levels, and loose files at the district level under a district named after the project. |
| 18 | Unit: a project `shop` with a top-level folder `shop` and a loose file yields two districts with the same label and different paths; and every district, sub-district, group and node carries the canonical path of what it stands for. |
| 19 | Unit: the same file list in shuffled order produces an identical `ProjectMap`; and the names at each level come back sorted byte-wise. |
| 20 | Unit, as a property over a generated tree (a few hundred files, varied depth and breadth): every pair of rectangles at the same level is disjoint, and every child's rectangle is strictly inside its parent's. One test, not a collection of examples, because the invariant is a property of the packing. |
| 21 | Unit: in a map with two districts each holding two sub-districts each holding two groups, the gap between sibling districts exceeds the gap between sibling sub-districts, which exceeds the gap between sibling groups. |
| 22 | Unit: the same file list laid out twice gives equal geometry, asserted field by field. |
| 23 | Unit: a project with ten files and one with a thousand, and the extent of the second is bounded by a small multiple of the first's per-file area — the map grows with content and does not string out. |
| 24 | Acceptance: with a panel of a given size, every district's rectangle lies inside the panel. |
| 25 | Acceptance: the frame's accessibility tree carries one label per district, sub-district, group and node of the map. |
| 26 | Acceptance: an empty folder, and the frame shows the message and no district. A second case: a tree whose every file is hidden or ignored, and the same message — the point being that "no files" is after the exclusions, not before. |
| 27 | Unit: a tree with a folder at mode `0o000`, probing first that the mode bites (the pattern `crates/workspace/src/discovery.rs:201` already uses), and the result holds `ProjectFolderSkipped` naming it **and** the districts found elsewhere. A second case: a project root that does not exist. Plus `grep` over the two crates: no `unwrap`/`expect` outside `#[cfg(test)]`. |
| 28 | By construction and by inspection: `DistrictStrategy` and `Directories` live in `crates/map`; `grep` finds no district, sub-district or group rule in `crates/code-analysis`. `10-quality.md` E2 is the measure — a second strategy would touch only `crates/map/src/districts.rs`. |
| 29 | Compile-time: the types and the enum exist with those names and past-tense variants. `grep -ri` over `src/` and `crates/` for the theme words (planet, factory, machine, crate, servitor, belt) finds none in an identifier. |
| 30 | `cargo test --workspace` runs `tests/workspace_structure.rs`, whose `GRAPH` now holds `bin → cellworks-map` and nothing else new; `git diff` shows `docs/arc42/05-building-blocks.md` §5.2 changed in the same commit. |
| 31 | Two parts. By construction: `GitListing` is a parameter, and `GitCli` is named only in `src/` — proven by `grep`. By test: the suite passes with `HOME` set to an empty temporary directory, and with `git` removed from `PATH`. |
| 32 | By construction and by test: `grep` over `crates/code-analysis` finds no `-C` argument and no shell invocation; a unit test on the adapter's command builder asserts the argument vector is fixed and the repository is set as `current_dir`. A second test opens a project whose folder name begins with `--` and gets a map rather than a git error. |
| 33 | Measurement, `#[ignore]`, beside the acceptance tests of `cellworks-map`: a synthetic tree of 20,000 files in 40 districts, printing the time to cut and lay out, and the resulting extent. It asserts nothing — the issue sets no target — and the numbers go into the pull request as the first data point for EPIC-002. |
| 34 | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `SHELLCHECK_OPTS=--exclude=SC2016 actionlint`, run locally and in CI, output pasted into the pull request. |

Two checks by hand on macOS, recorded in the pull request and outside the
automated suite, because no automated test opens a window or runs real git: open
a real repository and confirm that `node_modules` or `target` is absent and the
source tree reads as districts; and open the same folder with `git` renamed away
and confirm requirement 10's notice.
