//! A trackpad scroll pans the map, a mouse wheel zooms it.
//!
//! The project "shop" is a real folder tree in a temporary directory. No test
//! runs git, reads `$HOME` or opens a window.

mod common;

use egui::{Event, Modifiers, MouseWheelUnit, pos2, vec2};

use common::shop_city;

/// A trackpad scroll pans, a wheel zooms
#[test]
fn a_trackpad_scroll_pans_and_a_wheel_zooms() {
    let (_tree, mut harness) = shop_city();
    harness.hover_at(pos2(800.0, 450.0));
    harness.event(Event::Zoom(1.3));
    harness.run();
    let before = harness.state().view().expect("drawn");

    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: vec2(0.0, -80.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    let panned = harness.state().view().expect("drawn");
    assert!(
        (panned.scale() - before.scale()).abs() < 1e-4,
        "the trackpad zoomed"
    );
    assert!(
        (panned.visible.center() - before.visible.center()).length() > 1.0,
        "the trackpad did not pan"
    );

    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, 3.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    let zoomed = harness.state().view().expect("drawn");
    assert!(
        (zoomed.scale() / panned.scale() - 1.0).abs() > 0.05,
        "the wheel did not zoom"
    );
}
