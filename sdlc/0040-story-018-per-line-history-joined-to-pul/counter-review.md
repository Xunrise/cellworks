# Counter-review

Standards reviewed: factory:standards-change, factory:standards-security, factory:standards-testing.

- Nit: crates/code-analysis/src/git.rs:156 — a panicking `blame` worker thread is swallowed by `handle.join().unwrap_or_default()`, turning a genuine bug into a quietly-shrunk result instead of surfacing it. standards-change: "Fail loudly." Propagate or log the panic instead of discarding it.
- Nit: crates/code-analysis/src/git.rs:210 — `blame_one` builds its own `Command` (program, args, `current_dir`, `GIT_OPTIONAL_LOCKS`) instead of reusing the `command()` helper already used by `visible_files`, `history` and `log`. standards-change: "Reuse before adding." Extend `command()` (or factor the shared setup) so blame uses the same helper.
- Nit: crates/code-analysis/src/lib.rs:134 — the design states "a failing `log` leaves `history` empty" as a named behaviour, but no test exercises `git.log` failing inside a repository (only the "no git" path is covered). standards-testing: "New behaviour arrives with its tests, including the error paths the spec or plan names." Add a case where `log` returns `Err` inside a repository and assert `map.history` stays empty.
