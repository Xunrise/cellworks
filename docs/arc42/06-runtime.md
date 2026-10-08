> Status: draft

# 6. Runtime View

The three scenarios that shape the system. Building blocks are those of [chapter 5](05-building-blocks.md).

## 6.1 Opening a project

```mermaid
sequenceDiagram
  actor User
  participant Nav as navigation
  participant WS as workspace
  participant L as loader (code-analysis, sources)
  participant Map as map
  User->>Nav: Enter on a project
  Nav->>WS: project identity
  WS->>L: load on a background thread
  L-->>Map: finished graph
  Map->>Map: lay out, district homes first
  Map-->>Nav: graph ready, fitted
  Nav->>Nav: apply a pending jump, if any
```

- The UI thread never waits for a loader; until the graph arrives the project shows its loading state.
- A jump into a project that is still loading is applied after the first fit.
- A failing loader marks only its own project as failed.

## 6.2 Moving through layers with the keyboard

```mermaid
sequenceDiagram
  actor User
  participant Nav as navigation
  participant Map as map
  participant Ed as editing
  User->>Nav: Enter on a district
  Nav->>Map: open the folder view, frame it
  User->>Nav: Enter on a file
  Nav->>Map: frame the file at inspection distance
  User->>Nav: Enter on the selected file again
  Nav->>Ed: open the file, give it the keyboard
  User->>Ed: Esc
  Ed-->>Nav: editor closed, keyboard returned
  Note over Nav: the Esc that left the editor does not act on the map
  User->>Nav: Esc
  Nav->>Map: frame the folder
```

- Keyboard input goes to whatever has focus (text field, editor, terminal, menu); the map acts only otherwise.
- Every view change is recorded in the history for Ctrl-O and Ctrl-I.

## 6.3 A session working in a project

```mermaid
sequenceDiagram
  participant T as Claude transcript
  participant Ag as agent-sessions
  participant Map as map
  participant WS as workspace
  T-->>Ag: new event, a tool reads or writes a file
  Ag->>Map: move the session to that file's node
  Ag->>Map: mark a construction site for a created file
  Ag->>WS: request a rescan of the touched file
  WS->>Map: updated part of the graph
  T-->>Ag: permission prompt
  Ag->>Map: show the approval alert on the session
```

- Every session on the machine is discovered from its transcript, whether Cellworks started it or not.
- Unknown transcript events are ignored.
