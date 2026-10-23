# Prototype: STORY-019 A tower explains why it was built

## Confirmed behavior

- Selecting a tower dims the rest of the city and colours the tower's floors by the pull request that last wrote them.
- A why panel in a fixed right column (about 420 px) explains the selected tower:
  - its bounded context and that context's description, read from the Jarvis docs (`docs/domain/`); a tower in no
    documented context says "No bounded context";
  - the pull requests that built its floors, each with its share of the tower's floors, largest first, and under
    each pull request the stories its commit names with each story's "so that";
  - the decisions (ADRs) that bind the tower's crate, each with its decision line;
  - a tower whose floors all come from commits naming no story says "Built by commits without a story" and lists
    those commits.
- Stepping inside a tower lists its floors in code order, first line at the top; each floor row shows the line number,
  the code, and the pull request and stories that last wrote it. Selecting a floor shows its commit, subject and each
  story's "so that".
- The data comes from STORY-018's per-line history (`ProjectMap.history`, `LineHistory::of_line`); this story reads
  it and adds nothing to git access.
- The mock showed the panel in both skins; the code has one theme, and skins come with STORY-036.

## Feedback log

- "shouldnt the ADRs be clear what they impact based on the context cell they are in?" → ADRs reach towers through
  the bounded context, via the Jarvis scope on the decisions index line, instead of crate names.
- "alrihgt do 1 but make sure that Epic-012 does actually implement that" → story text local only now; EPIC-012
  asked to provide story text by id.

## Rejected

- A `## Binds` section of crate names in every ADR, a separate mapping file, and guessing crates from ADR text: the
  operator wants an ADR's reach to follow the context cell it belongs to.
- A `gh` issue lookup in this story: duplicates EPIC-012's story source.
- Crediting each story with a split share (1/n of each floor across the stories its commit names): hides which pull
  request did the work. Declined by the operator.
- Crediting every named story with all floors of its commit: shares add to more than 100 %. Declined by the operator.
- A floating card next to the tower, and a bottom drawer, for the why panel: declined in favour of the right column.
- An egui prototype before freezing: the operator chose to freeze from the Claude Design mock.

## Edge cases

- Epic pull requests name several stories (for example `floor_z` in `crates/map/src/city.rs`: 7 of 9 floors from #68
  naming 11 stories, 2 from #80 naming 7): the pull request carries the share; its stories are listed under it.
- A tower with floors from commits without a pull request (before the Factory flow): those floors are grouped as
  "Commits without a pull request" with their subjects.
- Uncommitted floors: counted as "Not committed yet" in the shares.
- A tower whose crate no ADR names: the decisions section says none bind it.
- Story text ("so that") not available locally: the story id is shown alone.
- On the cellworks repository no Rust tower is built only by commits without a story; the mock used a TypeScript
  file under `docs_website/` (from #2) for that case.

## Decisions

- Q: How should a tower credit stories when an epic pull request names many? → A: Per pull request, with its stories
  nested under it.
- Q: Where does the why panel sit? → A: A fixed right column.
- Q: Freeze from the mock or build an egui prototype first? → A: Freeze from the mock.
- Q: How does a tower find the ADRs that bind it? → A: Through its bounded context: the ADR's scope on its
  `docs/arc42/09-decisions.md` index line (the Jarvis format), not crate names in each ADR, a mapping file or a guess.
- Q: Where do story titles and "so that" come from? → A: Local `sdlc/*/issue.md` only for now, the id alone
  otherwise; EPIC-012 must provide story text by id through a public Sources API, which the panel then reads.
- Q: When to freeze, given that STORY-018 is not in `test` yet? → A: Freeze now; STORY-018 is a prerequisite.

## Evidence

- Claude Design canvas "Cellworks mind palace mockups" (https://claude.ai/artifact/5H97XvynwMnLeW5neamMJ5),
  page "Why it was built (v10)": renderer `project/City10.dc.html`, boards `X1-why` to `X6-inside`, data
  `project/cw-history.json` (real per-line history of this repository from the STORY-018 prototype) and
  `project/cw-city.json` (towers, stories with "so that" from the GitHub issues, ADR-0001 to ADR-0008 with the crates
  they bind, context descriptions from `docs/domain/`). The renderer logic was checked headlessly with node on the
  real data.
- Data path: branch `proto/line-history` (STORY-018, frozen, issue #40), `crates/code-analysis/src/history.rs` and
  `examples/history.rs`.
- Branch `proto/why-built`, cut from `origin/test`.
