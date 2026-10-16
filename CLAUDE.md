# Cellworks

A native desktop map that makes an engineer's projects, code, AI agent sessions, tickets, infrastructure and data navigable in one place.

## Tech stack

- Rust, edition 2024, stable toolchain; a Cargo workspace with one crate per bounded context
  ([ADR-0002](docs/adr/0002-cargo-workspace-crate-per-bounded-context.md))
- eframe/egui 0.36 with the wgpu backend ([ADR-0001](docs/adr/0001-egui-as-ui-framework.md))
- tree-sitter for code analysis ([ADR-0004](docs/adr/0004-tree-sitter-for-code-analysis.md))
- git, kubectl, nvim and curl as child processes ([ADR-0003](docs/adr/0003-external-tools-as-child-processes.md))
- No async runtime: background work on `std::thread`, results over `std::sync::mpsc`
- macOS first; the release workflow also builds Linux

## How to run things

```
cargo build --release                          # must stay warning-free
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo test --workspace                         # unit and acceptance tests
cargo test --workspace --release -- --ignored --nocapture   # measurement tests (chapter 10 scenarios)
```

Tests never open windows; the UI is driven headlessly. Do not open app windows over the user's screen without asking.

## Domain at a glance

Cellworks shows an engineer's projects (code, Jira, infrastructure, data) on one zoomable map, and their Claude Code
sessions as units moving across it. The people who use it are the code architect, the AI engineer / operator, the
ticket and ops persona, the data persona and an onboarding colleague. Contexts: **Workspace** (projects, settings,
rescan), **Map** (the source-agnostic graph of districts, groups, nodes, items and lanes, its layout and rendering),
**CodeAnalysis**, **DomainKnowledge**, **Sources** (Jira, infrastructure, data loaders), **AgentSessions**,
**Navigation** and **Editing**. Flow: Workspace identifies a project → a loader (CodeAnalysis, Sources,
DomainKnowledge) writes nodes and lanes into the Map → Navigation moves the selection through layers →
Editing opens a node on request, while AgentSessions place live sessions on the map. Neutral terms are canonical;
the Warhammer 40k words are display theme only. Full model: `docs/domain/`.

## Where things live

