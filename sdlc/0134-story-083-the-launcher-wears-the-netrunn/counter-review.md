# Counter-review

Standards applied: factory:standards-change, factory:standards-security, factory:standards-testing.

- Nit: crates/workspace/src/words.rs:20-28 — the comment and `#[allow(dead_code)]` say the struct's fields "sit unread" and that "step 2 is what rewires `ui.rs`" to use them, but this same change already wires `ui.rs` to read every field (`header`, `footer`, `chip_row`, `inspector`, etc. all call `launcher.words()` and read its fields). The comment narrates an intermediate, already-superseded implementation step rather than the shipped code. standards-change: "Comments say why" / docs stay true. Remove the stale "Order of work step 1/2" narration and the now-unneeded `#[allow(dead_code)]` (same issue on the `counts`, `host_group_heading`, `offline_body` and `notice` methods at lines 106, 146, 152 and 158, and on `Launcher::words()` in crates/workspace/src/launcher.rs:155-157, all of which are likewise already called from `ui.rs`).
