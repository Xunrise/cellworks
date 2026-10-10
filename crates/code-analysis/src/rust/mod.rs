//! The Rust files of a project read into map items: functions, types and
//! `use` blocks with their lines, tokens and resolved calls.

mod parse;
mod resolve;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use cellworks_map::{Floor, Item};

use parse::{FileCode, column};
use resolve::{Index, Module, module_of, package_name};

/// Files larger than this are left unread: generated code, not code to walk.
const LARGEST: u64 = 2 * 1024 * 1024;

/// The items of every Rust file among `files`, relative to `root`, keyed by
/// the file. A file that cannot be read has no items.
pub(crate) fn read(root: &Path, files: &[PathBuf]) -> BTreeMap<PathBuf, Vec<Item>> {
    let manifests: BTreeMap<PathBuf, String> = files
        .iter()
        .filter(|file| file.file_name().is_some_and(|name| name == "Cargo.toml"))
        .filter_map(|file| {
            let name = package_name(&std::fs::read_to_string(root.join(file)).ok()?)?;
            Some((file.parent()?.to_path_buf(), name))
        })
        .collect();
    let crates: HashMap<String, PathBuf> = manifests
        .iter()
        .map(|(folder, name)| (name.clone(), folder.clone()))
        .collect();

    let read: Vec<(PathBuf, Module, FileCode)> = files
        .iter()
        .filter(|file| file.extension().is_some_and(|ext| ext == "rs"))
        .filter_map(|file| {
            let full = root.join(file);
            if std::fs::metadata(&full).ok()?.len() > LARGEST {
                return None;
            }
            let source = std::fs::read_to_string(&full).ok()?;
            Some((
                file.clone(),
                module_of(file, &manifests),
                parse::parse(&source),
            ))
        })
        .collect();

    let paths: Vec<Vec<PathBuf>> = read
        .iter()
        .map(|(file, _, code)| item_paths(&root.join(file), code))
        .collect();
    let index = Index::new(
        read.iter()
            .zip(&paths)
            .flat_map(|((_, module, code), paths)| {
                code.items
                    .iter()
                    .zip(paths)
                    .map(move |(item, path)| (module, item, path.as_path()))
            }),
        crates,
    );

    read.iter()
        .zip(&paths)
        .map(|((file, module, code), paths)| {
            let items = code
                .items
                .iter()
                .zip(paths)
                .map(|(item, path)| {
                    let mut floors: Vec<Floor> = (item.start..=item.end)
                        .filter_map(|row| floor(code, row))
                        .collect();
                    for (row, call) in &item.calls {
                        if let Some(target) = index.resolve(module, code, item, call)
                            && let Some(floor) = floors.get_mut(row - item.start)
                            && !floor.calls.contains(target)
                        {
                            floor.calls.push(target.clone());
                        }
                    }
                    Item {
                        name: item.name.clone(),
                        path: path.clone(),
                        kind: item.kind,
                        tests: item.tests,
                        floors,
                        area: egui::Rect::NOTHING,
                    }
                })
                .collect();
            (file.clone(), items)
        })
        .collect()
}

/// The map path of each item of `code`: the file's path joined with the
/// item's qualified name, and the first line as well when two items of the
/// file would otherwise share it.
fn item_paths(file: &Path, code: &FileCode) -> Vec<PathBuf> {
    let mut taken = BTreeMap::<PathBuf, usize>::new();
    code.items
        .iter()
        .map(|item| {
            let path = file.join(item.qualified().join("::"));
            let seen = taken.entry(path.clone()).or_default();
            *seen += 1;
            if *seen == 1 {
                path
            } else {
                PathBuf::from(format!("{}@{}", path.display(), item.start + 1))
            }
        })
        .collect()
}

fn floor(code: &FileCode, row: usize) -> Option<Floor> {
    let text = code.lines.get(row)?;
    let trimmed = text.trim_end();
    let indent = column(text, text.len() - text.trim_start().len());
    Some(Floor {
        number: row + 1,
        text: text.clone(),
        indent: if trimmed.is_empty() { 0 } else { indent },
        width: column(trimmed, trimmed.len()),
        tokens: code.tokens.get(row).cloned().unwrap_or_default(),
        calls: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use cellworks_map::ItemKind;

    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tree = tempfile::tempdir().expect("a temporary directory");
        for (file, source) in files {
            let path = tree.path().join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, source).expect("write the file");
        }
        tree
    }

    #[test]
    fn every_rust_file_yields_its_items_with_floors_and_resolved_calls() {
        let tree = project(&[
            ("Cargo.toml", "[package]\nname = \"shop\"\n"),
            (
                "src/pricing.rs",
                "use crate::money::round;\n\nfn discount(p: u32) -> u32 {\n    round(p)\n}\n",
            ),
            ("src/money.rs", "pub fn round(p: u32) -> u32 { p }\n"),
            ("README.md", "# shop\n"),
        ]);
        let files: Vec<PathBuf> = ["Cargo.toml", "src/pricing.rs", "src/money.rs", "README.md"]
            .iter()
            .map(PathBuf::from)
            .collect();

        let items = read(tree.path(), &files);

        assert_eq!(items.len(), 2);
        let pricing = &items[Path::new("src/pricing.rs")];
        let kinds: Vec<_> = pricing.iter().map(|i| (i.name.as_str(), i.kind)).collect();
        assert_eq!(
            kinds,
            [("use", ItemKind::Uses), ("discount", ItemKind::Function)]
        );
        let discount = &pricing[1];
        assert_eq!(discount.path, tree.path().join("src/pricing.rs/discount"));
        assert_eq!(discount.floors.len(), 3);
        assert_eq!(discount.floors[1].number, 4);
        assert_eq!(discount.floors[1].indent, 4);
        assert_eq!(discount.floors[1].width, 12);
        assert_eq!(
            discount.floors[1].calls,
            [tree.path().join("src/money.rs/round")]
        );
    }

    #[test]
    fn two_items_of_one_file_with_one_name_get_their_line_as_well() {
        let code = parse::parse("#[cfg(a)]\nfn f() {}\n#[cfg(b)]\nfn f() {}\n");

        let paths = item_paths(Path::new("/p/a.rs"), &code);

        assert_eq!(
            paths,
            [PathBuf::from("/p/a.rs/f"), PathBuf::from("/p/a.rs/f@4")]
        );
    }
}
