# 0001. Vanilla TypeScript without a UI framework

Status: Accepted

## Context
The shop has three small views and little state. A framework would add a build dependency and
concepts the team does not need yet.

## Decision
Use TypeScript with direct DOM rendering, a small typed event bus for state changes, and Vite for tooling.

## Consequences
- Fewer dependencies and a fast build.
- Views re-render with `innerHTML`; if views grow complex, revisit this decision.
