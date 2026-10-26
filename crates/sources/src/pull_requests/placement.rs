//! Where a pull request's added and removed lines land on the city at its
//! head: an added line on the floor holding its number, a removed line on
//! exactly one tower (never two, even at a boundary between them); a line no
//! tower holds is counted outside, per file, never dropped.

use std::path::{Path, PathBuf};

use cellworks_map::{Item, ProjectMap};

use super::diff::{Diff, FileChange, Removal};

/// Where one tower's own lines of a changed file landed: the new-side line
/// numbers added to it, and the removals (old line and the new-side line each
/// sat just before) taken from it, so a removal's strike can be drawn at the
/// floor it sat before, not a floor number that no longer exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TowerPlacement {
    pub tower: PathBuf,
    pub added: Vec<usize>,
    pub removed: Vec<Removal>,
}

/// Where one changed file's lines landed: every tower the city holds for it
/// (even one that received nothing), and the counts of lines no tower held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlacement {
    pub file: String,
    pub towers: Vec<TowerPlacement>,
    pub outside_added: usize,
    pub outside_removed: usize,
}

/// Places every line `diff` added or removed onto the towers `map` holds for
/// each changed file, the city built at the pull request's head, at `root`.
///
/// An added line lands on the innermost tower whose own floors span it (the
/// narrowest `[min, max]` floor-number range containing the line), since a
/// type's own floors can be sparse around the methods nested inside it. A
/// removed line, named by the new-side line it sat just before, follows the
/// same innermost rule first; failing that, it lands on the tower whose last
/// floor is the line just above (the removal sat right after that tower, with
/// a gap before the next one); failing both, it is outside. Either way a line
/// lands on exactly one tower, never two, even at a boundary between them
/// (requirement 7). A file with no towers at all, or a line no tower's floors
/// contain, is still counted: outside, per file, never dropped (requirement
/// 8).
pub fn place(diff: &Diff, map: &ProjectMap, root: &Path) -> Vec<FilePlacement> {
    diff.files
        .iter()
        .map(|(file, change)| place_file(file, change, map, root))
        .collect()
}

fn place_file(file: &str, change: &FileChange, map: &ProjectMap, root: &Path) -> FilePlacement {
    let towers = map.items_under(&root.join(file));
    let mut by_tower: Vec<TowerPlacement> = towers
        .iter()
        .map(|item| TowerPlacement {
            tower: item.path.clone(),
            added: Vec::new(),
            removed: Vec::new(),
        })
        .collect();
    let mut outside_added = 0;
    let mut outside_removed = 0;

    for &line in &change.added {
        match innermost_containing(&towers, line) {
            Some(index) => by_tower[index].added.push(line),
            None => outside_added += 1,
        }
    }

    for removal in &change.removed {
        match place_removal(&towers, removal.before_new_line) {
            Some(index) => by_tower[index].removed.push(removal.clone()),
            None => outside_removed += 1,
        }
    }

    FilePlacement {
        file: file.to_owned(),
        towers: by_tower,
        outside_added,
        outside_removed,
    }
}

/// `item`'s own floor-number range, the smallest and largest `number` among
/// its floors; `None` when it stands no floor at all.
fn floor_range(item: &Item) -> Option<(usize, usize)> {
    let mut numbers = item.floors.iter().map(|floor| floor.number);
    let first = numbers.next()?;
    Some(numbers.fold((first, first), |(min, max), number| {
        (min.min(number), max.max(number))
    }))
}

/// The number of `item`'s last floor, in source order; `None` when it stands
/// no floor at all.
fn last_floor_number(item: &Item) -> Option<usize> {
    item.floors.last().map(|floor| floor.number)
}

/// The index, among `towers`, of the innermost one whose own floor range
/// contains `line`: the narrowest range among those that do. `None` when no
/// tower's range contains it.
fn innermost_containing(towers: &[&Item], line: usize) -> Option<usize> {
    towers
        .iter()
        .enumerate()
        .filter_map(|(index, item)| floor_range(item).map(|range| (index, range)))
        .filter(|(_, (min, max))| *min <= line && line <= *max)
        .min_by_key(|(_, (min, max))| max - min)
        .map(|(index, _)| index)
}

