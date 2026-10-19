//! A measurement, not a check: how long reading this repository's own code
//! with tree-sitter takes, and how many items, lines and resolved calls it
//! yields. Skips when the checkout is not there.
//!
//! cargo test --workspace --release -- --ignored --nocapture

use std::path::Path;
use std::time::Instant;

use cellworks_code_analysis::{CodeAnalysisEvent, GitCli, load};
use cellworks_map::{ProjectMap, default_prefix_rule};

#[test]
#[ignore = "measurement; run with --ignored --nocapture"]
fn read_the_code_of_this_repository() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(root) = root.canonicalize() else {
        println!("no checkout at {}; skipped", root.display());
        return;
    };

    let started = Instant::now();
    let events = load(&root, "cellworks", default_prefix_rule(), &GitCli, false);
    let elapsed = started.elapsed();

    let Some(CodeAnalysisEvent::ProjectMapped { map, .. }) = events.last() else {
        println!("no map: {events:?}");
        return;
    };
    let (items, floors, calls) = count(map);
    println!("read in {elapsed:?}: {items} items, {floors} lines, {calls} resolved calls");
}

fn count(map: &ProjectMap) -> (usize, usize, usize) {
    let mut totals = (0, 0, 0);
    let mut nodes = Vec::new();
    for district in &map.districts {
        nodes.extend(&district.nodes);
        let mut groups: Vec<_> = Vec::new();
        for sub in &district.sub_districts {
            nodes.extend(&sub.nodes);
            groups.extend(&sub.groups);
        }
        while let Some(group) = groups.pop() {
            nodes.extend(&group.nodes);
            groups.extend(&group.groups);
        }
    }
    for item in nodes.iter().flat_map(|node| &node.items) {
        totals.0 += 1;
        totals.1 += item.floors.len();
        totals.2 += item.floors.iter().map(|f| f.calls.len()).sum::<usize>();
    }
    totals
}
