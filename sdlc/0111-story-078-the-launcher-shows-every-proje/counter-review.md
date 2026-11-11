# Counter-review

Standards applied: standards-change, standards-security, standards-testing (factory plugin).

- Nit: crates/code-analysis/src/git.rs:148 — `GIT_CONFIG_NOSYSTEM=1`, the fix for the backend-security-reviewer's third Important finding, has no test asserting the child process carries it (only `GIT_OPTIONAL_LOCKS` is asserted, at crates/code-analysis/src/git.rs:230-231). Standards-testing: "New behaviour arrives with its tests." Add an assertion on `command().get_envs()` for `GIT_CONFIG_NOSYSTEM`.
- Nit: crates/workspace/src/recent.rs:60 — the newline-id filter in `RecentStore::save` (the fix for one of the two documented Nits) has no test exercising a remote id whose path holds a newline. Standards-testing: "New behaviour arrives with its tests." Add a case to the `recent.rs` test module.
- Nit: crates/workspace/src/universe.rs:43-44 — `Notice::path`'s doc comment ("The folder or file the failure belongs to") is now also used to carry a host name (`WorkspaceEvent::HostSkipped` folded in via `PathBuf::from(name)` at universe.rs:108-109), which is neither a folder nor a file. Standards-change: "Docs stay true." Update the comment to mention the host-name case.
