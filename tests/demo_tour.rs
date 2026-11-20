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
    start_sized(size())
}

/// [`start`] at a given window size.
fn start_sized(size: egui::Vec2) -> Harness<'static, State> {
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
        .with_size(size)
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

/// The README and landing-page pictures: stills of the city and of every
/// stage of the ticket loop, plus short frame films of the engine's own
/// transitions (one frame per harness step of 1/20 s, or every other one),
/// all with the tour hidden. Run with a private TMPDIR so the demo world is
/// not shared:
/// TMPDIR=$(mktemp -d /tmp/cw.XXXX)/ cargo test --release --test demo_tour readme_shots -- --ignored --nocapture
/// CELLWORKS_SHOTS_OUT=<dir> writes elsewhere than docs/assets;
/// CELLWORKS_SHOTS_PART=city stops after the city and its films.
#[test]
#[ignore]
fn readme_shots() {
    let out = std::env::var("CELLWORKS_SHOTS_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/assets"));
    let anim = out.join("anim");
    let _ = std::fs::remove_dir_all(&anim);
    std::fs::create_dir_all(&anim).unwrap();
    let only_city = std::env::var("CELLWORKS_SHOTS_PART").is_ok_and(|p| p == "city");
    let mut h = start_sized(egui::vec2(1920.0, 1080.0));
    drive(&mut h, "tour-hide");
    let shop = cellworks::demo::root().join("projects/shopfront");

    // The launcher: arrowing through the projects, then opening one.
    drive(&mut h, &format!("select {}", shop.display()));
    frames(&mut h, 800);
    still(&mut h, &out, "launcher.jpg");
    let projects = shop.parent().unwrap().to_path_buf();
    film(
        &mut h,
        &anim.join("launcher/move"),
        4,
        3,
        Crop::Full,
        |h, i| {
            let to = ["billing-api", "ops-runbooks", "shopfront"][i - 1];
            drive(h, &format!("select {}", projects.join(to).display()));
        },
    );
    film_paced(&mut h, &anim.join("launcher/open"), 4, 300, |h| {
        drive(h, &format!("open {}", shop.display()));
    });
    frames(&mut h, 3000);
    assert!(h.state().app.opened().is_some(), "shopfront opened");

    // The plain folder city every film starts from: no explorer, no
    // sessions pane, no captions, the whole city in view. City pictures are
    // cut to the towers so the city fills them.
    drive(&mut h, "tree");
    drive(&mut h, "bare on");
    h.key_press(egui::Key::S);
    drive(&mut h, "fit");
    settle(&mut h);
    city_still(&mut h, &out, "city.jpg", "");

    let city = anim.join("city");
    let back = |h: &mut Harness<'_, State>, lines: &[&str]| {
        script(h, lines);
        drive(h, "fit");
        settle(h);
    };
    for lens in ["imports", "calls", "co-change", "cells"] {
        film(&mut h, &city.join(lens), 15, 2, Crop::City(""), |h, i| {
            if i == 1 {
                drive(h, &format!("lens {lens}"));
            }
        });
        back(&mut h, &["lens folders"]);
    }
    // Import lanes for the whole city; call lanes for one selected tower
    // only, as a selection shows them: every call at once is too much.
    film(
        &mut h,
        &city.join("lanes-imports"),
        10,
        1,
        Crop::City(""),
        |h, i| {
            if i == 1 {
                drive(h, "lanes imports");
            }
        },
    );
    back(&mut h, &["lanes imports"]);
    film(&mut h, &city.join("explorer"), 12, 1, Crop::Full, |h, i| {
        if i == 1 {
            drive(h, "tree");
        }
    });
    back(&mut h, &["tree"]);
    // The turn is a cut, not a flight: two frames say it.
    film(&mut h, &city.join("turn"), 2, 1, Crop::City(""), |h, _| {
        h.key_press(egui::Key::Q);
    });
    for _ in 0..3 {
        h.key_press(egui::Key::Q);
        h.step();
    }
    back(&mut h, &[]);

    // The flat view: the switch, its belts, a machine up close.
    let flat = anim.join("flat");
    film(&mut h, &flat.join("view"), 4, 1, Crop::City(""), |h, i| {
        if i == 1 {
            h.key_press(egui::Key::V);
        }
    });
    settle(&mut h);
    city_still(&mut h, &out, "flat.jpg", "");
    film(
        &mut h,
        &flat.join("belts"),
        10,
        1,
        Crop::City(""),
        |h, i| {
            if i == 1 {
                drive(h, "lanes imports");
            }
        },
    );
    settle(&mut h);
    city_still(&mut h, &out, "flat-belts.jpg", "");
    back(&mut h, &["lanes imports"]);
    film(&mut h, &flat.join("zoom"), 14, 1, Crop::City(""), |h, i| {
        if i == 1 {
            drive(h, "frame src/cart");
        }
    });
    h.key_press(egui::Key::V);
    back(&mut h, &[]);
    deselect(&mut h);

    // The hero: the whole city, nothing selected, import lanes running as
    // highways between the districts with their traffic, sessions at work.
    script(
        &mut h,
        &[
            "lanes imports",
            "agent explorer src/catalog/search.ts",
            "agent shop-42 src/cart/totals.ts",
            "wait 1500",
            "agent shop-42 src/checkout/order.ts",
            "fit",
        ],
    );
    settle(&mut h);
    let frames_in_loop = hero_loop(&mut h, &anim.join("hero"));
    println!("hero loop: {frames_in_loop} frames");
    std::fs::copy(anim.join("hero/00.jpg"), out.join("city-lanes.jpg")).unwrap();
    // Call lanes for one selected tower, filmed from close over src.
    script(&mut h, &["frame src"]);
    settle(&mut h);
    let free = cellworks::demo::free_rect(&h.ctx).unwrap_or(egui::Rect::EVERYTHING);
    h.hover_at(free.center());
    h.event(egui::Event::Zoom(1.6));
    h.step();
    h.step();
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    script(&mut h, &["lanes imports"]);
    settle(&mut h);
    let totals = h
        .state()
        .app
        .towers()
        .iter()
        .filter(|t| t.path.to_string_lossy().contains("src/cart/totals.ts"))
        .max_by(|a, b| a.outline.area().total_cmp(&b.outline.area()))
        .map(|t| t.outline.center())
        .expect("the totals tower in view");
    click_at(&mut h, totals);
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    let calls = city.join("lanes-calls");
    film(&mut h, &calls, 10, 1, Crop::City("/src/"), |h, i| {
        if i == 1 {
            drive(h, "lanes calls");
        }
    });
    back(&mut h, &["lanes calls"]);
    deselect(&mut h);

    // Pinch from the whole city into the cart district, look under it,
    // then step inside a tower.
    let cart = city_box(&h, "src/cart/").expect("the cart towers").center();
    film(&mut h, &city.join("zoom"), 14, 1, Crop::City(""), |h, _| {
        h.hover_at(cart);
        h.event(egui::Event::Zoom(1.14));
    });
    h.event(egui::Event::PointerGone);
    settle(&mut h);
    for (name, key) in [("underground", egui::Key::U), ("cables", egui::Key::M)] {
        film(&mut h, &city.join(name), 3, 1, Crop::City(""), |h, i| {
            if i == 1 {
                h.key_press(key);
            }
        });
        h.key_press(key);
        settle(&mut h);
    }
    film(
        &mut h,
        &city.join("inside"),
        14,
        1,
        Crop::City(""),
        |h, i| {
            if i == 1 {
                drive(h, "enter src/cart/totals.ts");
            }
        },
    );
    // Esc layers out of the tower, and also toggles the tour back on.
    h.key_press(egui::Key::Escape);
    h.step();
    drive(&mut h, "tour-hide");
    back(&mut h, &[]);
    assert!(
        h.state().app.opened().is_some(),
        "Esc left the tower, not the city"
    );

    if only_city {
        return;
    }
    loop_shots(&mut h, &out, &anim.join("loop"));
    drop(h);
}