- `docs/domain/` — bounded contexts, glossary, context map
- `docs/arc42/` — architecture documentation
- `docs/adr/` — architecture decision records
- `docs/services/<component>/` — per-component docs; start at its `README.md` (purpose + index
  of that component's documents)
- `docs/discovery/` — discovery evidence: the prototype findings and spikes
- `Cargo.toml` — the `cellworks` binary package and the workspace; context crates in `crates/<name>/`

## Foreign systems

Detail in `docs/arc42/03-context.md`.

- Claude Code — sessions observed through transcripts in `~/.claude/projects`; how sessions are driven is still open
- git — status, numstat, branches, worktrees, via the CLI
- Neovim — embedded with `nvim --embed`
- Atlassian remote MCP — Jira sync over HTTPS, Cellworks as its own OAuth client
- Kubernetes — read-only `kubectl get`
- Terraform — files read from infrastructure repositories

## Cross-context communication

- In-process only. A downstream crate calls the upstream crate's public API; crate dependencies follow the
  context map, and the binary wires the flows that would otherwise form a cycle (`docs/arc42/05-building-blocks.md`).
- Background results reach the UI thread as `<Context>Event` enums with past-tense variants (e.g.
  `SessionEvent::FileWritten`), sent over `std::sync::mpsc` and drained once per frame.
- Identifiers use the glossary's neutral terms (District, Group, Node, Item, Lane, Session). The Warhammer 40k
  words are display theme only and appear only in UI strings.

## Working with the Factory

Implementation runs through The Factory v3.0.0 (`squer-solutions/The-Factory`, pinned in `.github/workflows/` and
`.claude/settings.json`). Design stays in the jarvis docs (`docs/`). Run `/factory:guide` before your first change here;
`/factory:setup` checks the admin setup; `/factory:learn` says where a lesson belongs.

The loop:

1. **Intake** — author the story with `gherkin-story-authoring` (behavior) or `chore-story` (everything else).
2. **Prototype** on `proto/<slug>` in a throwaway worktree and iterate until the behavior is confirmed.
3. **`/factory:freeze`** — distills the session into `prototype.md` and `design.md`, settles the planner's questions
   locally, pushes `proto/<slug>` and labels the issue `sdlc:intent`. Keep the branch until the merge.
4. **Hands-off** — `sdlc.yml` runs one resumable pipeline (build, checks, deliver, review) to an auto-merge into `test`.
   Progress is the bot's state comment on the issue; a binary per pull request lands on the release `pr-N`, the rolling
   `test` prerelease and `vX.Y.Z` on `main` (`.github/deliverables/`). Questions become assumptions, listed on the issue
   (label `sdlc:assumptions`) and in the pull request under "Decisions made without you".
5. **`/factory:answer <issue>`** — correct an assumption (or reply `Q-n: <answer>` on the issue); the pipeline rebuilds
   with it, or opens a follow-up issue after the merge.
6. **`/feedback <text>`** — anything off-topic found while testing goes to the `feedback` inbox and the session returns to
   its story; `/feedback triage` drops, folds or promotes the findings.

Prototyping and freezing run locally and need the Jarvis plugins (`more-jarvis` marketplace: jarvis-planning,
jarvis-tooling, jarvis-execute, jarvis-architecture) installed on your machine, because CI cannot read the private
`more-jarvis` repository yet: `/plugin marketplace add squer-solutions/more-jarvis`, then enable them.

`test` is promoted to `main` by a person; every push to `main` is released.

- Artifacts live in `sdlc/<issue-number>-<slug>/`. The directory name carries the issue number — the automation
  parses it, so do not rename directories.
- Never edit an `intent.md` or `spec.md` from a build session. Those are the approved upstream record; if the plan
  needs them changed, say so and stop.
- When implementation departs from `plan.md`, update `plan.md` in the same commit.
- The SDLC skills, hooks and agents come from the `factory` plugin. Change them in The-Factory, not here.
- Do not run cargo checks or watch CI in the main thread: spawn the `check-runner` agent for fmt/clippy/test and the
  `ci-watcher` agent for pull request checks; both return a digest. Subagents run on sonnet
  (`CLAUDE_CODE_SUBAGENT_MODEL`), these two on haiku.

## Verifying your work

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
SHELLCHECK_OPTS=--exclude=SC2016 actionlint   # zero findings
```

Run all of them before reporting any task complete, and paste the output. If a check fails, fix the code, not the
check. Keep the cargo commands the same as `verify-command` in `.github/workflows/sdlc.yml`.

## Things Claude gets wrong

The rule: **when Claude makes the same mistake twice, the correction goes here.** Nothing recorded yet.

## Per-component conventions

Each component's docs live in its own folder, fronted by a `README.md` that states the component's
purpose and indexes the folder's documents. Before working on a story, read the `README.md` of the
component(s) it touches and open only the documents its index points you at:
- `docs/services/cellworks/README.md` — the desktop app: all crates of the workspace

## Conventions

- **Every source code change must be motivated by a story.** Use `gherkin-story-authoring` for
  behavioral changes (new features, rule changes) and `chore-story` for non-behavioral changes
  (layout tweaks, refactorings, dependency updates, tooling).
- Story IDs: `STORY-NNN` / Epic IDs: `EPIC-NNN` / ADR IDs: `NNNN`
- Branch names: `feature/NNNN-slug`, created by The Factory from the issue number
- Commit messages: Conventional Commits with `Refs: STORY-NNN` in footer
- One pull request per story, from its feature branch into `test`, squash on merge
- Default branch: `main`
