//! Building or recalculating a project's context cells with Claude Code run
//! headless: the button under the Explorer's lens chips starts it, its
//! progress and result stand over the map, and once it is done the domain
//! docs are read again and the cells lens shows the new cells.
//!
//! A shell script stands in for `claude`: it writes one context the way
//! Claude would and prints a stream-json result. No test runs Claude.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use cellworks::app::CellworksApp;
use cellworks::cells::CellsBuild;
use cellworks_map::Lens;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use common::{Recorded, Tree};

/// A `claude` that writes the context "Catalog" for the folder `catalog`
/// and reports success, after `pause` seconds.
fn fake_claude(dir: &Path, pause: f32) -> PathBuf {
    let path = dir.join("claude");
    std::fs::write(
        &path,
        format!(
            r#"#!/bin/sh
mkdir -p docs/domain/contexts/Catalog
printf '# Context: Catalog\n\nThe products and their prices.\n' > docs/domain/contexts/Catalog/Catalog.md
echo '{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Write","input":{{"file_path":"docs/domain/contexts/Catalog/Catalog.md"}}}}]}}}}'
sleep {pause}
echo '{{"type":"result","is_error":false,"total_cost_usd":0.12,"result":"Contexts: Catalog"}}'
"#
        ),
    )
    .expect("write the fake claude");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    path
}

fn shop() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/pricing.rs", "fn total() {}\n"),
            ("orders/order.rs", "fn place() {}\n"),
        ],
    );
    let project = tree.as_project(None);
    let name = project.name.clone();
    let mut harness = common::harness(common::app(vec![project], Recorded::listing(&[])));
    common::double_click(&mut harness, &name);
    common::await_read(&mut harness);
    (tree, harness)
}

/// Steps until `done` or ten seconds have passed.
fn until(harness: &mut Harness<'_, CellworksApp>, done: impl Fn(&CellworksApp) -> bool) {
    for _ in 0..1000 {
        harness.state_mut().apply_pending();
        harness.step();
        if done(harness.state()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn claude_builds_the_cells_and_the_cells_lens_shows_them() {
    let (tree, mut harness) = shop();
    let bin = tempfile::tempdir().expect("a temporary directory");
    harness.state_mut().cells = CellsBuild::with_program(fake_claude(bin.path(), 0.3));
    assert!(
        !harness.state().opened().expect("open").offers(Lens::Cells),
        "no cells before Claude wrote any"
    );

    harness
        .get_by_label("build context cells with Claude")
        .click();
    harness.run_steps(3);
    assert!(harness.state().cells.is_running());
    let shown = common::labels(&harness).join(" | ");
    assert!(
        shown.contains("Claude is building the context cells"),
        "{shown}"
    );

    until(&mut harness, |app| {
        app.opened()
            .is_some_and(|o| o.lens() == Lens::Cells && !o.is_learning())
    });
    let opened = harness.state().opened().expect("open");
    assert_eq!(opened.lens(), Lens::Cells, "the cells lens is shown again");
    assert!(
        tree.root
            .join("docs/domain/contexts/Catalog/Catalog.md")
            .is_file()
    );
    common::await_lens(&mut harness);
    harness.run_steps(5);
    let shown = common::labels(&harness).join(" | ");
    assert!(shown.contains("Context cells rebuilt ($0.12)"), "{shown}");
    assert!(
        common::map(&harness)
            .districts
            .iter()
            .any(|d| d.name == "Catalog"),
        "the cells lens stands a district for the new context"
    );
}

#[test]
fn a_failed_build_says_why_until_dismissed() {
    let (_tree, mut harness) = shop();
    let bin = tempfile::tempdir().expect("a temporary directory");
    harness.state_mut().cells = CellsBuild::with_program(bin.path().join("no-claude-here"));
    harness.state_mut().build_cells();
    harness.run_steps(3);
    let shown = common::labels(&harness).join(" | ");
    assert!(
        shown.contains("Building the context cells failed"),
        "{shown}"
    );
    harness.get_by_label("dismiss").click();
    harness.run_steps(3);
    let shown = common::labels(&harness).join(" | ");
    assert!(!shown.contains("failed"), "{shown}");
}

/// PNGs of the button, the build running and the cells it built, written to
/// target/cells-shots.
#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn cells_shots() {
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/cells-shots");
    std::fs::create_dir_all(&out).expect("create target/cells-shots");
    let (_tree, mut harness) = shop();
    harness.ctx.set_theme(egui::Theme::Dark);
    cellworks_workspace::theme::apply(&harness.ctx);
    let bin = tempfile::tempdir().expect("a temporary directory");
    harness.state_mut().cells = CellsBuild::with_program(fake_claude(bin.path(), 1.5));
    let snap = |harness: &mut Harness<'_, CellworksApp>, name: &str| {
        let path = out.join(format!("{name}.png"));
        harness.render().expect("render").save(&path).expect("save");
        println!("{}", path.display());
    };
    harness.run_steps(10);
    snap(&mut harness, "0-button");
    harness
        .get_by_label("build context cells with Claude")
        .click();
    harness.run_steps(20);
    snap(&mut harness, "1-running");
    until(&mut harness, |app| {
        app.opened()
            .is_some_and(|o| o.lens() == Lens::Cells && !o.is_learning())
    });
    common::await_lens(&mut harness);
    harness.run_steps(60);
    snap(&mut harness, "2-done");
}
