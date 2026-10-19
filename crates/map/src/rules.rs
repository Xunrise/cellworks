//! The district rules: how a project's folder tree is cut into districts.
//!
//! A rule is a function from the folder tree to its [`Cut`]: the folders that
//! become districts, and the folder whose leftovers form the district named
//! after the project. Building the districts from a cut is
//! [`crate::districts`]'s job and the same for every rule.
//!
//! Adding a rule is one function here and one entry in [`PREFIX_RULES`]; the
//! settings file names it with `district_prefix`.

use crate::districts::{Cut, Folder};

/// A named rule that cuts a project's folder tree into districts.
pub struct PrefixRule {
    /// The name the settings file uses for it.
    pub name: &'static str,
    pub(crate) cut: fn(&Folder) -> Cut<'_>,
}

/// Every district rule there is. The first is the default.
pub const PREFIX_RULES: &[PrefixRule] = &[
    PrefixRule {
        name: "one-wrapper-folder",
        cut: one_wrapper_folder,
    },
    PrefixRule {
        name: "longest-shared-prefix",
        cut: longest_shared_prefix,
    },
    PrefixRule {
        name: "none",
        cut: none,
    },
    PrefixRule {
        name: "rust-crates",
        cut: rust_crates,
    },
];

/// The rule a project uses when its settings name none.
pub fn default_prefix_rule() -> &'static PrefixRule {
    &PREFIX_RULES[0]
}

/// The rule called `name`, or `None` when there is no such rule.
pub fn prefix_rule(name: &str) -> Option<&'static PrefixRule> {
    PREFIX_RULES.iter().find(|rule| rule.name == name)
}

/// The folder's only entry, when it has exactly one and that entry is a
/// folder.
fn sole_folder(folder: &Folder) -> Option<&Folder> {
    match (folder.folders.len(), folder.files.is_empty()) {
        (1, true) => folder.folders.values().next(),
        _ => None,
    }
}

/// Descends once into a single top folder, but only when that folder holds a
/// folder of its own; a sole top folder holding only files stays a district.
fn one_wrapper_folder(root: &Folder) -> Cut<'_> {
    match sole_folder(root) {
        Some(wrapper) if !wrapper.folders.is_empty() => Cut::level(wrapper),
        _ => Cut::level(root),
    }
}

/// Descends for as long as every file shares the next folder.
fn longest_shared_prefix(root: &Folder) -> Cut<'_> {
    let mut level = root;
    while let Some(next) = sole_folder(level) {
        level = next;
    }
    Cut::level(level)
}

/// Every top folder a district.
fn none(root: &Folder) -> Cut<'_> {
    Cut::level(root)
}

/// The manifest that makes a folder a Rust crate.
const CARGO_MANIFEST: &str = "Cargo.toml";

/// For Rust: every crate, a folder holding a `Cargo.toml`, is a district,
/// wherever it lies. A crate at the root is the project's district and keeps
/// what no other crate claims; without one, every top folder holding no
/// crate is a district of its own. A tree without a crate is cut as
/// [`one_wrapper_folder`] cuts it.
fn rust_crates(root: &Folder) -> Cut<'_> {
    let crates: Vec<&Folder> = root
        .all()
        .into_iter()
        .filter(|folder| folder.holds(CARGO_MANIFEST))
        .collect();
    if crates.is_empty() {
        return one_wrapper_folder(root);
    }
    let mut districts: Vec<&Folder> = crates
        .iter()
        .copied()
        .filter(|krate| !std::ptr::eq(*krate, root))
        .collect();
    if !root.holds(CARGO_MANIFEST) {
        let holds_a_crate =
            |top: &Folder| crates.iter().any(|krate| krate.path.starts_with(&top.path));
        districts.extend(root.folders.values().filter(|top| !holds_a_crate(top)));
    }
    Cut {
        districts,
        rest: root,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::graph::District;
    use crate::{Directories, DistrictStrategy};

    fn cut_with(rule: &str, files: &[&str]) -> Vec<District> {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let prefix = prefix_rule(rule).expect("the rule exists");
        Directories { prefix }.cut("shop", Path::new("/p/shop"), &files)
    }

    fn names(districts: &[District]) -> Vec<&str> {
        districts.iter().map(|d| d.name.as_str()).collect()
    }

    #[test]
    fn the_default_rule_is_one_wrapper_folder() {
        assert_eq!(default_prefix_rule().name, "one-wrapper-folder");
    }

    #[test]
    fn every_rule_resolves_by_its_name_and_an_unknown_name_does_not() {
        for rule in PREFIX_RULES {
            assert_eq!(prefix_rule(rule.name).map(|r| r.name), Some(rule.name));
        }
        assert!(prefix_rule("sideways").is_none());
    }

    #[test]
    fn every_crate_of_a_workspace_is_a_district_wherever_it_lies() {
        let districts = cut_with(
            "rust-crates",
            &[
                "Cargo.toml",
                "src/main.rs",
                "crates/README.md",
                "crates/catalog/Cargo.toml",
                "crates/catalog/src/lib.rs",
                "crates/orders/Cargo.toml",
                "crates/orders/src/order/mod.rs",
                "docs/index.md",
            ],
        );

        assert_eq!(names(&districts), ["catalog", "orders", "shop"]);
        let orders = &districts[1];
        assert_eq!(orders.path, Path::new("/p/shop/crates/orders"));
        assert_eq!(
            orders.sub_districts[0].name, "src/order",
            "src holds only the folder order"
        );
        let shop = &districts[2];
        let subs: Vec<&str> = shop.sub_districts.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            subs,
            ["crates", "docs", "src"],
            "the root crate keeps the rest"
        );
        assert_eq!(shop.sub_districts[0].nodes[0].name, "README.md");
        assert!(
            shop.sub_districts[0].groups.is_empty(),
            "no crate inside the root's district"
        );
    }

    #[test]
    fn without_a_root_crate_folders_holding_no_crate_stand_on_their_own() {
        let districts = cut_with(
            "rust-crates",
            &[
                "crates/catalog/Cargo.toml",
                "crates/catalog/src/lib.rs",
                "docs/index.md",
                "README.md",
            ],
        );

        assert_eq!(names(&districts), ["catalog", "docs", "shop"]);
        assert_eq!(districts[2].nodes[0].name, "README.md");
        assert!(districts[2].sub_districts.is_empty());
    }

    #[test]
    fn a_tree_without_a_crate_is_cut_like_the_default() {
        let files = ["src/catalog/item.py", "src/ordering/order.py"];

        assert_eq!(
            cut_with("rust-crates", &files),
            cut_with("one-wrapper-folder", &files)
        );
    }
}
