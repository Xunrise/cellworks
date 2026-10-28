//! PROTOTYPE: runs the whole stakeholder demo tour headlessly and saves one
//! PNG per beat to $CELLWORKS_TOUR_OUT (default target/demo-tour).
//! cargo test --release --test demo_tour -- --ignored --nocapture
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cellworks::app::{CellworksApp, drive_line};
use egui_kittest::Harness;

struct State {
    app: CellworksApp,
    lines: Vec<String>,
}

fn frames(h: &mut Harness<'_, State>, millis: u64) {
    let until = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < until {
        h.state_mut().app.apply_pending();
        h.step();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Steps until the tour has run its queue and waits, then lets reads settle.
fn settle(h: &mut Harness<'_, State>) {
    let started = Instant::now();
    loop {
        frames(h, 50);
        if cellworks::demo::idle(&h.state().app) || started.elapsed() > Duration::from_secs(60) {
            break;
        }
    }
    frames(h, 2500);
    while h
        .state()
        .app
        .opened()
        .is_some_and(|o| o.is_reading() || o.is_preparing())
        && started.elapsed() < Duration::from_secs(90)
    {
        frames(h, 100);
    }
    frames(h, 1200);
}

fn drive(h: &mut Harness<'_, State>, line: &str) {
    h.state_mut().lines.push(line.to_owned());
    h.step();
}

#[test]
#[ignore]
fn demo_tour() {
    // SAFETY: set before the app or any thread exists.
    unsafe {
        std::env::set_var("CELLWORKS_DEMO", "1");
        std::env::set_var("CELLWORKS_DRY_LAUNCH", "1");
    }
    let manifest = env!("CARGO_MANIFEST_DIR");
    let status = std::process::Command::new("bash")
        .arg(format!("{manifest}/demo/setup.sh"))
        .status()
        .expect("run setup.sh");
    assert!(status.success(), "setup.sh failed");
    cellworks::demo::init();
    let out = PathBuf::from(
        std::env::var("CELLWORKS_TOUR_OUT")
            .unwrap_or_else(|_| format!("{manifest}/target/demo-tour")),
    );
    std::fs::create_dir_all(&out).unwrap();

    let app = CellworksApp::start();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(1.0 / 20.0)
        .with_max_steps(1)
        .build_ui_state(
            |ui, st: &mut State| {
                for line in std::mem::take(&mut st.lines) {
                    drive_line(ui, &mut st.app, &line);
                }
                cellworks::draw(ui, &mut st.app);
            },
            State {
                app,
                lines: Vec::new(),
            },
        );
    h.ctx.set_theme(egui::Theme::Dark);
    cellworks_map::ui::case::install_fonts(&h.ctx);
    frames(&mut h, 3000); // the scan

    let mut last: Option<PathBuf> = None;
    for n in 0..60 {
        let (chapter, beat, beats, _) = cellworks::demo::position(&h.state().app).unwrap();
        if chapter == 11 && beat >= beats && n > 0 {
            break;
        }
        // Round 1 carries a screenshot: the previous beat's PNG.
        if chapter == 5 && beat == 2 {
            if let Some(png) = &last {
                drive(&mut h, &format!("image {}", png.display()));
            }
        }
        drive(&mut h, "tour-next");
        settle(&mut h);
        let (c, b, _, title) = cellworks::demo::position(&h.state().app).unwrap();
        let b = if c == chapter { b } else { 0 };
        let slug: String = title
            .to_lowercase()
            .chars()
            .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|w| !w.is_empty())
            .take(4)
            .collect::<Vec<_>>()
            .join("-");
        let path = out.join(format!("{c:02}-{b}-{slug}.png"));
        h.render().expect("render").save(&path).expect("save");
        println!(
            "ch{c} beat{b}: {} | {}",
            title,
            cellworks::demo::status(&h.state().app)
        );
        last = Some(path);
    }
    // Dropping the app kills the dev server the tour started.
    drop(h);
}
