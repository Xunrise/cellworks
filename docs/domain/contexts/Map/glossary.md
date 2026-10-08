# Map — glossary

Terms specific to this context. Shared terms and cross-context collisions are in
[`../../glossary.md`](../../glossary.md).

| Term | Definition | Source | Notes |
|------|------------|--------|-------|
| District | A node of the Project layer; for code projects the top folder below the common prefix, in Jira a component, in infrastructure an environment. | prototype-findings L43, L148 | Cut by directories for now; the district strategy is to become swappable |
| District strategy | The rule that decides how a project is cut into districts; directories for now. | prototype-findings product owner 2026-10-08 |  |
| Folder view | Showing only one group, its members packed in boxes by next subfolder, with outside connections on a ring as lanes. | prototype-findings L50, L77-81 | also called scope |
| Group | A map level: a folder in code, an epic in Jira, a namespace or Terraform group in infrastructure, a database in data. | prototype-findings L45 | theme: factory |
| Item | A map level inside a node: a function in code, a sub-task, a column, a field. | prototype-findings L47 | theme: crate |
| Lane kind | What a lane stands for: imports, calls, context links, cell references, Jira links, infrastructure wiring, foreign keys, field types, service usage. | prototype-findings L49 |  |
| Level of detail | Rendering that adapts to zoom so labels stay readable and the overview shows names only. | prototype-findings L72-76 |  |
| Node | A map level: a file in code, a ticket, a workload, a table, a message or a DTO. | prototype-findings L46 | theme: machine |
| Sub-district | The level below a district, with its own home inside its district. | prototype-findings L44, L68 |  |
| Universe | All projects together on one map. | prototype-findings L41 | The Universe layer in Navigation is the same thing seen as a layer |
| View options | What is drawn and how it is laid out. | prototype-findings L57, L83-89 |  |
