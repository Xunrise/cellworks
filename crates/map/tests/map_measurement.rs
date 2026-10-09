//! A measurement, not a check: how long cutting and laying out a large
//! project takes, and how large the result is. It asserts nothing; the
//! numbers are the first data point for EPIC-002's physics layout.
//!
//! cargo test --workspace --release -- --ignored --nocapture

use std::path::{Path, PathBuf};
use std::time::Instant;

use cellworks_map::{Directories, ProjectMap, default_prefix_rule};

#[test]
#[ignore = "measurement; run with --ignored --nocapture"]
fn cut_and_lay_out_twenty_thousand_files_in_forty_districts() {
    let files: Vec<PathBuf> = (0..20_000)
        .map(|n| {
            PathBuf::from(format!(
                "src/district{}/sub{}/group{}/file{n}.rs",
                n % 40,
                n % 7,
                n % 5
            ))
        })
        .collect();
    let strategy = Directories {
        prefix: default_prefix_rule(),
    };

    let started = Instant::now();
    let map = ProjectMap::of("large", Path::new("/large"), &files, &strategy);
    let elapsed = started.elapsed();

    println!(
        "20000 files, {} districts: cut and laid out in {elapsed:?}, extent {:?}",
        map.districts.len(),
        map.extent.size()
    );
}
