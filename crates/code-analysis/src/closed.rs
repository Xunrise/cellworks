//! Which ignored folders stay closed when ignored files are shown: a folder
//! with a well-known heavy name, or one holding more files than the limit,
//! unless it holds a file the map shows with ignored files off.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Folder names that stay closed whatever their size: dependency, build output
/// and cache folders.
const HEAVY: &[&str] = &[
    "node_modules",
    "target",
    ".venv",
    "venv",
    "dist",
    "build",
    ".gradle",
    "__pycache__",
];

/// The large-folder limit: a folder holding more files than this stays closed.
const LIMIT: usize = 1000;

/// `all` with every closed folder in place of the files inside it, and each
/// closed folder with the number of files it holds. Paths are relative to the
/// project root; `visible` are the files shown with ignored files off. Only
/// the outermost closed folder counts: what is inside it is not looked at.
pub(crate) fn close(
    all: Vec<PathBuf>,
    visible: &BTreeSet<PathBuf>,
) -> (Vec<PathBuf>, BTreeMap<PathBuf, usize>) {
    let mut counts: BTreeMap<&Path, usize> = BTreeMap::new();
    for file in &all {
        for folder in folders_of(file) {
            *counts.entry(folder).or_default() += 1;
        }
    }
    let shown: BTreeSet<&Path> = visible.iter().flat_map(|file| folders_of(file)).collect();

    let mut closed = BTreeMap::new();
    let mut kept = Vec::new();
    for file in &all {
        let closing = folders_of(file).find(|folder| {
            !shown.contains(folder)
                && (is_heavy(folder) || counts.get(folder).is_some_and(|n| *n > LIMIT))
        });
        match closing {
            Some(folder) => {
                closed.insert(folder.to_path_buf(), counts[folder]);
            }
            None => kept.push(file.clone()),
        }
    }
    kept.extend(closed.keys().cloned());
    (kept, closed)
}

/// The folders on `file`'s way down, outermost first.
fn folders_of(file: &Path) -> impl Iterator<Item = &Path> {
    let mut folders: Vec<&Path> = file
        .ancestors()
        .skip(1)
        .filter(|folder| !folder.as_os_str().is_empty())
        .collect();
    folders.reverse();
    folders.into_iter()
}

fn is_heavy(folder: &Path) -> bool {
    folder
        .file_name()
        .is_some_and(|name| HEAVY.iter().any(|heavy| name == *heavy))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_heavy_name_closes_the_outermost_folder_and_keeps_the_rest() {
        let all = paths(&[
            "src/a.rs",
            "node_modules/x/index.js",
            "node_modules/y.js",
            "logs/a.log",
            "logs/target/b",
        ]);
        let visible = paths(&["src/a.rs"]).into_iter().collect();

        let (mut kept, closed) = close(all, &visible);
        kept.sort();

        assert_eq!(
            kept,
            paths(&["logs/a.log", "logs/target", "node_modules", "src/a.rs"])
        );
        assert_eq!(closed[Path::new("node_modules")], 2);
        assert_eq!(closed[Path::new("logs/target")], 1);
    }

    #[test]
    fn a_folder_over_the_limit_closes_and_one_at_it_does_not() {
        for (count, closes) in [(LIMIT, false), (LIMIT + 1, true)] {
            let all: Vec<PathBuf> = (0..count)
                .map(|i| PathBuf::from(format!("logs/{i}")))
                .collect();

            let (kept, closed) = close(all, &BTreeSet::new());

            assert_eq!(closed.contains_key(Path::new("logs")), closes, "{count}");
            assert_eq!(kept.len(), if closes { 1 } else { count });
        }
    }

    #[test]
    fn a_folder_holding_a_visible_file_never_closes() {
        let all = paths(&["build/plan.rs", "build/out.o"]);
        let visible = paths(&["build/plan.rs"]).into_iter().collect();

        let (kept, closed) = close(all.clone(), &visible);

        assert_eq!(kept, all);
        assert!(closed.is_empty());
    }
}
