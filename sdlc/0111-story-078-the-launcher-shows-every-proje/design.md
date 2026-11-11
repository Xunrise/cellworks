---
stage: design
issue: 111
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-078 — the launcher shows every project grouped by where it lives

The contract is `issue.md` in this directory: its acceptance scenarios are what
the change must do. `prototype.md` beside it records the confirmed launcher
(mockup variant A) and every answered question; its Rejected items are not built.

## Open questions

None.

## Requirements

1. When no project is open, the launcher replaces the flat universe list. Layout
   as in `prototype.md` § Confirmed behavior: header (title, search, "Add
   project…"), left locations column 248 px (All projects, Recent, This Mac,
   HOSTS with one entry per host, "+ Connect host…"), main area (Recent row, then
   groups), right inspector 300 px, footer hints "↑↓ move   Enter open   ⌘K
   search   ⌘N add".
2. Remote hosts and their projects are read from the hand-edited settings file:
   `[hosts."<name>"] projects = ["~/work/api", …]`. Cellworks never writes the
   settings file. A host name that is empty, starts with `-`, or contains
   whitespace or control characters is skipped with a notice.
3. A remote project's identity is `host:path` as written, never canonicalized,
   and it is never read, opened or arranged as a local folder.
4. A host's state is decided once per launch, after the scan reports, by
   `ssh -o BatchMode=yes -o ConnectTimeout=5 -- <host> true` on a background
   thread: exit 0 is connected, anything else (including a missing `ssh`) is
   offline; until it answers the host is "checking". State is always a glyph plus
   a word: `● connected`, `○ offline`, `◌ checking`. A local project's state
   reads "this Mac".
5. The HOSTS heading lists only hosts with at least one project in the settings.
6. Recent: a `recent` file beside the settings file, one project id per line,
   most recent first, at most five distinct ids. Written whenever a project is
   opened; lines that do not parse are skipped. The Recent row shows up to five
   cards labelled `recent: <name>`.
7. Groups: a local project's parent folder with the home folder shown as `~`,
   or a remote project's host name. Local groups first, ordered by path, then
   hosts by name. Each row shows name, path, state and, once the project has been selected,
   its branch.
8. Search filters by a case-insensitive substring of the name or the displayed
   path, together with the picked location. No match shows
   `No project matches "<text>".` and the "Add project…" button.
9. Inspector: kind ("git repository", "folder" or "remote"), branch and file
   count, each left out when unknown; local-copy age and sessions are omitted in
   this story. A local project is a git repository when its branch reads; its
   file count is the number of visible files. A plain folder has no file count.
10. "Add project…", "+ Connect host…" and "Sync" are visible but disabled with
    the hover text "Coming soon". "Open (Enter)" is disabled for a remote project
    with "Connect the host to open it", and Enter on a remote selection opens
    nothing. ⌘K focuses the search box; ⌘N does nothing yet.
11. ↑/↓ move the launcher selection while no project is open; a double-click on
    a local project's name opens it.
12. Existing scanning, failed, empty-state and notice texts are kept.

## Decisions

