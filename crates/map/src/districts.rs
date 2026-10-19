//! The district strategy seam, and the directories strategy: a project's
//! included files, relative to its root, cut into districts, sub-districts,
//! groups and nodes.
//!
//! Pure: no filesystem, no git, no geometry. The rectangles are left empty
//! here and filled in by the layout.
//!
//! Which folders become districts is the project's district rule
//! ([`crate::rules`]): a rule looks at the folder tree and names its cut,
//! and this module builds the districts from it, the same for every rule.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use egui::Rect;

use crate::graph::{District, DistrictKind, Group, Node, NodeKind, SubDistrict};
use crate::rules::PrefixRule;

/// Decides how a project is cut into districts.
pub trait DistrictStrategy {
    /// Cuts a project's included files, relative to `root`, into districts.
    fn cut(&self, project: &str, root: &Path, files: &[PathBuf]) -> Vec<District>;
}

/// Districts cut by directories: the folders the project's rule names are
/// the districts, the folders inside them the sub-districts, deeper folders
/// the groups.
pub struct Directories {
    /// The rule that names the districts.
    pub prefix: &'static PrefixRule,
}

/// What a rule makes of a folder tree: the folders that become districts,
/// and the folder whose own files, with whatever no district claims below
/// it, form the district named after the project.
///
/// A district never holds a folder another district claims, however deep:
/// that folder is cut out of it and stands on its own.
pub(crate) struct Cut<'a> {
    pub(crate) districts: Vec<&'a Folder>,
    pub(crate) rest: &'a Folder,
}

impl<'a> Cut<'a> {
    /// Every folder directly inside `level` a district, the files of `level`
    /// the project's district.
    pub(crate) fn level(level: &'a Folder) -> Cut<'a> {
        Cut {
            districts: level.folders.values().collect(),
            rest: level,
        }
    }
}

/// The folder tree of the included files. Built from the file list, so a
/// folder with no file below it never enters it.
#[derive(Default)]
pub(crate) struct Folder {
    /// Relative to the project root.
    pub(crate) path: PathBuf,
    pub(crate) folders: BTreeMap<OsString, Folder>,
    pub(crate) files: BTreeSet<OsString>,
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

impl Folder {
    /// This folder and every folder below it.
    pub(crate) fn all(&self) -> Vec<&Folder> {
        let mut all = vec![self];
        let mut index = 0;
        while let Some(folder) = all.get(index) {
            all.extend(folder.folders.values());
            index += 1;
        }
        all
    }