/// Every stage of SHOP-42, driven with the tour's own commands.
fn loop_shots(h: &mut Harness<'_, State>, out: &std::path::Path, anim: &std::path::Path) {
    // The loop shows the app as it is, panels and all.
    h.key_press(egui::Key::S);
    drive(h, "bare off");
    let shot = |h: &mut Harness<'_, State>, name: &str| {
        settle(h);
        still(h, out, &format!("{name}.jpg"));
    };
    film(h, &anim.join("tickets"), 5, 1, Crop::Full, |h, i| {
        if i == 1 {
            drive(h, "tickets");
        }
    });
    shot(h, "loop-1-ticket");
    film(h, &anim.join("case"), 10, 1, Crop::Full, |h, i| {
        if i == 1 {
            drive(h, "case-open SHOP-42");
            drive(h, "lens-panel off");
        }
    });
    script(h, &["wait 1500", "case-ingest"]);
    shot(h, "loop-2-case");
    script(
        h,
        &[
            "lens-reset",
            "case-prototype",
            "wait 1500",
            "devserver",
            "wait 4000",
            "app-window",
            "wait 1500",
            "write round-1",
            "agent shop-42 src/cart/promo.ts",
            "wait 2500",
            "agent shop-42 src/ui/checkoutView.ts",
            "wait 1500",
            "pin src/ui/checkoutView.ts",
            "type Show the discount on the total, with the old total struck through",
        ],
    );
    shot(h, "loop-3-prototype");
    // The round carries a screenshot; a copy in the demo world keeps the
    // repository's path out of the feedback log the freeze shot shows.
    let capture = cellworks::demo::root().join("capture.jpg");
    std::fs::copy(out.join("loop-3-prototype.jpg"), &capture).unwrap();
    h.key_press(egui::Key::C);
    script(h, &[&format!("image {}", capture.display()), "send"]);
    film_paced(h, &anim.join("round"), 10, 300, |h| {
        script(h, &["write round-2", "agent shop-42 src/cart/totals.ts"]);
    });
    shot(h, "loop-4-round");
    script(
        h,
        &[
            "agent shop-42 src/ui/checkoutView.ts",
            "wait 1500",
            "round-close",
            "case-go 5",
            "wait 800",
            "case-freeze",
            "wait 600",
            "case-file prototyped",
            "case-go 5",
        ],
    );
    shot(h, "loop-5-freeze");
    script(
        h,
        &["case-file planned", "wait 1500", "case-go 2", "case-go 3"],
    );
    shot(h, "loop-6-plan");
    script(
        h,
        &[
            "case-approve",
            "lens-reset",
            "case-close",
            "open {ROOT}/projects/shopfront-shop-42",
            "wait 2000",
            "pipeline build",
            "write factory/step-1",
            "agent factory-1 src/cart/promo.ts",
            "wait 2000",
            "commit feat(cart): apply promo codes\\n\\nRefs: SHOP-42",
            "wait 2500",
            "pipeline checks",
            "write factory/step-2",
            "agent factory-2 src/checkout/order.ts",
            "wait 1500",
        ],
    );
    shot(h, "loop-7-factory");
    script(
        h,
        &[
            "commit feat(checkout): charge the discounted total\\n\\nRefs: SHOP-42",
            "wait 2500",
            "pipeline deliver",
            "write factory/step-3",
            "agent factory-3 docs/domain/contexts/Cart/glossary.md",
            "wait 2000",
            "commit docs(cart): promo code in the glossary and story\\n\\nRefs: SHOP-42",
            "wait 1500",
            "pipeline review",
            "case-file fixed",
            "wait 1500",
            "case-open SHOP-42",
            "wait 1500",
            "case-go 6",
            "file src/cart/promo.ts",
        ],
    );
    shot(h, "loop-8-review");
    script(
        h,
        &[
            "case-review",
            "case-close",
            "merge-case",
            "open {ROOT}/projects/shopfront",
            "wait 2000",
            "lens-reset",
            "fit",
            "wait 1500",
        ],
    );
    film_paced(h, &anim.join("deploy"), 12, 300, |h| drive(h, "deploy"));
    shot(h, "loop-9-deploy");
    script(h, &["history last", "wait 1500", "history 3", "wait 2500"]);
    shot(h, "loop-10-history");
    script(
        h,
        &[
            "history last",
            "wait 2500",
            "frame src/cart",
            "wait 1200",
            "enter src/cart/promo.ts",
            "wait 1500",
            "floor-story",
        ],
    );
    shot(h, "loop-11-why");
}

