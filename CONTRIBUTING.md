# Contributing to Cellworks

## Setup

Rust 1.85 or later on the stable toolchain, macOS on Apple silicon for the app bundle (the code also builds on
Linux). `git` must be on PATH; `claude`, `gh`, `nvim` and `kubectl` are needed only for the features that use them.

```sh
cargo run                  # opens the window
cellworks --version        # prints the version and exits
```

## Checks

Every change passes the same commands as the `rust` job in `.github/workflows/ci.yml`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
actionlint                 # when a workflow changed
shellcheck install.sh      # when the installer changed
```

Tests never open a window: the UI is driven headlessly through `egui_kittest`. Measurement and rendering tests are
`#[ignore]`d and run with `cargo test --workspace --release -- --ignored --nocapture`.

## Layout

- `src/` — the `cellworks` binary: the window, the frame loop, the command line, the updater
- `crates/<name>/` — one library crate per bounded context: `workspace`, `map`, `navigation`, `code-analysis`,
  `domain-knowledge`, `sources`, `agent-sessions`, `editing`
- `tests/` — acceptance tests, one file per story
- `demo/` — the stakeholder demo world (`demo/setup.sh`) and its guided tour (`CELLWORKS_DEMO=1`)
- `site/` — the landing page, deployed to GitHub Pages by `.github/workflows/pages.yml`
- `packaging/homebrew/` — the Homebrew cask template
- `docs/` — arc42, ADRs, the domain model and per-component conventions; `CLAUDE.md` is the short version

## Conventions

- Branches `<type>/<slug>` (`feat/`, `fix/`, `chore/`, `docs/`), commits in Conventional Commits, pull requests into
  `main`, squashed on merge.
- Identifiers use the glossary's neutral terms (District, Group, Node, Item, Lane, Session); the Warhammer 40k words
  are display theme only.
- Background work runs on `std::thread` and reports to the UI thread over `std::sync::mpsc`; there is no async runtime.

## Screenshots

The images in `docs/assets/` are rendered headlessly from the demo world:

```sh
TMPDIR=$(mktemp -d /tmp/cw.XXXX)/ cargo test --release --test demo_tour readme_shots -- --ignored --nocapture
for f in docs/assets/*.png; do sips -c 984 1584 "$f"; done   # trim the harness's edge
```

`demo/setup.sh` runs `npm install` once for the sample shop; point `CELLWORKS_NPM_CACHE` at a warm npm cache to stay
offline.

## Releases

1. Bump `version` in `Cargo.toml` and add `docs/release-notes/<version>.md`.
2. Merge to `main`, then push the tag `v<version>`.

`.github/workflows/release.yml` builds `Cellworks.app` with `scripts/package-app.sh`, publishes
`cellworks-app-<version>.zip` and `manifest.json` (which the in-app updater and `install.sh` read) as a GitHub release,
and, when the `HOMEBREW_TAP_TOKEN` secret is set, updates `Casks/cellworks.rb` in
[Xunrise/homebrew-tap](https://github.com/Xunrise/homebrew-tap).
