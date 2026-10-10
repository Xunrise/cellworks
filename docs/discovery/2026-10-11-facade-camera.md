# Discovery: the facade camera

**Date:** 2026-10-11
**Status:** confirmed by the product owner through a throwaway prototype on the built city (EPIC-009); built as
STORY-045 to STORY-049.

## Summary

Reading code in the city should feel like walking up to a building, not like opening files. Close up, the camera
turns from looking at the city from the corner to looking straight at one building's facade, where the code is
written line by line; the arrows then walk from facade to facade along the block. Buildings are solid and stand in
a skyline, so it is always clear what stands behind what, and the tests live in a cellar beneath the code instead
of crowding the view.

## How we got here

The prototype ran in a worktree off the city as built for STORY-010 to STORY-014 and was iterated with the product
owner on the running app.

| Tried | Verdict |
|---|---|
| Read mode: at reading distance each file of the district flies into a fixed-width column, the columns side by side like a horizontally scrolling window manager | Rejected: feels like opening files, not looking at buildings |
| Read mode as a front elevation: the district's towers rearranged side by side on one ground line | Rejected: "doesn't work at all" |
| A camera that turns towards the facade being zoomed into, one zoom step at a time, pivoting on that facade | Kept |
| Arrows to the neighbouring facade, across blocks | Kept within the block only |
| Up and down scroll a facade taller than the view before stepping to the next row | Kept |
| Depth on the lettering (a shadow or extrusion) | Rejected: depth belongs to the buildings, not the text |
| Buildings as outlines of their floors | Rejected: rows behind each other cannot be told apart |
| Solid boxes: dark front, side and roof, edged in the district colour, nearer buildings hiding farther ones | Kept |
| One row per block, so nothing ever stands behind anything | Rejected: blocks become long thin streets |
| Parallel avenues of blocks | Rejected: "looks like an equation, not a city" |
| A shelf grid of blocks, with and without height clearance | Rejected: too regular |
| Blocks on a spiral around the plaza, towers in a skyline (tallest at the back) | Kept |
| Tests in a cellar below the ground, faint, within reach only in the underground view | Kept |
| The layer following the zoom by default | Rejected: zooming into a building jumped into folder views; locked is the default now |

## Confirmed behaviour

- Between scale 4 and 16 each zoom step turns the camera further from the corner to a straight front view of one
  facade; zooming out turns it back. The facade is the selected tower, else the one already faced, else the one
  pointed at, else the one nearest the middle of the view. It settles in the middle of the view and zoom anchors on
  it.
- Once turned halfway, left and right walk the block's rows and stop at its ends; up and down scroll a tall facade
  (met at its first line) and then step a row back or forward. Walking stays on the block and on its level. A bar
  names where each arrow leads.
- Towers are solid boxes on ground 3 deep; the first line is the top floor; floors 1.3 apart (types and `use`
  blocks 0.6); far out each floor is a stroke as wide as its line. Within a block the towers stand in a skyline.
  Facing a facade, what stands in front of it all but vanishes and what stands behind fades with distance.
- Tests (`#[test]` functions and everything in a test module) hang in the cellar under their block; a file of only
  tests lies in its district's cellar under the plaza. U toggles the underground view, Shift shows it while held;
  outside it the cellar cannot be reached.
- Districts stand 60 apart, blocks 14, towers 4.5 in rows 12 apart.

## Open

- Very tall towers on the spiral can still stand in front of blocks behind them; glass shows those on demand.
- Text on facades that run along ground y reads mirrored after a quarter turn of the map.
