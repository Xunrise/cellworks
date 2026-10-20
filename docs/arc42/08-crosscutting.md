> Status: draft

# 8. Crosscutting Concepts

## 8.1 Concurrency

The UI thread owns all application state and draws every frame. Loaders, scans, transcript tailing and external tool
calls run on background threads and send finished results to the UI thread over channels. Nothing on the UI thread
waits for a process, a file system walk or the network.

## 8.2 Error handling

- A failure belongs to the project or session it occurred in: that project shows the error, everything else keeps
  working ([10, R1](10-quality.md)).
- Missing external tools (git, kubectl, nvim) disable only the features that need them, with a visible message.
- Parsing of external output (transcripts, tool JSON, MCP responses) is tolerant: unknown fields and events are
  ignored, not fatal.

## 8.3 Persistence

- Settings and caches (Jira tickets, OAuth tokens) are files in the user's application support directory.
- Tokens are stored with user-only file permissions.
- Nothing is stored in project folders, except what the embedded editor saves.

## 8.4 Security

- Secrets never go on a command line: `curl` receives headers through stdin config and bodies through private temp
  files ([ADR-0003](../adr/0003-external-tools-as-child-processes.md)).
- Cluster access is `kubectl get` only.
- Cellworks runs no network server.

## 8.5 Paths

A project or a file is identified by its canonical path — symlinks and `.`/`..` resolved — never by the spelling it
was added with.

Whether two paths are the *same* folder is the filesystem's answer, not a rule in the code: they are the same when
the filesystem reports the same file identity (device and inode) for them. A case-insensitive macOS volume therefore
folds `Alpha` and `alpha` into one project, and a case-sensitive Linux filesystem, where they are two directories,
keeps two. No `cfg!(target_os)` encodes it. Product owner, 2026-10-08.

A remote project is identified by `host:path` as written, never canonicalized or compared by file identity.

## 8.6 Test strategy

| Level | Scope | Tool | Where |
|---|---|---|---|
| Unit | One function or type inside a crate: layout physics, graph building, parsing of transcripts, tool output and MCP responses | `cargo test`, plain `#[test]` | Inside each crate |
| Acceptance | One test per Gherkin scenario, black box through the app's public behaviour: input in, visible state out | `cargo test` with `egui_kittest` driving the UI headlessly | The crate that owns the behaviour, or the binary for cross-crate scenarios |
| Measurement | Quality scenarios of [chapter 10](10-quality.md): frame times, layout growth, scan time on the reference project | `cargo test --release -- --ignored`, printing what was measured | `#[ignore]` tests; run when touching layout, rendering or loaders |
| Smoke | Real external tools: git, nvim, kubectl, Atlassian | Command-line smoke modes of the binary | Run by hand or in CI where the tool exists |

- **From Gherkin to tests:** each scenario of a story becomes one acceptance test named after the scenario. Tests
  drive the app through its input and assert on what is visible (the selected node, the layer, the open folder view,
  the editor's focus), not on internal fields.
- **External tools in tests:** unit and acceptance tests use recorded tool output and fixture folders; only smoke
  modes and measurement tests touch real tools and checkouts.
- **No windows:** tests never open a window on the user's screen; the UI is driven headlessly.
- **Environments:** the developer's Mac, and CI on macOS and Linux (the release workflow runs the tests on Linux).
- **Coverage:** every Gherkin scenario has its acceptance test; no line-coverage target.
