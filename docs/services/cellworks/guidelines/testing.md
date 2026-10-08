# Testing

The test levels are defined in `docs/arc42/08-crosscutting.md`. This file says how they are applied here.

| Level | Where | Command |
|---|---|---|
| Unit | `#[cfg(test)] mod tests` beside the code, in each crate | `cargo test --workspace` |
| Acceptance | `tests/` of the crate that owns the behaviour; `tests/` at the root for scenarios across crates | `cargo test --workspace` |
| Measurement | `#[ignore]` tests next to the acceptance tests of the crate measured | `cargo test --workspace --release -- --ignored --nocapture` |
| Smoke | command-line modes of the binary (`cellworks --<mode>`) | run by hand |

## Acceptance tests

- One test per Gherkin scenario, named after the scenario in `snake_case`, in a file named after the story
  (`tests/story_012_enter_and_esc.rs`). The scenario title is repeated in a doc comment on the test.
- Tests drive the app through input with `egui_kittest` and assert on what is visible: the selected node, the layer,
  the open folder view, which part has the keyboard. They do not assert on private fields.
- No test opens a window.

## Fixtures

- Fixture projects and recorded tool output (git, kubectl JSON, MCP responses, transcripts) live in
  `tests/fixtures/` of the crate that uses them.
- Graphs for map and navigation tests are built with small builder functions in the test file (`a_project()`,
  `a_group()`, `a_node()`); move them into a shared test module only once a second file needs them.
- Unit and acceptance tests never touch real tools, the network or the user's home directory. Measurement tests may use
  real checkouts and print what they measured; they skip when the checkout is missing.
