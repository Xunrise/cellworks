//! The district strategy seam, and the directories strategy: a project's
//! included files, relative to its root, cut into districts, sub-districts,
//! groups and nodes.
//!
//! Pure: no filesystem, no git, no geometry. The rectangles are left empty
//! here and filled in by the layout.
//!
//! Which folder level the districts start at is the project's prefix rule.
//! The rules are the one table [`PREFIX_RULES`]: adding a rule is one function
//! and one entry there, and touches nothing else.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use egui::Rect;

use crate::graph::{District, Group, Node, SubDistrict};

/// Decides how a project is cut into districts.
pub trait DistrictStrategy {
    /// Cuts a project's included files, relative to `root`, into districts.
    fn cut(&self, project: &str, root: &Path, files: &[PathBuf]) -> Vec<District>;
}

/// Districts cut by directories: the folders at the district level are the
/// districts, the next level the sub-districts, deeper folders the groups.
pub struct Directories {
    /// Where the district level is.
    pub prefix: &'static PrefixRule,
}

/// A named rule that finds the district level of a project's folder tree.
pub struct PrefixRule {
    /// The name the settings file uses for it.
    pub name: &'static str,
    district_level: fn(&Folder) -> &Folder,
}

/// Every prefix rule there is. The first is the default.
pub const PREFIX_RULES: &[PrefixRule] = &[
    PrefixRule {
        name: "one-wrapper-folder",
        district_level: one_wrapper_folder,
    },
    PrefixRule {
        name: "longest-shared-prefix",
        district_level: longest_shared_prefix,
    },
    PrefixRule {
        name: "none",
        district_level: |root| root,
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
fn one_wrapper_folder(root: &Folder) -> &Folder {
    match sole_folder(root) {
        Some(wrapper) if !wrapper.folders.is_empty() => wrapper,
        _ => root,
    }
}

/// Descends for as long as every file shares the next folder.
fn longest_shared_prefix(root: &Folder) -> &Folder {
    let mut level = root;
    while let Some(next) = sole_folder(level) {
        level = next;
    }
    level
}

/// The folder tree of the included files. Built from the file list, so a
/// folder with no file below it never enters it.
#[derive(Default)]
pub(crate) struct Folder {
    /// Relative to the project root.
    path: PathBuf,
    folders: BTreeMap<OsString, Folder>,
    files: BTreeSet<OsString>,
}

impl Folder {
    fn of(files: &[PathBuf]) -> Folder {
        let mut root = Folder::default();
        for file in files {
            let mut components: Vec<&OsStr> = file.iter().collect();
            let Some(name) = components.pop() else {
                continue;
            };
            let mut folder = &mut root;
            for component in components {
                let path = folder.path.join(component);
                folder = folder
                    .folders
                    .entry(component.to_owned())
                    .or_insert_with(|| Folder {
                        path,
                        ..Folder::default()
                    });
            }
            folder.files.insert(name.to_owned());
        }
        root
    }
}

impl DistrictStrategy for Directories {
    fn cut(&self, project: &str, root: &Path, files: &[PathBuf]) -> Vec<District> {
        let tree = Folder::of(files);
        let level = (self.prefix.district_level)(&tree);

        let mut districts: Vec<District> = level
            .folders
            .values()
            .map(|folder| District {
                name: name_of(&folder.path),
                path: root.join(&folder.path),
                sub_districts: folder
                    .folders
                    .values()
                    .map(|sub| SubDistrict {
                        name: name_of(&sub.path),
                        path: root.join(&sub.path),
                        groups: groups(root, sub),
                        nodes: nodes(root, sub),
                        area: Rect::NOTHING,
                    })
                    .collect(),
                nodes: nodes(root, folder),
                area: Rect::NOTHING,
            })
            .collect();

        if !level.files.is_empty() {
            districts.push(District {
                name: project.to_owned(),
                path: root.to_path_buf(),
                sub_districts: Vec::new(),
                nodes: nodes(root, level),
                area: Rect::NOTHING,
            });
            districts.sort_by(|one, other| {
                (one.name.as_bytes(), &one.path).cmp(&(other.name.as_bytes(), &other.path))
            });
        }

        districts
    }
}

fn groups(root: &Path, folder: &Folder) -> Vec<Group> {
    folder
        .folders
        .values()
        .map(|group| Group {
            name: name_of(&group.path),
            path: root.join(&group.path),
            groups: groups(root, group),
            nodes: nodes(root, group),
            area: Rect::NOTHING,
        })
        .collect()
}

fn nodes(root: &Path, folder: &Folder) -> Vec<Node> {
    folder
        .files
        .iter()
        .map(|file| {
            let path = folder.path.join(file);
            Node {
                name: name_of(&path),
                path: root.join(path),
                area: Rect::NOTHING,
            }
        })
        .collect()
}

/// The last component, labelled lossily when it is not valid UTF-8.
fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut_with(rule: &str, files: &[&str]) -> Vec<District> {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let prefix = prefix_rule(rule).expect("the rule exists");
        Directories { prefix }.cut("shop", Path::new("/p/shop"), &files)
    }

    fn cut(files: &[&str]) -> Vec<District> {
        cut_with("one-wrapper-folder", files)
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
    fn one_wrapper_folder_holding_folders_is_stripped() {
        assert_eq!(
            names(&cut(&["src/catalog/item.rs", "src/ordering/order.rs"])),
            ["catalog", "ordering"]
        );
    }

    #[test]
    fn one_wrapper_folder_holding_only_files_stays_a_district() {
        assert_eq!(names(&cut(&["build/output.o"])), ["build"]);
    }

    #[test]
    fn two_top_folders_are_not_stripped() {
        assert_eq!(names(&cut(&["docs/a.md", "src/b.rs"])), ["docs", "src"]);
    }

    #[test]
    fn a_top_folder_beside_a_loose_file_is_not_stripped() {
        assert_eq!(
            names(&cut(&["README.md", "src/catalog/item.rs"])),
            ["shop", "src"]
        );
    }

    #[test]
    fn one_wrapper_folder_is_stripped_at_most_once() {
        let districts = cut(&["src/catalog/pricing/price.rs"]);

        assert_eq!(names(&districts), ["catalog"]);
        assert_eq!(districts[0].sub_districts[0].name, "pricing");
    }

    #[test]
    fn longest_shared_prefix_strips_every_folder_all_files_share() {
        assert_eq!(
            names(&cut_with(
                "longest-shared-prefix",
                &[
                    "src/main/java/acme/billing/Invoice.java",
                    "src/main/java/acme/orders/Order.java",
                ]
            )),
            ["billing", "orders"]
        );
    }

    #[test]
    fn longest_shared_prefix_leaves_files_at_the_level_to_the_project_district() {
        let districts = cut_with(
            "longest-shared-prefix",
            &["src/a/price.rs", "src/a/rules/discount.rs"],
        );

        assert_eq!(names(&districts), ["rules", "shop"]);
        assert_eq!(
            districts[1].path,
            Path::new("/p/shop"),
            "identified by the root"
        );
    }

    #[test]
    fn no_prefix_keeps_the_top_folders_as_districts() {
        assert_eq!(
            names(&cut_with(
                "none",
                &["src/catalog/item.rs", "src/ordering/order.rs"]
            )),
            ["src"]
        );
    }

    #[test]
    fn levels_follow_depth_below_the_district_level() {
        let districts = cut(&[
            "src/catalog/item.rs",
            "src/catalog/pricing/price.rs",
            "src/catalog/pricing/rules/discount.rs",
            "src/catalog/pricing/rules/seasonal/winter.rs",
            "src/ordering/order.rs",
        ]);

        let catalog = &districts[0];
        assert_eq!(catalog.name, "catalog");
        assert_eq!(catalog.nodes[0].name, "item.rs");
        let pricing = &catalog.sub_districts[0];
        assert_eq!(pricing.name, "pricing");
        assert_eq!(pricing.nodes[0].name, "price.rs");
        let rules = &pricing.groups[0];
        assert_eq!(rules.name, "rules");
        assert_eq!(rules.nodes[0].name, "discount.rs");
        assert_eq!(rules.groups[0].name, "seasonal");
        assert_eq!(rules.groups[0].nodes[0].name, "winter.rs");
    }

    #[test]
    fn every_element_carries_the_canonical_path_it_stands_for() {
        let districts = cut(&["src/catalog/pricing/rules/discount.rs", "src/ordering/o.rs"]);

        let catalog = &districts[0];
        assert_eq!(catalog.path, Path::new("/p/shop/src/catalog"));
        assert_eq!(
            catalog.sub_districts[0].path,
            Path::new("/p/shop/src/catalog/pricing")
        );
        let rules = &catalog.sub_districts[0].groups[0];
        assert_eq!(rules.path, Path::new("/p/shop/src/catalog/pricing/rules"));
        assert_eq!(
            rules.nodes[0].path,
            Path::new("/p/shop/src/catalog/pricing/rules/discount.rs")
        );
    }

    #[test]
    fn a_folder_named_like_the_project_is_a_second_district_with_another_path() {
        let districts = cut(&["README.md", "shop/a.rs"]);

        assert_eq!(names(&districts), ["shop", "shop"]);
        assert_ne!(districts[0].path, districts[1].path);
    }

    #[test]
    fn a_shuffled_file_list_cuts_identically_and_sorted_byte_wise() {
        let files = ["src/b/z.rs", "src/B/y.rs", "src/a/x.rs", "src/a/W.rs"];
        let mut shuffled = files;
        shuffled.reverse();

        let districts = cut(&files);
        assert_eq!(districts, cut(&shuffled));
        assert_eq!(names(&districts), ["B", "a", "b"]);
        assert_eq!(districts[1].nodes[0].name, "W.rs");
    }

    #[test]
    fn no_files_cut_into_no_districts() {
        assert!(cut(&[]).is_empty());
    }
}
