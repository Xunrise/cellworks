# Architecture

## Crates

- The root `Cargo.toml` is the `cellworks` binary package and declares the workspace. The binary in `src/` wires the
  crates, registers loaders, owns the window and the frame loop, and holds the command-line entry points.
- Each bounded context is a library crate in `crates/<name>/`: `workspace`, `map`, `navigation`, `code-analysis`,
  `domain-knowledge`, `sources`, `agent-sessions`, `editing`.
- Crate dependencies follow `docs/arc42/05-building-blocks.md`. A crate never depends on a downstream crate; when two
  contexts would depend on each other, the binary wires the flow.
- Shared dependency versions are declared once in `[workspace.dependencies]`.
- A dependency is added only when a few lines cannot do the job.

## Public API

- `lib.rs` exposes a small API: the types and functions other crates use. Everything else is `pub(crate)` or private.
- The Map's graph types are the language every loader speaks. Loaders build a graph; they do not reach into layout
  or rendering.
- Code that draws with egui lives in a `ui` module of the crate that owns the behaviour. Model and logic modules do not
  depend on egui beyond its geometry types.

## Threads and events

- The UI thread owns all application state. Nothing on it blocks on a process, a file system walk or the network.
- Background work runs on `std::thread`; results come back as `<Context>Event` values over `std::sync::mpsc` and are
  drained once per frame.
- No async runtime.

## Errors

- Library crates return `Result` with an error type of their own, or `anyhow::Result` inside loaders where the error
  only ends up as a message.
- A failure is attached to the project or session it belongs to and shown there; it never takes down other projects.
- No `unwrap` or `expect` on data from outside the process (files, tool output, transcripts, network).

## External tools

- git, kubectl, nvim, curl and gh are run as child processes behind a small adapter per tool in the crate that uses it.
- Secrets reach curl through stdin config or private temp files, never through arguments.
- Cluster calls are `kubectl get` only.
- `gh` runs read-only commands only (`gh pr list`), never a write command.
- Tool output is parsed tolerantly: unknown fields and events are ignored.
