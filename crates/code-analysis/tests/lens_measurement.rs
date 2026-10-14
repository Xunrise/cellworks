//! A measurement, not a check: how long seeing this repository's own code,
//! or the project at `LENS_PROJECT`, through each lens takes, and how many
//! districts each makes. Skips when the checkout is not there.
//!
//! cargo test --workspace --release -- --ignored --nocapture

use std::path::Path;
use std::time::Instant;

use cellworks_code_analysis::{CodeAnalysisEvent, GitCli, load};
use cellworks_map::{Lens, prefix_rule, relens};

#[test]
#[ignore = "measurement; run with --ignored --nocapture"]
fn see_this_repository_through_every_lens() {
    let root = std::env::var("LENS_PROJECT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let Ok(root) = root.canonicalize() else {
        println!("no checkout at {}; skipped", root.display());
        return;
    };
    let rule = prefix_rule("rust-crates").expect("the rule");
    let events = load(&root, "cellworks", rule, &GitCli, false);
    let Some(CodeAnalysisEvent::ProjectMapped { map, .. }) = events.last() else {
        println!("no map: {events:?}");
        return;
    };
    println!("{} co-change pairs", map.co_changes.len());
    for lens in Lens::ALL {
        let started = Instant::now();
        let seen = relens(map, lens);
        let elapsed = started.elapsed();
        let names: Vec<&str> = seen
            .districts
            .iter()
            .map(|d| d.name.as_str())
            .take(6)
            .collect();
        println!(
            "{:<10} {elapsed:>10.2?}  {:>3} districts  {names:?}",
            lens.name(),
            seen.districts.len()
        );
    }
}