/// The harness draws an 8 px edge; every picture leaves it out.
const FULL: Option<[u32; 4]> = Some([8, 8, 1904, 1064]);
/// Stills and film frames are scaled to these widths.
const STILL_WIDTH: u32 = 1600;
const FILM_WIDTH: u32 = 1000;

fn grab(h: &mut Harness<'_, State>) -> image::RgbaImage {
    h.render().expect("render")
}

/// A still for the README and the site, `name` in `out`.
fn still(h: &mut Harness<'_, State>, out: &std::path::Path, name: &str) {
    keep(&grab(h), &out.join(name), FULL, Some(STILL_WIDTH), 82);
}

/// Saves `img` cut to `crop` (x, y, w, h) and scaled to `width`; a `.jpg`
/// goes through `sips` at `quality`.
fn keep(
    img: &image::RgbaImage,
    path: &std::path::Path,
    crop: Option<[u32; 4]>,
    width: Option<u32>,
    quality: u8,
) {
    use image::imageops;
    let mut img = match crop {
        Some([x, y, w, h]) => imageops::crop_imm(img, x, y, w, h).to_image(),
        None => img.clone(),
    };
    if let Some(w) = width {
        let h = img.height() * w / img.width();
        img = imageops::resize(&img, w, h, imageops::FilterType::Lanczos3);
    }
    if path.extension().is_some_and(|e| e == "jpg") {
        let png = path.with_extension("png");
        img.save(&png).expect("save");
        let done = std::process::Command::new("sips")
            .args(["-s", "format", "jpeg", "-s", "formatOptions"])
            .arg(quality.to_string())
            .arg(&png)
            .arg("--out")
            .arg(path)
            .output()
            .expect("sips");
        assert!(done.status.success(), "sips failed on {}", png.display());
        std::fs::remove_file(&png).unwrap();
    } else {
        img.save(path).expect("save");
    }
}

