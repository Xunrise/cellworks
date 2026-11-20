//! PROTOTYPE: the Netrunner look shared by every panel of the app: the
//! colour tokens, the two typefaces, chamfered shapes, tracked labels, a
//! chamfered tab and egui's global style. The launcher and the case screens
//! keep their own copies for now; new chrome draws through this module.

use std::sync::Arc;

use egui::{
    Color32, CornerRadius, FontFamily, FontId, Galley, Pos2, Rect, Response, Sense, Shape, Stroke,
    WidgetInfo, WidgetType, pos2, vec2,
};

pub const BG: Color32 = Color32::from_rgb(0x07, 0x09, 0x0c);
pub const WELL: Color32 = Color32::from_rgb(0x04, 0x05, 0x07);
pub const PANEL: Color32 = Color32::from_rgb(0x0d, 0x11, 0x17);
pub const PANEL_RAISED: Color32 = Color32::from_rgb(0x14, 0x1a, 0x22);
pub const LINE: Color32 = Color32::from_rgb(0x1e, 0x27, 0x31);
pub const LINE_BRIGHT: Color32 = Color32::from_rgb(0x34, 0x50, 0x6a);
pub const INK: Color32 = Color32::from_rgb(0xd7, 0xe2, 0xea);
pub const DIM: Color32 = Color32::from_rgb(0x7d, 0x8f, 0xa0);
pub const TERRAIN_INK: Color32 = Color32::from_rgb(0x5f, 0x71, 0x83);
pub const YELLOW: Color32 = Color32::from_rgb(0xfc, 0xee, 0x0a);
pub const YELLOW_SOFT: Color32 = Color32::from_rgb(0x3a, 0x36, 0x08);
pub const ON_YELLOW: Color32 = BG;
pub const CYAN: Color32 = Color32::from_rgb(0x5e, 0xf6, 0xff);
pub const OK: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x84);
pub const WARN: Color32 = Color32::from_rgb(0xff, 0x9f, 0x1c);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x3b, 0x5c);

/// Chakra Petch (or JetBrains Mono when `mono`) at `size`, falling back to
/// egui's own families before the fonts are installed.
pub fn font(ctx: &egui::Context, size: f32, mono: bool) -> FontId {
    let name = if mono {
        "jetbrains-mono"
    } else {
        "chakra-petch"
    };
    let family = FontFamily::Name(name.into());
    if ctx.fonts(|f| f.families().contains(&family)) {
        FontId::new(size, family)
    } else if mono {
        FontId::monospace(size)
    } else {
        FontId::proportional(size)
    }
}

/// The installed family `name` (`semibold`, `bold`, `mono-bold`) at `size`,
/// or egui's proportional family where it is not installed, as in tests.
pub fn named(ctx: &egui::Context, size: f32, name: &str) -> FontId {
    let family = FontFamily::Name(name.into());
    if ctx.fonts(|f| f.families().contains(&family)) {
        FontId::new(size, family)
    } else {
        FontId::proportional(size)
    }
}

/// `text` spread out by 0.14 em, as the eyebrows and labels are.
pub fn tracked(
    ctx: &egui::Context,
    text: &str,
    size: f32,
    colour: Color32,
    mono: bool,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: font(ctx, size, mono),
            color: colour,
            extra_letter_spacing: size * 0.14,
            ..Default::default()
        },
    );
    job
}

/// An upper-case tracked label at 11 px.
pub fn eyebrow(ctx: &egui::Context, text: &str, colour: Color32) -> Arc<Galley> {
    let job = tracked(ctx, &text.to_uppercase(), 11.0, colour, false);
    ctx.fonts_mut(|f| f.layout_job(job))
}

/// The same eyebrow as a widget-free label, for laying out with `ui.label`.
pub fn eyebrow_label(ui: &mut egui::Ui, text: &str, colour: Color32) -> Response {
    let job = tracked(ui.ctx(), &text.to_uppercase(), 11.0, colour, false);
    ui.label(job)
}

