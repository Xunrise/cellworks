> Status: draft

# 10. Quality Requirements

Scenarios elaborate the quality goals of [chapter 1](01-introduction.md), in their priority order. The reference
machine is an Apple silicon Mac (M-series, Pro class) with a 1600x1000 window; the reference project is synthetic with
20,000 files in 40 districts, or a real checkout of similar size. Numbers marked *assumed* are first targets to be
confirmed by measurement.

## 10.1 Usability

| # | Stimulus | Environment | Response | Measure |
|---|---|---|---|---|
| U1 | The user navigates from the universe to a function and back | Any project type | Every step is reachable by keyboard: arrows or h/j/k/l, Enter one layer in, Esc one layer out | The acceptance tests for navigation use no pointer input |
| U2 | The user presses Esc in the embedded editor | The editor has the keyboard | The editor leaves insert mode or closes; the map does not also act | Map selection and layer are unchanged after that key press |
| U3 | The user zooms continuously from universe to file | Reference project | Labels stay readable during the transition | No label is drawn below 6 px; overlapping labels are hidden, not stacked |
| U4 | Any node is selected | Any layer and zoom | The selection is visible | The pulsing outline is drawn in every frame while the node is on screen |
| U5 | The editor would open | The user only zoomed close to a file | The editor stays closed | The editor opens only on Enter on a selected file, `i`, double-click or an edit action |

## 10.2 Performance

| # | Stimulus | Environment | Response | Measure |
|---|---|---|---|---|
| P1 | The user pans and zooms | Reference project, overview, level of detail on | Smooth rendering | Frame CPU p95 ≤ 16 ms (60 Hz) and peak memory ≤ 1 GB (*assumed*; the spike measured 8 ms and 227 MB for drawing alone, [ADR-0001](../adr/0001-egui-as-ui-framework.md)) |
| P2 | A project is opened | Reference project, cold | The UI stays responsive while the scan runs; the map appears when ready | No UI frame exceeds 50 ms during the scan; the full map is shown within 10 s (*assumed*) |
| P3 | The user clicks, selects and opens folder views 100 times | Reference project, laid out | The layout settles and does not grow | The bounding box of the map grows by less than 5% |
| P4 | A session writes a file | Project open | The map reflects the change | The rescan of the touched file appears within 2 s, without a full rescan (*assumed*) |

## 10.3 Extensibility

| # | Stimulus | Environment | Response | Measure |
|---|---|---|---|---|
| E1 | A developer adds a new project type | Codebase as structured by [ADR-0002](../adr/0002-cargo-workspace-crate-per-bounded-context.md) | The type's map gets navigation, folder views, palette search, lanes and history | The change touches only the loader's crate and one registration in the binary; Map and Navigation crates are unchanged |
| E2 | A developer adds a district strategy | Code projects | Districts are cut by the new rule | The change touches only the Map's district strategy seam |

## 10.4 Lower-priority goals

| # | Goal | Stimulus | Response | Measure |
|---|---|---|---|---|
| R1 | Reliability | One loader fails (cluster unreachable, Jira offline, parse error) | Only that project shows the error; cached data is shown where it exists | Other projects keep loading and rendering |
| R2 | Reliability | The user edits in the embedded editor | Changes are saved | Saved within 1 s of the last change; git sees them |
| S1 | Security | Any external call with a secret | The secret is not exposed | No secret appears in any process argument list; cluster calls are `kubectl get` only |
