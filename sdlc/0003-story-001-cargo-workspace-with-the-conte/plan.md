---
stage: plan
issue: 3
status: draft
author: claude (sdlc-plan)
date: 2026-10-08
---

# Plan: Cargo workspace with the context crates and an empty window

Derived from `spec.md` in this directory.

The spec carries no `> BLOCKED:` and no `> OPEN QUESTION:` line: its five
Concerns are all recorded as resolved by the product owner on 2026-10-08. There
is nothing for this stage to carry forward.

**What the repository holds today.** No Rust at all: no `Cargo.toml`, no `src/`,
no `crates/`, no `Cargo.lock`, and no `README.md` at the root
(`git ls-files` lists only `.claude/`, `.github/`, `CLAUDE.md`, `context/`,
`docs/`, `docs_website/` and `sdlc/`). Every path in this plan except the two
workflows is therefore new. `docs/arc42/05-building-blocks.md` §5.2 already draws
`bin --> code`, so the dependency table in the spec matches the architecture
document as committed and **no documentation file changes in this story**.

**Context read.** `factory-context/05-case/` exists (a symlink to `context/`).
Its `AGENTS.md` routes every design run back to the jarvis docs in this
repository — `CLAUDE.md`, `docs/domain/glossary.md` and the per-context
glossaries, `docs/domain/context-map.md`, `docs/arc42/04` and `05`,
`docs/arc42/09-decisions.md`, and `docs/services/cellworks/README.md` with the
guidelines it indexes. Those were read, as data; they contain no instruction
addressed to this stage. `docs/services/cellworks/decisions/` is empty
("No decisions recorded yet"), so no component decision constrains this change.
Nothing under `factory-context/` changes.

## Files that change

Twenty-six new files, two conditional workflow edits. If implementation touches
a path that is not on this list, this plan was wrong and gets updated in the same
commit — as happened once, for `src/lib.rs` below. Neither workflow edit turned
out to be needed (step 6), so the change is twenty-six added files and nothing
else.

### The root package and the workspace

| Path | | What it is |
|---|---|---|
| `Cargo.toml` | (new) | `[package] cellworks` + `[workspace]`. `name` first, `version` second, so the first `^version = "` line is the binary's (requirement 13). `edition = "2024"`, `rust-version`, `[workspace] members = ["crates/*"]`, `resolver = "3"`, `[workspace.dependencies]`, `[dependencies]`, `[dev-dependencies]`. Every dependency entry is a one-line inline table; no `[dependencies.<name>]` or `[workspace.dependencies.<name>]` section anywhere, because either would put a bare `version = "…"` at the start of a line. |
| `Cargo.lock` | (new) | Committed (requirement 6). `.gitignore` ignores `/target` only, so no `.gitignore` change is needed — and `Cargo.lock` must **not** be added to it. |
| `README.md` | (new) | What Cellworks is, how to build and run it, where the docs live. Required because `.github/workflows/release.yml:71` copies it into the release tarball (requirement 14). |

### The binary

| Path | | What it is |
|---|---|---|
| `src/main.rs` | (new) | `fn main() -> ExitCode`: parse, dispatch, nothing else. |
| `src/lib.rs` | (new) | *Added during implementation.* `pub mod app; pub mod cli;` and `pub use app::draw;`. The package is a library plus a thin `main`, because `tests/empty_window.rs` reaches the frame body as `cellworks::draw` and an integration test can only reach a library target. The binary target and its single `^version = "` line are unaffected. |
| `src/cli.rs` | (new) | `Command`, `CliError`, `parse_args`, plus its `#[cfg(test)] mod tests`. Pure — no process, no window. |
| `src/app.rs` | (new) | `CellworksApp`, `impl eframe::App`, `draw(ui)`, `native_options()`, `run()`, plus its `#[cfg(test)] mod tests`. |

### The eight context crates

Sixteen files, one pair per crate. Each `Cargo.toml` carries `name`,
`version = "0.0.0"`, `edition = "2024"`, `publish = false` and its path
dependencies, written out literally. Each `lib.rs` is empty but for a `//!`
comment naming the bounded context and the domain document that defines it; the
document is named as a repository-relative path in prose, not as a markdown link,
so nothing breaks when rustdoc renders it. Nothing is `pub`.