/// `rect` cut at top-left and bottom-right.
pub fn chamfer(rect: Rect, cut: f32) -> Vec<Pos2> {
    vec![
        pos2(rect.left(), rect.top() + cut),
        pos2(rect.left() + cut, rect.top()),
        rect.right_top(),
        pos2(rect.right(), rect.bottom() - cut),
        pos2(rect.right() - cut, rect.bottom()),
        rect.left_bottom(),
    ]
}

/// A chamfered panel surface with a bright edge.
pub fn panel(painter: &egui::Painter, rect: Rect) {
    panel_cut(painter, rect, 12.0);
}

pub fn panel_cut(painter: &egui::Painter, rect: Rect, cut: f32) {
    painter.add(Shape::convex_polygon(
        chamfer(rect, cut),
        PANEL,
        Stroke::new(1.0, LINE_BRIGHT),
    ));
}

/// A chamfered tab: yellow with dark ink when `on`, outlined otherwise. Its
/// label is drawn upper-case and tracked; the accessibility tree keeps
/// `text` as written. A plain button, or a toggle when `toggle` is set.
pub fn tab(ui: &mut egui::Ui, text: &str, on: bool, toggle: bool) -> Response {
    let job = tracked(
        ui.ctx(),
        &text.to_uppercase(),
        11.0,
        Color32::PLACEHOLDER,
        false,
    );
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let size = galley.size() + vec2(14.0, 9.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        if toggle {
            WidgetInfo::selected(WidgetType::Button, true, on, text)
        } else {
            WidgetInfo::labeled(WidgetType::Button, true, text)
        }
    });
    if ui.is_rect_visible(rect) {
        let (fill, edge, ink) = if on {
            (YELLOW, YELLOW, ON_YELLOW)
        } else if response.hovered() {
            (PANEL_RAISED, LINE_BRIGHT, INK)
        } else {
            (Color32::TRANSPARENT, LINE, DIM)
        };
        ui.painter().add(Shape::convex_polygon(
            chamfer(rect, 5.0),
            fill,
            Stroke::new(1.0, edge),
        ));
        let at = rect.center() - galley.size() / 2.0;
        ui.painter().galley(at, galley, ink);
    }
    response
}

/// A thin vertical rule between groups on a bar.
pub fn rule(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 18.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, LINE));
}

/// A flat panel frame for egui panels and areas.
pub fn frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, LINE))
        .inner_margin(egui::Margin::symmetric(12, 10))
}

/// Puts egui's own widgets, windows, tooltips and menus in the look: square
/// corners, panel fills, quiet lines, yellow selection, cyan links.
pub fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = PANEL;
        v.window_fill = PANEL;
        v.extreme_bg_color = WELL;
        v.faint_bg_color = PANEL_RAISED;
        v.code_bg_color = WELL;
        v.window_stroke = Stroke::new(1.0, LINE_BRIGHT);
        v.window_corner_radius = CornerRadius::ZERO;
        v.menu_corner_radius = CornerRadius::ZERO;
        v.window_shadow = egui::Shadow::NONE;
        v.popup_shadow = egui::Shadow::NONE;
        v.hyperlink_color = CYAN;
        v.warn_fg_color = WARN;
        v.error_fg_color = DANGER;
        v.selection.bg_fill = YELLOW_SOFT;
        v.selection.stroke = Stroke::new(1.0, YELLOW);
        v.text_cursor.stroke.color = YELLOW;
        let w = &mut v.widgets;
        for (state, fill, edge, ink) in [
            (&mut w.noninteractive, PANEL, LINE, DIM),
            (&mut w.inactive, PANEL_RAISED, LINE, INK),
            (&mut w.hovered, PANEL_RAISED, LINE_BRIGHT, INK),
            (&mut w.active, YELLOW_SOFT, YELLOW, YELLOW),
            (&mut w.open, PANEL_RAISED, LINE_BRIGHT, INK),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, edge);
            state.fg_stroke = Stroke::new(1.0, ink);
            state.corner_radius = CornerRadius::ZERO;
            state.expansion = 0.0;
        }
        w.noninteractive.weak_bg_fill = PANEL;
        style.spacing.button_padding = vec2(8.0, 3.0);
    });
}
