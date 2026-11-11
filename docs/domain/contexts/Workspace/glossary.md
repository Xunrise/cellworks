# Workspace — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| Launcher | The view over the `Universe` scan state in which the engineer searches, picks a location, groups and selects a project to open. | STORY-078 | The `Universe` stays; the launcher is not a second registry. |
| Location | Where projects live: this Mac or a remote host. | STORY-078, STORY-082 | Shown as the location chips above the list; avoid "sources" in code, it clashes with the Sources context and the `Sources` struct. |
| Recent | The projects opened most recently, at most five, kept in a `recent` file beside the settings file. | STORY-078 |  |
| Theme words | The launcher's display vocabulary in the Netrunner theme (fortress, net, block, ICE). On by default; `[launcher] theme_words = false` in the settings file switches to plain words. | STORY-083 | Display only, never in identifiers. Replaced by the skin choice of STORY-036. |
| Remote host | A machine named in the settings file, reached over ssh, with a state of connected or offline. | STORY-078, STORY-082 | Shown as a location chip with its state. State comes from a reachability probe once per launch. |
| Remote project | A project on a remote host, identified by `host:path`. | STORY-078 | Listed and grouped only; never read, opened or arranged as a local folder. |
| Rescan | Rebuilding a project's map: ⌘R, or automatically after sessions write files. | prototype-findings L222 |  |
| Settings | Persisted user preferences. | prototype-findings L221 |  |
