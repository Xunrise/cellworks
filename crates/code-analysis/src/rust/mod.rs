//! The Rust files of a project read into map items: functions, types,
//! constants and `use` blocks with their lines, tokens and resolved calls,
//! and what each declares: a method is part of its type wherever in the
//! project the type is written, and a type implements the traits of every
//! `impl` block for it.

mod parse;
mod resolve;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use cellworks_map::{Declaration, Declares, Floor, Item, ItemKind};

use crate::items;
use parse::{Call, FileCode, ParsedItem};
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
    let mut index = Index::new(
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

    for (_, module, code) in &read {
        index.add_uses(module, code);
    }

    let mut implements: HashMap<&PathBuf, Vec<String>> = HashMap::new();
    for ((_, module, code), paths) in read.iter().zip(&paths) {
        for found in &code.impls {
            if let Some(ty) = type_path(&index, module, code, paths, &found.module, &found.owner) {
                implements.entry(ty).or_default().push(found.name.clone());
            }
        }
    }

    read.iter()
        .zip(&paths)
        .map(|((file, module, code), paths)| {
            let items = code
                .items
                .iter()
                .zip(paths)
                .map(|(item, path)| {
                    let mut floors: Vec<Floor> = (item.start..=item.end)
                        .filter_map(|row| items::floor(&code.lines, &code.tokens, row))
                        .collect();
                    if item.kind == cellworks_map::ItemKind::Uses {
                        let lines = item.start..=item.end;
                        for import in code
                            .imports
                            .iter()
                            .filter(|i| i.module == item.module && lines.contains(&i.row))
                        {
                            let named = Call::Path(import.path.clone());
                            if let Some(target) = index.resolve(module, code, item, &named)
                                && let Some(floor) = floors.get_mut(import.row - item.start)
                                && !floor.imports.contains(target)
                            {
                                floor.imports.push(target.clone());
                            }
                        }
                    }
                    for (row, call) in &item.calls {
                        if let Some(target) = index.resolve(module, code, item, call)
                            && let Some(floor) = floors.get_mut(row - item.start)
                            && !floor.calls.contains(target)
                        {
                            floor.calls.push(target.clone());
                        }
                    }
                    let ty = |name: &str| {
                        type_path(&index, module, code, paths, &item.module, name)
                            .filter(|found| *found != path)
                    };
                    let entry = item.name == "main"
                        && item.kind == ItemKind::Function
                        && item.module.is_empty()
                        && item.owner.is_none()
                        && module.path.is_empty();
                    let declaration = Declaration {
                        declares: if entry {
                            Declares::EntryPoint
                        } else {
                            item.declares.clone()
                        },
                        public: item.public,
                        part_of: item.owner.as_deref().and_then(ty).cloned(),
                        implements: implements.get(path).cloned().unwrap_or_default(),
                        holds: item
                            .fields
                            .iter()
                            .filter_map(|(row, name)| Some((row - item.start, ty(name)?.clone())))
                            .collect(),
                    };
                    Item {
                        name: item.name.clone(),
                        path: path.clone(),
                        kind: item.kind,
                        tests: item.tests,
                        floors,
                        area: egui::Rect::NOTHING,
                        base: 0.0,
                        declaration,
                    }
                })
                .collect();
            (file.clone(), items)
        })
        .collect()
}

/// The path of the type `name` stands for, written in the module `inner` of
/// the file at `module` whose items have `paths`: resolved through the
/// module tree as a call is, else a type of that name in the same file.
fn type_path<'a>(
    index: &'a Index,
    module: &Module,
    code: &FileCode,
    paths: &'a [PathBuf],
    inner: &[String],
    name: &str,
) -> Option<&'a PathBuf> {
    let written = Call::Path(vec![name.to_owned()]);
    let is_type = |item: &ParsedItem| item.kind == ItemKind::Type && item.owner.is_none();
    index
        .resolve_in(module, code, inner, None, &written)
        .or_else(|| {
            code.items
                .iter()
                .zip(paths)
                .find(|(item, _)| is_type(item) && item.name == name)
                .map(|(_, path)| path)
        })
}

/// The map path of each item of `code`: the file's path joined with the
/// item's qualified name, and the first line as well when two items of the
/// file would otherwise share it.
fn item_paths(file: &Path, code: &FileCode) -> Vec<PathBuf> {
    items::unique_paths(
        file,
        code.items
            .iter()
            .map(|item| (item.qualified().join("::"), item.start)),
    )
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
    fn each_use_line_imports_the_items_it_names() {
        let tree = project(&[
            ("Cargo.toml", "[package]\nname = \"shop\"\n"),
            (
                "src/pricing.rs",
                "use crate::money::round;\nuse crate::money::{Money, unknown};\n\nfn f() {}\n",
            ),
            ("src/money.rs", "pub fn round() {}\npub struct Money;\n"),
        ]);
        let files: Vec<PathBuf> = ["Cargo.toml", "src/pricing.rs", "src/money.rs"]
            .iter()
            .map(PathBuf::from)
            .collect();

        let items = read(tree.path(), &files);

        let uses = &items[Path::new("src/pricing.rs")][0];
        assert_eq!(uses.kind, ItemKind::Uses);
        assert_eq!(
            uses.floors[0].imports,
            [tree.path().join("src/money.rs/round")]
        );
        assert_eq!(
            uses.floors[1].imports,
            [tree.path().join("src/money.rs/Money")]
        );
        assert!(uses.floors.iter().all(|floor| floor.calls.is_empty()));
    }

    #[test]
    fn a_method_is_part_of_its_type_and_the_type_implements_its_traits_across_files() {
        let tree = project(&[
            ("Cargo.toml", "[package]\nname = \"shop\"\n"),
            ("src/main.rs", "mod cart;\nmod show;\nfn main() {}\n"),
            (
                "src/cart.rs",
                "pub struct Cart { price: Price }\npub struct Price;\n\
                 impl Cart { pub fn add(&self) {} }\nimpl Default for Cart { fn default() {} }\n",
            ),
            (
                "src/show.rs",
                "use crate::cart::Cart;\nimpl std::fmt::Display for Cart { fn fmt(&self) {} }\n\
                 impl Cart { fn total(&self) {} }\nfn helper() {}\n",
            ),
        ]);
        let files: Vec<PathBuf> = ["Cargo.toml", "src/main.rs", "src/cart.rs", "src/show.rs"]
            .iter()
            .map(PathBuf::from)
            .collect();

        let items = read(tree.path(), &files);

        let declared = |file: &str, name: &str| {
            items[Path::new(file)]
                .iter()
                .find(|item| item.name == name)
                .map(|item| item.declaration.clone())
                .unwrap_or_else(|| panic!("no {name} in {file}"))
        };
        let cart = tree.path().join("src/cart.rs/Cart");
        assert_eq!(declared("src/cart.rs", "add").part_of.as_ref(), Some(&cart));
        assert_eq!(
            declared("src/show.rs", "total").part_of.as_ref(),
            Some(&cart)
        );
        assert_eq!(declared("src/show.rs", "fmt").part_of.as_ref(), Some(&cart));
        assert_eq!(declared("src/show.rs", "helper").part_of, None);
        assert_eq!(
            declared("src/cart.rs", "Cart").implements,
            ["Default", "Display"]
        );
        assert_eq!(
            declared("src/cart.rs", "Cart").holds,
            [(0, tree.path().join("src/cart.rs/Price"))]
        );
        assert_eq!(
            declared("src/main.rs", "main").declares,
            Declares::EntryPoint
        );
        assert_eq!(declared("src/show.rs", "helper").declares, Declares::Plain);
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
