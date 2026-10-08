# ADR-0002: One crate per bounded context in a Cargo workspace

## Status
Accepted

## Date
2026-10-08

## Context
The domain model has eight bounded contexts (Workspace, Map, CodeAnalysis, DomainKnowledge, Sources, AgentSessions,
Navigation, Editing) with directed relationships in the [context map](../domain/context-map.md). The prototype was a
single crate whose `main.rs` grew to about 5,000 lines mixing drawing, input, navigation and session handling.
Extensibility is a top-three quality goal: new project types must plug into the Map without touching navigation or
rendering.

## Decision
We will structure the code as a Cargo workspace with one library crate per bounded context and a thin `cellworks`
binary crate that wires them together. Crate dependencies follow the context map's upstream → downstream directions.

## Consequences

### Positive
- The compiler enforces context boundaries: a downstream crate cannot be imported by its upstream.
- Incremental builds recompile only the touched context and its dependents.
- Each context can be tested in isolation with its own fast unit tests.

### Negative
- More `Cargo.toml` files and public interfaces to maintain than a single crate.
- Moving a concept between contexts means moving it between crates.
- A dependency cycle the context map does not show must be resolved by a shared type in the upstream crate, not by a
  quick import.

### Neutral
- The release workflow builds the binary crate; workspace-wide `cargo test` runs all crates.

## Alternatives considered
- **Single crate with one module per context**: simplest to start. Rejected because boundaries would be kept by
  convention only, which is how the prototype's `main.rs` grew.
- **Crates by technical layer** (ui, model, io): rejected because it cuts across the bounded contexts and spreads each
  context over several crates.

## References
- Domain: [context map](../domain/context-map.md)
- arc42: [04 Solution strategy](../arc42/04-solution-strategy.md)
- Related: [ADR-0001](0001-egui-as-ui-framework.md)
