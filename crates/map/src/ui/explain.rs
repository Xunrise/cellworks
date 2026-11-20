//! A lens explains itself: the lens bar offering every lens the project can
//! have, a caption saying what blocks and districts are in the current one,
//! a banner naming it for a moment after a switch, and a legend of the lanes
//! shown. All of it is labels, so the accessibility tree reads it too.

use egui::{Align, Align2, Color32, Frame, Margin, Rect, Stroke, pos2, vec2};

use super::{DIM, LINE_STRONG, PANEL};
use crate::{Lanes, Lens};

/// How long the banner stands, and how long it then takes to fade.
const BANNER: f64 = 1.0;
const FADE: f64 = 1.0;

/// What a lens chip explains when hovered: its blurb, what a block and a
/// district are in it.
pub fn lens_help(lens: Lens) -> String {
    format!(
        "{}\nBlock: {}.\nDistrict: {}.",
        lens.blurb(),
        lens.block(),
        lens.district()
    )
}

/// The lens bar: one button per lens in `offered`, `current` marked, each
/// explained when pointed at. Returns the lens picked, if one was.
pub fn lens_bar(ui: &mut egui::Ui, current: Lens, offered: &[Lens]) -> Option<Lens> {
    let mut picked = None;
    for lens in offered {
        let help = format!("{}\nL cycles the lenses.", lens_help(*lens));
        let response = switch(ui, lens.name(), *lens == current);
        cellworks_workspace::mark(ui.ctx(), &format!("lens.{}", lens.name()), response.rect);
        if response.on_hover_text(help).clicked() {
            picked = Some(*lens);
        }
    }
    picked
}

/// The lane toggles beside the lens bar. Returns which was switched.
pub fn lane_toggles(ui: &mut egui::Ui, shown: Lanes) -> (bool, bool) {
    let imports = switch(ui, "import lanes (G)", shown.imports);
    cellworks_workspace::mark(ui.ctx(), "lanes.imports", imports.rect);
    let imports = imports
        .on_hover_text("Imports between files, along the ground.")
        .clicked();
    let calls = switch(ui, "call lanes (A)", shown.calls);
    cellworks_workspace::mark(ui.ctx(), "lanes.calls", calls.rect);
    let calls = calls
        .on_hover_text("Calls from a line to the function it calls, through the air.")
        .clicked();
    (imports, calls)
}

/// A chamfered tab, yellow when `on`: a plain button to the accessibility
/// tree, not a toggle like the rows of the tree.
fn switch(ui: &mut egui::Ui, text: &str, on: bool) -> egui::Response {
    cellworks_workspace::theme::tab(ui, text, on, false)
}

fn card() -> Frame {
    Frame::new()
        .fill(PANEL.gamma_multiply(0.94))
        .stroke(Stroke::new(1.0, LINE_STRONG))
        .inner_margin(Margin::symmetric(12, 9))
}

/// `text` as a card's body: its first line a yellow tracked title, the rest
/// dim; one label, so the accessibility tree reads `text` whole.
fn titled(ctx: &egui::Context, text: &str) -> egui::text::LayoutJob {
    let (head, rest) = text.split_once('\n').unwrap_or((text, ""));
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &format!("{head}\n"),
        0.0,
        egui::TextFormat {
            font_id: cellworks_workspace::theme::named(ctx, 12.0, "semibold"),
            color: super::case::ATTENTION,
            extra_letter_spacing: 1.4,
            line_height: Some(20.0),
            ..Default::default()
        },
    );
    job.append(
        rest,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(12.5),
            color: DIM,
            line_height: Some(17.0),
            ..Default::default()
        },
    );
    job
}

/// The caption's text for `lens`: it starts with "Lens: ".
pub fn caption_text(lens: Lens) -> String {
    format!(
        "Lens: {}\n{}\nBlock: {}.\nDistrict: {}.\nColour always shows the crate a tower comes from.",
        lens.name(),
        lens.blurb(),
        lens.block(),
        lens.district()
    )
}