    /// Whether this folder holds a file called `name`.
    pub(crate) fn holds(&self, name: &str) -> bool {
        self.files.contains(OsStr::new(name))
    }
}

impl DistrictStrategy for Directories {
    fn cut(&self, project: &str, root: &Path, files: &[PathBuf]) -> Vec<District> {
        let tree = Folder::of(files);
        let cut = (self.prefix.cut)(&tree);
        let claimed: BTreeSet<&Path> = cut.districts.iter().map(|f| f.path.as_path()).collect();
        let shape = Shape { root, claimed };

        let mut districts: Vec<District> = cut
            .districts
            .iter()
            .map(|folder| District {
                name: name_of(&folder.path),
                path: root.join(&folder.path),
                sub_districts: shape.sub_districts(folder),
                nodes: nodes(root, folder),
                area: Rect::NOTHING,
                kind: DistrictKind::Folder,
                elevation: 0.0,
            })
            .filter(|district| !district.sub_districts.is_empty() || !district.nodes.is_empty())
            .collect();
        let rest = District {
            name: project.to_owned(),
            path: root.to_path_buf(),
            sub_districts: shape.sub_districts(cut.rest),
            nodes: nodes(root, cut.rest),
            area: Rect::NOTHING,
            kind: DistrictKind::Folder,
            elevation: 0.0,
        };
        if !rest.sub_districts.is_empty() || !rest.nodes.is_empty() {
            districts.push(rest);
        }
        districts.sort_by(|one, other| {
            (one.name.as_bytes(), &one.path).cmp(&(other.name.as_bytes(), &other.path))
        });
        districts
    }
}

/// How the folders below a district become sub-districts and groups: rooted
/// at `root`, leaving out the folders other districts claim, and any folder
/// left with nothing in it. A folder holding only one folder and no file is
/// one with it, named after both: `src/main/kotlin`.
struct Shape<'a> {
    root: &'a Path,
    claimed: BTreeSet<&'a Path>,
}

impl Shape<'_> {
    fn unclaimed<'f>(&self, folder: &'f Folder) -> impl Iterator<Item = &'f Folder> {
        folder
            .folders
            .values()
            .filter(|inner| !self.claimed.contains(inner.path.as_path()))
    }

    /// The folder at the end of the chain from `folder` in which each holds
    /// only the next and no file, and the chain's name.
    fn chain<'f>(&self, folder: &'f Folder) -> (&'f Folder, String) {
        let mut name = name_of(&folder.path);
        let mut folder = folder;
        while folder.files.is_empty() {
            let mut inner = self.unclaimed(folder);
            let (Some(only), None) = (inner.next(), inner.next()) else {
                break;
            };
            name = format!("{name}/{}", name_of(&only.path));
            folder = only;
        }
        (folder, name)
    }

    fn sub_districts(&self, folder: &Folder) -> Vec<SubDistrict> {
        self.unclaimed(folder)
            .map(|sub| self.chain(sub))
            .map(|(sub, name)| SubDistrict {
                name,
                path: self.root.join(&sub.path),
                groups: self.groups(sub),
                nodes: nodes(self.root, sub),
                area: Rect::NOTHING,
            })
            .filter(|sub| !sub.groups.is_empty() || !sub.nodes.is_empty())
            .collect()
    }

    fn groups(&self, folder: &Folder) -> Vec<Group> {
        self.unclaimed(folder)
            .map(|group| self.chain(group))
            .map(|(group, name)| Group {
                name,
                path: self.root.join(&group.path),
                groups: self.groups(group),
                nodes: nodes(self.root, group),
                area: Rect::NOTHING,
            })
            .filter(|group| !group.groups.is_empty() || !group.nodes.is_empty())
            .collect()
    }
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
                items: Vec::new(),
                area: Rect::NOTHING,
                kind: NodeKind::Code,
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

    use crate::rules::prefix_rule;

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
        let districts = cut(&[
            "src/catalog/pricing/price.rs",
            "src/catalog/pricing/rules/discount.rs",
            "src/ordering/o.rs",
        ]);

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
    fn a_folder_holding_only_one_folder_is_one_folder_named_after_both() {
        let districts = cut(&[
            "src/catalog/item.rs",
            "src/catalog/pricing/rules/seasonal/winter.rs",
            "src/catalog/pricing/rules/discount.rs",
            "src/catalog/web/src/main/app.ts",
            "src/ordering/order.rs",
        ]);

        let catalog = &districts[0];
        let names: Vec<&str> = catalog
            .sub_districts
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(names, ["pricing/rules", "web/src/main"]);
        let rules = &catalog.sub_districts[0];
        assert_eq!(rules.path, Path::new("/p/shop/src/catalog/pricing/rules"));
        assert_eq!(rules.nodes[0].name, "discount.rs");
        assert_eq!(
            rules.groups[0].name, "seasonal",
            "a folder holding a file ends the chain"
        );
        assert_eq!(
            catalog.sub_districts[1].nodes[0].path,
            Path::new("/p/shop/src/catalog/web/src/main/app.ts")
        );
    }

    #[test]
    fn a_folder_another_district_claims_does_not_count_in_a_chain() {
        let districts = cut_with(
            "rust-crates",
            &[
                "Cargo.toml",
                "src/main.rs",
                "crates/catalog/Cargo.toml",
                "crates/catalog/src/lib.rs",
                "crates/notes/drafts/idea.md",
            ],
        );

        let shop = districts.iter().find(|d| d.name == "shop").expect("shop");
        let names: Vec<&str> = shop.sub_districts.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["crates/notes/drafts", "src"],
            "the claimed catalog leaves crates holding only notes"
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
