# Map — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| Bay | One section of a document standing on the roof of its library hall, side by side with the others, as tall as its section and at most twelve wide; its lines show from afar. | architecture prototype 2026-10-13; STORY-067 |  |
| Billboard | An image: a panel on a post on its block, which holds no item. | architecture prototype 2026-10-13; STORY-067 |  |
| Block | A node drawn as a plot of ground in the city, its buildings standing on it like a town in a skyline, its centrepiece in the middle. It holds its buildings whole, wherever their parts are written. Blocks whose code reaches each other stand together; a folder's documents gather in one quarter. | living-city discovery 2026-10-10; STORY-047; STORY-064; STORY-068 |  |
| Building | The items of a block that stand together, chosen by the map from what each item declares: a tower, a skyscraper, a terrace or a monument. | architecture prototype 2026-10-13; STORY-064 |  |
| Cabinet | One top-level key of a configuration file: a square box with each level nested under the key stacked on it, each three quarters the side of the one below. | architecture prototype 2026-10-13; STORY-067 |  |
| Cable | A dashed line from the tip of a mast to the roof of the building it calls, shown on request; buildings in front of part of it hide that part. Above the ground only the cables of the city, underground only those of the masts hanging in the cellar. | architecture prototype 2026-10-13; STORY-066 |  |
| Cellar | The ground beneath a block or a district's plaza where the items belonging to the tests hang, faint unless the view is underground. A district's own blocks of only tests lie under its plaza; deeper ones stay in their folder's outline. | facade-camera prototype 2026-10-11; STORY-048; STORY-056; STORY-068 |  |
| Centrepiece | The building of a block with the most doors: it stands first, in the middle of its block, with open ground around it that grows with its doors. | architecture prototype 2026-10-13; STORY-068 |  |
| Courthouse | The building of a feature file: its header, the feature and background, a hall on a plinth, with a column in front of it for each scenario, standing on the plinth too. | architecture prototype 2026-10-13; STORY-067 |  |
| Courtyard | A low walled yard behind a building that has private parts. | architecture prototype 2026-10-13; STORY-065 |  |
| Crown | A two-step cap on top of a building for each interface its type implements, in that interface's colour; every type implementing the same interface wears the same crown. | architecture prototype 2026-10-13; STORY-064 |  |
| Custom arrangement | Where the engineer moved the blocks and districts of a project in the custom lens; kept outside the project. | lenses prototype 2026-10-12; EPIC-010; STORY-017 |  |
| Customs house | The building of a contract between services: a square house with an upper storey, straddling a dashed border, a road out through each gate. | architecture prototype 2026-10-13; STORY-067 |  |
| Depot | The building of a build file, a Dockerfile, a Makefile or a script: a wide shed with a sawtooth roof and loading docks along its front. | architecture prototype 2026-10-13; STORY-067 |  |
| District | A node of the Project layer; for code projects the top folder below the common prefix, in Jira a component, in infrastructure an environment. | prototype-findings L43, L148 | Cut by directories for now; the district strategy is to become swappable |
| District strategy | The rule that decides how a project is cut into districts; directories for now. | prototype-findings product owner 2026-10-08 |  |
| Door | A lit door on the front of a building's base for each public part, a method of a trait implementation counting as public; at most sixteen. | architecture prototype 2026-10-13; STORY-065 |  |
| Facade | The front face of a tower, where its lines are written; a turned camera faces one facade. | facade-camera prototype 2026-10-11; STORY-045 |  |
| Floor | One line of an item, drawn as one storey of its tower; the first line is the top floor. | living-city discovery 2026-10-10; STORY-047 |  |
| Folder outline | A folder drawn in the folders lens as a dashed outline inside the outline of the folder holding it; a folder holding only one folder and no file is one outline named after both (`src/main/kotlin`). Its name is written on its front once the outline has room for it; clicking the name or the folder's open ground selects the folder. | nested-folders prototype 2026-10-12; STORY-056; STORY-068 | was Folder ring until STORY-068 |
| Folder view | Showing only one group, its members packed in boxes by next subfolder, with outside connections on a ring as lanes. | prototype-findings L50, L77-81 | also called scope |
| Forecourt | A paved court in front of a building called from three places or more outside the tests, deeper the more places call it. | architecture prototype 2026-10-13; STORY-065 |  |
| Ghost | The building of a generated file, a lockfile or a snapshot: a dashed, dim outline with nothing inside. | architecture prototype 2026-10-13; STORY-067 |  |
| Group | A map level: a folder in code, an epic in Jira, a namespace or Terraform group in infrastructure, a database in data. | prototype-findings L45 | theme: factory |
| Highway | The import lanes between two districts, running side by side, one lane per pair of files. | lenses prototype 2026-10-12; EPIC-010; STORY-052 |  |
| Import cycle | Files that import each other in a loop; in the imports lens they share one block, marked. | lenses prototype 2026-10-12; EPIC-010; STORY-054 |  |
| Item | A map level inside a node: a function in code, a sub-task, a column, a field. | prototype-findings L47 | theme: crate |
| Lane kind | What a lane stands for: imports, calls, context links, cell references, Jira links, infrastructure wiring, foreign keys, field types, service usage. | prototype-findings L49 |  |
| Lens | The definition of a connection that decides which blocks and districts exist and where they stand: folders, imports, calls, co-change, cells or custom. | lenses prototype 2026-10-12; EPIC-010 | Extends View options; towers keep their shape in every lens |
| Level of detail | Rendering that adapts to zoom so labels stay readable and the overview shows names only. | prototype-findings L72-76 |  |
| Library hall | The building of a document: a low hall with a colonnade along its front, drawn by the map rather than standing for an item, with a bay on its roof for each section. | architecture prototype 2026-10-13; STORY-067 |  |
| Mast | A mast on a building's roof for each other file its code calls, at most eight; a test's masts hang upside down below it and show only underground, and the masts above the ground only above it. | architecture prototype 2026-10-13; STORY-065 |  |
| Monument | The building of a constant: a plinth, an obelisk and its cap, its lines shown only close enough to read them. | architecture prototype 2026-10-13; STORY-064 |  |
| Node | A map level: a file in code, a ticket, a workload, a table, a message or a DTO. | prototype-findings L46 | theme: machine |
| Outline | The ground of a block, folder or district drawn round what it holds: the convex hull of its buildings, blocks or folders, a little way out, its corners cut round. Blocks are drawn by their outlines in every lens, folders and districts in the folders lens. | architecture prototype 2026-10-13; STORY-068 |  |
| Outskirts | The district at the edge where code the lens connects to nothing stands. | lenses prototype 2026-10-12; EPIC-010; STORY-015 |  |
| Plant | The building of a Terraform file: its largest block the body, a chimney on its roof for each further block, and pipes running out along the ground. | architecture prototype 2026-10-13; STORY-067 |  |
| Plaza | The free ground at the centre of a district, the district's cellar below it. It is placed first and the district's ground reaches as far on every side of it. | living-city discovery 2026-10-10; STORY-047; STORY-068 |  |
| Pull request share | The floors of a tower last written by one pull request, as a share of all its floors. | STORY-019 freeze 2026-10-19 |  |
| Skybridge | A bridge from a type's building to the building of each type one of its fields holds on the same block, at the height of that field, below the other's roof, from wall to wall; buildings in front of part of it hide that part. No skybridge spans two blocks; pointing at one says which type holds which, through which field. | architecture prototype 2026-10-13; STORY-065 |  |
| Skyline | How buildings, blocks, folders and districts stand towards each other: one shorter than the one it stands beside stands in front of it, a taller one behind it. | facade-camera prototype 2026-10-11; STORY-047; STORY-064; STORY-068 |  |
| Skyscraper | The building of an item with parts, a type with its methods: the item is its base and each part a tier stepping back above it, the largest lowest. | architecture prototype 2026-10-13; STORY-064 |  |
| Slab | The building of a schema: a dashed box sunk below the ground, its top at ground level. | architecture prototype 2026-10-13; STORY-067 |  |
| Spire | A shaft with a haloed beacon on top of an entry point's tower, in sight from far out. | architecture prototype 2026-10-13; STORY-064 |  |
| Step | A district of the imports lens raised as a solid box, one level above what it imports. | lenses prototype 2026-10-12; EPIC-010; STORY-054 |  |
| Sub-district | The level below a district, with its own home inside its district. | prototype-findings L44, L68 |  |
| Switchboard | The building of a configuration file: a cabinet for each top-level key, with dashed wires on the ground between them. | architecture prototype 2026-10-13; STORY-067 |  |
| Terrace | The building of an enum without methods: a row house per variant, with an annex behind each variant that carries data. | architecture prototype 2026-10-13; STORY-064 |  |
| Tier | One part of a skyscraper, drawn as a box of its own with its floors, standing on the part below it. | architecture prototype 2026-10-13; STORY-064 |  |
| Tower | An item drawn as a solid building, one floor per line, its silhouette the shape of its code; a text file stands as one tower of its lines. | living-city discovery 2026-10-10; STORY-047; STORY-057 | theme: crate |
| Universe | All projects together on one map. | prototype-findings L41 | The Universe layer in Navigation is the same thing seen as a layer |
| Vault | The building of secrets: a sealed dark box with a light on its front whose lines are never shown, at any distance; an example of the secrets is the same vault in dashed glass. | architecture prototype 2026-10-13; STORY-067 |  |
| View options | What is drawn and how it is laid out. | prototype-findings L57, L83-89 |  |
