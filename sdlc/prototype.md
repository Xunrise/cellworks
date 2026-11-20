# Prototype: Freeze a prototype and review the change (STORY-087)

## Confirmed behavior

- FREEZE (stage 3) shows what the rounds confirmed (each closed round with its note and files), `.cellworks/feedback.log` numbered line by line, and what happens next.
- Its action FREEZE marks the case prototyped and starts PLAN in the prototype's own session (typed into its pane): on a Factory project `/factory:freeze`; with jarvis `/jarvis-execute:plan-story` from the prototype; otherwise the ticket-plan skill, told to plan from what the prototype changed (`git diff <base>`) and the rounds in `feedback.log`, write the steps into the case file, set it planned and commit nothing. The case moves to PLAN.
- The hand-off panel names what follows: for the Factory PROTOTYPE.MD, PLAN, BUILD; otherwise PLAN, APPROVE, REVIEW.
- REVIEW (stage 5) shows the worktree's plan view with every file changed since the merge base of the base branch (committed or not) marked, a column with the commits since the base and the changed files (each opens the code panel with its diff to the base), and REVIEWED, which sets the case reviewed. The plan/city swap works here too.
- The REVIEW act waits until the change is built and says who builds it ("the session builds it" or "the pipeline builds it").

## Feedback log

- "also i think it should be other way around: it should be ingest -> prototype -> plan -> approve -> review" → freezing the prototype now leads to the plan, and review closes the case.
- "1 PROBLEM, 2 ITERATE, 3 FREEZE, 4 PLAN, 5 REVIEW" → Freeze stays a stage of its own before the plan.

## Rejected

- FINISH as the iterate loop's last step (the session commits and closes the case): the plan, approval and build come after freezing.
- Folding freeze into the plan stage: the operator kept Freeze as a stage.
- Starting a new Terminal session to plan: planning runs in the prototype's session, which holds the context.

## Edge cases

- Freezing while the case is still prototyping: the case is first set prototyped, then PLAN runs.
- A project without a base branch in the case file: `main` is assumed.
- No closed rounds yet: the confirmed list is empty and the feedback log shows whatever was sent.

## Decisions

- Q: Keep Freeze as a stage? → A: Yes, 3 FREEZE before 4 PLAN.
- Q: What does freezing do? → A: It marks the prototype frozen and has the prototype's session plan from it (/factory:freeze on Factory projects).
- Q: Is the case's REVIEW the Sources "Review" (a pull request opened in the city) or a distinct term? → A: The same concept. Review becomes a change judged against its base, a pull request at its head or a case's worktree, and the case's REVIEW reuses its look: size, critical-change flags, added and removed lines on towers and floors.
- Q: Does REVIEW wait until the change is built? → A: No. REVIEW opens right away and is where the engineer watches the change being built live: the changes since the base as they land and the sessions building it on the map; it says who builds it. This replaces the confirmed "REVIEW waits until built".
- Q: On a Factory project, what does REVIEW show while the pipeline builds in CI? → A: Cellworks fetches the pipeline's feature branch periodically and shows its change against the base as it lands.
- Q: When can REVIEWED be pressed? → A: Any time, also while the change is still being built.
- Q: Does freezing on a Jarvis project use `/jarvis-execute:plan-story`? → A: Yes, it is in this story; the Gherkin gains the scenario "Freezing on a Jarvis project plans with Jarvis".

## Evidence

- Prototype branch: `proto/iterate-loop` (this branch carries the same commits), worktree `~/Projects/cellworks-proto-iterate-loop`, based on `test` at 79dbc40.
- The ticket case itself was prototyped first on `proto/ticket-lens` (an old base) and ported onto `test` here (commit c437299).
- Files the prototype touched: `crates/map/src/ui/case.rs`, `crates/map/src/ui/casefile.rs`, `crates/map/src/ui/jira.rs`, `crates/map/src/ui/ticket.rs`, `crates/map/src/ui.rs`, `crates/navigation/src/lib.rs`, `crates/agent-sessions/src/ui.rs`, `src/app.rs`, `src/iterate.rs`, `src/iterate_view.rs`, `assets/fonts/`, `.claude/skills/ticket-{case,ingest,plan,fix}/`, `.claude/skills/ticket-case/example-DEMO-1.json`.
- Run: `cargo build --release`, then `CELLWORKS_CASES=<dir with DEMO-1.json> CELLWORKS_JIRA_OFFLINE=1 CELLWORKS_OPEN_CASE=DEMO-1 CELLWORKS_DRIVE=<file> ./target/release/cellworks`. `CELLWORKS_DRIVE` is a prototype-only command file (open, case-go, case-prototype, pin, type, send, file, swap, dock, fold, shot) used to drive and screenshot the real window.
- Target under work: a tiny Vite 5 + TypeScript app at `~/Projects/vite-demo` with `.claude/launch.json` (port 5173); the case DEMO-1 is `.claude/skills/ticket-case/example-DEMO-1.json`.
- Design: Claude Design canvas "Iterate loop" (https://claude.ai/artifact/X92NEQejPnCAvGGSe6WAib) on the "Cellworks Netrunner" design system v4 (https://claude.ai/artifact/FQBGWeM9B6HhfaeTzwBMrT), which now carries the case-flow components (CaseBar, StagePanel, PrimaryAction, StepCard, DiffView, RiskCard, SegmentToggle, MapFocus); the ticket-case look comes from the "Ticket investigation flow" canvas (https://claude.ai/artifact/57gBHL52458ZhCeocYocwJ).
