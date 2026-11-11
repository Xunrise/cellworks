//! The camera with the Explorer open: input acts on the logical camera, not
//! on the area drawn beside the panel; a flight from the Explorer lands where
//! it stays.
//!
//! The project "shop" is a real folder tree in a temporary directory. No test
//! runs git, reads `$HOME` or opens a window.

mod common;

use cellworks::app::CellworksApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;

use common::{map, shop_city};

/// Right of the Explorer panel, inside the map.
const POINTER: Pos2 = pos2(800.0, 450.0);

/// "shop" with the Explorer open (the default), zoomed in off the
/// whole-project view, the pointer resting at [`POINTER`].
fn zoomed_in() -> Harness<'static, CellworksApp> {
    let (_tree, mut harness) = shop_city();
    assert!(harness.state().explorer_open());
    harness.hover_at(POINTER);
    harness.event(Event::Zoom(1.3));
    harness.run();
    harness
}

fn ground_under_pointer(harness: &Harness<'_, CellworksApp>) -> Pos2 {
    harness.state().view().expect("drawn").to_ground(POINTER)
}

fn drag(harness: &mut Harness<'_, CellworksApp>, by: egui::Vec2) {
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    harness.event(button(POINTER, true));
    harness.step();
    for k in 1..=10 {
        harness.event(Event::PointerMoved(POINTER + by * (k as f32 / 10.0)));
        harness.step();
    }
    harness.event(button(POINTER + by, false));
    harness.run();
}

/// Zooming and dragging with the Explorer open keep the pointer's ground point
#[test]
fn zooming_and_dragging_with_the_explorer_open_keep_the_ground_under_the_pointer() {
    let mut harness = zoomed_in();
    for _ in 0..2 {
        let ground = ground_under_pointer(&harness);
        harness.event(Event::Zoom(1.5));
        harness.run();
        let view = harness.state().view().expect("drawn");
        let seen = view.to_screen(ground, 0.0);
        assert!((seen - POINTER).length() < 1.0, "{seen:?} left {POINTER:?}");
    }

    let ground = ground_under_pointer(&harness);
    let by = vec2(-60.0, 40.0);
    drag(&mut harness, by);
    let view = harness.state().view().expect("drawn");
    let seen = view.to_screen(ground, 0.0);
    assert!(
        (seen - (POINTER + by)).length() < 1.0,
        "{seen:?} is not {POINTER:?} + {by:?}"
    );
}

/// Picking a section in the Explorer lands where the camera stays
#[test]
fn picking_a_section_in_the_explorer_lands_where_it_stays() {
    let (_tree, mut harness) = shop_city();
    harness.key_press(Key::ArrowDown);
    let mut centres = Vec::new();
    for _ in 0..90 {
        harness.step();
        centres.push(harness.state().view().expect("drawn").visible);
    }
    // The flight is linear: no frame may move the camera far more than the
    // one before, and once landed it is still.
    let steps: Vec<f32> = centres
        .windows(2)
        .map(|pair| {
            (pair[1].center() - pair[0].center()).length()
                + (pair[1].width() - pair[0].width()).abs()
        })
        .collect();
    let peak = steps.iter().cloned().fold(0.0, f32::max);
    let flight = steps[10..20].iter().sum::<f32>() / 10.0;
    assert!(
        peak < flight * 2.0 + 0.01,
        "a jump of {peak} against {flight} a frame"
    );
    assert!(
        steps[80..].iter().all(|step| *step < 1e-3),
        "moves once landed: {steps:?}"
    );
}

/// The width the Explorer takes from the left of the map, margin included.
const EXPLORER_SIDE: f32 = 330.0;

/// Steps `frames` frames, returning the view's scale after each.
fn scales(harness: &mut Harness<'_, CellworksApp>, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|_| {
            harness.step();
            harness.state().view().expect("drawn").scale()
        })
        .collect()
}

/// A section picked in the Explorer lands centred in the map beside the panel
#[test]
fn a_section_picked_in_the_explorer_lands_centred_beside_the_panel() {
    let (_tree, mut harness) = shop_city();
    harness.key_press(Key::ArrowDown);
    scales(&mut harness, 90);

    let map = map(&harness);
    let framing = harness.state().navigation().framing(&map);
    let view = harness.state().view().expect("drawn");
    let area = framing.area;
    let corners = [
        area.left_top(),
        area.right_top(),
        area.right_bottom(),
        area.left_bottom(),
    ]
    .map(|corner| view.to_screen(corner, 0.0));
    let mut seen = egui::Rect::from_points(&corners);
    seen.min.y -= framing.headroom * view.scale();

    let panel = harness.state().panel().expect("drawn");
    let beside = egui::Rect::from_min_max(panel.min + vec2(EXPLORER_SIDE, 0.0), panel.max);
    assert!(
        (seen.center() - beside.center()).length() < 1.0,
        "{:?} is not centred in {beside:?}",
        seen.center()
    );
    assert!(
        beside.contains_rect(seen),
        "{seen:?} does not fit in {beside:?}"
    );
}
