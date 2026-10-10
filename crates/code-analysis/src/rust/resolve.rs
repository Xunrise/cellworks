//! Calls resolved to the item they name, through the module tree rather than
//! by name: `f()` is the `f` in scope where it is called, never every `f` in
//! the project. A call that leads nowhere known (a method on a value of
//! unknown type, the standard library, a macro) is left unresolved.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use super::parse::{Call, FileCode, ParsedItem};

/// Where one file sits in the module tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Module {
    /// The crate's own key: the folder of its `Cargo.toml` for the crate under
    /// `src/`, the file itself for a file compiled on its own (`tests/`,
    /// `src/bin/`, `build.rs`).
    pub krate: PathBuf,
    /// The file's module path inside the crate.
    pub path: Vec<String>,
}

/// Every item of the project by crate and full module path, and the crate
/// each package name stands for.
pub(crate) struct Index {
    items: HashMap<(PathBuf, Vec<String>), PathBuf>,
    crates: HashMap<String, PathBuf>,
    /// Per crate and module, the names its `use` statements bring in, so a
    /// `pub use` can be followed to the item it re-exports.
    uses: HashMap<(PathBuf, Vec<String>), BTreeMap<String, Vec<String>>>,
}

impl Index {
    /// Indexes `items`: for each file, its module and its items with the path
    /// each has on the map. `crates` maps a package's name, as code writes it
    /// (`cellworks_map`), to its crate.
    pub fn new<'a>(
        items: impl IntoIterator<Item = (&'a Module, &'a ParsedItem, &'a Path)>,
        crates: HashMap<String, PathBuf>,
    ) -> Index {
        let mut index = HashMap::new();
        for (module, item, path) in items {
            let mut full = module.path.clone();
            full.extend(item.qualified());
            index
                .entry((module.krate.clone(), full))
                .or_insert_with(|| path.to_path_buf());
        }
        Index {
            items: index,
            crates,
            uses: HashMap::new(),
        }
    }

    /// Records what the `use` statements of the file at `module` bring into
    /// each of its modules, for following re-exports.
    pub fn add_uses(&mut self, module: &Module, code: &FileCode) {
        for (inner, uses) in &code.uses {
            let mut path = module.path.clone();
            path.extend(inner.iter().cloned());
            self.uses
                .entry((module.krate.clone(), path))
                .or_default()
                .extend(uses.names.clone());
        }
    }

    /// The path of the item `call`, made in `item` of a file at `module`,
    /// names.
    pub fn resolve(
        &self,
        module: &Module,
        code: &FileCode,
        item: &ParsedItem,
        call: &Call,
    ) -> Option<&PathBuf> {
        let mut scope = module.path.clone();
        scope.extend(item.module.iter().cloned());
        let written = match call {
            Call::Method(name) => vec!["Self".to_owned(), name.clone()],
            Call::Path(segments) => segments.clone(),
        };
        let written = match (written.first().map(String::as_str), &item.owner) {
            (Some("Self"), Some(owner)) => {
                let mut path = vec!["self".to_owned(), owner.clone()];
                path.extend(written.into_iter().skip(1));
                path
            }
            (Some("Self"), None) => return None,
            _ => written,
        };
        let inner = &item.module;
        self.candidates(&module.krate, &scope, code, inner, &written, 0)
            .into_iter()
            .find_map(|key| self.follow(key, 0))
    }

    /// The item at `key`, or the one a re-export on the way names:
    /// `cellworks_map::ProjectMap` is `graph::ProjectMap` when the crate's
    /// root says `pub use graph::ProjectMap`.
    fn follow(&self, key: (PathBuf, Vec<String>), depth: usize) -> Option<&PathBuf> {
        if let Some(found) = self.items.get(&key) {
            return Some(found);
        }
        // A re-export that names itself in a loop leads nowhere.
        if depth > 8 {
            return None;
        }
        let (krate, path) = key;
        (0..path.len()).find_map(|at| {
            let (module, rest) = path.split_at(at);
            let target = self
                .uses
                .get(&(krate.clone(), module.to_vec()))?
                .get(&rest[0])?;
            let mut written = target.clone();
            written.extend(rest[1..].iter().cloned());
            let (first, after) = written.split_first()?;
            let next = match first.as_str() {
                "crate" => (krate.clone(), after.to_vec()),
                "self" => (krate.clone(), [module, after].concat()),
                "super" => {
                    let up = &module[..module.len().saturating_sub(1)];
                    (krate.clone(), [up, after].concat())
                }
                name => match self.crates.get(name) {
                    Some(other) => (other.clone(), after.to_vec()),
                    // Edition 2018 paths: a sibling module of the re-exporting one.
                    None => (krate.clone(), [module, &written[..]].concat()),
                },
            };
            self.follow(next, depth + 1)
        })
    }

