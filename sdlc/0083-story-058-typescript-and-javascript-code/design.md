---
stage: design
issue: 83
status: frozen
author: claude (freeze)
date: 2026-10-19
---

# Design: STORY-058 — TypeScript and JavaScript code stands as towers

The contract is `issue.md` in this directory: its acceptance scenarios are what
the change must do. `prototype.md` beside it records the confirmed behaviour, the
edge cases and every answered question; its Rejected items are not built. The
prototype's code (`crates/code-analysis/src/ts.rs`, the `proto_*` tests, the
kittest `wgpu`/`snapshot` features) is evidence only and is not carried over.

## Open questions

None.

## Requirements

1. Files ending in `.ts`, `.mts`, `.cts` are read with the tree-sitter TypeScript
   grammar; `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` with the TSX grammar.
2. Top-level statements become items: adjacent imports one `Uses` item named
   `imports`; function and generator declarations a `Function`; each
   `const`/`let`/`var` declarator named by an identifier a `Function` when its
   value is an arrow function or function expression, else a `Type` with
   `Declares::Constant`; a destructuring declarator nothing; classes a `Type`;
   interfaces, type aliases and enums a `Type` with `Declares::Plain`.
3. A class's item keeps its lines down to the line before its first method; each
   method is a `Function` item named after the method, at the path
   `<file>/<Class>::<method>`, `part_of` the class.
4. A declaration inside `export` is public, a method takes its class's
   visibility, everything else is not public.
5. In a file whose name contains `.test.` or `.spec.` every item has
   `tests: true`, the import block included.
6. Test framework calls, at top level or inside a suite: a callee `describe`,
   `it`, `test`, or that name with `.only` / `.skip`, or `X.each(table)` (call or
   tagged template), with a string or template-string first argument. `describe`
   forms are suites: no item, their function body read as statements with
   `tests: true`. `it`/`test` forms are `Test` items named by the label without
   quotes; `.each` labels keep their placeholders, so all rows share one tower.
   Any other member call (`it.todo`, `foo.it`, `expect(x).toBe`) is no item.
7. A file whose syntax tree has any error yields no items from this reader, so
   the existing plain fallback makes it one tower with one floor per line.
8. Files over 2 MiB are skipped by this reader, as the Rust reader does.
9. Two items of one file with one name: the later one's path gets `@<line>`. A
   `/` in a name (a test label) becomes U+2215 in the path.
10. Floors carry the line's text, indent, width and tokens as Rust floors do;
    `calls` and `imports` stay empty (STORY-059, STORY-060).

## Decisions

- **Shared seam `crates/code-analysis/src/items.rs`.** `column`/`TAB`, a
  `tokens(root, source, lines)` built from `rust::parse::leaves`, `floor(lines,
  tokens, row)` from `rust/mod.rs`, and `unique_paths(file, (name, first_row))`
  generalised from `plain::part_paths`. Rust, plain and TypeScript import from
  it. Rejected: exposing `rust::FileCode` and `leaves` (the prototype's hack,
  couples readers to Rust's types); a `trait Reader` per language (one method,
  no second caller needing dynamic dispatch).
- **Reader `crates/code-analysis/src/typescript.rs`** with
  `read(root, files) -> BTreeMap<PathBuf, Vec<Item>>`, called in `lib::load`
  after `rust::read` and before the plain fallback. Rejected: folding it into
  `rust/` or `plain.rs`.
- **TSX grammar for the JavaScript family** (operator, at freeze). Rejected: the
  TypeScript grammar for `.js`, which sends React files holding markup to the
  plain fallback.
- **`.only`, `.skip`, `.each` recognised** (operator, at freeze). Rejected: bare
  names only, which drops skipped and table-driven tests from the map.
- **`.each` rows share one tower**, labelled with placeholders. Rejected: one
  tower per row, which needs evaluating the table.
- **Enums keep `Declares::Plain`** and stand as towers, as confirmed. Rejected:
  `Declares::Enum`, which would make them terraces against the confirmed "type
  tower".
- **Destructuring declarators yield no item.** Rejected: naming a tower after
  the pattern text, as the prototype did.
- **2 MiB limit** as `rust::LARGEST`. Rejected: no limit, which reads minified
  bundles into thousands of towers.
- **STORY-057's fixture `web/app.ts` becomes `web/app.rb`.** Once `.ts` is
  parsed it no longer illustrates "a file no parser reads". The behaviour that
  test pins is unchanged; issue #82's Gherkin text still names `app.ts`.
- **Dependency** `tree-sitter-typescript = "0.23"`, compiled against
  `tree-sitter = "0.25"` in the prototype. No root `Cargo.toml` change.

## Order of work

1. Add `crates/code-analysis/src/items.rs` with `column`, `TAB`, `tokens`,
   `floor`, `unique_paths`; switch `rust/parse.rs`, `rust/mod.rs` and
   `plain.rs` to it, deleting the moved code there; declare `mod items` in
   `lib.rs`. Existing tests pass unedited apart from `use` paths.