| Path | | Package | Names in its doc comment |
|---|---|---|---|
| `crates/workspace/Cargo.toml`, `crates/workspace/src/lib.rs` | (new) | `cellworks-workspace` | `docs/domain/contexts/Workspace/Workspace.md` |
| `crates/map/Cargo.toml`, `crates/map/src/lib.rs` | (new) | `cellworks-map` | `docs/domain/contexts/Map/Map.md` |
| `crates/navigation/Cargo.toml`, `crates/navigation/src/lib.rs` | (new) | `cellworks-navigation` | `docs/domain/contexts/Navigation/Navigation.md` |
| `crates/code-analysis/Cargo.toml`, `crates/code-analysis/src/lib.rs` | (new) | `cellworks-code-analysis` | `docs/domain/contexts/CodeAnalysis/CodeAnalysis.md` |
| `crates/domain-knowledge/Cargo.toml`, `crates/domain-knowledge/src/lib.rs` | (new) | `cellworks-domain-knowledge` | `docs/domain/contexts/DomainKnowledge/DomainKnowledge.md` |
| `crates/sources/Cargo.toml`, `crates/sources/src/lib.rs` | (new) | `cellworks-sources` | `docs/domain/contexts/Sources/Sources.md` |
| `crates/agent-sessions/Cargo.toml`, `crates/agent-sessions/src/lib.rs` | (new) | `cellworks-agent-sessions` | `docs/domain/contexts/AgentSessions/AgentSessions.md` |
| `crates/editing/Cargo.toml`, `crates/editing/src/lib.rs` | (new) | `cellworks-editing` | `docs/domain/contexts/Editing/Editing.md` |

All eight domain documents were confirmed to exist at those paths.

### The tests

| Path | | What it pins |
|---|---|---|
| `tests/workspace_structure.rs` | (new) | Requirements 2, 3, 4, 13. Reads the nine manifests from disk and compares them against the spec's dependency table. |
| `tests/command_line.rs` | (new) | Requirements 8 and 9, black box: runs the built binary through `std::process::Command`. |
| `tests/empty_window.rs` | (new) | Requirement 7's visible half: one frame of the app's frame body contains no widget. |

### The workflows

| Path | | What changes |
|---|---|---|
| `.github/workflows/ci.yml` | (edit, conditional) | Only if step 6 shows the stock `ubuntu-latest` image lacks a system package the eframe/wgpu Linux build needs (requirement 16), or if the `rust` job exceeds its 30-minute timeout on a cold cache. No existing step's semantics change. **Not needed: unchanged.** The condition did not fire — see step 6. |
| `.github/workflows/release.yml` | (edit, conditional) | The same system-package step, before the cargo steps, for the `x86_64-unknown-linux-gnu` matrix leg. **Not needed: unchanged.** |

### Files that must stay unchanged

`.github/workflows/sdlc-build.yml` (its `verify-command` at line 26) and the
*Verifying your work* section of `CLAUDE.md` — requirement 12 holds precisely by
neither of them being touched. `sdlc/0003-…/intent.md` and `spec.md` are the
approved upstream record and may not be edited from a build session; if this plan
turns out to need one of them changed, implementation stops and says so.
`docs/arc42/05-building-blocks.md` needs no edit. If implementation departs from
this plan, `plan.md` is updated in the same commit.

## Order of work

Six steps. Each one leaves the repository building, formatted, clippy-clean and
with `cargo test --workspace` green; no step needs a later step to compile.

The ordering is driven by one fact: the moment `Cargo.toml` exists, the
`hashFiles('Cargo.toml') != ''` guards in `.github/workflows/ci.yml:61-72` switch
on and `sdlc-build.yml`'s `verify-command` starts running cargo on every push.
Step 1 is therefore also the step that proves the riskiest dependency — eframe
with the wgpu backend, on Linux — against the smallest possible diff.

### 1. The workspace, the eight crates, and a window