    /// The full paths `written`, in the module `scope`, may stand for, most
    /// likely first. `inner` is the module inside the file whose `use`
    /// statements apply.
    fn candidates(
        &self,
        krate: &Path,
        scope: &[String],
        code: &FileCode,
        inner: &[String],
        written: &[String],
        depth: usize,
    ) -> Vec<(PathBuf, Vec<String>)> {
        let Some((first, rest)) = written.split_first() else {
            return Vec::new();
        };
        // A `use` that names itself in a loop leads nowhere.
        if depth > 8 {
            return Vec::new();
        }
        let joined = |base: &[String], tail: &[String]| {
            let mut path = base.to_vec();
            path.extend(tail.iter().cloned());
            path
        };
        match first.as_str() {
            "crate" => return vec![(krate.to_path_buf(), rest.to_vec())],
            "self" => return vec![(krate.to_path_buf(), joined(scope, rest))],
            "super" => {
                let mut up = scope.to_vec();
                up.pop();
                let mut supers = rest;
                while let Some(("super", tail)) = supers.split_first().map(|(s, t)| (s.as_str(), t))
                {
                    up.pop();
                    supers = tail;
                }
                return vec![(krate.to_path_buf(), joined(&up, supers))];
            }
            _ => {}
        }
        let uses = code.uses.get(inner);
        let mut found = Vec::new();
        if let Some(target) = uses.and_then(|uses| uses.names.get(first)) {
            // A `use` path is read from the module it is written in.
            let resolved = joined(target, rest);
            found.extend(self.candidates(krate, scope, code, inner, &resolved, depth + 1));
        }
        if let Some(other) = self.crates.get(first) {
            found.push((other.clone(), rest.to_vec()));
        }
        found.push((krate.to_path_buf(), joined(scope, written)));
        if rest.is_empty() {
            for glob in uses.map(|uses| uses.globs.as_slice()).unwrap_or_default() {
                let through = joined(glob, written);
                found.extend(self.candidates(krate, scope, code, inner, &through, depth + 1));
            }
        }
        found
    }
}

/// The module `file` is, inside the crate of the nearest `Cargo.toml` above
/// it; `manifests` maps each folder holding one, relative to the root, to its
/// package name. Files outside a crate's `src/` are crates of their own.
pub(crate) fn module_of(file: &Path, manifests: &BTreeMap<PathBuf, String>) -> Module {
    let own = || Module {
        krate: file.to_path_buf(),
        path: Vec::new(),
    };
    let Some(folder) = file
        .ancestors()
        .skip(1)
        .find(|folder| manifests.contains_key(*folder))
    else {
        return own();
    };
    let Ok(inside) = file.strip_prefix(folder.join("src")) else {
        return own();
    };
    if inside.starts_with("bin") {
        return own();
    }
    let mut path: Vec<String> = inside
        .with_extension("")
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    if path.last().is_some_and(|last| last == "mod")
        || (path.len() == 1 && matches!(path[0].as_str(), "lib" | "main"))
    {
        path.pop();
    }
    Module {
        krate: folder.to_path_buf(),
        path,
    }
}

