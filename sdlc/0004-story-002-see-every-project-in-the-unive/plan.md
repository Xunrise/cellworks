---
stage: plan
issue: 4
status: draft
author: claude (sdlc-plan)
date: 2026-10-08
---

# Plan: See every project in the universe

Derived from `spec.md` in this directory.

## The spec's blocked line, carried forward

`spec.md` → Concerns, item 1 carries an unresolved line. It is reproduced here
verbatim, unplanned-around, so the gate holds this artifact until the product
owner answers:

**Resolved by the product owner, 2026-10-08.** The empty state reads, with the
real projects folder and settings path filled in:

`No projects yet. Put a git repository in <projects folder>, or add a folder's path to <settings file>.`

The clause about the settings file is replaced when EPIC-008 brings the
interface for adding a folder.

**What it touches, and what it does not.** Requirement 20 is one string in one
function (`crates/workspace/src/ui.rs`) and one `assert` in one acceptance test.
Steps 1–5, 7 and 8 below are independent of the answer; step 6 builds the empty
state's structure and its test, and the literal copy is the single thing left
waiting. This plan does not invent that copy, and no step may ship a guess at it.

**What the answer changed in the plan, recorded at build time.** The copy names
the projects folder and the settings file, so the view needs the two paths the
scan was run against. `Universe` therefore carries the `Sources` it is scanning
(see step 5), and `ui.rs` fills them into the string. The `$HOME`-unset case has
no paths to name — there is no projects folder and no settings file — so its
empty state is a sentence of its own, and it is a failure-mode message rather
than requirement 20's held copy.

## What the repository holds today

Read before writing, and every claim below was checked against the files.

- `crates/workspace/` is one `Cargo.toml` (five lines: name, version, edition,
  publish) and one `src/lib.rs` that is a `//!` comment and nothing else. The
  crate exposes no item, so there is no existing public API to preserve and no
  caller of this crate to break.