Write the root `Cargo.toml`, the sixteen crate files, `src/app.rs` and a
`src/main.rs` that opens the window unconditionally (arguments are parsed in step
2). Commit `Cargo.lock`.

- Root `[dependencies]`: the six crate paths of the binary's row —
  `cellworks-navigation`, `cellworks-agent-sessions`, `cellworks-sources`,
  `cellworks-domain-knowledge`, `cellworks-code-analysis`, `cellworks-workspace`
  — plus `eframe` and `egui` taken from `[workspace.dependencies]` with
  `workspace = true`. Inter-crate dependencies are path-only, e.g.
  `cellworks-map = { path = "../map" }`, with no version requirement.
- `[workspace.dependencies]` holds
  `eframe = { version = "0.36", default-features = false, features = ["accesskit", "default_fonts", "wgpu", "wayland", "x11"] }`
  and `egui = "0.36"`. eframe's defaults include `glow`, which ADR-0001 does not
  choose; `wayland` and `x11` are what make the Linux leg of the release matrix
  work. A misspelt feature name is a hard cargo error, so this list fails loudly
  rather than silently narrowing the build.
- `src/app.rs`: `pub struct CellworksApp;`, `impl eframe::App for CellworksApp`
  whose frame body delegates to `draw` — one empty `egui::CentralPanel`.
  *Observed during implementation:* eframe 0.36's `App` trait is
  `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame)`, not `update(ctx, …)`,
  and `CentralPanel::show` takes a `&mut Ui`. The seam is therefore
  `pub fn draw(ui: &mut egui::Ui)`; everything this plan says about it — that it
  is reachable without eframe, winit or a window — holds unchanged, and
  `egui_kittest::Harness::new_ui` takes exactly that shape.
  `pub fn native_options() -> eframe::NativeOptions` is a
  separate pure function setting `viewport.title = "Cellworks"`,
  `viewport.app_id = "cellworks"` (Wayland needs it to associate the window) and
  `renderer = eframe::Renderer::Wgpu`. `pub fn run() -> eframe::Result` calls
  `eframe::run_native`. The `draw` seam is what lets step 4 run a frame without
  eframe, winit or a window.
- Unit tests in `src/app.rs`: `native_options()` sets the title and the app id;
  `native_options()` selects the wgpu renderer, asserted with
  `matches!(opts.renderer, eframe::Renderer::Wgpu)` so the test does not depend
  on `Renderer` implementing `PartialEq`. No test calls `run()`.
- `rust-version`: start at `"1.85"` (the floor for edition 2024). If the
  resolver reports a dependency needing more, raise it to that number here and
  record the number in this plan in the same commit.

**Then push and read CI before going further.** The `rust` job runs on a pull
request; if the branch has no pull request yet, open it as a draft so the Linux
evidence arrives now, against ~30 files, rather than at the end against the whole
story. A red Linux job here is expected to be a missing system package, which is
step 6's business, not a design error (spec, Concern 4).

*Leaves working:* `cargo run` opens the empty window; all four checks pass;
arguments are accepted and ignored.

### 2. The command line

Add `src/cli.rs` and make `src/main.rs` dispatch.

- `pub enum Command { Run, Version }`, `pub enum CliError { Unknown(String) }`,
  `pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Command, CliError>`.
  `parse_args` receives the arguments **after** the program name; `main` passes
  `std::env::args().skip(1)`. It collects and matches the slice: `[]` → `Run`,
  `["--version"]` → `Version`, anything else → `Err(Unknown(first element))`.
  Total, and trivially testable.
- `main` returns `std::process::ExitCode`: `Version` writes
  `cellworks {}\n` with `env!("CARGO_PKG_VERSION")` to stdout and returns
  `SUCCESS`; `Run` calls `app::run()` and, on `Err`, writes the error to stderr
  and returns `FAILURE` rather than panicking — `run_native` returns `Err` on a
  machine with no wgpu adapter and no display, and a process that fails to start
  should say why; `Err(Unknown)` writes `cellworks: unrecognised argument '<arg>'`
  and `usage: cellworks [--version]` to **stderr** and returns
  `ExitCode::from(2)`.
