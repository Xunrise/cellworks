//! A measurement, not a check: how long one frame takes in every lens, with
//! and without lanes, and how long a switch of lens takes. Measures this
//! repository, or the project at `MEASURE_PROJECT`; skips when it is not
//! there.
//!
//! cargo test --release --test frame_measurement -- --ignored --nocapture

mod common;

use std::sync::Arc;
use std::time::Instant;

use cellworks::app::CellworksApp;
use cellworks_code_analysis::GitCli;
use cellworks_map::Lens;
use cellworks_workspace::Project;
use egui_kittest::Harness;

/// Frames timed per measurement, after a few to settle.
const FRAMES: u32 = 10;

fn per_frame(harness: &mut Harness<'static, CellworksApp>) -> f64 {
    harness.run_steps(3);
    let started = Instant::now();
    harness.run_steps(FRAMES as usize);
    started.elapsed().as_secs_f64() * 1000.0 / f64::from(FRAMES)
}

#[test]
#[ignore = "measurement; run with --ignored --nocapture"]
fn every_lens_frame_by_frame() {
    let root =
        std::env::var("MEASURE_PROJECT").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    let Ok(root) = std::fs::canonicalize(&root) else {
        println!("no project at {root}; skipped");
        return;
    };
    let project = Project {
        district_prefix: Some("rust-crates".to_owned()),
        ..Project::at(root)
    };
    let name = project.name.clone();
    let mut harness = common::harness(common::app(vec![project], Arc::new(GitCli)));
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    for _ in 0..30 {
        harness.state_mut().apply_pending();
        harness.step();
    }
    let lens = |harness: &Harness<'_, CellworksApp>| harness.state().opened().map(|o| o.lens());
    for _ in 0..Lens::ALL.len() {
        let Some(current) = lens(&harness) else {
            println!("nothing to measure");
            return;
        };
        println!(
            "{:<12} {:>7.1} ms/frame, its own lanes",
            current.name(),
            per_frame(&mut harness)
        );
        for key in [egui::Key::G, egui::Key::A] {
            harness.key_press(key);
            harness.step();
        }
        println!(
            "{:<12} {:>7.1} ms/frame, lanes toggled",
            "",
            per_frame(&mut harness)
        );
        for key in [egui::Key::G, egui::Key::A] {
            harness.key_press(key);
            harness.step();
        }
        let started = Instant::now();
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::L);
        harness.step();
        let pressed = started.elapsed().as_secs_f64() * 1000.0;
        common::await_lens(&mut harness);
        let arrived = started.elapsed().as_secs_f64() * 1000.0;
        let flying = Instant::now();
        harness.run_steps(FRAMES as usize);
        let flying = flying.elapsed().as_secs_f64() * 1000.0 / f64::from(FRAMES);
        harness.run_steps(120);
        if let Some(next) = lens(&harness) {
            println!(
                "{:<12} {:>7.1} ms frame of the switch to {}, its map there after {:.1} ms, \
                 {:.1} ms/frame in flight",
                "",
                pressed,
                next.name(),
                arrived,
                flying
            );
        }
    }
}
