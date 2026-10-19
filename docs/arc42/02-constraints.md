> Status: draft

# 2. Architecture Constraints

## 2.1 Technical

| Constraint | Background |
|---|---|
| Rust, as a native desktop application | Performance on large repositories and personal daily use; the UI framework is chosen for what works best on macOS: eframe/egui ([ADR-0001](../adr/0001-egui-as-ui-framework.md)). |
| macOS is the primary platform | Personal daily use. The release pipeline also builds Linux; keeping it is a goal, not a constraint. |
| No web front end, no localhost HTTP server | Rejected during prototyping: a local server that spawns shells is off the table. |
| Read-only infrastructure access | Cluster access uses `kubectl get` only; Terraform is read from files. |
| Secrets never on a command line | Tokens and credentials travel through stdin or private temp files, never through process arguments. |

## 2.2 Organizational

| Constraint | Background |
|---|---|
| Every push to `main` is released | `.github/workflows/deliver.yml` publishes a release per push (`scripts/version.sh` picks the version); patch versions bump automatically, minor and major via `Cargo.toml`. |
| Every change is motivated by a story | Gherkin stories for behaviour, chore stories otherwise; tracked as GitHub issues and PRs on `squer-solutions/cellworks`. |

## 2.3 Conventions

Project-wide conventions live in [`CLAUDE.md`](../../CLAUDE.md); per-component conventions under
[`docs/services/`](../services/README.md).