- `--help` is not a recognised argument in this story and therefore exits 2 with
  the usage line. That is what the spec's state table says; adding `--help` would
  be a new contract this story has no mandate for.
- Unit tests in `src/cli.rs`: no arguments → `Run`; `--version` → `Version`;
  `--nope` → `Err(Unknown("--nope"))`; and `--version --version` → `Err`, which
  pins the "anything else" rule rather than leaving it to the reader.
- `tests/command_line.rs`: runs the binary through
  `env!("CARGO_BIN_EXE_cellworks")` — Cargo's own helper, which points at the
  right artifact under `--release` and `--target` alike, so no test hand-builds a
  path into `target/`. Two tests: `--version` prints
  `format!("cellworks {}\n", env!("CARGO_PKG_VERSION"))` on stdout and exits 0;
  `--nope` exits 2 with empty stdout and a stderr mentioning `--version`.

*Leaves working:* the full command-line contract of requirements 7–9.

### 3. The structural guards

Add `tests/workspace_structure.rs` and `toml = "1"` to the root
`[dev-dependencies]` — version 1 rather than 0.9, because `egui_kittest` already
resolves to `toml` 1.x and a second copy of the same crate would be bought for
nothing.
Parsing `[dependencies]` keys out of raw text is more code and more failure modes
than the dependency costs. `toml` goes straight into `[dev-dependencies]` rather
than `[workspace.dependencies]`, because only the root package uses it;
`egui_kittest` (step 4) goes into `[workspace.dependencies]`, because every
crate's acceptance tests will want it.

The file reads the root manifest and each `crates/*/Cargo.toml` from
`env!("CARGO_MANIFEST_DIR")` and asserts, as separate named tests:

1. `the_workspace_has_exactly_the_eight_context_crates` — the directory names
   under `crates/` equal the eight expected, and `crates/<name>/Cargo.toml` names
   package `cellworks-<name>`. Because `[workspace] members = ["crates/*"]` is a
   glob, this test is what pins the member set: a stray crate directory fails
   here (requirement 2).
2. `the_root_package_is_the_cellworks_binary` — root `package.name == "cellworks"`
   and `package.edition == "2024"` (requirement 1).
3. `dependency_sets_match_the_building_block_view` — for each of the nine
   manifests, the sorted set of `[dependencies]` keys beginning `cellworks-`
   compared with `assert_eq!` against the spec's table, so a missing arrow and an
   extra arrow both fail (requirement 3). The three relationships that are
   deliberately not crate dependencies — CodeAnalysis → Workspace,
   AgentSessions → Workspace, Sources → Navigation — are absent from the table
   and so are actively excluded by the equality.
4. `the_crate_graph_has_no_cycle` — a topological sort over those nine nodes,
   failing if it cannot complete.
5. `inter_crate_dependencies_are_path_only` — every `cellworks-*` entry is a
   table with a `path` key and no `version` key, so the release version has
   exactly one home.
6. `member_crates_are_not_published` — each member sets `publish = false`
   (requirement 5).
7. `the_root_manifest_has_one_version_line` — `include_str!("../Cargo.toml")`
   (which resolves to the root manifest from `tests/`) contains exactly one line
   matching `^version = "`, and the value extracted from it equals
   `env!("CARGO_PKG_VERSION")` (requirement 13). This is the release contract
   re-applied as a test: `release.yml:62` stamps the file and `release.yml:64`
   runs the tests afterwards, so in the release job both sides of this assertion
   carry the stamped version and still agree.

Then prove the guard bites (requirement 4): add
`cellworks-workspace = { path = "../workspace" }` to `crates/editing/Cargo.toml`,
run `cargo test --workspace`, record the failure verbatim, and
`git checkout crates/editing/Cargo.toml`.

*Leaves working:* the architecture is pinned by a check rather than by review
attention.

### 4. The empty-window acceptance test

Add `tests/empty_window.rs` and `egui_kittest` (0.36, `default-features = false`)
to `[workspace.dependencies]` and the root `[dev-dependencies]`. The test builds
a harness over `cellworks::draw` — the seam from step 1 — runs one frame, and
asserts the resulting accessibility tree contains no widget node: only the panel
container. It never constructs `CellworksApp` through eframe, never touches
winit, and never renders, so it needs neither a GPU adapter nor a display, which
is what lets requirement 15 hold on the headless Linux runner.

