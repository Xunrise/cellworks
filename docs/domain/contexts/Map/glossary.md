# Map — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| Block | A node drawn as a plot of ground in the city, its towers standing on it in a skyline. | living-city discovery 2026-10-10; STORY-047 |  |
| Cellar | The ground beneath a block or a district's plaza where the items belonging to the tests hang, faint unless the view is underground. A district's own blocks of only tests lie under its plaza; deeper ones stay in their folder's ring. | facade-camera prototype 2026-10-11; STORY-048; STORY-056 |  |
| District | A node of the Project layer; for code projects the top folder below the common prefix, in Jira a component, in infrastructure an environment. | prototype-findings L43, L148 | Cut by directories for now; the district strategy is to become swappable |
| District strategy | The rule that decides how a project is cut into districts; directories for now. | prototype-findings product owner 2026-10-08 |  |
| Facade | The front face of a tower, where its lines are written; a turned camera faces one facade. | facade-camera prototype 2026-10-11; STORY-045 |  |
| Floor | One line of an item, drawn as one storey of its tower; the first line is the top floor. | living-city discovery 2026-10-10; STORY-047 |  |
| Folder view | Showing only one group, its members packed in boxes by next subfolder, with outside connections on a ring as lanes. | prototype-findings L50, L77-81 | also called scope |
| Folder ring | A folder drawn in the folders lens as a dashed ring inside the ring of the folder holding it; a folder holding only one folder and no file is one ring named after both (`src/main/kotlin`). Its name is written on its front once the ring has room for it; clicking the name or the ring's open ground selects the folder. | nested-folders prototype 2026-10-12; STORY-056 |  |
| Group | A map level: a folder in code, an epic in Jira, a namespace or Terraform group in infrastructure, a database in data. | prototype-findings L45 | theme: factory |
| Item | A map level inside a node: a function in code, a sub-task, a column, a field. | prototype-findings L47 | theme: crate |
| Lane kind | What a lane stands for: imports, calls, context links, cell references, Jira links, infrastructure wiring, foreign keys, field types, service usage. | prototype-findings L49 |  |
| Level of detail | Rendering that adapts to zoom so labels stay readable and the overview shows names only. | prototype-findings L72-76 |  |
| Node | A map level: a file in code, a ticket, a workload, a table, a message or a DTO. | prototype-findings L46 | theme: machine |
| Skyline | The order of the towers on a block: the tallest at the back, the shortest at the front. | facade-camera prototype 2026-10-11; STORY-047 |  |
| Sub-district | The level below a district, with its own home inside its district. | prototype-findings L44, L68 |  |
| Tower | An item drawn as a solid building, one floor per line, its silhouette the shape of its code; a text file stands as one tower of its lines. | living-city discovery 2026-10-10; STORY-047; STORY-057 | theme: crate |
| Universe | All projects together on one map. | prototype-findings L41 | The Universe layer in Navigation is the same thing seen as a layer |
| View options | What is drawn and how it is laid out. | prototype-findings L57, L83-89 |  |
| Lens | The definition of a connection that decides which blocks and districts exist and where they stand: folders, imports, calls, co-change, cells or custom. | lenses prototype 2026-10-12; EPIC-010 | Extends View options; towers keep their shape in every lens |
| Outskirts | The district at the edge where code the lens connects to nothing stands. | lenses prototype 2026-10-12; EPIC-010; STORY-015 |  |
| Plaza | The free ground at the centre of a district, the district's cellar below it. | living-city discovery 2026-10-10; STORY-047 |  |
| Highway | The import lanes between two districts, running side by side, one lane per pair of files. | lenses prototype 2026-10-12; EPIC-010; STORY-052 |  |
| Import cycle | Files that import each other in a loop; in the imports lens they share one block, marked. | lenses prototype 2026-10-12; EPIC-010; STORY-054 |  |
| Step | A district of the imports lens raised as a solid box, one level above what it imports. | lenses prototype 2026-10-12; EPIC-010; STORY-054 |  |
| Custom arrangement | Where the engineer moved the blocks and districts of a project in the custom lens; kept outside the project. | lenses prototype 2026-10-12; EPIC-010; STORY-017 |  |
