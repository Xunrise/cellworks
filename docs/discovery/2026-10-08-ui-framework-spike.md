# UI framework spike: eframe/egui vs GPUI

Date: 2026-10-08. Throwaway spikes, not kept in the repository. Machine: Apple M5 Pro, window 1600x1000, display awake,
spikes run one at a time.

## Setup (identical for both)

- 40 districts × 10 groups × 50 nodes = 20,000 nodes (rounded rectangles 120x40 with labels of 10–40 characters),
  30,000 straight lanes (70% within a group), deterministic layout, no physics.
- Level of detail: node labels when a node is at least 60 px wide, group labels from 150 px, district labels always;
  culling outside the viewport.
- Scripted camera, 300 frames per phase: A overview with a slow pan; B exponential zoom into one group; C pan at close
  zoom; D overview with all 20,000 labels and all lanes forced.

## Results

CPU time per frame in ms, p50 / p95 / max. The egui figure is eframe's full-frame CPU (build, tessellation, wgpu
render). The GPUI figure covers render and canvas paint, but not the scene submit or the Metal encode.

| Phase | egui 0.36.2 (wgpu 30) | GPUI (Zed tag v1.23.2) |
|---|---|---|
| A overview | 7.25 / 7.96 / 8.53 | 15.50 / 17.13 / 18.37 |
| B zoom | 3.85 / 10.26 / 12.02 | 2.44 / 13.92 / 15.72 |
| C close | 1.10 / 1.36 / 2.21 | 0.75 / 1.14 / 1.38 |
| D worst | 28.88 / 30.39 / 79.96 | 62.86 / 63.73 / 72.49 |

| | egui | GPUI |
|---|---|---|
| Peak RSS, phases A–C | 227 MB | about 164 MB without lanes |
| Peak RSS, worst case | 3.25 GB, entirely from the 20,000 forced labels | 1.32 GB, almost entirely from lane paths |
| Clean release build | 18.2 s | 28.5 s, plus about 100 s for the first git fetch |
| Binary | 12.6 MB | 8.0 MB |
| Distribution | crates.io | git dependency on the Zed monorepo; no official crates.io release |

## Observations

- egui: building shapes is cheap (1 ms at overview, 6 ms worst case); tessellation and rendering dominate. Text is
  rasterised by egui itself, greyscale, without Core Text hinting, so it looks softer than native text. A continuous
  zoom creates a new font size per frame, which raises phase B's p95. Vsync cannot be fully disabled on Metal.
- GPUI: Core Text shaping and a subpixel glyph atlas give crisp text. 20,000 labels still cost about 45 ms per frame.
  Thin lines go through lyon paths with a 16-bit index limit and high memory use. The default build requires the Xcode
  Metal Toolchain; `runtime_shaders` avoids it. The window stops drawing when occluded, so no rendering on a locked
  screen.
- Both: level of detail is mandatory for a 20,000-node map. Dense text grids (Neovim, terminals, around 10k glyphs) sit
  well inside what both handled.