- "Launcher" is a new view model (`Launcher`, `crates/workspace/src/launcher.rs`)
  over the existing `Universe` scan state, which stays. Rejected: renaming
  `Universe` (churns STORY-002 code and the Map's use of "universe"), or
  "launcher" as a UI title only (leaves the new state without a home).
- The domain word for a left-column entry is `Location { ThisMac, Host(String) }`;
  "sources" and "HOSTS" are UI strings only. Rejected: "Source", which would be a
  third meaning beside the Sources context and the `Sources` struct.
- `ProjectId` becomes `enum { Local(PathBuf), Remote { host, path } }`, shown and
  parsed as `/abs/path` or `host:path`; `path()` is replaced by
  `local_path() -> Option<&Path>` so the compiler forces every caller to handle
  remote ids. Rejected (operator): a launcher-only remote type; deferring remote
  projects to STORY-074.
- Host state from a real ssh probe behind a `HostProbe` trait (`SshProbe` in
  production, fakes in tests), following ADR-0003 (external tools as child
  processes). `--` before the host and the name check in requirement 2 keep a
  settings value from becoming an ssh option. Rejected (operator): showing every
  host offline until STORY-074. Rejected: re-probing on a timer (no story asks).
- Recent follows ADR-0006 (Cellworks' own files beside the settings file): a
  `RecentStore` plus a `RecentKeeper` thread, the same shape as
  `ArrangementStore`/`ArrangementKeeper`. Rejected: storing recent in the
  settings file, which stays read-only. No new ADR.
- Branch and file count belong to CodeAnalysis: `GitListing::branch`
  (`git rev-parse --abbrev-ref HEAD`, default implementation returns an error so
  existing fakes compile) and `start_details`, emitting
  `CodeAnalysisEvent::DetailsRead`. The binary wires it into the launcher,
  because `workspace` must not depend on `code-analysis`
  (`docs/arc42/05-building-blocks.md`). Rejected: running git from `workspace`.
- Selection of a local project still goes through `Navigation::select`, so Enter
  keeps the existing `Navigation::enter` → `Moved::Opened` → `app.open` path; a
  remote selection calls `navigation.deselect()`. Rejected: a second open path
  inside the launcher.
- Details are read only for the project the engineer selects, when it is
  selected, once per project (never at launch, never for unselected projects),
  so git runs only in a project the engineer chose.
- `CellworksApp::over()` starts neither probes nor details; only `start()`,
  `with_probe` and `with_git` turn them on, so no app test runs git or ssh;
  the two tests of `crates/code-analysis/tests/git_cli_against_real_git.rs`
  run real git against repositories they create, in the default
  `cargo test --workspace`.
- The Recent scenario is pinned through a fixture `recent` file, because a
  remote project cannot be opened in this story.
- One story, not split into local and remote halves (operator).

## Order of work

1. [x] Project identity. `ProjectId` enum with `local_path()`, `host()`, `Display`,
   `parse`, and `Project::remote(host, path)`; move every `id.path()` caller to
   `local_path()`; registry sorts by `ProjectId` and skips remote projects when
   attaching district prefixes and contexts; arrangement files key on the
   `Display` form. Files: `crates/workspace/src/project.rs`,
   `crates/workspace/src/registry.rs`, `crates/workspace/src/arrangement.rs`,
   `crates/workspace/src/lib.rs`, `src/app.rs`, `src/opened.rs`.
2. [x] Hosts in settings and scan. Read `[hosts]` tolerantly into
   `AddedFolders::remote`; add `WorkspaceEvent::HostSkipped`, `HostProbed`,
   `RecentRead`; fold `HostSkipped` into a new `NoticeKind` in `Universe::apply`
   and ignore the other two there; `scan` appends remote projects after the local
   ones and emits `RecentRead` (when non-empty) before `ProjectsDiscovered`.
   Files: `crates/workspace/src/settings.rs`, `crates/workspace/src/event.rs`,
   `crates/workspace/src/universe.rs`, `crates/workspace/src/lib.rs`.
3. [x] Recent store and host probe. `RecentStore { beside, at, load, save }` with an
   atomic write-and-rename, `start_keeping() -> RecentKeeper`; `HostProbe`,
   `SshProbe` (`fn command(host)`, all three streams null, failed spawn =
   offline) and `start_probes` (one thread per host, `HostProbed` over mpsc).
   Files: `crates/workspace/src/recent.rs`, `crates/workspace/src/hosts.rs`,
   `crates/workspace/src/lib.rs`.
4. [x] Project details in CodeAnalysis. `GitListing::branch`, `GitCli` impl,
   `CodeAnalysisEvent::DetailsRead { project, branch, files }`, `start_details`;
   add the ignoring arm in the binary's exhaustive match. Files:
   `crates/code-analysis/src/git.rs`, `crates/code-analysis/src/event.rs`,
   `crates/code-analysis/src/lib.rs`, `src/opened.rs`.
5. [x] Launcher model. `Location`, `Pick`, `HostState`, `ProjectKind`,
   `ProjectDetails`, `Launcher` (search, pick, select, move_selection, apply,
   opened, detailed, kind, hosts, recent_row, groups) per requirements 5–9.
   Files: `crates/workspace/src/launcher.rs`, `crates/workspace/src/lib.rs`.
6. [x] Launcher view and app wiring. `ui::launcher(ui, &Universe, &mut Launcher) ->
   Option<LauncherAction>` replaces `ui::universe`/`UniverseClick`;
   `CellworksApp` gains the launcher, probe, details and recent keeper (with
   `with_probe`, `with_home`, `with_recent`), forwards drained workspace events
   to the launcher, starts probes on the first `Reported` frame and a project's details when it is selected,
   repaints while their receivers live, records opens into Recent, and maps
   selection to Navigation as decided. Port the STORY-002 and universe-frame
   tests to the launcher; add the STORY-078 acceptance tests; add ssh to the
   tech stack and foreign systems in `CLAUDE.md`. Files:
   `crates/workspace/src/ui.rs`, `src/app.rs`,
   `crates/workspace/tests/story_002_see_every_project_in_the_universe.rs`,
   `tests/story_003_see_a_code_project_as_districts.rs`,
   `tests/universe_frame.rs`,
   `tests/story_078_the_launcher_shows_every_project_grouped_by_where_it_lives.rs`,
   `CLAUDE.md`.

   Departures from the files listed above, each necessary for the crate to
   compile or the suite to stay green, recorded here per CLAUDE.md's
   instruction to update the plan in the same change: `universe.rs`'s
   `drain` now returns the `Vec<WorkspaceEvent>` it applied, so `CellworksApp`
   can forward the same events to the `Launcher`; `launcher.rs` gained one
   public method, `path_of`, exposing the display-path logic `groups`'s search
   already used internally, for the view to show the same text; `lib.rs`
   re-exports `hosts::start_probes` (step 3 re-exported `HostProbe`/`SshProbe`
   but not this), which the binary needs; `tests/story_003_...rs` and
   `tests/story_004_...rs` had one assertion each pinned to the universe's
   exact label list, which this story surrounds with the launcher's own
   chrome — each changed to a presence check, the same adjustment already
   made to STORY-002's and the universe-frame's own tests, preserving what
   each originally proved.

   The backend-security-reviewer found three Important issues after the above
   landed, each fixed in the same change: (1) requirement 2's host-name check
   was a denylist (empty, leading `-`, whitespace or control characters) that
   let a name through with shell metacharacters ssh itself expands for
   `ProxyCommand`/`LocalCommand`/`Match exec` (each run through a shell) —
   `settings::unsafe_host_name` is now an allowlist (letters, digits, `.`,
   `-` not leading, `_`, `@`), tighter than requirement 2's literal three
   conditions but serving the same stated intent; (2) `CellworksApp::over()`
   defaulted to the real `SshProbe`/`GitCli` with nothing to stop the
   first-`Reported`-frame logic from starting them, so a test that never
   called `with_probe`/`with_git` (`tests/universe_frame.rs`, pre-dating this
   story) could still trigger a real `ssh`/`git` child process by accident —
   added a `CellworksApp::auto_detail` field, false from `over()` and true
   from `start()`, `with_probe` and `with_git`, so the mechanism is only ever
   live when something has actually been asked to use it; (3) the launch-time details read ran git in every discovered project, so it was
   dropped: details are now read only for the selected project (decided by the
   operator), and `GitCli`'s `command()` sets `GIT_CONFIG_NOSYSTEM=1` and passes
   `-c core.fsmonitor=false` and `--no-pager`, so a repository's config cannot
   make git run a program for those; the global config is deliberately kept so
   the engineer's excludes and `safe.directory` still apply.
   Two Nits were raised: a recent id whose remote path holds a newline could
   split across two lines — `RecentStore::save` now leaves such an id out;
   and `ProjectId::parse` does not itself re-apply `unsafe_host_name` to a
   host it reads back from the `recent` file — left as is, since every
   caller of `parse` already discards an id that no longer names a
   discovered project, so an unsafe host name synthesised this way reaches
   nothing.

   The inspector panel is drawn on every launcher frame, empty while nothing
   is selected, rather than only once a project is selected
   (`crates/workspace/src/ui.rs`). Appearing on the first click of a
   double-click, it narrowed the main area by 300 px; a group heading whose
   folder path then no longer fit wrapped to a second line, the row moved
   down under the pointer, and the second click missed it, so the project did
   not open. The acceptance tests hit this on roughly a third of runs,
   depending on the width of the temporary directory's random name; an
   engineer hits it with any folder path that wraps at the narrower width.
   `a_double_click_opens_a_project_under_a_long_folder_path`
   (`tests/story_003_...rs`) pins it with a fixed path that wraps.

## Risks

- Blast radius: `ProjectId` is shared by Workspace, Map, Navigation and the
  binary; step 1 is the riskiest. Mitigation in that step: `local_path()`
  returning `Option` makes the compiler find every caller, and the full test
  suite runs before step 2.
- ssh: a host whose config needs a password reads as offline; a missing `ssh`
  binary marks every host offline; `ConnectTimeout=5` bounds a probe thread.
  The host name reaches ssh's argument vector, guarded by the name check and `--`.
- egui 0.36 may expose disabled-button hover text and `SidePanel` labels
  differently in the accessibility tree than the tests expect; adjust the test
  queries, not the behaviour. Recent cards are labelled `recent: <name>` so
  existing `get_by_label(<name>)` queries stay unique.
- Rollback: revert the squash-merged pull request; the `recent` file and the
  `[hosts]` table are ignored by the previous build, so no data migration is
  needed.

## Proof

Commands (all must pass, as in `CLAUDE.md` § Verifying your work):

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    SHELLCHECK_OPTS=--exclude=SC2016 actionlint

Acceptance tests, one per scenario, headless `egui_kittest` through
`cellworks::draw`, in
`tests/story_078_the_launcher_shows_every_project_grouped_by_where_it_lives.rs`.
Fixture: tempdir home with `Projects/cellworks` and `Projects/shop` (both with
`.git`), plain folder `scratch/tool`, `settings.toml` with
`added_folders = ["<home>/scratch/tool"]` and
`[hosts."az.wsl"] projects = ["~/work/api"]`; fake `GitListing` (branch "main")
and fake `HostProbe`.

- `projects_are_grouped_by_their_folder_or_remote_host` — headings "~/Projects",
  "~/scratch", "az.wsl"; a name and a path label per project.
- `the_sources_column_narrows_the_list` — clicking "az.wsl" leaves only "api";
  a connected and an offline host read "● connected" and "○ offline".
- `searching_narrows_the_list_by_name_or_path` — "api" leaves only `~/work/api`.
- `a_search_that_matches_nothing_says_so` — `No project matches
  "nothing-like-this".` and the "Add project…" button.
- `recently_opened_projects_come_first` — fixture `recent` lists cellworks then
  `az.wsl:~/work/api`; `recent: cellworks` precedes `recent: api`.
- `git_reads_only_the_selected_project_and_only_once` — nothing is read at launch;
  selecting a project reads it once, no other.
- `the_selected_project_shows_its_details_and_opens_on_enter` — inspector shows
  "git repository", "main" and the file count; Enter opens the city.
- `enter_on_a_remote_project_opens_nothing` — Open is disabled with "Connect the
  host to open it"; `app.opened()` stays `None`.

Unit tests: `a_remote_id_has_no_local_path`, `an_id_reads_back_what_it_wrote`
(project.rs); `hosts_in_the_settings_file_become_remote_projects`,
`a_host_named_like_an_option_is_skipped` (settings.rs);
`remote_projects_follow_the_local_ones_and_the_settings_file_is_untouched`
(lib.rs); `the_ssh_probe_puts_the_host_after_the_options_and_double_dash`,
`start_probes_reports_each_host_over_the_channel` (hosts.rs);
`recent_projects_read_back_most_recent_first`,
`a_recent_line_that_does_not_parse_is_left_out` (recent.rs);
`opening_again_moves_a_project_to_the_front_and_five_are_kept`,
`groups_are_parent_folders_then_hosts`, `home_is_shown_as_a_tilde`,
`picking_this_mac_hides_remote_projects`,
`search_matches_the_name_or_the_path_ignoring_case` (launcher.rs);
`the_branch_is_asked_in_the_repository_folder`,
`git_is_stopped_from_running_commands_a_repository_names` (git.rs);
`git_does_not_run_the_fsmonitor_a_repository_names`
(code-analysis tests/git_cli_against_real_git.rs, real git, run with
`git_lists_tracked_and_untracked_files_but_not_ignored_or_deleted_ones` in the
default `cargo test --workspace`);
`a_double_click_opens_a_project_under_a_long_folder_path`
(tests/story_003_...rs);
`git_reads_only_the_selected_project_and_only_once`
(tests/story_078_...rs);
`details_carry_a_file_count_only_for_a_repository` (code-analysis lib.rs);
`arrow_order_lists_each_project_once` (launcher.rs).
STORY-002 and universe-frame tests keep their intent on the launcher.

## Docs impact

Written at freeze in the commit `docs: ingest STORY-078 prototype decisions`;
the build changes no other doc.

- **Business decisions** — `docs/domain/contexts/Workspace/Workspace.md`: remote
  hosts and their projects come from the settings file; a host's state comes from
  an ssh reachability probe; recently opened projects are kept beside the
  settings file; a remote project is never read as a local folder.
- **Glossary** — `docs/domain/contexts/Workspace/glossary.md`: Launcher,
  Location, Remote host, Remote project, Recent ("sources" and "HOSTS" are UI
  words only). `docs/domain/glossary.md`: Project may be identified by
  `host:path`.
- **Guidelines** — None.
- **arc42** — `03-context.md`: ssh as a foreign system (node, neighbour row,
  § 3.2 section). `05-building-blocks.md`: workspace row adds `host:path`
  identity, the launcher, the recent store and the host probe.
  `08-crosscutting.md` § 8.5: remote ids are not canonicalized.
- **ADR needed** — No. ssh follows ADR-0003, the recent file ADR-0006.