- `src/app.rs` is 64 lines: `CellworksApp` (a unit struct — "it holds no state
  yet"), `draw(ui: &mut egui::Ui)` painting one empty `CentralPanel`
  (`src/app.rs:17-19`), `native_options()`, `run()`, and two unit tests on
  `native_options`. **The only callers of `draw`** are `src/app.rs:12`,
  the re-export at `src/lib.rs:10`, and `tests/empty_window.rs:18` — grepped,
  nothing else in the repository names it.
- `tests/empty_window.rs` asserts that the frame's accessibility tree carries
  no node outside `[Role::Window, Role::GenericContainer]` and that no node has
  a label or a value (`tests/empty_window.rs:14, 27-32`). The moment the
  universe draws one project, that test is false. The spec retires it
  deliberately (Concerns, item 2).
- **The egui_kittest pattern already exists in this repo** and is what the new
  acceptance tests reuse rather than re-invent: `Harness::new_ui(…)`,
  `harness.run()`, `harness.root()`, `root.children_recursive()`,
  `node.accesskit_node()`, `.role()`, `.label()`
  (`tests/empty_window.rs:8-32`). Nothing new has to be learned about the
  harness to assert on labels — only the direction of the assertion flips.
- `tests/workspace_structure.rs` filters dependency keys on the `cellworks-`
  prefix (`tests/workspace_structure.rs:113`) and expects
  `cellworks-workspace`'s set to be `&[]` (line 78). Adding `egui`, `toml` and
  `tempfile` to that manifest therefore leaves it green and
  **requirement 23 holds by this story adding no `cellworks-*` arrow at all**.
  `the_root_manifest_has_one_version_line` (lines 247-271) is why no dependency
  below is written in `[section.name]` table form.
- `toml 1.1.7` and `egui_kittest 0.36.2` are already in `Cargo.lock`; `toml` is
  a root dev-dependency today (`Cargo.toml:32`). `tempfile 3.27.0` is **already
  in the lock graph** transitively, which is what decides spec Concern 8 below.
- `docs/services/cellworks/decisions/` holds no entry ("No decisions recorded
  yet"), so no component decision constrains this change.

**Context read.** `factory-context/05-case/` exists. Its `AGENTS.md` routes
every run back to the jarvis docs in this repository rather than holding content
of its own; `README.md` indexes the same. Both were read as data; neither
contains an instruction addressed to this stage, and nothing under
`factory-context/` changes. The documents it names were read:
`docs/arc42/05-building-blocks.md` §5.2, `docs/arc42/08-crosscutting.md`
§8.1–8.6, `docs/arc42/10-quality.md` (R1, P2), `docs/domain/glossary.md`
(`Project`), `docs/domain/contexts/Workspace/{Workspace.md,glossary.md}`, and
`docs/services/cellworks/guidelines/{architecture,naming,testing}.md`.

**Scenario titles come from issue #4.** `guidelines/testing.md` requires one
acceptance test per Gherkin scenario, named after the scenario in `snake_case`,
with the scenario title repeated in a doc comment. `spec.md` Concern 7 records
that the scenarios could not be read at spec time; `gh` is likewise not
reachable from this session. **Step 6 opens issue #4 first and takes the test
names from its scenarios**; the names given below describe the behaviour and are
placeholders for them, not a substitute.

**At build time issue #4 could not be opened either**: `gh` in the build session
has no token. The build named the acceptance tests after the behaviour they
pin; after review, the five scenarios of issue #4 were added as tests named
after them, ahead of the behaviour tests, which stay and name their
requirement of `spec.md`.

## Files that change

Twenty paths. If implementation touches a path that is not on this list, this
plan was wrong and gets updated in the same commit.

Added after review, by the product owner's decision on 2026-10-09:
`.gitignore` gains `/Claude outputs/`, the folder local Claude sessions write
to, after a commit swept it into this branch; and `crates/workspace/src/ui.rs`
wraps the universe in a vertical scroll area, pinned by
`a_universe_taller_than_the_window_scrolls`, so no project is cut off below the
window.

### Manifests

| Path | | What changes |
|---|---|---|
| `.gitignore` | (edit) | Ignores `/Claude outputs/`. |
| `Cargo.toml` | (edit) | `toml = "1"` moves from `[dev-dependencies]` into `[workspace.dependencies]` (a second package now uses it); the root's `[dev-dependencies]` entry becomes `toml = { workspace = true }`. `tempfile = "3"` is added to `[workspace.dependencies]`. Written as one-line inline tables only — a `[workspace.dependencies.toml]` section would put a bare `version = "…"` at the start of a line and break `the_root_manifest_has_one_version_line`. No `[dependencies]` key changes, so `dependency_sets_match_the_building_block_view` is untouched. |
| `crates/workspace/Cargo.toml` | (edit) | `[dependencies]`: `egui = { workspace = true }`, `toml = { workspace = true }`. `[dev-dependencies]`: `egui_kittest = { workspace = true }`, `tempfile = { workspace = true }`. No `cellworks-*` key is added. |
| `Cargo.lock` | (edit) | Regenerated by cargo; committed. |

### The Workspace crate

| Path | | What it holds |
|---|---|---|
| `crates/workspace/src/lib.rs` | (edit) | The module declarations, the re-exports that are the crate's first public API, `Sources` and `Sources::for_this_user`, and `scan` / `start_scan`. The doc comment stops saying "Nothing is exposed yet". |
| `crates/workspace/src/project.rs` | (new) | `Project { id: ProjectId, name: String }`, `ProjectId` (the canonical path), `FolderIdentity` (device + inode) and its constructor from a path. |
| `crates/workspace/src/event.rs` | (new) | `WorkspaceEvent` with the four past-tense variants of the spec. |
| `crates/workspace/src/discovery.rs` | (new) | The walk under the projects folder (requirements 1–5). |
| `crates/workspace/src/settings.rs` | (new) | The settings file path builder and the reader (requirements 11–15). **No write function, no `OpenOptions`, no `File::create`** — requirement 12 by construction. |
| `crates/workspace/src/registry.rs` | (new) | Merging the two sources into one deterministic project list (requirements 6–10). |
| `crates/workspace/src/universe.rs` | (new) | `Universe` — the view state and the drain. **A departure from the spec's module sketch,** which folded this into `ui.rs`; see *Rationale* in step 5. |
| `crates/workspace/src/ui.rs` | (new) | `ui::universe(ui, &Universe)`: the scanning, populated and empty states, and the notices. The only module that depends on egui beyond geometry types. |
| `crates/workspace/src/test_tree.rs` | (new, `#[cfg(test)]`) | The builders every unit test uses to raise a tree in a temporary directory: `TestTree::new()`, `.repository(rel)`, `.worktree_repository(rel)`, `.plain_folder(rel)`, `.file(rel, text)`, `.symlink(target, link)`, `.sources()`. |
| `crates/workspace/tests/story_002_see_every_project_in_the_universe.rs` | (new) | The acceptance tests, one per scenario of issue #4, driving `ui::universe` headlessly. |

### The binary

| Path | | What changes |
|---|---|---|
| `src/app.rs` | (edit) | `CellworksApp` gains `universe: Universe` and `events: Option<Receiver<WorkspaceEvent>>`; `CellworksApp::start()`; `apply_pending(&mut self)`; `draw` takes the universe state. |
| `src/lib.rs` | (unchanged, as built) | The `pub use app::draw;` line stays and now re-exports the two-argument `draw` — a re-export follows the signature, so the file needed no edit at all. |
| `tests/empty_window.rs` | (delete) | Retired, as `spec.md` Concern 2 records. It is the proof of STORY-001's requirement 7, and STORY-001's own spec calls the empty panel "the seam the map canvas replaces". |
| `tests/universe_frame.rs` | (new) | Its replacement, and the binary's own responsibility rather than the crate's: that the frame body renders the Workspace universe, and that `apply_pending` drains without blocking. |

### Documentation

| Path | | What changes |
|---|---|---|
| `docs/arc42/08-crosscutting.md` | (edit) | §8.5 only. Today it reads "Paths are compared case-insensitively on macOS" — read as string normalisation that folds case on Linux too, contradicting the product owner's 2026-10-08 ruling. It is reworded to say sameness is the filesystem's answer (device + inode), identity is the canonical path, and never the spelling a folder was added with. Recommended by `spec.md` Concern 5; `factory:standards-change` requires the docs stay true in the same commit. |

### Files that must stay unchanged

`docs/arc42/05-building-blocks.md` and `tests/workspace_structure.rs`
(requirement 23); `CLAUDE.md` → *Verifying your work* and
`.github/workflows/sdlc-build.yml:27`, whose `verify-command` must stay
character-identical to it; `docs/domain/contexts/Workspace/Workspace.md`
(`spec.md` Concern 3: it describes the target, not the current state, and the
epic tracks the half this story does not deliver); everything under
`factory-context/`; and `intent.md` and `spec.md` in this directory — the
approved upstream record, which a build session may not edit. If implementation
needs one of them changed, it stops and says so.

Note that §5.2's crate table says `workspace` has "case-insensitive paths on
macOS". Requirement 23 holds that file unchanged, so that phrasing is **not**
corrected here; §8.5, which is the load-bearing statement, is. The leftover is
named in Risks rather than fixed silently.

## Order of work

Eight steps. Each leaves the repository building, formatted, clippy-clean and
with `cargo test --workspace` green; no step needs a later step to compile.
Steps 1–5 add no egui code at all, which is what keeps the risky filesystem
rules provable without a harness.

### 1. The manifests, the vocabulary, and the test tree

Make the two manifest edits. Then add `project.rs`, `event.rs`, the `Sources`
type in `lib.rs`, and `test_tree.rs`.

- `Sources { projects_folder: PathBuf, settings_file: PathBuf }`.
  `Sources::for_this_user() -> Option<Sources>` reads `$HOME` and
  `$XDG_CONFIG_HOME` through `std::env::var_os` and returns `None` when `$HOME`
  is unset — and, as built, when it is set but empty, because an empty `$HOME`
  would otherwise make the projects folder the relative path `Projects` and walk
  whatever the working directory happened to be. An empty `$XDG_CONFIG_HOME` is
  read as absent for the same reason. **This is the only function in the crate
  that reads the environment**, which is what makes requirement 22 true by
  construction rather than by remembering.
- `ProjectId(PathBuf)` — the canonical path, from `std::fs::canonicalize`.
  `Project { id, name }`, `name` being the last component of the canonical path
  (requirement 10). `FolderIdentity { dev: u64, ino: u64 }`, built from
  `std::fs::metadata` through `std::os::unix::fs::MetadataExt`, deriving
  `PartialEq, Eq, Hash` so it can be a map key.
- `WorkspaceEvent` with exactly the four variants of the spec:
  `ProjectsDiscovered(Vec<Project>)`, `ProjectsFolderUnreadable { path, reason }`,
  `AddedFolderSkipped { path, reason }`, `SettingsUnreadable { path, reason }`.
  Past tense, neutral terms, no Warhammer 40k word (requirement 17).
- `test_tree.rs` resolves `spec.md` Concern 8, which asked the plan to choose:
  **`tempfile::TempDir` for the directory, in-repo builder functions for the
  tree.** `tempfile` is already in `Cargo.lock` transitively, so the marginal
  build cost is zero, and it gets unique names and removal-on-unwind right —
  which a hand-rolled `Drop` would have to re-earn. The builders are the
  `a_project()`-style functions `guidelines/testing.md` asks for, and they are
  in a crate-private `#[cfg(test)]` module from the start because three unit
  test modules need them on day one.
- Unit tests here: two paths to the same directory have equal `FolderIdentity`;
  two different directories do not; `Project::name` is the canonical path's last
  component.

*Leaves working:* the crate exposes types and nothing that walks yet.

### 2. The walk (requirements 1–5)

`discovery.rs` exposes one crate-private function taking `&Path` and returning
the canonical paths it found plus any `WorkspaceEvent`s.

- `std::fs::read_dir` with an explicit stack, recursing by hand. Each entry's
  kind comes from `DirEntry::file_type()`, **not** `metadata()`, because
  `file_type()` does not follow a symlink and so delivers requirement 4 without
  a second decision. An entry that is a symlink is skipped outright.
- An entry whose name begins with `.` is skipped (requirement 4) — which is also
  why no symlink loop can hang the scan, since nothing is followed.
- A directory directly containing an entry named `.git`, of **either** kind
  (directory or file — a linked worktree writes a file), is a project, and the
  walk does not descend into it (requirement 2). A directory that is not a
  repository is still descended into (requirement 3).
- No depth cap. A cap would silently hide a project, and the two stopping rules
  already prevent the walk entering `node_modules`, `target` or `vendor`.
- A projects folder that does not exist yields nothing and **no event**
  (requirement 5). A `read_dir` that fails at the root or part-way down yields a
  `ProjectsFolderUnreadable { path, reason: error.to_string() }` for that path
  and the walk continues. No `unwrap`, no `expect`, anywhere near the
  filesystem.
- Unit tests, using `test_tree`: two repositories found by canonical path; `.git`
  as a directory; `.git` as a file; a nested repository not becoming a second
  project; a plain sibling folder absent; a repository two levels below a plain
  folder present; a dotted directory holding a repository yielding nothing; a
  symlink to a sibling repository yielding no second project; a symlink to its
  own ancestor terminating; an absent projects folder giving an empty list and
  no event; an unreadable one giving `ProjectsFolderUnreadable`.

**Mitigation, in this step rather than deferred.** The unreadable-directory test
sets mode `0o000` through `std::os::unix::fs::PermissionsExt` — and a process
running as **root ignores that bit entirely**, so the test would pass vacuously
in any root container even though it passes honestly on the GitHub runners,
which are not root. The test therefore *probes first*: it attempts
`fs::read_dir` on the chmodded directory itself and, if that succeeds, fails
with "this test needs a non-root user" rather than reporting a green that proves
nothing. The same test restores mode `0o755` before the `TempDir` drops, because
`remove_dir_all` cannot remove a `0o000` directory and the cleanup would
otherwise leak.

*Leaves working:* requirements 1–5, proven without a thread, a channel or a
frame.

### 3. The settings file (requirements 11–15)

`settings.rs`, read-only.

- The path builder is a pure function with the environment as arguments, which
  is what makes requirement 11 testable on both runners:
  `settings_file(platform: Platform, home: &Path, xdg_config_home: Option<&Path>) -> PathBuf`,
  with `Platform::{MacOs, Xdg}`. macOS →
  `$HOME/Library/Application Support/Cellworks/settings.toml`; Xdg →
  `$XDG_CONFIG_HOME/cellworks/settings.toml`, else
  `$HOME/.config/cellworks/settings.toml`.
  `Sources::for_this_user` picks the platform with
  `cfg!(target_os = "macos")`. **This is not the thing requirement 8 forbids:**
  requirement 8 bans a `cfg!(target_os)` encoding the *case rule*, and no such
  cfg exists anywhere in this story. Where a settings file lives genuinely
  differs per platform. A reviewer will meet this cfg and should meet this
  sentence with it.
- The reader returns the `added_folders` entries plus any `WorkspaceEvent`.
  `fs::read_to_string`, then `text.parse::<toml::Table>()` — the same `toml`
  crate and the same `parse::<Table>()` call `tests/workspace_structure.rs:93`
  already uses, read as a `Table` so no serde derive enters the crate. A missing
  file is `Ok(no folders)` and **no event** (requirement 13). An unreadable or
  unparseable file is `SettingsUnreadable { path, reason }` (requirement 15).
  A key the file carries that this story does not understand, and a
  non-string element inside `added_folders`, are ignored, not rejected
  (`08-crosscutting.md` §8.2 tolerant parsing).
- Each entry is then checked: a relative path, a path that does not exist, a
  path that is not a directory, and a path that cannot be canonicalised each
  yield `AddedFolderSkipped { path, reason }` and no project (requirement 14).
  `~` is not expanded.
- **The module exposes no write function.** Not `fs::write`, not `File::create`,
  not `remove_file`, not `OpenOptions`. There is nothing for a later edit to
  call by accident (requirement 12).
- Unit tests: the three paths of the table from a known `HOME` and
  `XDG_CONFIG_HOME`; an absent file giving no folders and no event; four entries
  in one file (missing, a regular file, relative, valid) giving one folder and
  three `AddedFolderSkipped` naming their paths; a truncated `added_folders = [`
  giving `SettingsUnreadable` naming the file; a file with `added_folders` plus
  an unknown key parsing normally and emitting nothing; and a file that is
  byte-identical after being read.

*Leaves working:* requirements 11–15, with no path into the real application
support directory from any test.

### 4. The merge, `scan`, and `start_scan` (requirements 6–10, 16)

`registry.rs`, then the two public functions in `lib.rs`.

- `pub fn scan(sources: &Sources) -> Vec<WorkspaceEvent>` — an ordinary blocking
  function. It runs the walk, reads the settings, merges, and returns the events
  with exactly one `ProjectsDiscovered` among them. Being separate from
  `start_scan` is what lets every rule above be unit-tested without a thread, a
  channel or a frame.
- The merge keys a `HashMap<FolderIdentity, Project>` and inserts in a fixed
  precedence: everything discovered under the projects folder first, sorted by
  canonical path; then everything from the settings file, sorted by canonical
  path. First insertion wins, so the universe is identical whichever order the
  sources were seen in (requirement 9). The result is sorted by canonical path,
  byte-wise, before it goes into `ProjectsDiscovered` (requirement 10).
- A path whose `FolderIdentity` cannot be read at merge time is dropped with the
  event its source calls for, never with an `unwrap`.
- `pub fn start_scan(sources: Sources) -> mpsc::Receiver<WorkspaceEvent>` spawns
  a `std::thread`, calls `scan`, sends each event, and lets the sender drop. A
  send that fails because the UI has gone away ends the thread quietly; it is
  not an error worth reporting to a receiver that no longer exists.
- Unit tests: a settings file naming a non-git folder outside the projects
  folder returns it as a project; a folder both discovered and listed yields
  exactly one project with the canonical path as its id; the same two sources in
  both orders produce an identical list; a tree created in shuffled order comes
  back sorted; an unreadable projects folder still reports the added folders;
  and `start_scan` delivers a `ProjectsDiscovered` over the channel.
- **Requirement 8's test asks the filesystem first,** and this is the step's own
  mitigation rather than a deferred one. It creates `Alpha`, probes whether
  `alpha` opens the same directory on this volume, then asserts **one** project
  if the probe says the volume is case-insensitive and **two** if it does not.
  The same test is meaningful on the macOS runner and the Linux runner, and it
  fails the moment anyone hard-codes a case rule into the code.

*Leaves working:* the whole registry, with no UI anywhere.

### 5. The universe state (requirements 18, 21, and the scan that dies)

`universe.rs`. **Rationale for the departure:** `spec.md`'s module sketch put
the state in `src/app.rs` and only the drawing in the crate. But
`guidelines/testing.md` puts acceptance tests in the crate that owns the
behaviour, and requirements 18 and 21 are about *which state is shown when* —
behaviour, not drawing. Keeping the state in the binary would force every one of
those tests into the root `tests/`, away from the crate that owns them. So the
state lives in the crate and the binary owns an instance of it. Nothing else in
the spec changes: `src/app.rs` still holds the receiver, still starts the scan,
still drains once per frame.

- `Universe { scan: ScanState, notices: Vec<Notice>, sources: Option<Sources> }`
  with `ScanState::{ Scanning, Reported(Vec<Project>), Failed }`.
  `Universe::scanning(&Sources)`, `Universe::without_home()` (the `$HOME`-unset
  path, which starts already reported and already empty, and has no projects
  folder or settings file it could name), `apply(&mut self, WorkspaceEvent)`,
  and `drain(&mut self, &mpsc::Receiver<WorkspaceEvent>)`.
  **`sources` and the two constructor names are a departure from the sketch
  above**, which had `scanning()` and `reported(Vec<Project>)`: the empty
  state's copy, settled on 2026-10-08, names the projects folder and the
  settings file, so the view has to be able to read them off the universe it is
  drawing. A `Notice { kind, path, reason }` carries the failure and `ui.rs`
  turns it into a sentence, so no user-facing word lives outside `ui.rs`.
- `drain` loops on `try_recv` and never blocks (requirement 16).
  `TryRecvError::Empty` ends the loop. `TryRecvError::Disconnected` is the
  subtle one and gets its own test: after a successful scan the sender drops
  too, so disconnection is **only** a failure when the state is still
  `Scanning` — that is the "scan thread panics" row of the spec's failure-mode
  table, and it is what stops the view showing "Scanning" for ever.
- `ProjectsFolderUnreadable`, `AddedFolderSkipped` and `SettingsUnreadable` each
  append a notice and **never replace the project list** (requirement 21, §8.2,
  R1).
- Unit tests: a fresh `Universe` is `Scanning`; `ProjectsDiscovered` with
  projects reaches `Reported`; with none, also `Reported` but empty;
  a notice arriving before `ProjectsDiscovered` leaves the state `Scanning`;
  a notice arriving after it leaves the project list intact; disconnect while
  `Scanning` reaches `Failed`; disconnect after `Reported` does not.

*Leaves working:* requirement 18's ordering is already provable here, before any
pixel is drawn.

### 6. The view and the acceptance tests (requirements 19–21, and the held 20)

**Open issue #4 first and take the scenario titles from it** — one `#[test]` per
scenario, named after it in `snake_case`, the title repeated in a doc comment,
all in `crates/workspace/tests/story_002_see_every_project_in_the_universe.rs`.

`ui.rs` exposes `pub fn universe(ui: &mut egui::Ui, universe: &Universe)`,
drawing one label per project in list order for `Reported` with projects, the
scanning text for `Scanning`, the empty state for `Reported` with none, the
failure text for `Failed`, and the notices alongside whichever is current. No
physics layout, no districts, no zoom, no level of detail, no selection, no
keyboard navigation — `spec.md` Concern 4 draws that boundary, and
`crates/map` is not touched.

The tests reuse the harness pattern of `tests/empty_window.rs` verbatim, with
the assertion inverted: build a `Universe`, run
`Harness::new_ui(|ui| ui::universe(ui, &universe))`, walk
`root.children_recursive()` and collect `accesskit_node().label()`. They assert
on what is visible, never on private fields. Expected:

- a `Universe` that has had no event drained shows the scanning text and does
  **not** show the empty state's text — then `ProjectsDiscovered([])` is applied
  and the empty state appears, which proves the *order* and not just the end
  state (requirement 18);
- `ProjectsDiscovered` with three projects puts exactly three labels in the
  tree, in sorted order (requirement 19);
- a `SettingsUnreadable` event shows a notice naming the settings file **and**
  the view still lists the projects that were found (requirement 21);
- a channel whose sender the test still holds: frames run, nothing blocks, the
  view stays in Scanning (requirement 16, without having to wedge a real
  thread).

**Requirement 20 stops here.** The empty state's structure, its test, and the
assertion's shape are written; the string is not. That is the `> BLOCKED:` line
at the top of this plan, and this step may not ship a guess at the copy. Until
the product owner answers, the empty-state test asserts the behaviour
(the project list is empty, a guidance node is present) and takes its literal
when the answer arrives.

**The answer arrived before the build, so the literal is in.** The empty state
reads `No projects yet. Put a git repository in <projects folder>, or add a
folder's path to <settings file>.` with the universe's own `Sources` filled in,
and the acceptance test asserts that whole string. The `$HOME`-unset universe
has no paths to fill in and says so in a sentence of its own; that sentence is
the spec's failure-mode row, not requirement 20's copy.

A note on how the tests read the frame: egui carries a label's text in the
accessibility node's **value**, not its label, and repeats it on the text runs
below, so `labels_in` reads the `value()` of the `Role::Label` nodes. That is
the same tree `tests/empty_window.rs` walked — it asserted both were `None`,
which was true of an empty panel either way.

*Leaves working:* everything, including the string.

### 7. Wiring the binary

- `CellworksApp` gains `universe: Universe` and
  `events: Option<mpsc::Receiver<WorkspaceEvent>>`, and a
  `CellworksApp::start()` that calls `Sources::for_this_user()`: `Some` →
  `start_scan` and `Universe::scanning(&sources)`; `None` (no `$HOME`) → no
  thread, no receiver, and `Universe::without_home()`, which is the spec's
  failure-mode row.
- `CellworksApp::over(universe, events)` builds the app over a channel the
  caller owns. **Added at build time**, because `tests/universe_frame.rs` below
  needs a `CellworksApp` whose sender the test holds and `start()` would spawn
  a real scan of the user's home directory.
- `apply_pending(&mut self)` calls `universe.drain(..)` when there is a
  receiver. `impl eframe::App for CellworksApp` calls `apply_pending` then
  `draw(ui, &self.universe)` — drained once per frame, `try_recv` only, the UI
  thread never waiting (requirement 16).
- While the state is still `Scanning`, the frame body asks for the next frame
  with `ui.ctx().request_repaint()`. **Added at build time:** egui draws on
  demand and a scan finishing is not an input event, so without it a result
  that arrives while the window sits idle would wait for the engineer to move
  the mouse. It repaints only while a scan is running, and stops the moment one
  reports.
- `draw(ui: &mut egui::Ui, universe: &Universe)` delegates to
  `cellworks_workspace::ui::universe`. `src/lib.rs` keeps its
  `pub use app::draw;`. `run()` passes `CellworksApp::start()` to
  `eframe::run_native` instead of the unit struct.
- **`tests/empty_window.rs` is deleted in this step**, not earlier, so no step
  leaves the repository red. `tests/universe_frame.rs` replaces it and pins what
  is now the binary's own share: a `CellworksApp` built over a channel the test
  holds shows the scanning state; after a `ProjectsDiscovered` is sent,
  `apply_pending` leaves the project labels in the frame; `apply_pending` on an
  empty channel returns immediately; and a sender dropped before reporting
  leaves the scanning state rather than showing "Scanning" for ever.
  `Sources::for_this_user` is never called from a test, and no test opens a
  window.

*Leaves working:* the whole story, end to end, except the held string.

### 8. The documentation that would otherwise go stale

Reword `docs/arc42/08-crosscutting.md` §8.5 so it states what the code now does:
a project's identity is its canonical path; whether two paths are the same
folder is the filesystem's answer (device and inode), which on a
case-insensitive macOS volume folds case and on a case-sensitive Linux
filesystem does not; never the spelling a folder was added with. `factory:standards-change`
requires the docs stay true in the same change, and `spec.md` Concern 5 asked
for exactly this.

*Leaves working:* the architecture document no longer contradicts the product
owner's 2026-10-08 ruling.

## Risks

**Blast radius.** Three things reach outside this story. First, `draw`'s
signature: it is re-exported at `src/lib.rs:10` and is the seam every future
frame test uses, so changing its arity changes the shape of the binary's whole
test surface, and it is what kills `tests/empty_window.rs`. That deletion is
intended (`spec.md` Concern 2) but it removes an accepted story's proof, and it
should be a visible decision on the pull request rather than a quiet diff.
Second, `crates/map` already depends on `cellworks-workspace`
(`docs/arc42/05-building-blocks.md` §5.2, line 42), so the moment this crate
pulls in egui, every downstream crate's build graph grows it too; the egui
version therefore has to come from `[workspace.dependencies]` and nowhere else,
because two egui copies in one binary give type errors that read as if the
*code* is wrong. Third, touching `Cargo.toml` and `Cargo.lock` trips the
`dependencies` flag in `sdlc/critical.yaml:46-57`, so the dashboard puts this
change in front of a person. That is expected, not something to route around.

A quieter one: `WorkspaceEvent` is where context-map relationship 6 (Workspace →
Map) first takes a shape. Map and Sources will consume it. Adding a variant
later is additive; renaming one is not, and the four names chosen here are
chosen once.

A fourth, smaller: `FolderIdentity` uses `std::os::unix::fs::MetadataExt`, so
`cellworks-workspace` stops compiling on Windows. Windows is not a target
(`CLAUDE.md`: macOS first, the release workflow also builds Linux), and no CI
leg would notice. The alternative — `cfg`-gating a Windows path nothing builds
and nothing tests — is worse than a crate that is honestly Unix-only, so this is
accepted rather than mitigated. The day Windows becomes a target, this is the
file to open.

And one leftover that this plan deliberately does not fix: §5.2's crate table
still describes `workspace` as having "case-insensitive paths on macOS".
Requirement 23 holds `docs/arc42/05-building-blocks.md` unchanged by this story,
so the correction goes only into §8.5, where the statement is load-bearing.
A follow-up chore story should align the table row; naming it here is what keeps
it from being lost.

**The riskiest step is step 4,** the merge. Everything else in this story fails
loudly — a bad walk returns the wrong folders and a test says so; a bad parse
returns an event and a test says so. The merge is the one place where being
wrong is *silent and platform-shaped*: key the map on the canonical path string
instead of `(dev, ino)` and macOS quietly keeps two projects for one folder, or
fold case into the key and Linux quietly collapses two real projects into one —
and on the author's Mac, both mistakes look correct. The mitigation is inside
step 4 rather than after it: requirement 8's test asks the filesystem whether
this volume is case-insensitive *before* it asserts, so the one assertion runs
honestly on both runners and fails the moment a case rule is hard-coded. The
second-order risk, that `canonicalize` on macOS returns the on-disk spelling and
therefore hides a merge bug by converging the two paths anyway, is why
requirement 9's precedence is a fixed rule with its own both-orders test rather
than an assumption about what `realpath` returns.

**The second risk is the permission test in step 2,** and it is the kind that
passes rather than fails. Mode `0o000` means nothing to root, so in any
root-running container the unreadable-folder test goes green without exercising
a single line of the error path, and requirement 5's `ProjectsFolderUnreadable`
would ship unproven. The probe described in step 2 — read the chmodded directory
and refuse to continue if it opens — is what makes that failure loud. Its twin
is cleanup: a `0o000` directory defeats `remove_dir_all`, so the test restores
`0o755` itself rather than trusting `TempDir`'s drop.

**Rejected alternatives.** `walkdir`: rejected — `read_dir` with an explicit
stack is a few lines, `guidelines/architecture.md` adds a dependency only when a
few lines cannot do the job, and the two custom stopping rules would have to be
written against `walkdir`'s filter API anyway. `dirs` / `directories` for the
application support path: rejected for the same reason and by `spec.md`'s own
reckoning — a dozen lines. `serde` derive for the settings file: rejected —
`toml::Table` reads the one key this story understands and keeps serde out of
the crate entirely, which is also what makes "unknown keys are ignored" the
default rather than a configured exception. A hand-rolled temporary-directory
helper instead of `tempfile`: rejected — `tempfile` is already in `Cargo.lock`
through the existing dependency tree, so it costs no new compilation, and
removal-on-unwind and collision-free naming are exactly the two things a
hand-rolled version gets wrong. Comparing lowercased path strings, or
`cfg!(target_os = "macos")` around a case fold, for de-duplication: rejected —
both are the thing requirement 8 and the product owner's ruling forbid, and both
are wrong on a case-sensitive macOS volume and on a case-insensitive Linux
mount. A depth cap on the walk: rejected — it would silently hide a project, and
the `.git` and dot-entry rules already keep the scan cheap. Drawing the universe
in `crates/map`: rejected — layout, districts and zoom are Map stories, and
`spec.md` Concern 4 draws the line at one labelled entry per project. Writing
the settings file when it is absent, or pruning entries that no longer resolve:
rejected by requirement 12 — until EPIC-008 the file is the user's only way to
add a folder, so it is the user's data and this story does not touch it.
Keeping `tests/empty_window.rs`: rejected — the only way is to keep a windowless
code path nothing calls, which `spec.md` Concern 2 rightly calls worse. An
`#[ignore]` measurement test for the walk: rejected as out of scope — the
nearest quality scenario, `10-quality.md` P2, measures opening a *project*, not
scanning the universe, and `spec.md`'s Verification asks for no measurement
test. Amending `docs/domain/contexts/Workspace/Workspace.md` to mark its
discovery bullet half-done: rejected per `spec.md` Concern 3 — it describes the
target, and the epic tracks the remainder.

**Rollback.** Everything here is reversible by a plain `git revert` of this
story's squash-merge commit on `test`, and on `main` too if it has already been
promoted. The revert restores `draw(ui)` without state, restores
`tests/empty_window.rs`, empties `crates/workspace/src/` back to its doc
comment, and returns both manifests and `Cargo.lock` to their current contents.
There is nothing else to undo: requirement 12 means this story never creates,
writes or deletes the settings file, so no user data and no on-disk state exists
for a rollback to migrate, and no external system is touched.

## Proof

### The four checks of `CLAUDE.md`

Run from the repository root, output pasted into the pull request. Identical to
`verify-command` in `.github/workflows/sdlc-build.yml:27` for the three cargo
commands.

```sh
cargo fmt --all --check                                 # no output, exit 0
cargo clippy --workspace --all-targets -- -D warnings   # "Finished", zero warnings
cargo test --workspace                                  # every suite: 0 failed
SHELLCHECK_OPTS=--exclude=SC2016 actionlint             # no output, zero findings
```

`cargo test --workspace` runs six test binaries that hold a test after this
story: the root's `command_line`, `workspace_structure` and `universe_frame`,
the root's unit tests, and `cellworks-workspace`'s unit tests plus its one
acceptance binary — 69 tests, all passing. Every row below is at least one
`#[test]`:

| Where | Pins |
|---|---|
| `crates/workspace/src/project.rs` unit | same folder two ways has one `FolderIdentity`; two folders do not; `name` is the last component (reqs 8, 10) |
| `crates/workspace/src/discovery.rs` unit | the eleven cases of step 2 (reqs 1–5) |
| `crates/workspace/src/settings.rs` unit | the seven cases of step 3 (reqs 11–15) |
| `crates/workspace/src/registry.rs` unit | the six cases of step 4, including the case-sensitivity probe (reqs 6–10, 16) |
| `crates/workspace/src/universe.rs` unit | the seven state transitions of step 5 (reqs 18, 21) |
| `crates/workspace/tests/story_002_…rs` | one test per scenario of issue #4, named after it; then the universe's visible behaviours, each naming its requirement (reqs 16, 18–21), and that the universe scrolls |
| `tests/universe_frame.rs` | the binary's frame body renders the universe; `apply_pending` drains without blocking (reqs 16, 19) |
| `tests/workspace_structure.rs` | unchanged, still green — requirement 23 |
| `tests/command_line.rs`, `src/cli.rs`, `src/app.rs` unit | unchanged from STORY-001 |

### Requirement 12 — the settings file is never written

```sh
grep -rnE 'fs::write|File::create|remove_file|OpenOptions|set_len|write_all' \
     crates/workspace/src/settings.rs
#   -> two lines, both in the module's own doc comment, naming the calls it
#      does not make
```

**The grep is narrowed to `settings.rs`, which is a departure recorded at build
time.** Over all of `crates/workspace/src` it also matches
`src/test_tree.rs`, the `#[cfg(test)]` module that raises a tree in a temporary
directory: it writes a `.git` file and a settings file *for a test*, it is
compiled out of every non-test build, and it cannot reach the user's data. The
statement requirement 12 needs is about the module that reads the settings
file, and that is the grep above.

Plus the unit test that reads a settings file holding an unknown key and a bad
entry and asserts the file's bytes are identical afterwards
(`the_settings_file_is_byte_identical_after_a_scan`).

### Requirement 16 — nothing blocks, no async runtime

```sh
grep -rnE '\b(async|await|tokio)\b' src crates/workspace/src     # no match
grep -rnE '\brecv\(' src crates/workspace/src | grep -v try_recv # no match
cargo tree --workspace | grep -E '(tokio|async-std|smol) v'      # no match
```

`pollster` and the `futures-*` helpers appear under `wgpu` and are wgpu's own
blocking helpers, not an async runtime; they are not what this excludes.

### Requirement 17 — the theme words stay out of identifiers

```sh
grep -rinE '\b(planet|servitor|belt|machine|factory)\b' src crates/*/src
#   -> five lines, four of them English prose in a doc comment or a test
#      message ("a fresh machine", "a headless machine"), and one identifier:
#      Sources::for_this_user
```

`crate` is on the spec's list of theme words but is also a Rust keyword, so a
grep for it is meaningless here; it is checked by eye instead, and the only
`crate` in this story is the keyword. The enum is `WorkspaceEvent` and its four
variants are past-tense, which the compiler pins.

**The function is `Sources::for_this_user`, not the `for_this_machine` of
`spec.md`'s API sketch.** `guidelines/naming.md` lists *machine* among the
theme words that may not appear in an identifier, and requirement 17 repeats
that as a must. The build shipped the sketch's name; the review flagged it, and
the product owner had it renamed on 2026-10-09. The function reads the user's
home directory, which the new name says.

### Requirement 22 — no test reads the user's home

```sh
grep -rn 'for_this_user' src crates           # only crates/workspace/src/lib.rs and src/app.rs
grep -rn 'env::var' crates/workspace/src         # only crates/workspace/src/lib.rs
cargo test --workspace --no-run                  # build once with the real HOME
CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" HOME="$(mktemp -d)" \
  cargo test --workspace                         # still green
```

`CARGO_HOME` is kept at the real one deliberately: cargo needs its registry, and
the thing under test is whether the *code* reads `$HOME`, not whether cargo
does. `RUSTUP_HOME` is kept for the same reason and was added at build time —
without it rustup cannot find a toolchain under the empty `$HOME` and the run
fails before a single test starts.

### Requirement 23 — no crate arrow moved

```sh
git diff --stat main -- tests/workspace_structure.rs docs/arc42/05-building-blocks.md
#   -> empty
```

### What a reviewer checks by eye

- `crates/workspace/src/settings.rs` contains one `cfg!(target_os = "macos")`,
  and it chooses *where the settings file lives* — not how two paths compare.
  No `cfg!(target_os)` appears anywhere near `FolderIdentity` or the merge
  (requirement 8).
- The merge in `registry.rs` keys on `FolderIdentity`, not on a path string, and
  inserts discovery before settings with each group sorted by canonical path
  (requirement 9), then sorts the result (requirement 10).
- `crates/workspace/src/` depends on egui in `ui.rs` and nowhere else
  (`guidelines/architecture.md`).
- `crates/workspace/Cargo.toml` gained no `cellworks-*` key, and
  `[workspace.dependencies]` in the root manifest is written as one-line inline
  tables with no `[workspace.dependencies.<name>]` section — so the root still
  has exactly one `^version = "` line.
- Every error path returns an event; no `unwrap` or `expect` on anything read
  from the filesystem.
- `docs/arc42/08-crosscutting.md` §8.5 now describes what the code does, and the
  diff changes §8.5 only.
- The deletion of `tests/empty_window.rs` is in the diff and is explained on the
  pull request as `spec.md` Concern 2's intended succession.

### One manual smoke, outside the automated suite

On macOS, by hand, with the user's agreement and recorded in the pull request:
open Cellworks on a real machine with a real `~/Projects` and confirm the
repositories appear, sorted, one entry each. `CLAUDE.md` forbids opening app
windows over the user's screen unasked and `08-crosscutting.md` §8.6 forbids a
test doing it, so this is never run from an automated session.

### What this plan cannot prove before the owner answers

Requirement 20's string. The empty state's structure, its placement and its test
are all above; the copy is held by the `> BLOCKED:` line at the top of this
document and arrives with the product owner's decision.
