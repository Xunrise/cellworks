# Counter-review

Standards applied: factory:standards-change, factory:standards-security, factory:standards-testing.

None.

- Nit: crates/workspace/src/lib.rs:179 — the root-detection probe for the `0o000` permission test (`read_dir` first, assert `!readable_anyway`, restore `0o755`) is copy-pasted near-verbatim three times (here, and twice more in crates/workspace/src/discovery.rs:201 and :234), rather than factored into `test_tree.rs`, which exists precisely to hold shared test machinery. factory:standards-change rule 2 (reuse before adding). Extract a `TestTree` helper (e.g. `assert_unreadable(&path)` that probes, asserts, and restores `0o755`) and call it from all three sites.
- Nit: crates/workspace/src/project.rs:18 — `ProjectId` derives `PartialOrd, Ord, Hash`, none of which any code in this change uses (sorting is done by `.id.path().cmp(...)` in registry.rs:65, and the de-dup map is keyed on `FolderIdentity`, not `ProjectId`). factory:standards-change rule 2 (no abstraction added beyond what the change needs). Drop the unused derives, or add the doc comment explaining which future consumer (Map/Sources) needs them, so the derive isn't mistaken for dead weight.
