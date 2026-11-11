# Naming

- Crates: directory `crates/<name>/`, package `cellworks-<name>` (kebab-case), imported as `cellworks_<name>`.
- Identifiers use the ubiquitous language from `docs/domain/` with its neutral terms: `District`, `SubDistrict`,
  `Group`, `Node`, `Item`, `Lane`, `LaneKind`, `Session`, `ContextCell`, `Layer`, `Selection`.
- In code-analysis, `File`, `Folder` and `Function` mean real files, folders and functions; in the map they become
  `Node`, `Group` and `Item`. Do not use `File` for a map node.
- Theme words, Warhammer 40k (planet, factory, machine, crate, servitor, belt) and Netrunner (fortress, net, block,
  ICE, jack in), appear only in user-facing strings, never in identifiers. The launcher's come from its word table.
- Events: one enum per context named `<Context>Event`, variants in past tense (`SessionEvent::FileWritten`,
  `WorkspaceEvent::ProjectAdded`).
- Rust naming follows the standard style: `snake_case` functions and modules, `CamelCase` types, `SCREAMING_SNAKE_CASE`
  constants.