/// The index, among `towers`, that a removal sitting just before
/// `before_new_line` on the new side lands on: the innermost tower whose
/// range contains that line; else the tower whose last floor is the line just
/// above (never checked when `before_new_line` is `0`, since nothing stands
/// "just above" line 0); else `None`, outside every tower.
fn place_removal(towers: &[&Item], before_new_line: usize) -> Option<usize> {
    if let Some(index) = innermost_containing(towers, before_new_line) {
        return Some(index);
    }
    let just_above = before_new_line.checked_sub(1)?;
    towers
        .iter()
        .position(|item| last_floor_number(item) == Some(just_above))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use cellworks_map::{
        Declaration, Directories, Floor, Item, ItemKind, ProjectMap, default_prefix_rule,
    };

    use super::super::diff::{Diff, FileChange, Removal};
    use super::*;

    const ROOT: &str = "/project";

    /// A single line of a tower, with nothing this module cares about filled
    /// in beyond its number.
    fn floor(number: usize) -> Floor {
        Floor {
            number,
            text: String::new(),
            indent: 0,
            width: 0,
            tokens: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
        }
    }

    /// A tower (an item) standing on `file`, holding `lines`. Its rectangle is
    /// borrowed from an empty map rather than named directly: the layout
    /// always overwrites it, and this crate has no reason of its own to
    /// depend on egui just to spell the type out.
    fn tower(name: &str, file: &str, lines: &[usize]) -> Item {
        let area = ProjectMap::of(
            "blank",
            Path::new("/blank"),
            &[],
            &Directories {
                prefix: default_prefix_rule(),
            },
        )
        .extent;
        Item {
            name: name.to_owned(),
            path: PathBuf::from(format!("{file}/{name}")),
            kind: ItemKind::Function,
            tests: false,
            floors: lines.iter().copied().map(floor).collect(),
            area,
            base: 0.0,
            declaration: Declaration::default(),
        }
    }

    /// A map of one project, under [`ROOT`], whose only file is `file`,
    /// standing `towers`.
    fn map_with(file: &str, towers: Vec<Item>) -> ProjectMap {
        let root = Path::new(ROOT);
        let files = vec![PathBuf::from(file)];
        let mut items = BTreeMap::new();
        items.insert(PathBuf::from(file), towers);
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of_code("review", root, &files, &strategy, items, &BTreeMap::new())
    }

    fn diff_of(file: &str, added: Vec<usize>, removed: Vec<Removal>) -> Diff {
        let mut files = BTreeMap::new();
        files.insert(file.to_owned(), FileChange { added, removed });
        Diff { files }
    }

    fn towers_of<'a>(placements: &'a [FilePlacement], file: &str) -> &'a FilePlacement {
        placements
            .iter()
            .find(|placement| placement.file == file)
            .unwrap_or_else(|| panic!("no placement for {file} among {placements:?}"))
    }

    fn tower_named<'a>(placement: &'a FilePlacement, name: &str) -> &'a TowerPlacement {
        placement
            .towers
            .iter()
            .find(|tower| tower.tower.to_string_lossy().ends_with(name))
            .unwrap_or_else(|| panic!("no tower named {name} among {:?}", placement.towers))
    }

    #[test]
    fn an_added_line_lands_on_the_floor_holding_its_number() {
        let file = "src/lib.rs";
        let map = map_with(
            file,
            vec![
                tower("first", file, &[1, 2, 3]),
                tower("second", file, &[10, 11, 12]),
            ],
        );
        let diff = diff_of(file, vec![2, 11], vec![]);

        let placements = place(&diff, &map, Path::new(ROOT));

        let placement = towers_of(&placements, file);
        assert_eq!(tower_named(placement, "first").added, vec![2]);
        assert_eq!(tower_named(placement, "second").added, vec![11]);
        assert_eq!(placement.outside_added, 0);
        assert_eq!(placement.outside_removed, 0);
    }

    #[test]
    fn a_removal_at_the_boundary_between_two_adjacent_towers_lands_on_exactly_one() {
        let file = "src/lib.rs";
        // Adjacent, no gap: "first" ends at 5, "second" starts right at 6.
        let map = map_with(
            file,
            vec![
                tower("first", file, &[1, 2, 3, 4, 5]),
                tower("second", file, &[6, 7, 8, 9, 10]),
            ],
        );
        let diff = diff_of(
            file,
            vec![],
            vec![Removal {
                old_line: 50,
                before_new_line: 6,
            }],
        );

        let placements = place(&diff, &map, Path::new(ROOT));

        let placement = towers_of(&placements, file);
        assert_eq!(
            tower_named(placement, "second").removed,
            vec![Removal {
                old_line: 50,
                before_new_line: 6
            }],
            "line 6 is inside second's own range"
        );
        assert!(
            tower_named(placement, "first").removed.is_empty(),
            "the old span rule counted a boundary removal on both towers; exactly one must get it here"
        );
        assert_eq!(placement.outside_removed, 0);
    }

    #[test]
    fn a_removal_after_the_last_floor_lands_on_the_tower_whose_last_floor_is_just_above() {
        let file = "src/lib.rs";
        // A gap (lines 6-9) between the two towers holds no item.
        let map = map_with(
            file,
            vec![
                tower("first", file, &[1, 2, 3, 4, 5]),
                tower("second", file, &[10, 11, 12]),
            ],
        );
        let diff = diff_of(
            file,
            vec![],
            vec![Removal {
                old_line: 42,
                before_new_line: 6,
            }],
        );

        let placements = place(&diff, &map, Path::new(ROOT));

        let placement = towers_of(&placements, file);
        assert_eq!(
            tower_named(placement, "first").removed,
            vec![Removal {
                old_line: 42,
                before_new_line: 6
            }],
            "first's last floor is 5, one less than before_new_line 6"
        );
        assert!(tower_named(placement, "second").removed.is_empty());
        assert_eq!(placement.outside_removed, 0);
    }

    #[test]
    fn nested_towers_pick_the_innermost_for_an_added_and_a_removed_line() {
        let file = "src/model.rs";
        // "Account" is a type whose own floors are sparse (its declaration's
        // open and close); "Account::deposit" is one of its methods, nested
        // well inside Account's numeric range.
        let map = map_with(
            file,
            vec![
                tower("Account", file, &[1, 20]),
                tower("Account::deposit", file, &[8, 9, 10]),
            ],
        );
        let diff = diff_of(
            file,
            vec![9],
            vec![Removal {
                old_line: 7,
                before_new_line: 9,
            }],
        );

        let placements = place(&diff, &map, Path::new(ROOT));

        let placement = towers_of(&placements, file);
        let method = tower_named(placement, "Account::deposit");
        assert_eq!(method.added, vec![9]);
        assert_eq!(
            method.removed,
            vec![Removal {
                old_line: 7,
                before_new_line: 9
            }]
        );
        let type_tower = tower_named(placement, "Account");
        assert!(
            type_tower.added.is_empty() && type_tower.removed.is_empty(),
            "the innermost tower (the method) must take the line, not the type around it"
        );
    }

    #[test]
    fn a_file_with_no_towers_at_all_has_every_line_outside() {
        let map = map_with("src/lib.rs", vec![tower("first", "src/lib.rs", &[1, 2, 3])]);
        let diff = diff_of(
            "Cargo.lock",
            vec![5, 6],
            vec![Removal {
                old_line: 3,
                before_new_line: 5,
            }],
        );

        let placements = place(&diff, &map, Path::new(ROOT));

        let placement = towers_of(&placements, "Cargo.lock");
        assert!(placement.towers.is_empty(), "{:?}", placement.towers);
        assert_eq!(placement.outside_added, 2);
        assert_eq!(placement.outside_removed, 1);
    }
}
