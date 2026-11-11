//! The review overlay: what a pull request's head wrote, removed and
//! impacted, worked out over a mapped head once the overlay is built
//! (STORY-022).
//!
//! The binary turns `cellworks_sources::pull_requests::FilePlacement` (its
//! `TowerPlacement.added` and the `before_new_line` of each removal) into the
//! plain, by-tower-path maps [`ReviewOverlay::new`] takes, so this crate gains
//! no dependency on Sources: the join stays published language, read only
//! from what the map already knows.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::graph::{self, ProjectMap};

/// What the review overlay marks of one tower.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TowerReview {
    /// The line numbers of its own floors the pull request wrote, in no
    /// particular order.
    pub written: Vec<usize>,
    /// The floor numbers lines were removed before: the `before_new_line` of
    /// each removal that landed on this tower, so a strike lands where the
    /// removal sat, not on a line number that no longer exists.
    pub removed: Vec<usize>,
    /// Whether a line of this tower resolves a call to a changed tower, never
    /// true for a tower the pull request itself changed.
    pub impacted: bool,
    /// Whether the tower pulses: every changed or impacted tower does.
    pub pulsing: bool,
}

/// The pull request's reach over a mapped head: which towers it wrote,
/// removed lines from, or merely touches through a call.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReviewOverlay {
    /// Every tower the overlay marks, by its path on the mapped head.
    pub towers: BTreeMap<PathBuf, TowerReview>,
}

impl ReviewOverlay {
    /// Builds the overlay over `map`: `written` and `removed` name, for each
    /// tower path they changed, the floor numbers written and the floor
    /// numbers lines were removed before. Every item of `map` is then walked;
    /// one whose path is not already a changed tower but has a floor calling
    /// one is marked impacted. A changed tower is never also marked impacted.
    pub fn new(
        written: &BTreeMap<PathBuf, Vec<usize>>,
        removed: &BTreeMap<PathBuf, Vec<usize>>,
        map: &ProjectMap,
    ) -> ReviewOverlay {
        let mut towers: BTreeMap<PathBuf, TowerReview> = BTreeMap::new();
        for (path, lines) in written {
            towers.entry(path.clone()).or_default().written = lines.clone();
        }
        for (path, lines) in removed {
            towers.entry(path.clone()).or_default().removed = lines.clone();
        }
        for review in towers.values_mut() {
            review.pulsing = true;
        }
        let impacted: Vec<PathBuf> = map
            .districts
            .iter()
            .flat_map(graph::items)
            .filter(|item| !towers.contains_key(&item.path))
            .filter(|item| {
                item.floors
                    .iter()
                    .flat_map(|floor| &floor.calls)
                    .any(|call| towers.contains_key(call))
            })
            .map(|item| item.path.clone())
            .collect();
        for path in impacted {
            towers.insert(
                path,
                TowerReview {
                    written: Vec::new(),
                    removed: Vec::new(),
                    impacted: true,
                    pulsing: true,
                },
            );
        }
        ReviewOverlay { towers }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::graph::{Declaration, Floor, Item, ItemKind};
    use crate::{Directories, default_prefix_rule};

    /// One line of a tower, with nothing this module cares about filled in
    /// beyond its number and what it calls.
    fn floor(number: usize, calls: Vec<PathBuf>) -> Floor {
        Floor {
            number,
            text: String::new(),
            indent: 0,
            width: 0,
            tokens: Vec::new(),
            calls,
            imports: Vec::new(),
        }
    }

    /// A tower (an item) named `name` standing on `file`, holding `floors`.
    /// Its rectangle is borrowed from an empty map rather than named
    /// directly: the layout always overwrites it.
    fn tower(name: &str, file: &str, floors: Vec<Floor>) -> Item {
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
            floors,
            area,
            base: 0.0,
            declaration: Declaration::default(),
        }
    }

    /// A map of one project whose only file is `file`, standing `towers`.
    fn map_with(file: &str, towers: Vec<Item>) -> ProjectMap {
        let root = Path::new("/project");
        let files = vec![PathBuf::from(file)];
        let mut items = BTreeMap::new();
        items.insert(PathBuf::from(file), towers);
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of_code("review", root, &files, &strategy, items, &BTreeMap::new())
    }

    #[test]
    fn the_overlay_holds_changed_towers_and_those_that_call_them() {
        let file = "src/lib.rs";
        let discount = PathBuf::from(format!("{file}/discount"));
        let checkout_path = PathBuf::from(format!("{file}/checkout"));
        let discount_item = tower("discount", file, vec![floor(1, vec![]), floor(2, vec![])]);
        let checkout = tower("checkout", file, vec![floor(1, vec![discount.clone()])]);
        let map = map_with(file, vec![discount_item, checkout]);
        let mut written = BTreeMap::new();
        written.insert(discount.clone(), vec![1, 2]);

        let overlay = ReviewOverlay::new(&written, &BTreeMap::new(), &map);

        assert_eq!(overlay.towers.len(), 2, "{:?}", overlay.towers);
        let discount_review = overlay.towers.get(&discount).expect("discount changed");
        assert_eq!(discount_review.written, vec![1, 2]);
        assert!(discount_review.removed.is_empty());
        assert!(!discount_review.impacted);
        assert!(discount_review.pulsing);
        let checkout_review = overlay
            .towers
            .get(&checkout_path)
            .expect("checkout calls discount");
        assert!(checkout_review.impacted);
        assert!(checkout_review.written.is_empty());
        assert!(checkout_review.pulsing);
    }

    #[test]
    fn a_changed_tower_is_never_also_impacted() {
        let file = "src/lib.rs";
        let discount = PathBuf::from(format!("{file}/discount"));
        let total = PathBuf::from(format!("{file}/total"));
        let discount_item = tower("discount", file, vec![floor(1, vec![total.clone()])]);
        let total_item = tower("total", file, vec![floor(1, vec![])]);
        let map = map_with(file, vec![discount_item, total_item]);
        let mut written = BTreeMap::new();
        written.insert(discount.clone(), vec![1]);
        let mut removed = BTreeMap::new();
        removed.insert(total.clone(), vec![5]);

        let overlay = ReviewOverlay::new(&written, &removed, &map);

        assert_eq!(overlay.towers.len(), 2, "{:?}", overlay.towers);
        assert!(
            !overlay.towers[&discount].impacted,
            "discount is changed, never also impacted, even though it calls total"
        );
        assert!(!overlay.towers[&total].impacted, "total is changed too");
    }

    #[test]
    fn an_empty_diff_impacts_no_tower() {
        let file = "src/lib.rs";
        let discount = PathBuf::from(format!("{file}/discount"));
        let discount_item = tower("discount", file, vec![floor(1, vec![])]);
        let checkout = tower("checkout", file, vec![floor(1, vec![discount])]);
        let map = map_with(file, vec![discount_item, checkout]);

        let overlay = ReviewOverlay::new(&BTreeMap::new(), &BTreeMap::new(), &map);

        assert!(
            overlay.towers.is_empty(),
            "nothing is changed to be impacted by: {:?}",
            overlay.towers
        );
    }
}
