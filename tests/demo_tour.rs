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

/// The window size: CELLWORKS_TOUR_SIZE=WxH (default 1600x1000).
fn size() -> egui::Vec2 {
    std::env::var("CELLWORKS_TOUR_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some(egui::vec2(w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or(egui::vec2(1600.0, 1000.0))
}

/// Builds the demo world and a scanned app in a headless harness.
fn start() -> Harness<'static, State> {
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
    let app = CellworksApp::start();
    let mut h = Harness::builder()
        .with_size(size())
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
    h
}

/// Clicking a tower on the map in the tour's free play selects it and opens
/// the why panel, as it does outside the demo.
#[test]
#[ignore]
fn map_click_opens_why() {
    let mut h = start();
    let root = cellworks::demo::root();
    drive(
        &mut h,
        &format!("open {}/projects/shopfront", root.display()),
    );
    frames(&mut h, 6000);
    drive(&mut h, "tour-go 2");
    frames(&mut h, 300);
    use cellworks::demo::TutorialPhase as P;
    for _ in 0..40 {
        if info(&h).phase == P::Free {
            break;
        }
        drive(&mut h, "tour-next");
        settle(&mut h);
    }
    assert_eq!(info(&h).phase, P::Free, "chapter 2 reaches free play");
    drive(&mut h, "fit");
    settle(&mut h);
    let free = cellworks::demo::free_rect(&h.ctx).expect("the free map rect");
    let taken = cellworks::demo::overlay_rects(&h.ctx);
    // The tallest tower clear of the overlays: its middle is on the tower.
    let tower = h
        .state()
        .app
        .towers()
        .iter()
        .filter(|t| {
            let at = t.outline.center();
            free.shrink(20.0).contains(at) && !taken.iter().any(|r| r.contains(at))
        })
        .max_by(|a, b| a.outline.area().total_cmp(&b.outline.area()))
        .map(|t| (t.path.clone(), t.outline.center()))
        .expect("a tower clear of the overlays");
    click_at(&mut h, tower.1);
    frames(&mut h, 600);
    let app = &h.state().app;
    println!("clicked {:?} -> selection {:?}", tower.0, app.selection());
    assert!(app.selection().is_some(), "the click selected something");
    assert!(
        app.why().is_some(),
        "a click on a tower opens the why panel"
    );
    // A click that lands on a block's ground picks the file: its tallest
    // tower explains it.
    drive(&mut h, "frame src/cart/totals.ts");
    frames(&mut h, 600);
    let app = &h.state().app;
    println!("jumped -> selection {:?}", app.selection());
    assert!(app.why().is_some(), "a selected file opens the why panel");
}

#[test]
#[ignore]
fn demo_tour() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut h = start();
    let out = PathBuf::from(
        std::env::var("CELLWORKS_TOUR_OUT")
            .unwrap_or_else(|_| format!("{manifest}/target/demo-tour")),
    );
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

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
                if cur.target == "floor.story" {
                    use egui_kittest::kittest::{NodeT, Queryable};
                    let floor = h.query_by_label("This floor").is_some();
                    let story: Vec<String> = h
                        .query_all_by_label_contains("SHOP-42")
                        .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
                        .collect();
                    println!("floor panel: {floor}, SHOP-42 labels: {story:?}");
                    if !floor || story.is_empty() {
                        fallbacks.push(format!("{label}: the why panel shows no SHOP-42 floor"));
                    }
                }
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

/// The README and landing-page screenshots, written to docs/assets/ with the
/// tour hidden. Run with a private TMPDIR so the demo world is not shared:
/// TMPDIR=$(mktemp -d /tmp/cw.XXXX)/ cargo test --release --test demo_tour readme_shots -- --ignored --nocapture
/// then trim the harness's 8 px edge: for f in docs/assets/*.png; do sips -c 984 1584 "$f"; done
#[test]
#[ignore]
fn readme_shots() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/assets");
    std::fs::create_dir_all(&out).unwrap();
    let mut h = start();
    drive(&mut h, "tour-hide");
    let shop = cellworks::demo::root().join("projects/shopfront");

    drive(&mut h, &format!("select {}", shop.display()));
    frames(&mut h, 800);
    snap(&mut h, &out, "launcher");

    drive(&mut h, &format!("open {}", shop.display()));
    frames(&mut h, 6000);
    drive(&mut h, "agent explorer src/catalog/search.ts");
    drive(&mut h, "tree");
    drive(&mut h, "fit");
    settle(&mut h);
    // Closer than the fit: the city fills the frame beside the panels.
    h.hover_at(egui::pos2(900.0, 560.0));
    h.event(egui::Event::Zoom(1.3));
    h.step();
    h.step();
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(130.0, -40.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    snap(&mut h, &out, "city");

    h.key_press(egui::Key::V);
    settle(&mut h);
    snap(&mut h, &out, "flat");
    h.key_press(egui::Key::V);
    settle(&mut h);

    drive(&mut h, "tickets");
    settle(&mut h);
    snap(&mut h, &out, "tickets");

    drive(&mut h, "case-open SHOP-42");
    settle(&mut h);
    drive(&mut h, "case-ingest");
    settle(&mut h);
    snap(&mut h, &out, "case");
    drop(h);
}