/// Runs tour script lines: `wait <ms>` lets frames pass, the rest are drive
/// commands with `{ROOT}` the demo root.
fn script(h: &mut Harness<'_, State>, lines: &[&str]) {
    let root = cellworks::demo::root().display().to_string();
    for line in lines {
        match line.strip_prefix("wait ") {
            Some(ms) => frames(h, ms.parse().unwrap_or(0)),
            None => {
                drive(h, &line.replace("{ROOT}", &root));
                frames(h, 100);
            }
        }
    }
}

/// How a film or still is cut.
#[derive(Clone, Copy)]
enum Crop {
    /// The whole window.
    Full,
    /// The towers on the free map whose path holds this, so the city fills
    /// the picture.
    City(&'static str),
}

/// The screen box of the towers on the free map whose path holds `only`,
/// if any are drawn.
fn city_box(h: &Harness<'_, State>, only: &str) -> Option<egui::Rect> {
    let free = cellworks::demo::free_rect(&h.ctx)?;
    let r = h
        .state()
        .app
        .towers()
        .iter()
        .filter(|t| t.path.to_string_lossy().contains(only))
        .map(|t| t.outline.intersect(free))
        .filter(|o| o.is_positive())
        .fold(egui::Rect::NOTHING, |a, b| a.union(b));
    r.is_positive().then_some(r)
}

/// A crop of the window's proportions around `bbox` with a margin, inside
/// the free map, at least `min_w` wide so the picture is not blown up.
fn fit_crop(h: &Harness<'_, State>, bbox: Option<egui::Rect>, min_w: f32) -> Option<[u32; 4]> {
    let (bbox, free) = (bbox?, cellworks::demo::free_rect(&h.ctx)?);
    let aspect = 1904.0 / 1064.0;
    let mut w = (bbox.width() * 1.1)
        .max(bbox.height() * 1.1 * aspect)
        .max(min_w);
    let mut ht = w / aspect;
    if w > free.width() {
        w = free.width();
        ht = w / aspect;
    }
    if ht > free.height() {
        ht = free.height();
        w = ht * aspect;
    }
    let x = (bbox.center().x - w / 2.0).clamp(free.min.x, free.max.x - w);
    let y = (bbox.center().y - ht / 2.0).clamp(free.min.y, free.max.y - ht);
    Some([x as u32, y as u32, w as u32, ht as u32])
}

/// A still of the city alone, cut to its towers whose path holds `only`.
fn city_still(h: &mut Harness<'_, State>, out: &std::path::Path, name: &str, only: &str) {
    let crop = fit_crop(h, city_box(h, only), 1200.0).or(FULL);
    keep(&grab(h), &out.join(name), crop, Some(STILL_WIDTH), 84);
}

/// Clicks the empty corner of the free map, which drops the selection.
fn deselect(h: &mut Harness<'_, State>) {
    let free = cellworks::demo::free_rect(&h.ctx).expect("the free map");
    click_at(h, free.left_bottom() + egui::vec2(12.0, -12.0));
    settle(h);
    if h.state().app.selection().is_some() {
        println!("deselect: still {:?}", h.state().app.selection());
    }
}

/// A film of `n` frames in `dir` (00.jpg, 01.jpg, ...): frame 0 as things
/// stand, then `act(h, i)` and `every` harness steps before frame `i`. A
/// city film keeps one cut for all its frames, around every tower it shows.
fn film(
    h: &mut Harness<'_, State>,
    dir: &std::path::Path,
    n: usize,
    every: usize,
    crop: Crop,
    mut act: impl FnMut(&mut Harness<'_, State>, usize),
) {
    std::fs::create_dir_all(dir).unwrap();
    let mut shots = vec![grab(h)];
    let only = match crop {
        Crop::City(only) => only,
        Crop::Full => "",
    };
    let mut bbox = city_box(h, only);
    for i in 1..n {
        act(h, i);
        for _ in 0..every {
            h.state_mut().app.apply_pending();
            h.step();
        }
        shots.push(grab(h));
        bbox = match (bbox, city_box(h, only)) {
            (Some(a), Some(b)) => Some(a.union(b)),
            (a, b) => a.or(b),
        };
    }
    let cut = match crop {
        Crop::Full => FULL,
        Crop::City(_) => fit_crop(h, bbox, 900.0).or(FULL),
    };
    for (i, shot) in shots.iter().enumerate() {
        keep(
            shot,
            &dir.join(format!("{i:02}.jpg")),
            cut,
            Some(FILM_WIDTH),
            72,
        );
    }
    println!("filmed {}", dir.display());
}

/// The hero's loop in `dir`: the fitted city cut to its towers, filmed a
/// frame per 0.1 s. Each lane's traffic runs on its own seeded period, so
/// nothing repeats exactly; the loop ends where a frame comes closest to the
/// first, which keeps the seam quiet. Returns the frame count.
fn hero_loop(h: &mut Harness<'_, State>, dir: &std::path::Path) -> usize {
    let mut shots = Vec::new();
    for _ in 0..40 {
        for _ in 0..2 {
            h.state_mut().app.apply_pending();
            h.step();
        }
        shots.push(grab(h));
    }
    // The district grounds and the staging node reach past the towers.
    let cut = fit_crop(h, city_box(h, "").map(|r| r.expand(40.0)), 1200.0).or(FULL);
    let apart = |a: &image::RgbaImage, b: &image::RgbaImage| -> u64 {
        a.as_raw()
            .iter()
            .zip(b.as_raw())
            .step_by(7)
            .map(|(x, y)| u64::from(x.abs_diff(*y)))
            .sum()
    };
    let len = (20..=32)
        .min_by_key(|&k| apart(&shots[0], &shots[k]))
        .unwrap();
    std::fs::create_dir_all(dir).unwrap();
    for (i, shot) in shots[..len].iter().enumerate() {
        keep(shot, &dir.join(format!("{i:02}.jpg")), cut, Some(1600), 64);
    }
    len
}

/// A film paced in wall time, for what background threads drive: `start`,
/// then a frame every `millis`.
fn film_paced(
    h: &mut Harness<'_, State>,
    dir: &std::path::Path,
    n: usize,
    millis: u64,
    start: impl FnOnce(&mut Harness<'_, State>),
) {
    std::fs::create_dir_all(dir).unwrap();
    film_frame(h, dir, 0);
    start(h);
    for i in 1..n {
        frames(h, millis);
        film_frame(h, dir, i);
    }
    println!("filmed {}", dir.display());
}

fn film_frame(h: &mut Harness<'_, State>, dir: &std::path::Path, i: usize) {
    let path = dir.join(format!("{i:02}.jpg"));
    keep(&grab(h), &path, FULL, Some(FILM_WIDTH), 72);
}
