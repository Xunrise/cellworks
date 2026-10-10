# Context: Navigation

How the user moves through the map with keyboard and mouse: layers of detail, a selection the keyboard acts on, Enter and Esc to move one layer in or out, a per-node item menu, a global palette, a per-layer tree, and back/forward history.

Terms: [`glossary.md`](glossary.md) — this context's own vocabulary, in the folder beside this file.

## Participants
- Every persona — keyboard first, mouse as alternative

## Behaviour
Source refs are line numbers of [the prototype findings](../../../discovery/2026-10-08-prototype-findings.md).

- Layers Universe → Project → Directory → File; the layer follows from selection and zoom (L143-150). Districts are the nodes of the Project layer (product owner, 2026-10-08).
- h/j/k/l and arrows move between nodes of the current layer, in spatial or list-order mode (`m`); the camera pans only when the selection leaves the middle of the view (L152-157).
- Enter goes one layer in: lock a project, open a district's folder view, frame a group (~70% of the view), frame a node close enough to read it (~55%), and a second Enter on a node opens the editor (L159-167).
- Esc goes one layer out: editor → node → group → leave the folder view → district → project → universe (L169-171).
- The selection always shows a pulsing outline (L173).
- Item menu (Space / right-click): a pop-up on the node with only its connections and actions, busy kinds in submenus, typing filters (L175-180). Palette (⌘K) searches everything and lists every action (L182). View options menu (`v`) stays open for several changes (L184).
- Tree (E): per-layer explorer that frames the map on the row under the cursor, re-roots as the layer changes, sized to its content (L186-204).
- ⌘-click a lane travels to its far end; ⌘-hover previews it (L90). Back/forward history of views (L91, L212).
- A jump into a project still loading applies after its first fit; a jump to a node hidden by the folder view opens the folder view containing it; switching projects closes the folder view (L230-233).
- The keyboard belongs to whatever has focus; map keys act only otherwise (L236).

## Tactical model

> Status: not yet discovered
> The tactical model emerges during implementation. A story adds an
> entry here only for a new aggregate, a changed aggregate boundary or
> a new invariant — the code is the rest of the model. Do not
> pre-populate from the discovery source: tactical choices are made at
> code time, not at analysis time.

### Aggregates
- **Navigation** (`cellworks_navigation::Navigation`): what is framed (the Universe, the open project, a folder or a
  node) and the selection. Invariants: the layer follows from what is framed (a folder is the Directory layer, a node
  the File layer); Enter moves the frame to the selection. A framed folder is its folder view: only the folder and
  what it holds are shown, and a framed node shows the folder view of the folder holding it. Esc has one step per
  layer: from a node to its folder view with the node still selected, from a folder view to the whole project with the
  folder's district selected. A jump frames the folder holding its target and selects the target. The selection is a
  path read on the current layer, so the district of a project's top-level files, which has the project's own path,
  is never mistaken for the project. Zooming moves the view; the layer is locked by default, and once it follows the
  zoom, zooming also moves the layer: the
  whole project in view is the Project layer with the selection's district selected, and the innermost shown element
  under the zoom point that spans at least half the view is framed, a node also selected. Enter, Esc and a jump drop
  the zoom and frame what they move to; locking or unlocking changes neither the layer, the selection nor the view.
  The tree lists what the current layer holds (the districts on the Project layer, the folder view's contents below
  it), and its highlighted row is the selection, so the tree and the map never disagree.
  (STORY-004, STORY-007, STORY-006, STORY-008)

### Entities (non-aggregate roots)
_No entries yet._

### Value objects
_No entries yet._

### Domain services
_No entries yet._
