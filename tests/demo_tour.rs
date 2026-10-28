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

fn info(h: &Harness<'_, State>) -> cellworks::demo::TutorialInfo {
    cellworks::demo::tutorial_info(&h.state().app).expect("tutorial state")
}

/// A primary click at `pos`, one frame per event.
fn click_at(h: &mut Harness<'_, State>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.step();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
}

fn snap(h: &mut Harness<'_, State>, out: &std::path::Path, name: &str) -> PathBuf {
    let path = out.join(format!("{name}.png"));
    h.render().expect("render").save(&path).expect("save");
    path
}

fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|w| !w.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join("-")
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
    let _ = std::fs::remove_dir_all(&out);
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
    cellworks_workspace::theme::apply(&h.ctx);
    println!("scanning");
    frames(&mut h, 3000); // the scan
    println!("scanned");

    // CELLWORKS_TOUR_FROM=<n>: open shopfront and start at chapter n (debug).
    if let Ok(from) = std::env::var("CELLWORKS_TOUR_FROM") {
        let root = cellworks::demo::root();
        drive(
            &mut h,
            &format!("open {}/projects/shopfront", root.display()),
        );
        frames(&mut h, 6000);
        drive(&mut h, &format!("tour-go {from}"));
    }
    let mut fallbacks: Vec<String> = Vec::new();
    let mut last: Option<PathBuf> = None;
    let mut seq = 0;
    'tour: for _ in 0..400 {
        let now = info(&h);
        let c = now.chapter;
        println!(
            "loop ch{c} {:?} target={:?} ready={}",
            now.phase, now.target, now.ready
        );
        let tag = slug(cellworks::demo::chapter_title(c));
        use cellworks::demo::TutorialPhase as P;
        match now.phase {
            P::Intro => {
                frames(&mut h, 600);
                seq += 1;
                snap(&mut h, &out, &format!("{c:02}-{seq:03}-intro-{tag}"));
                let start = cellworks_workspace::marked(&h.ctx, "tour.start");
                match start {
                    Some(r) => click_at(&mut h, r.center()),
                    None => {
                        fallbacks.push(format!("ch{c} intro: no tour.start mark"));
                        drive(&mut h, "tour-next");
                    }
                }
                frames(&mut h, 200);
            }
            P::Step(i) => {
                // Wait for the prep to finish and the target to show.
                let started = Instant::now();
                let mut cur = now;
                while started.elapsed() < Duration::from_secs(40) {
                    cur = info(&h);
                    if cur.phase != P::Step(i) {
                        break;
                    }
                    let waits = cur.action == Some("click") && cur.rect.is_none();
                    if cur.ready && !waits {
                        break;
                    }
                    // A click target that never marks falls back after a wait.
                    if cur.ready && waits && started.elapsed() > Duration::from_secs(10) {
                        break;
                    }
                    frames(&mut h, 100);
                }
                if cur.phase != P::Step(i) {
                    continue;
                }
                let label = format!("ch{c} step{}/{} [{}]", i + 1, cur.steps, cur.target);
                frames(&mut h, 400);
                cur = info(&h);
                if cur.phase != P::Step(i) {
                    continue;
                }
                seq += 1;
                let name = format!("{c:02}-{seq:03}-step{}", i + 1);
                match (cur.action, cur.rect, cur.key) {
                    (Some("click"), Some(rect), _) => {
                        snap(&mut h, &out, &format!("{name}-before-{}", slug(cur.target)));
                        click_at(&mut h, rect.center());
                    }
                    (Some("key"), _, Some(k)) => {
                        snap(&mut h, &out, &format!("{name}-before-key"));
                        h.key_press(k);
                        h.step();
                    }
                    (Some("watch"), _, _) => {
                        snap(&mut h, &out, &format!("{name}-before-watch"));
                    }
                    _ => {
                        let towers = h.state().app.towers();
                        println!(
                            "no mark for {}: {} towers, e.g. {:?}",
                            cur.target,
                            towers.len(),
                            towers
                                .iter()
                                .take(3)
                                .map(|t| t.path.clone())
                                .collect::<Vec<_>>()
                        );
                        fallbacks.push(format!("{label}: no mark, fell back to N"));
                        snap(&mut h, &out, &format!("{name}-before-missing"));
                        drive(&mut h, "tour-next");
                    }
                }
                if cur.action == Some("click") && cur.target == "btn.CAPTURE" {
                    if let Some(png) = &last {
                        drive(&mut h, &format!("image {}", png.display()));
                    }
                }
                // Let the step finish: a Watch runs its beats; others a moment.
                let began = Instant::now();
                loop {
                    frames(&mut h, 200);
                    let again = info(&h);
                    let moved = again.phase != P::Step(i);
                    if moved && cellworks::demo::idle(&h.state().app) {
                        break;
                    }
                    if began.elapsed() > Duration::from_secs(120) {
                        fallbacks.push(format!("{label}: did not finish"));
                        break;
                    }
                }
                if info(&h).phase == P::Step(i) {
                    drive(&mut h, "tour-next");
                }
                settle(&mut h);
                let after = snap(&mut h, &out, &format!("{name}-after"));
                println!("{label}: {}", cellworks::demo::status(&h.state().app));
                last = Some(after);
                // Skip the explanation.
                if matches!(info(&h).phase, P::Explain(_)) {
                    drive(&mut h, "tour-next");
                    frames(&mut h, 300);
                }
            }
            P::Explain(_) => {
                drive(&mut h, "tour-next");
                frames(&mut h, 300);
            }
            P::Free => {
                frames(&mut h, 800);
                seq += 1;
                snap(&mut h, &out, &format!("{c:02}-{seq:03}-free-{tag}"));
                if c + 1 == cellworks::demo::chapter_count() {
                    break 'tour;
                }
                match cellworks_workspace::marked(&h.ctx, "tour.next") {
                    Some(r) => click_at(&mut h, r.center()),
                    None => {
                        fallbacks.push(format!("ch{c} free: no tour.next mark"));
                        drive(&mut h, "tour-next");
                    }
                }
                frames(&mut h, 300);
            }
        }
    }
    println!("FALLBACKS ({}):", fallbacks.len());
    for f in &fallbacks {
        println!("  {f}");
    }
    // Dropping the app kills the dev server the tour started.
    drop(h);
}
