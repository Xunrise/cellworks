# cellworks — guidelines

Conventions for `cellworks`, in one place. **Each file states the rule as it is now — no dates, no
history, no ticket references.** Why a rule exists and when it changed belongs in
[`../decisions/`](../decisions/README.md); this folder only says what is true today.

| File | Covers |
|---|---|
| [`architecture.md`](architecture.md) | Crate layout and dependencies, public APIs, threads and events, error handling, external tools |
| [`naming.md`](naming.md) | Crate, module, type and event names; domain terms versus theme words |
| [`testing.md`](testing.md) | Test levels, where tests live, naming tests after scenarios, fixtures, measurement tests |

A rule here applies to the whole component. Only rules that are **persistent and non-obvious from
the code** belong here — not the name of one class, not something the next refactor changes.
