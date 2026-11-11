# Prototype: STORY-058 TypeScript and JavaScript code stands as towers

Issue #83, epic EPIC-021. Branch `proto/ts-towers`.

## Confirmed behavior

- Files ending in `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` and `.cjs` are read with tree-sitter; `.ts`, `.mts` and `.cts` with the TypeScript grammar, the others with the TSX grammar.
- A top-level function declaration (plain, generator, exported or not) stands as a function tower named after the function.
- A `const`/`let`/`var` whose value is an arrow function or function expression stands as a function tower named after the variable.
- Any other top-level `const`/`let`/`var` stands as a constant tower, one per declarator.
- A class stands as a type tower holding its own lines down to its first method; each method stands as its own tower named `Class::method`, part of the class.
- Interfaces, type aliases and enums stand as type towers.
- Adjacent `import` statements form one tower named `imports`.
- An exported declaration is public; everything else is not.
- In a file named `*.test.*` or `*.spec.*` every tower stands in the cellar, its imports included.
- `describe("…", () => { … })` is not a tower itself; its body is read and each `it("label", …)` or `test("label", …)` inside stands as a test tower in the cellar, named by its label. Nested `describe` blocks are read the same way.
- A file whose syntax tree has any error stands as one tower with one floor per line, as any file no parser reads.
- Each tower has one floor per line with its tokens, as Rust towers do; no calls and no import lanes yet (STORY-059 and STORY-060).
- On the real project `squantine` (206 TypeScript files, Angular and React) the folders lens shows the TypeScript districts as cities of towers: 585 functions, 296 types, 188 import blocks, 720 tests, one file falling back to plain lines.

## Feedback log

- "Yes, prototype STORY-058 on proto/ts-towers off test." → prototype written in `crates/code-analysis/src/ts.rs` and wired into `load`.
- "Yes, open the app from the proto worktree on a real TypeScript project and show me." → added a headless render of `~/Projects/squantine` to a PNG, and walked nested groups in the demo to count every file.
- "Behavior confirmed. Fix nothing more in the proto; run /factory:freeze for STORY-058." → freeze.

## Rejected

- Another epic (EPIC-013 code health, EPIC-011 history, EPIC-014 agent sessions) as the parallel Factory trial: they touch the map crate the in-flight epics change; EPIC-021 stays in code-analysis.
- Showing the live app window by screenshot: the host has no screen-recording permission; the headless kittest render shows the same frame instead.

## Edge cases

- A test file's import block stands in the cellar with its tests.
- A file that holds only re-exports (`export { X } from './x'`) or only a triple-slash reference (`vite-env.d.ts`) yields no towers at all; accepted as is.
- A file with a syntax error anywhere (seen: a 1228-line Angular spec) falls back to one tower of its lines rather than keeping the parts that parsed.
- Two declarations of one name in a file get their first line appended to the path (`name@12`), as in Rust.
- `.js`, `.mjs` and `.cjs` files are parsed with the TSX grammar (decided at freeze; the prototype used the TypeScript grammar), so JavaScript holding markup still stands as towers.
- Focused, skipped and table-driven tests (`it.only`, `it.skip`, `test.each`, `describe.each` and the like) count as tests and suites (decided at freeze).
- Calls such as `it(` or `test(` outside a `describe`, at top level, are test towers too.

## Decisions

- Q: Which epic runs alongside the in-flight ones? → A: EPIC-021, starting with STORY-058.
- Q: Which grammar reads `.js`, `.mjs` and `.cjs`, given React code often holds markup in plain `.js`? → A: The TSX grammar.
- Q: Do `it.only`, `it.skip`, `test.each`, `describe.each` and the like count as tests and suites? → A: Yes, recognise them.
- Q: Is the behaviour shown on the sample project and on squantine the one wanted, including the edge cases above? → A: Yes, "Behavior confirmed. Fix nothing more in the proto".

## Evidence

- Branch `proto/ts-towers`, commits `b967fff` and `12e6d62`.
- Files: `crates/code-analysis/src/ts.rs` (new), `crates/code-analysis/src/lib.rs` (calls `ts::read` after `rust::read`), `crates/code-analysis/src/rust/mod.rs` and `rust/parse.rs` (expose `FileCode` and `leaves`), `crates/code-analysis/Cargo.toml` (`tree-sitter-typescript = "0.23"`), `crates/code-analysis/tests/proto_ts_towers.rs` (prints a project's towers), `tests/proto_render.rs` and root `Cargo.toml` (kittest `wgpu` and `snapshot` features, renders a real project to PNG).
- Run: `cargo test -p cellworks-code-analysis --test proto_ts_towers -- --nocapture` on the story's sample project; `PROTO_ROOT=~/Projects/squantine` for the real one; `PROTO_ROOT=~/Projects/squantine PROTO_OUT=squantine.png cargo test --release --test proto_render` for the rendered folders lens.