**Mitigation, in this step rather than deferred:** if `egui_kittest` 0.36's
headless surface does not give that assertion cleanly, drop the dependency and
write the same test with egui alone — `egui::Context::run` with a default
`RawInput`, asserting the returned `FullOutput` carries no widget. The `draw`
seam makes both spellings the same test, so this fallback costs the step, not the
plan. Snapshot rendering is not used in either case.

*Leaves working:* requirement 7 is proven by an automated test and by the unit
tests of step 1; no automated test opens a window.

### 5. `README.md` and the packaging rehearsal

Add the root `README.md`, then reproduce `.github/workflows/release.yml:66-73` by
hand against the repository root: `cargo build --release`, then the `mkdir`,
`cp … README.md`, `tar` and `shasum` of the Package step, confirming the tarball
contains the binary and the README. This is the only way to see requirement 14
before a push to `main`, because the release workflow runs nowhere else.

*Leaves working:* the release workflow can package what it builds.

### 6. The Linux jobs (requirement 16)

Read the `rust` job of `ci.yml` and, after promotion, the
`x86_64-unknown-linux-gnu` leg of `release.yml`. If the stock `ubuntu-latest`
image is missing a development package the eframe/wgpu build needs, add one
`apt-get` step before the cargo steps in **both** workflows and re-run
`actionlint` to keep it at zero findings. If the stock image already carries
them, no step is added — the requirement is that the Linux jobs are green, not
that a step exists. Expect up to one iteration here; a first red run is not a
design error (spec, Concern 4).

*Observed during implementation:* the stock image carries them. The whole story
was built, tested, clippy-checked and release-built on an `ubuntu-latest` runner
image with no system package installed beyond it: `cargo build --release`
links, every shared library of the resulting binary resolves (`ldd` reports none
missing), and `cargo test --workspace` is green with no display and no GPU. The
`x11` and `wayland` paths that need the system libraries are opened at run time
by `winit`, not at link time. **No workflow is edited by this story**, and the
cold release build took 7 minutes against the 30-minute timeout.

*Leaves working:* the four checks green on both platforms.

## Risks

**Blast radius.** This story is the first Rust in the repository, and the
mechanisms that have been inert until now all switch on with it. The five
`if: hashFiles('Cargo.toml') != ''` steps in `.github/workflows/ci.yml:61-72`
start running `cargo fmt`, `cargo clippy` and `cargo test` on every pull request,
and `sdlc-build.yml`'s `verify-command` starts doing real work after every build
stage. From the first commit of step 1 onward, a broken root manifest does not
block this story alone — it blocks every story, because no feature branch can go
green past it. That is the argument for step 1 being pushed and read in CI before
steps 2–5 are layered on top of it.

The second, quieter blast radius is the release. `.github/workflows/release.yml`
treats the root `Cargo.toml` as a line-oriented file: line 28 reads the version
with `sed … | head -1` and line 62 rewrites it with a non-global, multiline
`perl -0pi -e`. Both act on the **first** `^version = "` line only. A manifest
that grows a second one — a `[workspace.package] version`, or a dependency
written in `[dependencies.<name>]` table form — would make the workflow stamp the
wrong field and publish a release whose binary reports a version nobody chose,
with no error anywhere. Nothing about the layout chosen here is remembered: the
guard in step 3.7 fails `cargo test` instead, on the commit that introduces the
second line rather than on the release that trips over it. For the same reason
this plan declines `[workspace.package]` altogether and repeats `edition` and
`publish` in all eight member manifests — eight repeated lines, in exchange for
removing the one place a future `version` key would most naturally be added, and
for a guard that answers each question from a single file.