/// The package name a `Cargo.toml` declares, as code writes it: dashes become
/// underscores. Read by hand: only `name` under `[package]` matters here.
pub(crate) fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some(value) = line.strip_prefix("name")
            && let Some(value) = value.trim_start().strip_prefix('=')
        {
            return Some(value.trim().trim_matches('"').replace('-', "_"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;

    #[test]
    fn a_file_is_the_module_its_path_below_src_names() {
        let manifests = BTreeMap::from([(PathBuf::from("crates/map"), "cellworks_map".to_owned())]);
        let module = |file: &str| module_of(Path::new(file), &manifests);

        assert_eq!(module("crates/map/src/lib.rs").path, Vec::<String>::new());
        assert_eq!(module("crates/map/src/ui/mod.rs").path, ["ui"]);
        assert_eq!(module("crates/map/src/a/b.rs").path, ["a", "b"]);
        assert_eq!(
            module("crates/map/src/a/b.rs").krate,
            Path::new("crates/map")
        );
        assert_eq!(
            module("crates/map/tests/x.rs").krate,
            Path::new("crates/map/tests/x.rs")
        );
    }

    #[test]
    fn the_package_name_is_written_with_underscores() {
        let manifest = "[workspace]\nname = \"no\"\n[package]\nname = \"cellworks-map\"\n";

        assert_eq!(package_name(manifest).as_deref(), Some("cellworks_map"));
    }

    /// Two files of one crate, both with a function `round`.
    struct Shop {
        files: Vec<(Module, FileCode, PathBuf)>,
        index: Index,
    }

    fn shop(sources: &[(&str, &str)]) -> Shop {
        let manifests = BTreeMap::from([(PathBuf::from(""), "shop".to_owned())]);
        let files: Vec<(Module, FileCode, PathBuf)> = sources
            .iter()
            .map(|(file, source)| {
                let file = PathBuf::from(file);
                (module_of(&file, &manifests), parse(source), file)
            })
            .collect();
        let paths: Vec<(usize, usize, PathBuf)> = files
            .iter()
            .enumerate()
            .flat_map(|(f, (_, code, file))| {
                code.items
                    .iter()
                    .enumerate()
                    .map(move |(i, item)| (f, i, file.join(item.qualified().join("::"))))
            })
            .collect();
        let mut index = Index::new(
            paths
                .iter()
                .map(|(f, i, path)| (&files[*f].0, &files[*f].1.items[*i], path.as_path())),
            HashMap::new(),
        );
        for (module, code, _) in &files {
            index.add_uses(module, code);
        }
        Shop { files, index }
    }

    impl Shop {
        fn calls_of(&self, file: usize, name: &str) -> Vec<Option<PathBuf>> {
            let (module, code, _) = &self.files[file];
            let item = code
                .items
                .iter()
                .find(|item| item.name == name)
                .expect("the item");
            item.calls
                .iter()
                .map(|(_, call)| self.index.resolve(module, code, item, call).cloned())
                .collect()
        }
    }

    #[test]
    fn a_call_names_the_function_in_scope_not_every_function_of_that_name() {
        let shop = shop(&[
            (
                "src/pricing.rs",
                "use crate::money::round;\nfn total() { round(); helper(); }\nfn helper() {}\n",
            ),
            ("src/money.rs", "pub fn round() {}\n"),
            ("src/tax.rs", "fn round() {}\nfn helper() {}\n"),
        ]);

        assert_eq!(
            shop.calls_of(0, "total"),
            [
                Some(PathBuf::from("src/money.rs/round")),
                Some(PathBuf::from("src/pricing.rs/helper"))
            ]
        );
    }

    #[test]
    fn methods_resolve_through_self_and_the_impl_type() {
        let shop = shop(&[(
            "src/lib.rs",
            "struct Cart;\nstruct Till;\n\
             impl Cart { fn total(&self) { self.count(); Self::fee(); Till::count(); } \
             fn count(&self) {} fn fee() {} }\n\
             impl Till { fn count() {} }\n",
        )]);

        assert_eq!(
            shop.calls_of(0, "total"),
            [
                Some(PathBuf::from("src/lib.rs/Cart::count")),
                Some(PathBuf::from("src/lib.rs/Cart::fee")),
                Some(PathBuf::from("src/lib.rs/Till::count")),
            ]
        );
    }

    #[test]
    fn super_and_glob_imports_reach_the_parent_module() {
        let shop = shop(&[(
            "src/lib.rs",
            "fn discount() {}\nmod tests {\n use super::*;\n fn a() { discount(); super::discount(); }\n}\n",
        )]);

        assert_eq!(
            shop.calls_of(0, "a"),
            [
                Some(PathBuf::from("src/lib.rs/discount")),
                Some(PathBuf::from("src/lib.rs/discount"))
            ]
        );
    }

    #[test]
    fn a_call_through_a_re_export_reaches_the_item_re_exported() {
        let shop = shop(&[
            (
                "src/cart.rs",
                "use crate::Money;\nfn total() { Money::round(); }\n",
            ),
            ("src/lib.rs", "mod money;\npub use money::Money;\n"),
            (
                "src/money.rs",
                "pub struct Money;\nimpl Money { pub fn round() {} }\n",
            ),
        ]);

        assert_eq!(
            shop.calls_of(0, "total"),
            [Some(PathBuf::from("src/money.rs/Money::round"))]
        );
    }

    #[test]
    fn a_call_to_something_unknown_stays_unresolved() {
        let shop = shop(&[(
            "src/lib.rs",
            "fn a(v: Vec<u8>) { v.len(); Vec::new(); println!(\"x\"); }\nfn len() {}\n",
        )]);

        assert_eq!(shop.calls_of(0, "a"), [None]);
    }
}
