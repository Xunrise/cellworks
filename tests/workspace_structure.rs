//! The structure of the workspace, pinned by a check rather than by review
//! attention: the member crates, the `cellworks-*` dependency graph of
//! `docs/arc42/05-building-blocks.md` section 5.2, and the single `version`
//! line the release workflow reads out of the root manifest.
//!
//! A story that needs a new arrow between two crates changes the architecture
//! document and the table below in the same commit, or these tests fail.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use toml::Table;

/// The eight bounded contexts, as directory names under `crates/`.
const CONTEXT_CRATES: [&str; 8] = [
    "agent-sessions",
    "code-analysis",
    "domain-knowledge",
    "editing",
    "map",
    "navigation",
    "sources",
    "workspace",
];

/// The dependency graph of `docs/arc42/05-building-blocks.md` section 5.2:
/// the manifest, the package it declares, and the `cellworks-*` crates it
/// depends on, in sorted order.
///
/// Three relationships of the context map are deliberately absent, because the
/// binary wires them: CodeAnalysis to Workspace, AgentSessions to Workspace and
/// Sources to Navigation. The equality below is what keeps them absent.
const GRAPH: [(&str, &str, &[&str]); 9] = [
    (
        "Cargo.toml",
        "cellworks",
        &[
            "cellworks-agent-sessions",
            "cellworks-code-analysis",
            "cellworks-domain-knowledge",
            "cellworks-map",
            "cellworks-navigation",
            "cellworks-sources",
            "cellworks-workspace",
        ],
    ),
    (
        "crates/agent-sessions/Cargo.toml",
        "cellworks-agent-sessions",
        &["cellworks-map"],
    ),
    (
        "crates/code-analysis/Cargo.toml",
        "cellworks-code-analysis",
        &["cellworks-map"],
    ),
    (
        "crates/domain-knowledge/Cargo.toml",
        "cellworks-domain-knowledge",
        &["cellworks-code-analysis", "cellworks-map"],
    ),
    ("crates/editing/Cargo.toml", "cellworks-editing", &[]),
    (
        "crates/map/Cargo.toml",
        "cellworks-map",
        &["cellworks-workspace"],
    ),
    (
        "crates/navigation/Cargo.toml",
        "cellworks-navigation",
        &["cellworks-editing", "cellworks-map"],
    ),
    (
        "crates/sources/Cargo.toml",
        "cellworks-sources",
        &["cellworks-map", "cellworks-workspace"],
    ),
    ("crates/workspace/Cargo.toml", "cellworks-workspace", &[]),
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn manifest(relative: &str) -> Table {
    let path = repository_root().join(relative);
    read_manifest(&path)
}

fn read_manifest(path: &Path) -> Table {
    let text =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    text.parse::<Table>()
        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn package_name(manifest: &Table) -> &str {
    manifest["package"]["name"]
        .as_str()
        .expect("package.name is a string")
}

/// The `cellworks-*` keys of a manifest's `[dependencies]`, sorted.
fn crate_dependencies(manifest: &Table) -> BTreeSet<String> {
    let Some(dependencies) = manifest.get("dependencies") else {
        return BTreeSet::new();
    };

    dependencies
        .as_table()
        .expect("[dependencies] is a table")
        .keys()
        .filter(|key| key.starts_with("cellworks-"))
        .cloned()
        .collect()
}

#[test]
fn the_workspace_has_exactly_the_eight_context_crates() {
    let directories: BTreeSet<String> = fs::read_dir(repository_root().join("crates"))
        .expect("crates/ exists")
        .map(|entry| entry.expect("a readable directory entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();

    assert_eq!(
        directories,
        CONTEXT_CRATES
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        "crates/ holds exactly one directory per bounded context"
    );

    for context in CONTEXT_CRATES {
        let manifest = manifest(&format!("crates/{context}/Cargo.toml"));
        assert_eq!(package_name(&manifest), format!("cellworks-{context}"));
    }
}

#[test]
fn the_root_package_is_the_cellworks_binary() {
    let root = manifest("Cargo.toml");

    assert_eq!(package_name(&root), "cellworks");
    assert_eq!(
        root["package"]["edition"].as_str(),
        Some("2024"),
        "the workspace is on Rust edition 2024"
    );
    assert!(
        repository_root().join("src/main.rs").is_file(),
        "the root package has a binary at src/main.rs"
    );
}

#[test]
fn dependency_sets_match_the_building_block_view() {
    for (path, package, expected) in GRAPH {
        let manifest = manifest(path);

        assert_eq!(package_name(&manifest), package, "{path} names {package}");
        assert_eq!(
            crate_dependencies(&manifest),
            expected.iter().map(|name| (*name).to_owned()).collect(),
            "{package}: its cellworks-* dependencies are the arrows of \
             docs/arc42/05-building-blocks.md section 5.2"
        );
    }
}

#[test]
fn the_crate_graph_has_no_cycle() {
    let mut remaining: HashMap<&str, BTreeSet<&str>> = GRAPH
        .iter()
        .map(|(_, package, dependencies)| (*package, dependencies.iter().copied().collect()))
        .collect();

    let mut sorted: Vec<&str> = Vec::new();
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining
            .iter()
            .filter(|(_, dependencies)| dependencies.is_empty())
            .map(|(package, _)| *package)
            .collect();

        assert!(
            !ready.is_empty(),
            "the crate graph has a cycle among {:?}",
            remaining.keys().collect::<BTreeSet<_>>()
        );

        for package in ready {
            remaining.remove(package);
            for dependencies in remaining.values_mut() {
                dependencies.remove(package);
            }
            sorted.push(package);
        }
    }

    assert_eq!(sorted.len(), GRAPH.len());
}

#[test]
fn inter_crate_dependencies_are_path_only() {
    for (path, package, _) in GRAPH {
        let manifest = manifest(path);
        let Some(dependencies) = manifest.get("dependencies") else {
            continue;
        };

        for (name, entry) in dependencies.as_table().expect("[dependencies] is a table") {
            if !name.starts_with("cellworks-") {
                continue;
            }

            let entry = entry
                .as_table()
                .unwrap_or_else(|| panic!("{package}: {name} is written as an inline table"));
            assert!(
                entry.contains_key("path"),
                "{package}: {name} is declared by path"
            );
            assert!(
                !entry.contains_key("version"),
                "{package}: {name} carries no version requirement — the release \
                 version has one home, the root manifest"
            );
        }
    }
}

#[test]
fn member_crates_are_not_published() {
    for context in CONTEXT_CRATES {
        let manifest = manifest(&format!("crates/{context}/Cargo.toml"));

        assert_eq!(
            manifest["package"]["publish"].as_bool(),
            Some(false),
            "cellworks-{context} sets publish = false"
        );
    }
}

#[test]
fn the_root_manifest_has_one_version_line() {
    // `.github/workflows/release.yml` reads this file line by line: line 28
    // takes the first `^version = "` line and line 62 rewrites it. A second one
    // would stamp the wrong field with no error anywhere.
    let lines: Vec<&str> = include_str!("../Cargo.toml")
        .lines()
        .filter(|line| line.starts_with("version = \""))
        .collect();

    assert_eq!(
        lines.len(),
        1,
        "the root manifest has exactly one line starting a version: {lines:?}"
    );

    let stamped = lines[0]
        .trim_start_matches("version = \"")
        .trim_end_matches('"');
    assert_eq!(
        stamped,
        env!("CARGO_PKG_VERSION"),
        "the version the release workflow stamps is the version the binary reports"
    );
}