/// What the current lens shows, at the top left of `panel`.
pub fn caption(ui: &egui::Ui, panel: Rect, lens: Lens) {
    let shown = egui::Area::new(ui.id().with("lens caption"))
        .fixed_pos(panel.min + vec2(12.0, 12.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            card().show(ui, |ui| {
                ui.label(titled(ui.ctx(), &caption_text(lens)));
            });
        });
    cellworks_workspace::mark(ui.ctx(), "slot.caption", shown.response.rect);
}

/// The banner naming `lens`, `age` seconds after the switch to it: full for a
/// second, then fading over another; nothing after. Its first line is the
/// lens's name in capitals and "LENS".
pub fn banner(ui: &egui::Ui, panel: Rect, lens: Lens, age: f64) {
    let alpha = (1.0 - (age - BANNER) / FADE).clamp(0.0, 1.0) as f32;
    if alpha <= 0.0 {
        return;
    }
    let mut text = egui::text::LayoutJob::default();
    let line = |size: f32, colour: Color32| egui::TextFormat {
        font_id: egui::FontId::proportional(size),
        color: colour.gamma_multiply(alpha),
        ..Default::default()
    };
    text.append(
        &format!("{} LENS\n", lens.name().to_uppercase()),
        0.0,
        egui::TextFormat {
            font_id: cellworks_workspace::theme::named(ui.ctx(), 34.0, "bold"),
            color: super::case::ATTENTION.gamma_multiply(alpha),
            extra_letter_spacing: 34.0 * 0.14,
            ..Default::default()
        },
    );
    text.append(lens.blurb(), 0.0, line(15.0, DIM));
    text.halign = Align::Center;
    egui::Area::new(ui.id().with("lens banner"))
        .pivot(Align2::CENTER_CENTER)
        .fixed_pos(panel.center_top() + vec2(0.0, panel.height() * 0.22))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            ui.label(text);
        });
    ui.ctx().request_repaint();
}

/// The legend's text for the lanes `shown`: it starts with "Lanes: ".
pub fn legend_text(shown: Lanes) -> String {
    let mut lines = vec!["Lanes: what connects the towers.".to_owned()];
    if shown.imports {
        lines.push("Import lanes: imports between files, along the ground.".to_owned());
        lines.push(
            "Between districts they run in a highway, one lane per pair of files.".to_owned(),
        );
    }
    if shown.calls {
        lines.push(
            "Call lanes: calls from a line to the function it calls, through the air.".to_owned(),
        );
    }
    lines.push("Colour: the building a lane starts from.".to_owned());
    lines.push(
        "Traffic runs downstream now and then; select a tower to follow its lanes.".to_owned(),
    );
    lines.join("\n")
}

/// The legend of the lanes `shown`, at the bottom left of `panel`; nothing
/// when no lane is shown.
pub fn legend(ui: &egui::Ui, panel: Rect, shown: Lanes) {
    if !shown.imports && !shown.calls {
        return;
    }
    // The minimap holds the bottom right corner: the legend wraps short of
    // it, or stands above it when the map is too narrow for both.
    let margin = super::MINIMAP_MARGIN;
    let room = panel.width() - super::MINIMAP.x - 4.0 * margin;
    let (width, bottom) = if room >= 240.0 {
        (room, panel.bottom() - margin)
    } else {
        (
            panel.width() - 2.0 * margin,
            panel.bottom() - super::MINIMAP.y - 2.0 * margin,
        )
    };
    let drawn = egui::Area::new(ui.id().with("lane legend"))
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(pos2(panel.left() + margin, bottom))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            ui.set_max_width(width.max(120.0));
            card().show(ui, |ui| {
                ui.label(titled(ui.ctx(), &legend_text(shown)));
            });
        });
    cellworks_workspace::mark(ui.ctx(), "slot.legend", drawn.response.rect);
}
