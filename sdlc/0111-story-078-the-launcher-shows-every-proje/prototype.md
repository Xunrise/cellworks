# Prototype: STORY-078 — the launcher shows every project grouped by where it lives

Issue: squer-solutions/cellworks#111 (EPIC-008). The launcher was prototyped as
mockups in Claude Design, not in code. The canvas
(https://claude.ai/artifact/7RGjKbyhMxHj1wNhwTNoAn) is private and CI cannot
open it, so this record describes what it shows. Theme tokens come from the Clean
Cellworks design system (https://claude.ai/artifact/7eqPqvt2Bfe3xgcfKFivqY),
also private.

## Confirmed behavior

The operator chose mockup variant A, "One launcher, local and remote". It replaces
the current flat universe list that is shown when no project is open.

- Header: the title "Cellworks", a search box (hint: projects, folders or
  host:/path, ⌘K) and a primary button "Add project…".
- Left sources column, 248 px wide: "All projects", "Recent", "This Mac", then a
  "HOSTS" heading listing each remote host with its state (connected or offline),
  then "+ Connect host…". Picking a source narrows the main list to it.
- Main area: a "Recent" row of up to five project cards, most recently opened
  first, then every project grouped by its parent folder (for example
  `~/Projects`) or, for a remote project, by its host. Each row shows the
  project's name, its path, its branch and its state.
- State is always a glyph paired with a word, never colour alone.
- Right inspector, 300 px wide: the selected project's kind, branch, file count,
  age of its local copy and its sessions, with an "Open" button (Enter) and a
  "Sync" button.
- Footer hints: ↑↓ move, Enter open, ⌘K search, ⌘N add.
- Search narrows the list by name or path. A search with no match says so and
  offers to add a project.
- Clean Cellworks theme: one colour per meaning, IBM Plex Sans for text and IBM
  Plex Mono for paths and branches.

## Feedback log

- "kinda shit" (about the current flat universe menu) → three launcher mockups
  were drawn in Claude Design (variants A, B, C).
- Feedback issue #105: show the path, group by folder, allow browsing files →
  every row in the mockups shows its path, and projects are grouped by folder or
  host.
- The operator picked variant A → it is the confirmed launcher; B's parts were
  moved behind "Connect host…" and "Add project…".

## Rejected

- Variant C, a command-palette-only launcher: not chosen. The operator wanted a
  persistent screen with sources and an inspector, not a transient palette.
- Keeping the flat universe list: rejected as the starting complaint.
- Variant B as the main screen (connect form and breadcrumb folder browser up
  front): not chosen as the launcher. Its connect form belongs behind
  "+ Connect host…" (STORY-074) and its folder browser behind "Add project…"
  (STORY-079, STORY-075). Neither is built in this story.

## Edge cases

- No projects match a search → a message naming the search text and an offer to
  add a project.
- A remote host that is not reachable → listed under HOSTS as offline; its
  projects are still listed and grouped under it.
- Fewer than five recently opened projects → the Recent row shows only those.
- A project never opened has no local-copy age and no sessions; the inspector
  leaves those out rather than showing zero.

## Decisions

- Q: Which mockup variant? → A: Variant A, one launcher for local and remote.
- Q: Does STORY-078 connect to hosts, copy or sync remote projects, or handle
  offline work? → A: No. Remote projects are only listed and grouped here.
  Connecting, the local copy and offline behaviour are STORY-074..077 under
  EPIC-023, which build on this launcher. The "Sync" button and "+ Connect host…"
  are placeholders for those stories.
- Q: Is the launcher slow? → A: No. Release builds were fine; the slow
  impression came from a debug build. No performance work in this story.
- Q: Which branch carries this freeze? → A: `proto/launcher`, holding only these
  sdlc files. The SSH prototype stays on `proto/ssh-remote` for the EPIC-023
  freezes, so the two handoffs do not overwrite each other.
- Q: How does "launcher" relate to the existing "universe" (STORY-002)? → A:
  "Launcher" is a new Workspace term: the view (search, picked location,
  grouping, selection) over the existing `Universe` scan state, which stays.
- Q: What is a left-column entry called in the domain, given "source" already
  names the Sources context and the `Sources` struct? → A: "Location" (this Mac or
  a remote host). "Sources" is a UI word only.
- Q: How does a remote project exist? → A: Project identity is extended with a
  `host:path` form, not canonicalized locally, like the existing `jira:`/`otc:`/
  `data:` prefixes. Rejected: a launcher-only remote type, and deferring remote
  projects to STORY-074.
- Q: Where do remote hosts and projects come from, and how is connected or
  offline decided? → A: Hand-edited settings, for example
  `[hosts."az.wsl"] projects = ["~/work/api"]`, like `added_folders`. The state
  comes from a background reachability probe,
  `ssh -o BatchMode=yes <host> true`, so ssh becomes a foreign system now.
  Rejected: every host shown offline until STORY-074.
- Q: What do Open on a remote project, "Add project…", "+ Connect host…" and
  "Sync" do here? → A: Visible but disabled. Open on a remote row hints "Connect
  the host to open it"; the others say they are coming soon. The no-match offer
  to add a project points at the disabled "Add project…".
- Q: Which kinds does the inspector show? → A: git repository, folder, remote.
- Q: Recent, branch and file count, inspector fields? → A: Recent is a new
  `recent` file beside the settings file (last five project ids, most recent
  first, written when a project opens, per ADR-0006; the settings file stays
  read-only). Branch and file count load off the UI thread. The inspector omits
  local-copy age and sessions until EPIC-023 and AgentSessions provide them.

## Evidence

- Mockups: Claude Design canvas https://claude.ai/artifact/7RGjKbyhMxHj1wNhwTNoAn,
  variants A (chosen), B and C, built on the Clean Cellworks design system.
- No launcher code was prototyped; this branch (`proto/launcher`, off `test`
  at 941c3ff) carries only the handoff files.
- Related evidence for EPIC-023, not part of this story: branch
  `proto/ssh-remote` (commit 9ead1de) adds `src/remote.rs` and hooks it into
  `src/app.rs`. It connects to a host with an ssh ControlMaster and SSH_ASKPASS,
  lists remote folders, mirrors one with rsync into
  `~/Library/Caches/Cellworks/remote` and opens the mirror as a city. It was run
  with `cargo run --release` against a WSL host over SSH and worked.
