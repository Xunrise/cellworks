---
stage: spec
issue: 3
status: draft
author: claude (sdlc-spec)
date: 2026-10-08
---

# Spec: Cargo workspace with the context crates and an empty window

Derived from `intent.md` in this directory. Answers, or explicitly carries
forward, every open question it raised.

The intent's **Open questions** section says "None." — there is nothing to
answer or carry forward from it. Two questions this stage raised itself are in
[Concerns](#concerns); one of them blocked the stage until the product owner
resolved it.

The intent was read as data. It contains no text addressed to the design stage
and no instruction to change behaviour or skip policy.

**Policies applied:** `CLAUDE.md`; `docs/services/cellworks/guidelines/`
(`architecture.md`, `naming.md`, `testing.md`); `docs/arc42/02-constraints.md`,
`04-solution-strategy.md`, `05-building-blocks.md`, `08-crosscutting.md`;
`docs/adr/0001`, `docs/adr/0002`; `docs/domain/context-map.md`. The one product
policy skill, `.claude/skills/secure-api-review`, does not apply: its trigger is
an external-facing endpoint or request/response schema, and this story adds
none — Cellworks runs no network server (`docs/arc42/02-constraints.md` §2.1,
`08-crosscutting.md` §8.4). The business context
(`factory-context/05-case/AGENTS.md`) routes back to the jarvis docs above.

## Requirements

Each is independently verifiable; the proof for each is in
[Verification](#verification).

### The workspace

1. **(must)** The repository root holds a `Cargo.toml` that is both the
   `cellworks` binary package and the workspace declaration, on Rust edition
   2024, building a binary named `cellworks`.
2. **(must)** Eight library crates exist, one per bounded context: directory
   `crates/<name>/`, package `cellworks-<name>`, for `workspace`, `map`,
   `navigation`, `code-analysis`, `domain-knowledge`, `sources`,
   `agent-sessions`, `editing`. No other member crates exist.
3. **(must)** The `cellworks-*` dependencies declared by each manifest are
   exactly the arrows of `docs/arc42/05-building-blocks.md` §5.2, as tabulated
   in [Design](#the-dependency-graph). No crate depends on a crate downstream of
   it, and the graph has no cycle.
4. **(must)** A test in the repository fails if the declared `cellworks-*`
   dependency graph stops matching that table — the graph is pinned by a check,
   not by review attention.
5. **(should)** Versions of third-party dependencies shared by more than one
   crate are declared once in `[workspace.dependencies]` and referenced with
   `workspace = true`; each member crate sets `publish = false`.
6. **(should)** The root manifest records the minimum supported Rust version
   (`rust-version`), and `Cargo.lock` is committed.

### The binary

7. **(must)** Run with no arguments, `cellworks` opens exactly one window,
   titled `Cellworks`, rendered by eframe/egui 0.36 on the **wgpu** backend
   (ADR-0001), whose content is empty — no widgets, no text.
8. **(must)** Run as `cellworks --version`, it writes `cellworks <version>` and
   a newline to stdout, exits 0, and opens no window. `<version>` is the
   `version` of the root `Cargo.toml` as compiled in, so that the version the
   release workflow stamps is the version the binary reports.
9. **(must)** Given any argument it does not recognise, it writes a usage line
   naming the arguments it accepts to **stderr**, exits with code 2, and opens
   no window. It does not silently fall through to opening a window.
10. **(must)** No background thread, channel, `<Context>Event` enum or async
    runtime is introduced by this story; there is nothing yet to report. The
    conventions for them (`std::thread`, `std::sync::mpsc`, past-tense
    `<Context>Event` variants) bind the first story that needs one.

### The checks and the workflows

11. **(must)** All four commands of `CLAUDE.md` → *Verifying your work* succeed
    with zero warnings and zero findings, on both macOS and Linux:
    `cargo fmt --all --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo test --workspace`, and
    `SHELLCHECK_OPTS=--exclude=SC2016 actionlint`.
12. **(must)** The cargo commands stay character-identical to the
    `verify-command` input in `.github/workflows/sdlc-build.yml:26`. If one
    changes, both change in the same commit.
13. **(must)** The root `Cargo.toml` contains **exactly one** line matching
    `^version = "`, and it is the `cellworks` package version. The release
    workflow reads the version with `sed -n 's/^version = "\(.*\)"$/\1/p' | head -1`
    and rewrites it with a non-global `perl -0pi -e 's/^version = ".*?"/.../m'`
    (`.github/workflows/release.yml:28,62`); both act on the first such line, so
    a second one would silently stamp the wrong field.
14. **(must)** `cargo build --release --target <target>` and the `Package` step
    of `.github/workflows/release.yml` both succeed for
    `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`. The Package step
    copies `README.md` from the repository root
    (`.github/workflows/release.yml:71`) and the root has no `README.md` today,
    so this story adds one (see [Concerns](#concerns), item 2).
15. **(must)** `cargo test --release --target x86_64-unknown-linux-gnu`
    (`.github/workflows/release.yml:64`) and `cargo test --workspace` pass on a
    headless Linux runner: no test opens a window, and no test requires a GPU
    adapter or a display server.
16. **(must)** Whatever system development packages the Linux build of
    eframe/wgpu needs beyond the stock `ubuntu-latest` image are installed by
    `.github/workflows/ci.yml` and `.github/workflows/release.yml` before the
    cargo steps. If the stock image already provides them, no step is added —
    the requirement is that the Linux jobs go green, not that a step exists.

## Design

### Layout

```
Cargo.toml              # [package] cellworks + [workspace]
Cargo.lock              # committed
README.md               # new; the release Package step copies it
src/
  main.rs               # entry point: parse, dispatch, nothing else
  cli.rs                # Command + parse_args  (pure, unit-tested)
  app.rs                # CellworksApp + native_options  (headless-testable)
tests/
  workspace_structure.rs  # the dependency-graph and manifest guards
crates/
  workspace/        Cargo.toml  src/lib.rs
  map/              Cargo.toml  src/lib.rs
  navigation/       Cargo.toml  src/lib.rs
  code-analysis/    Cargo.toml  src/lib.rs
  domain-knowledge/ Cargo.toml  src/lib.rs
  sources/          Cargo.toml  src/lib.rs
  agent-sessions/   Cargo.toml  src/lib.rs
  editing/          Cargo.toml  src/lib.rs
```

Each `crates/<name>/src/lib.rs` is empty but for a `//!` doc comment naming the
bounded context it owns and linking its domain document under
`docs/domain/contexts/`. Nothing is `pub` yet: `lib.rs` exposes a small API
(`guidelines/architecture.md`), and this story has no behaviour to expose. The
crates are seams, not placeholders for code to be dumped into.

Naming follows `guidelines/naming.md`: directory `crates/<name>/`, package
`cellworks-<name>`, imported as `cellworks_<name>`. No Warhammer 40k word
appears in an identifier; the only user-facing string in this story is the
window title `Cellworks`, which is the product name.

### The dependency graph

From `docs/arc42/05-building-blocks.md` §5.2. This table is the contract that
requirement 4's guard pins.

| Crate | `cellworks-*` dependencies | Third-party |
|---|---|---|
| `cellworks` (bin) | `navigation`, `agent-sessions`, `sources`, `domain-knowledge`, `code-analysis`, `workspace` | `eframe`, `egui` |
| `cellworks-navigation` | `map`, `editing` | — |
| `cellworks-code-analysis` | `map` | — |
| `cellworks-domain-knowledge` | `map`, `code-analysis` | — |
| `cellworks-sources` | `map`, `workspace` | — |
| `cellworks-agent-sessions` | `map` | — |
| `cellworks-map` | `workspace` | — |
| `cellworks-workspace` | — | — |
| `cellworks-editing` | — | — |

Three context-map relationships are deliberately **not** crate dependencies:
CodeAnalysis → Workspace and AgentSessions → Workspace (the binary wires them,
because as crate dependencies they would close a cycle through
`map → workspace`), and Sources → Navigation. The guard must therefore assert
the dependency set **exactly** — a missing arrow and an extra arrow are both
failures.

`cellworks-code-analysis` is a direct dependency of the binary: the binary
registers the code loader and wires its worktree events into `workspace`. The
product owner settled this on 2026-10-08 ([Concerns](#concerns), item 1), and
`docs/arc42/05-building-blocks.md` §5.2 now draws the arrow.

Inter-crate dependencies are declared **by path only**
(`cellworks-map = { path = "../map" }`), with no version requirement: the member
crates are never published, and a version requirement would be a second place
the release version could drift.

### The root manifest and the release workflow

The release workflow treats the root `Cargo.toml` as a line-oriented file. Two
things follow, and both are structural rather than remembered:

- `[package]` comes first in the file, with `version` as its second key, so the
  first `^version = "` line is the binary's.
- There is no `[workspace.package] version`, and no dependency is written in
  `[dependencies.<name>]` table form (which would put a bare `version = "…"` at
  the start of a line). Member crates carry their own `version = "0.0.0"` in
  their own manifests, where the workflow never looks.

A test (`tests/workspace_structure.rs`) re-applies the workflow's own extraction
to the root manifest and asserts the result equals `env!("CARGO_PKG_VERSION")`.
That is what makes requirement 13 hold by construction: a future edit that
introduces a second `^version = "` line fails `cargo test`, not the next
release.

`[workspace]` sets `resolver = "3"` explicitly, so the resolver never depends on
how the root package happens to be declared and no resolver warning can appear
under requirement 11.

### The binary

`src/cli.rs` is pure and takes the arguments as an iterator, so it is testable
without a process or a window:

```rust
pub enum Command { Run, Version }
pub enum CliError { Unknown(String) }
pub fn parse_args(args: impl Iterator<Item = String>) -> Result<Command, CliError>
```

States and their outcomes:

| Input | Command | Effect | Exit |
|---|---|---|---|
| no arguments | `Run` | opens the window | 0 on clean close |
| `--version` | `Version` | `cellworks <version>` to stdout | 0 |
| anything else | `Err(Unknown)` | usage line to stderr | 2 |

`main.rs` dispatches and does nothing else. The version string is
`env!("CARGO_PKG_VERSION")` — the compiled-in value, which is exactly what the
release workflow stamped, so requirement 8 cannot drift from requirement 13.
Argument parsing is hand-written: `guidelines/architecture.md` says a dependency
is added only when a few lines cannot do the job, and three arguments are a few
lines.

`src/app.rs` holds:

```rust
pub struct CellworksApp;            // no state yet
impl eframe::App for CellworksApp   // one empty egui::CentralPanel per frame
pub fn native_options() -> eframe::NativeOptions   // title + renderer, pure
pub fn run() -> eframe::Result                     // eframe::run_native
```

`native_options()` is a separate pure function precisely so the window title and
the renderer can be asserted in a unit test without opening anything: it sets
`viewport.title = "Cellworks"` (and `app_id = "cellworks"`, which Wayland needs
to associate the window) and `renderer = eframe::Renderer::Wgpu`.

The empty `CentralPanel` is deliberate: an eframe app that draws nothing at all
still shows a window, but it leaves no surface for the next story to attach to.
One empty panel is the seam the map canvas replaces.

**Failure modes.** `eframe::run_native` returns `Err` when no wgpu adapter or no
display is available — a headless machine, a broken driver, a remote shell.
`main` writes that error to stderr and exits non-zero rather than panicking; a
process that fails to start says why (`factory:standards-change`: fail loudly).
This is also why no test calls `run()`.

**Dependencies.** eframe 0.36's default features include `glow`, which ADR-0001
does not choose, so the dependency is
`eframe = { version = "0.36", default-features = false, features = ["accesskit", "default_fonts", "wgpu", "wayland", "x11"] }`,
with `egui = "0.36"` beside it. `wayland` and `x11` are what makes the Linux
target of the release matrix work; dropping either narrows the platforms
silently.

### Testing

Per `docs/arc42/08-crosscutting.md` §8.6 and
`docs/services/cellworks/guidelines/testing.md`. This story has no Gherkin
scenario to name a test after (see [Concerns](#concerns), item 3), so acceptance
tests are named after the behaviour they pin.

- **Unit**, in `src/cli.rs` and `src/app.rs`: `parse_args` for each of the three
  states; `native_options()` for title and renderer.
- **Acceptance**, in `tests/`: the structural guards (requirements 3, 4, 13, 2)
  read the manifests from disk and compare against the table above. They are at
  the root because they span crates.
- **Acceptance**, the empty UI: an `egui_kittest` harness runs one frame of
  `CellworksApp` and asserts the frame contains no widget. The harness is used
  **without** snapshot rendering, so it needs no GPU and no display — which is
  what lets requirement 15 hold on the headless Linux runner.
- **Smoke**: `cellworks` opened by hand on macOS, once, to confirm a real window
  appears with the right title. It is not part of any automated run, and no
  automated test opens a window.

The manifest guards parse TOML with a `toml` dev-dependency rather than by hand:
reading `[dependencies]` keys correctly out of raw text is more code and more
failure modes than the dependency costs.

### Continuous integration

`.github/workflows/ci.yml`'s `rust` job is already written and guarded by
`hashFiles('Cargo.toml') != ''`. This story is what makes those steps execute
for the first time; the job itself needs no change beyond requirement 16.
`.github/workflows/sdlc-build.yml`'s `verify-command` likewise starts doing real
work and must not be edited (requirement 12).

## Affected interfaces

- **New: the command line of `cellworks`.** `cellworks` and `cellworks --version`
  become a public contract — the smoke-mode surface
  `docs/arc42/05-building-blocks.md` §5.2 reserves for the binary. Later stories
  add modes; they must not change the meaning of these two or of exit code 2.
- **New: the root `Cargo.toml` as a release contract.** Its first
  `^version = "` line is read and rewritten by
  `.github/workflows/release.yml:28,62`. Any later manifest edit is bound by
  requirement 13.
- **New: nine crate manifests**, whose `cellworks-*` dependency sets are pinned
  by the guard in requirement 4. A later story that needs a new arrow changes
  `docs/arc42/05-building-blocks.md` §5.2 and the guard in the same commit, or
  the build fails — which is the intended way for architecture drift to surface.
- **New: nine library public APIs**, all empty. Downstream stories grow them.
- **New: `README.md` at the repository root**, consumed by the release
  `Package` step.
- **Changed: `.github/workflows/ci.yml` and `.github/workflows/release.yml`**,
  only if requirement 16 needs a system-package step. No existing step's
  semantics change.
- **Unchanged, but newly load-bearing:** `.github/workflows/ci.yml`'s
  `hashFiles('Cargo.toml')` conditions and `sdlc-build.yml`'s `verify-command`
  begin running cargo on every pull request.
- **Nothing breaks.** There is no existing Rust code, so no existing contract
  can be broken by this story.

## Concerns

1. **Resolved by the product owner, 2026-10-08.** The binary depends on
   `code-analysis` directly; the §5.2 diagram was missing the arrow and now
   has it. The dependency table above and the requirement-4 guard include it.
   The original question, kept for the record: `docs/arc42/05-building-blocks.md` §5.2
   contradicts itself about whether the binary depends on `code-analysis`, and
   the intent's Constraints adopt both halves. The mermaid diagram draws
   `bin --> nav`, `ag`, `src`, `dk`, `ws` and **no** `bin --> code`; the prose
   immediately below it says "the binary wires them: it takes the events from
   `code-analysis` and `agent-sessions` and calls `workspace`", which in Rust
   requires the binary to name `cellworks_code_analysis` types and therefore to
   declare a direct dependency on it. The two readings produce different root
   manifests and a different pinned graph in the requirement-4 guard, so the
   choice cannot be deferred to the story that first wires CodeAnalysis. I am
   not authorised to pick one.
   The stage's recommendation, which the owner rejected: follow the diagram now
   (no `bin → code-analysis`), because this story wires nothing, and add the
   arrow to the diagram, the table here and the guard together in the story that
   first consumes a CodeAnalysis event. The same question will recur for
   `bin → map` once the binary paints the map; the guard is where that gets
   recorded.

2. **Resolved during drafting, and accepted by the product owner on
   2026-10-08: requirement 14 stays in this story.** The
   repository root has no `README.md`, and
   `.github/workflows/release.yml:71` runs
   `cp "target/.../cellworks" README.md "$name/"`. Without it, every release
   fails at packaging on both targets, after a successful build — so the
   intent's outcome ("succeeds … in the release workflow, on both Linux and
   macOS") is not reachable without adding one. I made it requirement 14 rather
   than a blocker because the fix is a file the repository should have anyway
   and no policy argues against it. If the product owner would rather keep this
   story to the workspace, drop requirement 14 into its own chore story and
   accept that releases stay broken until it lands.

3. **Accepted by the product owner on 2026-10-08: tests are named after the
   behaviour.** The intent is framed as non-behavioural ("No persona sees a behavioural
   change from this story"), yet requirements 7–9 are user-observable behaviour:
   a window and a command-line contract. `docs/arc42/02-constraints.md` §2.2 and
   `CLAUDE.md` route behavioural changes through Gherkin stories and everything
   else through chore stories, so by the letter this story is on the boundary.
   The practical effect is only on test naming: `guidelines/testing.md` names
   acceptance tests after their scenario, and there is no scenario, so they are
   named after the behaviour instead. I have not treated this as a blocker
   because the design does not change either way. If the product owner wants
   scenarios, they belong in `intent.md`, which a build session may not edit.

4. Requirement 16 is stated as an outcome ("the Linux jobs go green") rather
   than as a named list of apt packages, because which development packages
   `ubuntu-latest` already carries for an eframe/wgpu build is a fact about the
   runner image on the day, not something this spec can pin. The plan stage
   should expect one iteration of the Linux job here, and should not treat a
   first red run as a design error.

5. `.claude/skills/secure-api-review` was read and found not to apply: this
   story adds no endpoint, no request or response schema and no external-facing
   surface. No secret, token or credential is read, written or logged by
   anything in it, so `docs/arc42/08-crosscutting.md` §8.4 imposes nothing
   further.

## Verification

| Req | How it is proven |
|---|---|
| 1 | `cargo metadata` shows the root package `cellworks` on edition 2024 with one `bin` target named `cellworks`; `cargo build --release` produces `target/release/cellworks`. |
| 2 | Test in `tests/workspace_structure.rs`: the set of workspace members equals exactly the nine expected packages, and each `cellworks-<name>` lives at `crates/<name>/`. Fails on a missing, extra or misplaced crate. |
| 3 | Test in `tests/workspace_structure.rs`: for each manifest, the set of dependency keys beginning `cellworks-` equals the row of the [dependency table](#the-dependency-graph), asserted with `assert_eq!` on sorted sets so both a missing and an extra arrow fail. A second assertion topologically sorts the graph and fails if it cannot. |
| 4 | Mutate one manifest (add `cellworks-workspace` to `crates/editing/Cargo.toml`), confirm `cargo test --workspace` fails, revert. Record the observed failure in the plan's proof. |
| 5 | Inspect the manifests; `[workspace.dependencies]` holds every version used by more than one crate, each member sets `publish = false`. Checked in review, not by a test. |
| 6 | `rust-version` present in the root manifest and `cargo build` succeeds on that toolchain; `git ls-files Cargo.lock` lists the file. |
| 7 | Unit test on `native_options()`: `viewport.title == Some("Cellworks")` and `renderer == Renderer::Wgpu`. `egui_kittest` acceptance test: one frame of `CellworksApp`, assert the frame contains no widget. Manual smoke on macOS, once, recorded in the pull request — no automated test opens a window. |
| 8 | Unit test: `parse_args(["--version"])` yields `Command::Version`. Acceptance test: run the built binary with `std::process::Command`, assert stdout is `cellworks {}\n` formatted with `env!("CARGO_PKG_VERSION")` and the exit code is 0. |
| 9 | Unit test: `parse_args(["--nope"])` yields `Err(Unknown("--nope"))`. Acceptance test: run the binary with `--nope`, assert exit code 2, empty stdout, and stderr containing `--version`. |
| 10 | `grep -r` over `src/` and `crates/` finds no `std::thread`, no `mpsc`, no `async`, no `tokio`; the dependency tree of `cargo tree` contains no async runtime. |
| 11 | All four commands run locally and in CI, output pasted into the pull request. Zero warnings, zero findings. |
| 12 | Compare the cargo commands in `CLAUDE.md` → *Verifying your work* with `.github/workflows/sdlc-build.yml:26` character for character; both unchanged by this story. |
| 13 | Test in `tests/workspace_structure.rs`: `include_str!("../Cargo.toml")` has exactly one line matching `^version = "`, and the value extracted from the first such line equals `env!("CARGO_PKG_VERSION")`. |
| 14 | The release workflow runs green end to end on the first push to `main` after this story: both matrix targets build, package and upload. Before that, `cargo build --release` locally on macOS, and the `Package` step's `cp` reproduced by hand against the repository root. |
| 15 | `.github/workflows/ci.yml`'s `rust` job (ubuntu, no display) passes `cargo test --workspace`; the release workflow's `cargo test --release --target x86_64-unknown-linux-gnu` passes. No test references `run_native`, a GPU adapter or a snapshot renderer — checked by grep and by the CI result together. |
| 16 | The Linux jobs of both workflows are green. If a system-package step was added, `actionlint` stays at zero findings with it. |
