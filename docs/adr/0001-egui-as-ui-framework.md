# ADR-0001: Use eframe/egui as the UI framework

## Status
Accepted

## Date
2026-10-08

## Context
Cellworks is rebuilt as a native Rust desktop app, macOS first; [chapter 2](../arc42/02-constraints.md) left the UI
framework open, to be chosen for what works best on macOS. The app draws a zoomable map of up to about 20,000 nodes and
30,000 lanes with level of detail, plus dense text grids for an embedded Neovim and terminals. The prototype used
eframe/egui. A measured spike compared egui with GPUI, Zed's Metal-first framework, on an identical synthetic map
([spike results](../discovery/2026-10-08-ui-framework-spike.md)).

## Decision
We will build the UI with eframe/egui (0.36, wgpu backend), drawing the map on a custom-painted canvas.

## Consequences

### Positive
- About 2× lower CPU per frame than GPUI at overview (7.3 vs 15.5 ms p50) and with dense text (29 vs 63 ms).
- Stable crates.io releases; no git pin on a monorepo; no extra toolchain to build.
- Immediate mode keeps drawing code plain; the prototype proved the Neovim and terminal grids in it.
- The Linux release build keeps working.

### Negative
- Text is rasterised by egui (greyscale, no Core Text hinting) and looks softer than native macOS text.
- Drawing many labels is costly: 20,000 forced labels cost about 29 ms per frame and 3.2 GB of memory. Level of
  detail is mandatory, not an optimisation.
- Continuous zoom creates a font size per frame; zoom frames have about 3× the median at p95.
- Vsync cannot be fully disabled on Metal; frames are paced by the display.

### Neutral
- Layout and other derived state must be cached by the app, since egui rebuilds shapes every frame.

## Alternatives considered
- **GPUI** (Zed, tag v1.23.2): crisp Core Text rendering and lower worst-case memory. Rejected because it was about
  twice as slow at overview and with dense text, has no official crates.io release (git pin on the Zed monorepo,
  unstable API, examples as the only docs), needs the Xcode Metal Toolchain for its default build, and stops drawing
  when the window is hidden.
- **A web front end (Tauri or a local server)**: excluded by chapter 2 (no web front end, no localhost server).

## References
- Evidence: [UI framework spike](../discovery/2026-10-08-ui-framework-spike.md)
- arc42: [02 Constraints](../arc42/02-constraints.md), [09 Decisions](../arc42/09-decisions.md)