2. Add `tree-sitter-typescript = "0.23"` to `crates/code-analysis/Cargo.toml`
   and `Cargo.lock`; add `crates/code-analysis/src/typescript.rs` (`read`,
   `grammar_of`, `items`, `framework`) with its unit tests; declare
   `mod typescript` and call `typescript::read` in `lib.rs`; mention TypeScript
   and JavaScript in the crate doc comment.
3. Change `tests/story_057_every_file_stands_as_a_tower.rs` to the `web/app.rb`
   fixture; add `tests/story_058_typescript_and_javascript_code_stands_as_towers.rs`
   with one test per scenario.

## Risks

- The riskiest step is 1: moving token and floor code under the Rust reader. The
  existing Rust and plain tests pin it unchanged.
- A syntax error anywhere sends a whole file to plain, including TypeScript
  syntax newer than grammar 0.23 (confirmed as wanted).
- Rare JavaScript the TSX grammar misparses falls back to plain.
- `let`/`var` stand as monuments though they can change (confirmed "constant
  tower").
- `it.todo`, `.concurrent`, `.failing` are not recognised.
- Files holding only re-exports, `declare` statements or triple-slash
  references yield no towers (confirmed edge case).
- Rollback: revert the pull request's squash commit; nothing persists, the next
  scan reads `.ts` files as plain text again.

## Proof

Commands, each expected to finish with no failure and no warning:

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

Acceptance, `tests/story_058_typescript_and_javascript_code_stands_as_towers.rs`,
one test per scenario, the project opened headlessly with `common::open`:

- `functions_classes_and_import_blocks_are_towers` — Functions, classes and
  import blocks are towers.
- `a_function_held_by_a_constant_is_a_function_tower` — A function held by a
  constant is a function tower.
- `type_declarations_are_type_towers` — Type declarations are type towers (loop
  over interface, type alias, enum).
- `a_method_is_a_tower_named_after_its_class` — A method is a tower named after
  its class (path `src/cart.ts/Cart::add`, `part_of` the class, drawn).
- `tests_stand_in_the_cellar` — Tests stand in the cellar.
- `each_test_of_a_suite_is_its_own_tower_the_suite_is_none` — Each test of a
  suite is its own tower, the suite is none.
- `every_file_of_the_family_is_read` — Every file of the family is read; the
  `.tsx`, `.js` and `.mjs` fixtures return markup, pinning the TSX grammar, and
  the `.ts` fixture holds an angle-bracket type assertion, pinning the
  TypeScript grammar. Each fixture is written in the syntax only its own
  grammar reads, so a file routed to the other grammar has a syntax error and
  loses its `load` tower to the plain fallback.
- `a_file_that_does_not_parse_stands_as_one_tower_of_its_lines` — A file that
  does not parse stands as one tower of its lines (one `Text` item, one floor
  per line).

Unit tests in `typescript.rs`: `an_exported_declaration_is_public_and_others_are_not`,
`a_class_keeps_its_lines_down_to_its_first_method`,
`every_other_variable_is_a_constant_one_per_declarator`,
`adjacent_imports_form_one_block_and_a_gap_starts_another`,
`in_a_test_file_every_item_is_in_the_cellar_its_imports_included`,
`the_js_family_and_tsx_read_with_the_tsx_grammar` (over `.tsx`, `.js`, `.jsx`,
`.mjs`, `.cjs`),
`a_type_assertion_in_a_ts_file_parses` (an angle-bracket assertion, over `.ts`,
`.mts`, `.cts`, so the extensions the acceptance test does not name are pinned
to their grammar too),
`focused_skipped_and_table_driven_tests_are_towers`,
`focused_skipped_and_table_driven_suites_are_read`,
`other_member_calls_are_no_tests`,
`nested_suites_are_read_and_top_level_tests_count_too`,
`two_declarations_of_one_name_get_their_line`,
`a_test_label_with_a_slash_stays_one_path_segment`,
`a_file_with_a_syntax_error_has_no_items`,
`floors_carry_the_tokens_of_their_line`.

Regression: the Rust tests in `rust/` and the plain tests in `plain.rs` pass
unedited apart from `use` paths; `tests/story_057_…` passes with its new
fixture.

## Docs impact

Written at freeze in the commit `docs: ingest STORY-058 prototype decisions`;
the build changes no other doc.

- **Business decisions** — None.
- **Glossary** — `docs/domain/contexts/CodeAnalysis/glossary.md`: Visibility
  covers `export` in TypeScript and JavaScript; new term Test suite.
- **Guidelines** — None.
- **arc42** — None. `05-building-blocks.md` is language-neutral and
  `CodeAnalysis.md` already lists TypeScript and JavaScript.
- **ADR needed** — No. ADR-0004 already names TypeScript and JavaScript; a
  grammar crate is the dependency it foresees.