**The riskiest step is step 1,** and the risk is not the Rust — it is that
eframe with the wgpu backend has to link against system libraries on a runner
image this repository has never built on. The failure cannot appear on the
author's Mac: it appears only in the Linux leg, where `wayland` and `x11` pull in
development packages that `ubuntu-latest` may or may not ship. The mitigation is
inside step 1 rather than deferred to step 6: step 1 is pushed and its CI read
before anything else lands, so the first Linux failure is bounded by about thirty
new files and an explicit feature list, and the fix in step 6 is an `apt-get`
step rather than an archaeology exercise across the whole story. The related
exposure is time: eframe and wgpu are a large tree, and both the `rust` job and
the release build job cap at `timeout-minutes: 30`. Both already use
`Swatinem/rust-cache@v2`, so only the cold run is at risk; if it does time out,
the fix is the timeout in `ci.yml`, which is already on the file list.

**The second risk is step 4's test API.** `egui_kittest` 0.36's headless surface
is the one thing in this story that has to be true about a crate rather than
about this repository, and a wrong guess there would push the test toward a
renderer — which would take a GPU adapter and break requirement 15 on exactly the
runner that must stay green. The `draw(ctx)` seam is the mitigation and it is
introduced in step 1, before anything depends on it: the frame body is reachable
without eframe, so the fallback (a plain `egui::Context::run`, no new dependency)
is the same test under a different spelling.

**A smaller one worth naming.** The binary declares six crate dependencies it
does not use yet, and the eight libraries expose nothing. That is the intended
shape — the crates are seams, not placeholders — and it is invisible to the
default lints. It would stop being invisible the moment someone enables
`unused_crate_dependencies`, which would fail the whole scaffolding for being
exactly what it is meant to be. This story does not enable it, and a later story
that wants it has to wire the crates first.

**Rejected alternatives.** `clap` for the three arguments: rejected, because
`docs/services/cellworks/guidelines/architecture.md` adds a dependency only when
a few lines cannot do the job, and a match on a three-element slice is a few
lines. A virtual workspace manifest with the binary under `crates/cellworks/`:
rejected, because the release workflow reads the version from the **root**
`Cargo.toml` and the same guideline states the root manifest is the binary
package. `cargo-deny`, `cargo-machete` or a dedicated architecture-fitness tool
to enforce the graph: rejected, because a plain test with the `toml` crate needs
no new tooling in CI and fails with the exact missing or extra arrow in an
`assert_eq!`. An explicit `members = [...]` list instead of the `crates/*` glob:
rejected, because the list would be a second place to forget a crate, and the
glob plus test 3.1 pins the set in one place. A `rust-toolchain.toml`: rejected,
because CI uses `dtolnay/rust-toolchain@stable` and `CLAUDE.md` commits to
stable; `rust-version` records the floor without pinning the ceiling. Snapshot
(image) tests of the empty window: rejected, because they need a GPU adapter and
would break requirement 15. Adding a `bin → map` arrow now, in anticipation of
the map canvas: rejected, because §5.2 does not draw it; the guard is where that
arrives, in the story that needs it. Deferring `README.md` to its own chore
story: rejected by the product owner (spec, Concern 2) — without it every release
fails at packaging, after a successful build.

**Rollback.** Everything here is additive, so the rollback is a plain
`git revert` of this story's squash-merge commit on `test` — and on `main` too if
it has already been promoted. Reverting removes `Cargo.toml`, which switches the
`hashFiles` guards in `ci.yml` back off and returns the `rust` job to a no-op;
the release workflow returns to its previous state, which was already unable to
build. There is no data, no migration and no external state to undo.

## Proof

### The four checks of `CLAUDE.md`

Run from the repository root, with the output pasted into the pull request.

```sh
cargo fmt --all --check                                     # no output, exit 0
cargo clippy --workspace --all-targets -- -D warnings       # "Finished", zero warnings
cargo test --workspace                                      # every suite: 0 failed
SHELLCHECK_OPTS=--exclude=SC2016 actionlint                 # no output, zero findings
```

`cargo test --workspace` runs five test binaries; the eight library crates
contribute none. Expected, all passing:

| Suite | Tests |
|---|---|
| `src/cli.rs` unit | 4: no arguments, `--version`, `--nope`, `--version --version` |
| `src/app.rs` unit | 2: title and app id; the wgpu renderer |
| `tests/workspace_structure.rs` | 7: the seven assertions listed in step 3 |
| `tests/command_line.rs` | 2: `--version`, `--nope` |
| `tests/empty_window.rs` | 1: one frame, no widget |

