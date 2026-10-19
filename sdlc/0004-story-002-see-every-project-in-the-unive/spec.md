---
stage: spec
issue: 4
status: draft
author: claude (sdlc-spec)
date: 2026-10-08
---

# Spec: See every project in the universe

Derived from `intent.md` in this directory. Answers, or explicitly carries
forward, every open question it raised.

**The intent's open questions.** Both were already resolved by the product owner
on 2026-10-08 and are carried into the design rather than dropped:

1. *Letter case follows the filesystem* — "Alpha" and "alpha" are one project on
   macOS and two on Linux. Answered in [Design → Project identity and
   de-duplication](#project-identity-and-de-duplication), where the design makes
   it hold by asking the filesystem instead of applying a case rule in the code
   (requirements 7–8).
2. *No text in the issue was addressed to an AI assistant* — confirmed for
   `intent.md` as well. I read it as data. It contains no instruction to change
   behaviour, skip a policy, or act outside this stage.

**Policies applied:** `CLAUDE.md`; `factory-context/05-case/AGENTS.md`, which
routes to the jarvis docs and was followed rather than copied;
`docs/domain/glossary.md`, `docs/domain/contexts/Workspace/{Workspace.md,glossary.md}`,
`docs/domain/contexts/Map/{Map.md,glossary.md}`, `docs/domain/context-map.md`;
`docs/arc42/02-constraints.md`, `04-solution-strategy.md`,
`05-building-blocks.md`, `08-crosscutting.md` (§8.1, §8.2, §8.3, §8.5, §8.6),
`10-quality.md`; `docs/services/cellworks/guidelines/{architecture,naming,testing}.md`;
`docs/discovery/2026-10-08-prototype-findings.md` L219-222, L228-229;
ADR-0001, ADR-0002. `docs/services/cellworks/decisions/` holds no entry yet.
The one product policy skill, `.claude/skills/secure-api-review`, was read and
does not apply — see [Concerns](#concerns), item 6.

**Not in this story.** `docs/domain/contexts/Workspace/Workspace.md` lists five
Workspace behaviours; this story delivers one and a half of them. Rescan (⌘R and
after session writes), and rebuild and restart, are out by the intent's own scope
boundary. So is the other half of the discovery bullet — *folders recent Claude
sessions worked in* — which is context-map relationship 10 (AgentSessions →
Workspace) and needs a context that has no code yet. So is the interface for
adding a folder (EPIC-008). See [Concerns](#concerns), item 3.

## Requirements

Each is independently verifiable; the proof for each is in
[Verification](#verification). The terms are the glossary's: **Project**
(identified by its path), **Universe** (all projects together on one map).

### Discovering git repositories under the projects folder

1. **(must)** When Cellworks starts it discovers one project for every git
   repository found under the **projects folder**, which is `$HOME/Projects`
   (prototype findings L219, `Workspace.md`).
2. **(must)** A directory is a git repository exactly when it directly contains
   an entry named `.git`, whether that entry is a directory or a file. The walk
   does not descend into a directory once it has been recognised as a
   repository, so a submodule or a linked worktree inside a repository does not
   become a second project.
3. **(must)** A directory under the projects folder that is not a git repository
   does not appear as a project. It is still descended into, so a repository
   nested below a plain folder (`~/Projects/acme/service`) is found.
4. **(must)** The walk skips entries whose name begins with `.`, and does not
   follow symbolic links. A symlink under the projects folder therefore never
   produces a second project for a folder already found, and no symlink loop can
   hang the scan.
5. **(must)** A projects folder that does not exist contributes no projects and
   is **not** an error — that is the normal state of a fresh machine. A projects
   folder that exists but cannot be read, and any subdirectory that cannot be
   read part-way through the walk, is skipped and reported (requirement 15); the
   rest of the walk continues (`08-crosscutting.md` §8.2).

### Added folders

6. **(must)** Every folder listed in the settings file appears as a project,
   whether or not it is a git repository and whether or not it lies under the
   projects folder (`Workspace.md`: "any folder can be added").

### Project identity and de-duplication

7. **(must)** A project is identified by the canonical path of its folder — the
   path with symlinks and `.`/`..` resolved (`glossary.md`, `Project`;
   `08-crosscutting.md` §8.5: never by the spelling it was added with). The same
   folder reached two ways — discovered under the projects folder *and* listed in
   the settings file, or reached under two spellings — is exactly one project.
8. **(must)** Whether two paths are the same folder is decided by **the
   filesystem**, not by a case rule in the code: two paths are the same project
   when the filesystem reports the same file identity (device + inode) for them.
   On a case-insensitive macOS volume `~/Projects/Alpha` and `~/projects/alpha`
   are therefore one project; on a case-sensitive Linux filesystem, where they
   are two directories, they stay two projects. This is the product owner's
   ruling of 2026-10-08, and no `cfg!(target_os)` encodes it.
9. **(must)** When two sources resolve to one project, the universe is the same
   whichever order they were seen in: the surviving project is chosen by a fixed
   precedence (discovery under the projects folder before the settings file;
   within each, sorted by canonical path).
10. **(must)** The universe's order is deterministic: projects are sorted by
    canonical path, byte-wise, so two runs over the same filesystem produce the
    same universe. A project's display name is the last component of its
    canonical path.

### The settings file

11. **(must)** Added folders are read from a settings file in the user's
    application support directory (`08-crosscutting.md` §8.3), at startup.
12. **(must)** This story never writes, creates or deletes that file, and never
    removes an entry from it. Until EPIC-008 the file is how a folder is added
    (intent, Constraints), so it is the user's data.
13. **(must)** A settings file that is absent means no added folders. That is
    not an error and produces no message.
14. **(must)** An entry that does not resolve to a readable directory — missing,
    a file rather than a directory, an unmounted volume, no permission — does
    not appear as a project, does not abort the scan, and is left untouched in
    the file (requirement 12). It is reported (requirement 15).
15. **(must)** A settings file that cannot be read or parsed does **not**
    silently become "no added folders". The projects discovered under the
    projects folder still appear, and the failure is reported as a
    `WorkspaceEvent` carrying the path and the reason. Keys the file carries
    that this story does not understand are ignored, not rejected
    (`08-crosscutting.md` §8.2: tolerant parsing).

### Off the UI thread

16. **(must)** The scan runs on a `std::thread`; the UI thread never blocks on a
    filesystem walk (`08-crosscutting.md` §8.1,
    `guidelines/architecture.md`). Results reach the UI thread as
    `WorkspaceEvent` values over a `std::sync::mpsc` channel, drained once per
    frame. No async runtime is introduced.
17. **(must)** The event enum is named `WorkspaceEvent` and its variants are in
    the past tense (`guidelines/naming.md`). No Warhammer 40k word appears in
    any identifier.

### What the universe shows

18. **(must)** From the first frame until the first scan result arrives, the
    universe shows that it is still scanning. The empty state does not flash on
    the way.
19. **(must)** Once the scan has reported, the universe shows exactly one
    labelled entry per project, in the order of requirement 10, each labelled
    with the project's display name.
20. **(must)** When the scan found no project at all, the universe shows an
    empty state that tells the engineer how to add a project, rather than a
    blank screen. **The wording is held** — see [Concerns](#concerns), item 1.
21. **(should)** A reported failure (requirements 5, 14, 15) is visible in the
    universe rather than silent, attached to the folder or file it belongs to
    (`08-crosscutting.md` §8.2, `10-quality.md` R1).

### Platform, tests and checks

22. **(must)** Discovery and the universe view work on macOS and on Linux, and
    no unit or acceptance test reads the user's home directory or the real
    application support directory (`guidelines/testing.md`). This holds by
    construction: the projects folder and the settings file path are parameters
    of the Workspace API, and only the binary fills them from `$HOME`.
23. **(must)** No new `cellworks-*` crate arrow is introduced;
    `tests/workspace_structure.rs` and `docs/arc42/05-building-blocks.md` §5.2
    are unchanged by this story.
24. **(must)** All four commands of `CLAUDE.md` → *Verifying your work* succeed
    with zero warnings and zero findings, on macOS and on Linux.

## Design

### Where the code lives

The behaviour is Workspace's, so it lives in `cellworks-workspace` — today an
empty crate. `guidelines/architecture.md` puts egui drawing in a `ui` module of
the crate that owns the behaviour, so the universe view goes there too, and the
binary stays thin.

```
crates/workspace/src/
  lib.rs        # the small public API: Sources, scan, Project, WorkspaceEvent
  project.rs    # Project, ProjectId, FolderIdentity
  discovery.rs  # the walk under the projects folder
  settings.rs   # reading the settings file
  registry.rs   # merging the two sources into one deterministic project list
  event.rs      # WorkspaceEvent
  ui.rs         # the universe view: scanning, populated, empty
src/app.rs      # holds the universe state, starts the scan, drains the channel
```

No module outside `ui.rs` depends on egui beyond geometry types.

### Public API, in sketch

```rust
/// Where the projects come from. The binary fills this from $HOME; every test
/// fills it from a temporary directory — which is what keeps requirement 22
/// true by construction rather than by remembering.
pub struct Sources {
    pub projects_folder: PathBuf,
    pub settings_file: PathBuf,
}

impl Sources {
    /// `$HOME/Projects` and the settings file below. `None` when $HOME is unset.
    pub fn for_this_machine() -> Option<Sources>;
}

pub struct Project {
    pub id: ProjectId,   // the canonical path
    pub name: String,    // its last component
}

pub enum WorkspaceEvent {
    ProjectsDiscovered(Vec<Project>),
    ProjectsFolderUnreadable { path: PathBuf, reason: String },
    AddedFolderSkipped       { path: PathBuf, reason: String },
    SettingsUnreadable       { path: PathBuf, reason: String },
}

/// Walks, reads, merges. Blocking — called on the background thread, and
/// directly in unit tests.
pub fn scan(sources: &Sources) -> Vec<WorkspaceEvent>;

/// Spawns the std::thread and hands back the receiving end.
pub fn start_scan(sources: Sources) -> mpsc::Receiver<WorkspaceEvent>;
```

`scan` being an ordinary blocking function, separate from `start_scan`, is what
lets every rule above be unit-tested without a thread, a channel or a frame.

### The walk

Breadth or depth is irrelevant to the result; what matters is the two stopping
rules of requirements 2–4, which are also what keeps the scan cheap: the walk
stops at the first `.git`, so it never descends into a repository's
`node_modules`, `target` or `vendor`, and it skips dotted entries, so caches
below the projects folder cost nothing. No depth cap is needed, and none is
imposed — a cap would silently hide a project.

`std::fs::read_dir` + `DirEntry::file_type()` is used rather than `metadata()`,
because `file_type()` does not follow a symlink and so implements requirement 4
without a second decision.

### Project identity and de-duplication

Two different questions, answered by two different mechanisms, deliberately:

| Question | Answered by | Why that one |
|---|---|---|
| What *is* this project, for display and for persistence? | `std::fs::canonicalize` — the canonical path | A path is stable across restarts and across restores; the settings file and the glossary's `Project` both key on it. |
| Are these two folders the *same* folder? | the filesystem's file identity: `MetadataExt::dev()` + `ino()` of the canonical path | It is the filesystem's own answer, so the product owner's "letter case follows the filesystem" holds on APFS and on ext4 from one line of code. An inode is not stable enough to persist, which is why it is not the identity. |

The merge keys a map on `(dev, ino)` and inserts in the precedence order of
requirement 9; the first insertion wins. The result is then sorted by canonical
path (requirement 10). On macOS `canonicalize` normally returns the spelling
stored on disk, so both spellings converge anyway; requirement 9's precedence is
the deterministic tiebreak for when it does not, rather than an assumption about
what `realpath` returns.

Note the asymmetry this produces, and it is the right one: a *symlink under the
projects folder* is not followed (requirement 4) and so yields nothing, while a
*symlink added in the settings file* is canonicalised and so merges with the
real folder if that folder was also discovered.

### The settings file

| Platform | Path |
|---|---|
| macOS | `$HOME/Library/Application Support/Cellworks/settings.toml` |
| Linux | `$XDG_CONFIG_HOME/cellworks/settings.toml`, else `$HOME/.config/cellworks/settings.toml` |

§8.3 names the macOS directory; the Linux row is the XDG equivalent, chosen as
*config* rather than *data* because the file holds preferences. Computing it is
a dozen lines, so no `dirs`-style dependency is added
(`guidelines/architecture.md`).

```toml
# Folders added by hand until EPIC-008 brings the interface for it.
added_folders = [
  "/Users/engineer/work/infra",
]
```

Entries are absolute paths; `~` is not expanded and a relative entry is reported
under requirement 14 rather than resolved against an arbitrary working
directory. The file is parsed with the `toml` crate — already in `Cargo.lock` as
a dev-dependency of the root — read as a `toml::Table` so no serde derive enters
the crate. Parsing a path list out of raw text correctly (quoting, escapes,
non-UTF-8) is well past "a few lines".

**Read-only by construction.** `settings.rs` exposes a read function and no
write function at all (requirement 12). There is nothing to forget, and nothing
a later edit can accidentally call.

### States of the universe

The scan result and the view's state are the same thing seen twice:

| State | Entered when | Shown |
|---|---|---|
| Scanning | at startup, before the first `ProjectsDiscovered` | that a scan is running |
| Populated | `ProjectsDiscovered` with ≥ 1 project | one labelled entry per project, sorted |
| Empty | `ProjectsDiscovered` with 0 projects | the empty state of requirement 20 (wording held) |

Scanning is a state of its own rather than "empty until proven otherwise"
precisely so requirement 18 holds: a slow walk over a large `~/Projects` must
not show the engineer an empty universe that then fills in. Any
`ProjectsFolderUnreadable`, `AddedFolderSkipped` or `SettingsUnreadable` event
is accumulated and shown alongside whichever state is current (requirement 21);
none of them replaces the project list, because a failure belongs to the folder
it happened in and takes nothing else down (§8.2, R1).

### Failure modes

| Situation | Behaviour |
|---|---|
| `$HOME` unset | `Sources::for_this_machine()` returns `None`; no scan starts; the universe goes straight to Empty. |
| Projects folder absent | 0 discovered, no message (requirement 5). |
| Projects folder or a subdirectory unreadable | skipped, `ProjectsFolderUnreadable`, walk continues. |
| Settings file absent | no added folders, no message (requirement 13). |
| Settings file unreadable or malformed | `SettingsUnreadable`; discovered projects still shown (requirement 15). |
| Settings entry missing / not a directory / relative | `AddedFolderSkipped`; entry left in the file (requirements 12, 14). |
| Settings entry duplicating a discovered project | merged, one project (requirements 7–9). |
| Scan thread panics | the channel's sender drops; the view leaves Scanning and reports that discovery failed, rather than showing "Scanning" for ever. |

No `unwrap` or `expect` on anything read from the filesystem
(`guidelines/architecture.md`).

### What changes in the binary

`CellworksApp` gains state for the first time: the `Receiver<WorkspaceEvent>`
and the universe state. Each frame it drains the receiver with `try_recv` —
never blocking — applies the events, and calls the view. STORY-001's requirement
10 ("no background thread, channel or `<Context>Event` enum is introduced by
this story; the conventions bind the first story that needs one") is spent here:
this is that story, and it follows those conventions.

The empty `CentralPanel` of STORY-001 is the seam the universe view replaces, as
that spec intended.

## Affected interfaces

- **New: the public API of `cellworks-workspace`** — its first. `Sources`,
  `Project`, `ProjectId`, `WorkspaceEvent`, `scan`, `start_scan`,
  `ui::universe`. The crate's `lib.rs` doc comment stops saying "nothing is
  exposed yet".
- **New: `WorkspaceEvent`** — this is where context-map relationship 6
  (Workspace → Map, Open Host Service) starts having a shape. Map and Sources
  will consume it; added variants are additive, renamed ones are not.
- **New, and user-facing: the settings file.** Its path and its `added_folders`
  key become a contract the moment this ships, because the intent makes
  hand-editing it the only way to add a folder until EPIC-008. EPIC-008's
  interface must keep reading this file and this key, or migrate it
  deliberately. This is the lasting cost behind [Concerns](#concerns), item 1.
- **Changed: `cellworks::draw`.** It takes no state today
  (`src/app.rs:17`, re-exported at `src/lib.rs:10`) and must take the universe
  state. Every caller changes with it.
- **Broken, deliberately: `tests/empty_window.rs`.** It asserts that one frame
  contains no widget and that no node carries a label
  (`tests/empty_window.rs:17-37`) — which was STORY-001's proof of requirement
  7 and is false the moment the universe draws anything. This story retires it
  and replaces it with the universe's own acceptance tests. See
  [Concerns](#concerns), item 2.
- **Changed: `crates/workspace/Cargo.toml`** gains `egui` and `toml`, and
  `egui_kittest` as a dev-dependency; `toml` moves into
  `[workspace.dependencies]` as a second package now uses it. No `cellworks-*`
  dependency changes, so `tests/workspace_structure.rs` still passes untouched —
  its graph assertion filters on the `cellworks-` prefix
  (`tests/workspace_structure.rs:113`).
- **Process note:** touching any `Cargo.toml` trips the `dependencies` flag in
  `sdlc/critical.yaml:46-56`, so this change is surfaced to a person by the
  dashboard. That is expected, not a problem to route around.
- **Unchanged:** `docs/arc42/05-building-blocks.md` §5.2, the command line of
  `cellworks`, and every other crate's public API (all still empty).

## Concerns

1. **The empty state's wording, and what it is allowed to tell the engineer.**
   Requirement 20 says the empty universe tells the engineer how to add a
   project. The only truthful answer this story can give is "edit
   `~/Library/Application Support/Cellworks/settings.toml`" — the intent itself
   calls that a stopgap until EPIC-008, and the design-system or
   platform-guardrail cell the intent names as the owner of this wording does
   not exist in `factory-context/05-case/` or anywhere under `docs/`. Putting an
   internal file path in front of the engineer turns it into a documented
   contract users will rely on and EPIC-008 will have to honour. The alternative
   — a message that says only "no projects found in ~/Projects" — does not meet
   the intent's outcome. I am not authorised to make that product call.

   > BLOCKED: requirement 20 needs the empty state to say how to add a project, but the only mechanism until EPIC-008 is hand-editing the settings file, and publishing that path as user-facing copy is a product decision with no owning document — owner: product owner

   What the owner needs to decide: the exact copy, and whether the settings file
   path is spoken aloud in the UI at all. Everything else in this spec is
   independent of the answer; the plan can proceed on all other requirements and
   leave requirement 20's string as the one thing waiting.

2. **This story retires an accepted story's acceptance test.**
   `tests/empty_window.rs` is the proof of STORY-001's requirement 7 ("whose
   content is empty — no widgets, no text"). STORY-001's own spec calls the
   empty panel "the seam the map canvas replaces", so this is the intended
   succession rather than a regression — but an accepted artifact's proof is
   being deleted, and that should be a visible decision rather than a quiet
   diff. The replacement is named in [Verification](#verification). Flagging,
   not blocking; if the product owner wants STORY-001's test kept, the only way
   is to keep a windowless code path nothing calls, which is worse.

3. **Scope: `Workspace.md`'s discovery bullet is delivered by halves.** The
   bullet reads "git repositories under `~/Projects`, folders recent sessions
   worked in". This story does the first half only; the second needs
   AgentSessions (context-map relationship 10), which has no code. After this
   story the bullet will look done and will not be. Recommendation for the plan:
   do not amend `Workspace.md` — it describes the target, not the current
   state — and let the epic track the remainder. Flagging so the gap is on the
   record.

4. **Resolved during drafting: how much of the universe this story draws.** The
   intent calls this "Workspace context behaviour" and names `Workspace.md` as
   the scope boundary, yet its outcome is user-visible ("the universe shows one
   project for every git repository"). A strict Workspace-only reading would
   deliver a registry nobody can see, and requirements 18–20 would be
   unverifiable. I resolved it by drawing the *smallest* thing that makes the
   outcome true: one labelled entry per project, the scanning state and the
   empty state, in `cellworks-workspace`'s own `ui` module. No physics layout,
   no districts, no zoom, no level of detail, no selection, no keyboard
   navigation — those are Map and Navigation stories, and `crates/map` is not
   touched. If the product owner meant "registry only, nothing on screen",
   requirements 18–21 drop and the rest stands.

5. **Resolved during drafting: `08-crosscutting.md` §8.5 is worded for one
   platform.** It says "Paths are compared case-insensitively on macOS. Projects
   and files are matched by normalised path". Read as string normalisation, it
   would fold case on Linux too and contradict the product owner's ruling.
   Requirement 8's filesystem-identity design satisfies the *intent* of §8.5 on
   both platforms. Recommendation for the plan: update §8.5 in the same commit
   to say that sameness is the filesystem's answer, so the document stays true
   (`factory:standards-change`). I have not treated the wording as a blocker
   because the product owner's 2026-10-08 ruling is the later and more specific
   decision and the two do not actually disagree on macOS.

6. **`.claude/skills/secure-api-review` was read and does not apply.** Its
   trigger is an external-facing endpoint, request/response schema or OpenAPI
   spec; this story adds none, and Cellworks runs no network server
   (`02-constraints.md` §2.1, `08-crosscutting.md` §8.4). Its rule 2 ("reject
   unknown fields, do not ignore") would, if it applied, contradict
   `08-crosscutting.md` §8.2's tolerant parsing at requirement 15 — it does not
   apply, so there is no conflict to escalate. For the record, I resolved the
   settings file's own version of that question deliberately: **unknown keys are
   ignored** (forward compatibility, §8.2) but **a malformed file is an error**
   (requirement 15), because treating an unparseable settings file as "no added
   folders" would look to the engineer exactly like their projects had been
   deleted. No secret, token or credential is read, written or logged anywhere
   in this story.

7. **I could not read issue #4's Gherkin scenarios in this session.**
   `factory-context/05-case/AGENTS.md` says the scenarios stay in the work item,
   and `guidelines/testing.md` requires one acceptance test per scenario, named
   after the scenario in `snake_case`, in a file named after the story. `gh issue
   view 4` was not available to me here, so the acceptance test names below are
   placeholders describing the behaviour. The plan stage must take the scenario
   titles from issue #4 and name the tests after them. Flagging rather than
   blocking: the requirements are drawn from `intent.md`, which is the approved
   record, and nothing in the design depends on the scenario titles.

8. **A note for the plan, not a concern with the design: fixtures cannot be
   committed.** A test tree for requirements 1–4 needs real `.git` directories,
   and git will not let this repository commit a nested `.git` directory — so
   the fixtures in `guidelines/testing.md`'s sense cannot exist on disk. The
   tests must build their tree in a temporary directory at run time. I recommend
   a small in-repo helper that creates and removes one, rather than a
   `tempfile` dependency; either is acceptable, and the plan should say which.

## Verification

Per `08-crosscutting.md` §8.6 and `guidelines/testing.md`. Unit tests sit beside
the code in `crates/workspace`; acceptance tests go in
`crates/workspace/tests/story_002_*.rs` and drive the view headlessly with
`egui_kittest`. Every test builds its own temporary tree and passes it in as
`Sources` — none reads `$HOME` (requirement 22).

| Req | How it is proven |
|---|---|
| 1 | Unit: a temporary projects folder holding two git repositories and `scan` returns both, by canonical path. |
| 2 | Unit, three cases: a repository whose `.git` is a directory, one whose `.git` is a file (linked worktree), and a repository containing a nested repository — the nested one does not appear. |
| 3 | Unit: a plain folder beside a repository does not appear; a repository two levels below a plain folder does. |
| 4 | Unit: a dotted directory holding a repository yields nothing; a symlink to a sibling repository yields no second project; a symlink pointing at its own ancestor terminates the scan. |
| 5 | Unit: a `Sources` whose projects folder does not exist returns an empty list and no event; one whose projects folder has mode `0o000` returns `ProjectsFolderUnreadable` and still reports the added folders. |
| 6 | Unit: a settings file naming a non-git folder outside the projects folder, and `scan` returns it as a project. |
| 7 | Unit: a folder both discovered under the projects folder and listed in the settings file yields exactly one project, with the canonical path as its id. |
| 8 | Unit, one test that asks the filesystem first: it creates `Alpha`, probes whether `alpha` opens the same directory, then asserts **one** project if the probe says the volume is case-insensitive and **two** if it does not. The same test is meaningful on the macOS and the Linux runner, and it fails if the code ever hard-codes a case rule. A second test asserts the merge directly: two paths with equal `(dev, ino)` merge, two with different `(dev, ino)` do not. |
| 9 | Unit: the same two sources fed in both orders produce the identical project list. |
| 10 | Unit: `scan` over a tree created in a shuffled order returns the list sorted by canonical path; `name` is the last component. |
| 11 | Unit on the path builder: with `HOME` and `XDG_CONFIG_HOME` passed in as arguments, it yields the macOS and the two Linux paths of the table above. The real directory is never touched by a test. |
| 12 | Two parts. By construction: `settings.rs` exposes no write function — `grep` over `crates/workspace` finds no `fs::write`, `File::create`, `remove_file` or `OpenOptions`. By test: a settings file containing an unknown key and a bad entry is byte-identical after `scan`. |
| 13 | Unit: a `Sources` whose settings file does not exist returns the discovered projects and emits no event. |
| 14 | Unit, four entries in one file: a missing path, a path to a regular file, a relative path, and a valid one. Exactly one project; three `AddedFolderSkipped` events, each naming its path. |
| 15 | Unit: a settings file containing `added_folders = [` (truncated) yields `SettingsUnreadable` naming the file, **and** the projects discovered under the projects folder. A second case: a file with `added_folders` plus an unknown key parses normally and emits nothing. |
| 16 | Acceptance with `egui_kittest`: with a `start_scan` whose thread is made to block, the harness still runs frames and the view stays in Scanning — the frame loop never waits. Plus `grep` over `src/` and `crates/`: no `async`, no `tokio`, no `recv()` without `try_`. |
| 17 | Compile-time: the enum is `WorkspaceEvent` with past-tense variants. `grep -ri` over `src/` and `crates/` for the theme words (planet, factory, machine, crate, servitor, belt) finds none in an identifier. |
| 18 | Acceptance: a harness built before any event is drained shows the scanning state and does **not** show the empty state's text. Then `ProjectsDiscovered([])` is applied and the empty state appears — proving the order, not just the end state. |
| 19 | Acceptance: `ProjectsDiscovered` with three projects, and the view's accessibility tree carries exactly three labels, in sorted order. |
| 20 | Acceptance: `ProjectsDiscovered([])` and the view shows the empty state. **The asserted string is held on [Concerns](#concerns) item 1** — the test asserts the behaviour today and gets its literal when the product owner settles it. |
| 21 | Acceptance: after a `SettingsUnreadable` event the view shows a notice naming the settings file, *and* still lists the projects that were found. |
| 22 | Two parts. By construction: `Sources` is a parameter, and `Sources::for_this_machine` is called only from `src/app.rs` — proven by `grep`. By test: the suite passes with `HOME` set to an empty temporary directory. |
| 23 | `cargo test --workspace` runs `tests/workspace_structure.rs` unchanged; `git diff` shows no change to it or to `docs/arc42/05-building-blocks.md`. |
| 24 | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `SHELLCHECK_OPTS=--exclude=SC2016 actionlint`, run locally and in CI, output pasted into the pull request. |

One smoke check, by hand on macOS and recorded in the pull request, outside the
automated suite: open Cellworks on a real machine with a real `~/Projects` and
confirm the repositories appear. No automated test opens a window.
