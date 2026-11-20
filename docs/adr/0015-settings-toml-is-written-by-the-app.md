# ADR-0015: settings.toml is written by the app

## Status
Accepted

## Date
2026-11-20

## Context
`settings.toml` was read-only by design: the engineer edited it by hand and nothing in Cellworks wrote to it. Tickets
sources, map defaults, the editor and the update check are now configured in a settings window (Cmd+,), and a setting
that cannot be saved is not a setting.

## Decision
We will let the settings window write `settings.toml`.
- The file is loaded as a `toml::Table`, only the keys the window owns are replaced, and every other key
  (`added_folders`, `[projects."<path>"]`, `[hosts."<name>"]`, keys of a newer version) is kept as it was.
- It is written to a temporary file beside it and renamed, so a write cut short never leaves half a file.
- The `toml` crate does not keep comments. The first write keeps the previous file as `settings.toml.bak`.
- A file that cannot be parsed is never overwritten; the window shows the defaults and says why.
- Environment variables (`CELLWORKS_JIRA_SITE`, `CELLWORKS_COCKPIT_URL`, `CELLWORKS_CASES` and the rest) still override
  the file. The window shows an overridden field disabled, with the variable's name.
- The Linear API key is stored in the macOS Keychain through the `security` command line tool (ADR-0003), never in
  the file and never logged.

## Consequences

### Positive
- Every setting can be changed without leaving the app; hand edits still work and are not lost.

### Negative
- Comments in the file are lost on the first write (the backup keeps them).
- `security add-generic-password -w` puts the key on its argument vector for the moment the child runs.