### The binary (requirements 1, 7, 8, 9)

```sh
cargo build --release                       # produces target/release/cellworks
./target/release/cellworks --version        # "cellworks 0.1.0"; echo $? -> 0
./target/release/cellworks --nope >/dev/null  # usage on stderr naming --version; echo $? -> 2
cargo metadata --format-version 1 --no-deps   # nine packages; root "cellworks", edition "2024", one bin target
```

Manual smoke, once, on macOS and recorded in the pull request (requirement 7):
`cargo run` opens exactly one window titled `Cellworks` with empty content, and
closing it exits 0. This is run by hand with the user's agreement, never from an
automated run — `CLAUDE.md` forbids opening app windows over the user's screen
unasked, and `docs/arc42/08-crosscutting.md` §8.6 forbids a test doing it.

### The guard bites (requirement 4)

```sh
# add  cellworks-workspace = { path = "../workspace" }  to crates/editing/Cargo.toml
cargo test --workspace
#   ---- dependency_sets_match_the_building_block_view stdout ----
#   assertion `left == right` failed: cellworks-editing
#     left: ["cellworks-workspace"]
#    right: []
git checkout crates/editing/Cargo.toml
```

The observed failure output is pasted into the pull request. The same test is the
proof for requirements 2, 3 and 13; the mutation is what shows it is a check
rather than a comment.

### No threads, no channels, no async runtime (requirement 10)

```sh
grep -rnE 'std::thread|mpsc|async|await|tokio' src/ crates/     # no output
cargo tree --workspace | grep -E '(tokio|async-std|smol) v'     # no output
```

A reviewer should expect `pollster` and the `futures-*` helper crates to appear
under `wgpu` in `cargo tree`; those are wgpu's own blocking helpers, not an async
runtime, and are not what requirement 10 excludes.

### No test opens a window (requirement 15)

```sh
grep -rn 'run_native' src/ crates/ tests/      # only src/app.rs
```

Together with a green `rust` job on `ubuntu-latest` — no display, no GPU — and a
green `cargo test --release --target x86_64-unknown-linux-gnu` in the release
workflow, that is the whole of requirement 15.

### The release path (requirements 14, 16)

Before merge, by hand, reproducing `.github/workflows/release.yml:66-73`:

```sh
cargo build --release
mkdir cellworks-v0.1.0-local
cp target/release/cellworks README.md cellworks-v0.1.0-local/
tar czf cellworks-v0.1.0-local.tar.gz cellworks-v0.1.0-local
tar tzf cellworks-v0.1.0-local.tar.gz    # the binary and README.md
```

After merge, the real proof: the first push to `main` runs `release` green end to
end — both matrix targets build, package and upload, and the published
`cellworks --version` matches the tag. That evidence arrives after this pull
request and is recorded on the promotion, not here; this is the one requirement
the story cannot fully close before merge.

### What a reviewer checks by eye

- Root `Cargo.toml`: exactly one line matching `^version = "`, and it is
  `[package]`'s second key. No `[workspace.package]`. No `[dependencies.<name>]`
  or `[workspace.dependencies.<name>]` section. Every `cellworks-*` dependency is
  path-only, with no version requirement.
- The nine dependency rows against `docs/arc42/05-building-blocks.md` §5.2, and
  that CodeAnalysis → Workspace, AgentSessions → Workspace and
  Sources → Navigation are absent — the binary is where they will be wired.
- Each `crates/<name>/src/lib.rs` names its own bounded context and the right
  document under `docs/domain/contexts/`, exposes nothing, and contains no
  Warhammer 40k word in an identifier. The only user-facing string in the story
  is the window title `Cellworks`, the product name.
- `CLAUDE.md` → *Verifying your work* and `.github/workflows/sdlc-build.yml:26`
  are character-identical and unchanged by this story (requirement 12).
- `git ls-files Cargo.lock` lists the file, and `.gitignore` still ignores
  `/target` only (requirement 6).
- `README.md` exists at the root and says what the product is and how to build
  it.
