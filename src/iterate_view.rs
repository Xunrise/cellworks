//! PROTOTYPE (proto/iterate-loop): stages 4 (iterate) and 5 (freeze) of a
//! ticket case, after the "Iterate loop" mock. The case draws its bar; this
//! draws below it, on the same sheet, down to the session pane: the open
//! worktree as a flat plan in the map well, the rounds beside it and the
//! feedback bar under it. `m` swaps the plan for the live city: 0 shows the
//! plan, 1 the city, values between are the move from one to the other.

use std::path::Path;

use cellworks_map::ProjectMap;
use cellworks_map::ui::case::{
    ATTENTION, ATTENTION_SOFT, BG, BG_DEEP, DANGER, DANGER_FILL, DANGER_TEXT, FLOW, Font, INFO,
    INK, INK_DIM, LABEL, LINE, LINE_STRONG, MONO11, MONO12, OK, ON_ATTENTION, PANEL,
    PANEL_RAISED, Sheet, TERRAIN_EDGE, TERRAIN_INK, WARN, button, ease, font, sheet_frame,
};
use egui::{Align2, Color32, FontId, Id, Painter, Rect, Shape, Stroke, Ui, pos2, vec2};

use crate::iterate::Iterate;

/// What the screen keeps between frames.
#[derive(Default)]
pub struct ViewState {
    pub note: String,
    pub pin_armed: bool,
    /// Whether the city shows instead of the plan, and when that changed.
    pub city: bool,
    pub swapped: f64,
    /// The file open in the code panel: its path, which round's view, its
    /// lines, and when it opened.
    pub file: Option<OpenFile>,
    /// On the review: the base, what changed since it, and when it was read.
    pub review: Option<Review>,
}

pub struct Review {
    pub base: String,
    pub files: Vec<String>,
    pub commits: Vec<String>,
    pub read: f64,
}

pub struct OpenFile {
    pub rel: String,
    pub label: String,
    pub lines: Vec<crate::iterate::CodeLine>,
    pub opened: f64,
}

impl ViewState {
    /// 0 on the plan, 1 on the city, eased in between over 0.7 s.
    pub fn m(&self, now: f64) -> f32 {
        let t = ease(((now - self.swapped) / 0.7) as f32);
        if self.city { t } else { 1.0 - t }
    }

    pub fn swap(&mut self, now: f64) {
        // From wherever the move stands, so a swap mid-move turns back.
        let m = self.m(now);
        self.city = !self.city;
        let t = if self.city { m } else { 1.0 - m };
        self.swapped = now - inverse_ease(t) as f64 * 0.7;
    }
}

fn inverse_ease(target: f32) -> f32 {
    let (mut low, mut high) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (low + high) / 2.0;
        if ease(mid) < target {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) / 2.0
}

/// What the engineer asked for this frame.
pub enum Act {
    Send,
    CloseRound,
    Undo(usize),
    View(Option<usize>),
    Pin(String),
    ArmPin,
    Capture,
    Open,
    Swap,
    Freeze,
    OpenFile(String),
    CloseFile,
    Launch(bool),
    Reviewed,
}

const CTL: Font = font(12.0, "semibold", 0.12, 1.25);
const CTL_BOLD: Font = font(12.0, "bold", 0.12, 1.25);
const CTA: Font = font(16.0, "bold", 0.14, 1.25);
const BODY: Font = font(13.0, "sans", 0.0, 1.4);
const TITLE14: Font = font(14.0, "semibold", 0.0, 1.35);
const MONO_BOLD15: Font = font(15.0, "mono-bold", 0.0, 1.3);
const MONO_BOLD12: Font = font(12.0, "mono-bold", 0.0, 1.3);
const SESSION_1: Color32 = Color32::from_rgb(0xff, 0x5c, 0x7a);
const NODE: Color32 = Color32::from_rgb(0x18, 0x20, 0x2a);
const NODE_EDGE: Color32 = Color32::from_rgb(0x2a, 0x36, 0x44);
const NODE_HEADER: Color32 = Color32::from_rgb(0x22, 0x2d, 0x3a);

/// The pulse of an attention ring.
fn pulse(now: f64) -> f32 {
    0.625 + 0.275 * (std::f32::consts::TAU * now as f32 / 1.6).cos()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Draws stage `stage` (4 or 5) below the case bar, down to `bottom` on
/// screen, and answers what was asked.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    stage: u8,
    entered: f64,
    bottom: f32,
    vs: &mut ViewState,
    map: Option<&ProjectMap>,
    it: &Iterate,
    runner: Option<(&str, &Path)>,
    factory: bool,
    pane_right: Option<bool>,
    now: f64,
) -> (Vec<Act>, Option<Rect>) {
    let mut acts = Vec::new();
    let screen = ui.ctx().content_rect();
    let (o, k) = sheet_frame(screen);
    let s = Sheet {
        painter: ui.painter().clone(),
        o,
        k,
        dy: 0.0,
    };
    // Sheet units from the top of the sheet down to the pane.
    let h = ((bottom - o.y) / k).min(1000.0);
    let m = if stage == 4 || stage == 6 { vs.m(now) } else { 0.0 };
    let below = Rect::from_min_max(pos2(screen.min.x, s.p(0.0, 64.0).y), pos2(screen.max.x, bottom));

    // The plan's ground, fading out as the city comes through, and taking
    // the pointer from the city while it is mostly the plan.
    s.painter.rect_filled(below, 0.0, BG.gamma_multiply(1.0 - m));
    if m < 0.5 {
        ui.interact(below, Id::new("iterate ground"), egui::Sense::click_and_drag());
    }
    if stage == 5 {
        if freeze(ui, &s, it, entered, now, h, factory) {
            acts.push(Act::Freeze);
        }
        return (acts, None);
    }

    // The map well: the left column on the plan, the whole window on the
    // city.
    let well = [
        lerp(24.0, 0.0, m),
        lerp(80.0, 64.0, m),
        lerp(1092.0, 1600.0, m),
        lerp(h - 104.0, h - 64.0, m),
    ];
    let wr = s.r(well[0], well[1], well[2], well[3]);
    if let Some(map) = map
        && m < 1.0
    {
        let mut p = s.painter.with_clip_rect(wr.intersect(below));
        p.multiply_opacity(1.0 - m);
        p.rect_filled(wr, 0.0, BG_DEEP);
        p.rect_stroke(wr, 0.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
        let inner = Rect::from_min_max(wr.min + vec2(40.0, 76.0) * k, wr.max - vec2(40.0, 120.0) * k);
        plan(ui, &p, inner, map, it, vs, runner, m < 0.5, now, &mut acts);
    }
    overlays(ui, &s, vs, it, well, entered, now, &mut acts);
    if m < 0.5 && vs.file.is_some() {
        let mut c = s.clone();
        c.painter.multiply_opacity(1.0 - 2.0 * m);
        code_panel(ui, &c, vs, it, well, now, &mut acts);
    }
    // Docked right, the session's pane takes the lower part of the column:
    // its header when folded, half the column when open.
    let column_bottom = h - 24.0;
    let pane_h = match pane_right {
        Some(true) => 64.0,
        Some(false) => ((column_bottom - 80.0) * 0.5).max(240.0),
        None => 0.0,
    };
    let rounds_bottom = if pane_right.is_some() { column_bottom - pane_h - 12.0 } else { column_bottom };
    if m < 1.0 {
        let mut r = s.rising(entered, now, 0);
        r.painter.multiply_opacity(1.0 - m);
        match &vs.review {
            Some(review) if stage == 6 => {
                if review_column(ui, &r, review, 1140.0 + 460.0 * m, rounds_bottom, &mut acts) {
                    acts.push(Act::Reviewed);
                }
            }
            _ => rounds(ui, &r, it, 1140.0 + 460.0 * m, rounds_bottom, &mut acts),
        }
    }
    let slot = pane_right.map(|_| s.r(1140.0, column_bottom - pane_h, 436.0, pane_h));
    (acts, slot)
}

/// The open worktree as a flat plan: districts, folders and files from the
/// map's own layout, seen from straight above.
#[allow(clippy::too_many_arguments)]
fn plan(
    ui: &Ui,
    p: &Painter,
    inner: Rect,
    map: &ProjectMap,
    it: &Iterate,
    vs: &ViewState,
    runner: Option<(&str, &Path)>,
    live: bool,
    now: f64,
    acts: &mut Vec<Act>,
) {
    let ext = map.extent;
    if ext.width() <= 0.0 || ext.height() <= 0.0 {
        return;
    }
    let m = (inner.width() / ext.width()).min(inner.height() / ext.height());
    let origin = inner.center() - ext.size() * m / 2.0;
    let to = |r: Rect| Rect::from_min_max(origin + (r.min - ext.min) * m, origin + (r.max - ext.min) * m);
    let rel = |path: &Path| path.strip_prefix(&map.root).unwrap_or(path).to_string_lossy().into_owned();
    let tints = [
        Color32::from_rgb(0x3f, 0xd7, 0xe6),
        Color32::from_rgb(0xe0, 0x5c, 0xff),
        Color32::from_rgb(0x9c, 0xff, 0x57),
        Color32::from_rgb(0xff, 0x8a, 0x3d),
        Color32::from_rgb(0x5a, 0x8c, 0xff),
        Color32::from_rgb(0xff, 0x5c, 0x7a),
        Color32::from_rgb(0x3d, 0xff, 0xc8),
    ];
    let viewing: Option<&[String]> = it.viewing.map(|_| it.scaffolding());
    let scaffold: &[String] = match &vs.review {
        Some(review) => &review.files,
        None => it.scaffolding(),
    };
    let fires = it.status.fires.as_slice();
    let pinned = it
        .draft
        .as_ref()
        .and_then(|d| d.anchor.as_deref())
        .map(|a| a.split(':').next().unwrap_or(a).to_owned());
    let semibold = |size: f32| FontId::new(size, egui::FontFamily::Name("semibold".into()));
    let mono_bold = FontId::new(12.0, egui::FontFamily::Name("mono-bold".into()));

    fn groups(gs: &[cellworks_map::Group], out: &mut Vec<(Rect, std::path::PathBuf, usize, bool)>) {
        for g in gs {
            out.push((g.area, g.path.clone(), 0, true));
            for n in &g.nodes {
                out.push((n.area, n.path.clone(), n.items.len(), false));
            }
            groups(&g.groups, out);
        }
    }
    let mut files: Vec<(Rect, String, usize)> = Vec::new();
    for (i, d) in map.districts.iter().enumerate() {
        let tint = tints[i % tints.len()];
        let r = to(d.area);
        chamfer(p, r, tint.gamma_multiply(0.05), tint.gamma_multiply(0.3), 14.0_f32.min(r.height() / 4.0));
        if r.width() > 60.0 {
            p.text(r.min + vec2(10.0, 8.0), Align2::LEFT_TOP, d.name.to_uppercase(), semibold(12.0), INK);
        }
        let mut parts = Vec::new();
        for n in &d.nodes {
            parts.push((n.area, n.path.clone(), n.items.len(), false));
        }
        for sd in &d.sub_districts {
            parts.push((sd.area, sd.path.clone(), 0, true));
            for n in &sd.nodes {
                parts.push((n.area, n.path.clone(), n.items.len(), false));
            }
            groups(&sd.groups, &mut parts);
        }
        for (area, path, count, folder) in parts {
            let r = to(area);
            if folder {
                p.rect_stroke(r, 0.0, Stroke::new(1.0, TERRAIN_EDGE), egui::StrokeKind::Inside);
                if r.width() > 50.0 && r.height() > 30.0 {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_uppercase()).unwrap_or_default();
                    p.text(r.min + vec2(6.0, 4.0), Align2::LEFT_TOP, name, semibold(10.0), TERRAIN_INK);
                }
            } else {
                files.push((r, rel(&path), count));
            }
        }
    }
    for (r, path, count) in &files {
        let lit = viewing.is_none_or(|v| v.iter().any(|f| f == path));
        let a = ui.ctx().animate_value_with_time(Id::new(("iterate dim", path)), if lit { 1.0 } else { 0.22 }, 0.4);
        let mut fp = p.clone();
        fp.multiply_opacity(a);
        let r = r.shrink(1.0);
        let response = ui.interact(r, Id::new(("iterate file", path)), egui::Sense::click());
        let fire_lines: Vec<u32> = fires.iter().filter(|(f, _, _)| f == path).map(|(_, l, _)| *l).collect();
        let changed = scaffold.iter().any(|f| f == path);
        let is_pinned = pinned.as_deref() == Some(path.as_str());
        let edge = if is_pinned {
            ATTENTION
        } else if response.hovered() {
            LINE_STRONG
        } else {
            NODE_EDGE
        };
        chamfer(&fp, r, NODE, edge, 6.0_f32.min(r.height() / 3.0));
        let bar = if !fire_lines.is_empty() {
            DANGER
        } else if changed {
            WARN
        } else {
            NODE_HEADER
        };
        let bh = (r.height() * 0.12).clamp(2.0, 6.0);
        fp.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), bh)), 0.0, bar);
        let name = Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let size = (r.height() * 0.22).clamp(9.0, 13.0);
        if r.width() > 40.0 && r.height() > 18.0 {
            let clip = fp.with_clip_rect(r.shrink(2.0));
            clip.text(r.min + vec2(8.0, bh + 4.0), Align2::LEFT_TOP, &name, FontId::monospace(size), if lit { INK } else { INK_DIM });
            if r.height() > 52.0 {
                clip.text(pos2(r.min.x + 8.0, r.max.y - 6.0), Align2::LEFT_BOTTOM, format!("{count} items"), FontId::monospace(size - 2.0), INK_DIM);
            }
        }
        if changed {
            let rr = r.expand(5.0);
            let pts = [rr.left_top(), rr.right_top(), rr.right_bottom(), rr.left_bottom(), rr.left_top()];
            fp.extend(Shape::dashed_line(&pts, Stroke::new(2.0, WARN), 6.0, 4.0));
        }
        if !fire_lines.is_empty() {
            let b = Rect::from_center_size(pos2(r.max.x - 8.0, r.min.y - 4.0), vec2(20.0, 20.0));
            fp.rect_filled(b, 0.0, DANGER_FILL);
            fp.rect_stroke(b, 0.0, Stroke::new(1.0, DANGER), egui::StrokeKind::Inside);
            fp.text(b.center(), Align2::CENTER_CENTER, "!", mono_bold.clone(), DANGER_TEXT);
            let lines = fire_lines.iter().map(|l| format!("L{l}")).collect::<Vec<_>>().join(" ");
            fp.text(b.right_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, lines, mono_bold.clone(), DANGER_TEXT);
        }
        if is_pinned {
            fp.rect_stroke(r.expand(8.0), 0.0, Stroke::new(2.0, ATTENTION.gamma_multiply(pulse(now))), egui::StrokeKind::Middle);
        }
        if let Some((name, at)) = runner
            && Path::new(path) == at
        {
            let c = pos2(r.min.x + 12.0, r.max.y - 10.0);
            fp.circle_filled(c, 7.0, SESSION_1);
            fp.circle_stroke(c, 9.5, Stroke::new(1.0, SESSION_1));
            let g = fp.layout_no_wrap(format!("{name} · writing"), FontId::monospace(11.0), INK_DIM);
            let br = Rect::from_min_size(c + vec2(16.0, -9.0), g.size() + vec2(12.0, 4.0));
            fp.rect_filled(br, 0.0, BG_DEEP.gamma_multiply(0.85));
            fp.galley(br.min + vec2(6.0, 2.0), g, INK_DIM);
        }
        if live && response.clicked() {
            acts.push(if vs.pin_armed { Act::Pin(path.clone()) } else { Act::OpenFile(path.clone()) });
        }
        if vs.file.as_ref().is_some_and(|f| &f.rel == path) {
            fp.rect_stroke(r.expand(3.0), 0.0, Stroke::new(1.5, FLOW), egui::StrokeKind::Middle);
        }
        if live && response.hovered() && vs.pin_armed {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }
}

fn chamfer(p: &Painter, r: Rect, fill: Color32, edge: Color32, cut: f32) {
    let r = r.shrink(0.5);
    let c = cut.max(0.0);
    let pts = vec![
        pos2(r.min.x + c, r.min.y),
        pos2(r.max.x, r.min.y),
        pos2(r.max.x, r.max.y - c),
        pos2(r.max.x - c, r.max.y),
        pos2(r.min.x, r.max.y),
        pos2(r.min.x, r.min.y + c),
    ];
    p.add(Shape::convex_polygon(pts, fill, Stroke::new(1.0, edge)));
}

/// A control: 0 outline, 1 primary, 2 armed, 3 danger.
fn ctl(ui: &Ui, s: &Sheet, r: Rect, label: &str, key: Option<&str>, kind: u8) -> bool {
    ui.interact(r, Id::new(("iterate block", label)), egui::Sense::hover());
    let response = button(ui, r, label);
    let (fill, edge, ink) = match kind {
        0 => (Color32::TRANSPARENT, if response.hovered() { FLOW } else { LINE_STRONG }, INK),
        1 => (ATTENTION, ATTENTION, ON_ATTENTION),
        2 => (ATTENTION_SOFT, ATTENTION, ATTENTION),
        _ => (Color32::TRANSPARENT, DANGER, DANGER_TEXT),
    };
    s.plate(r, fill, edge, 8.0);
    let f = if kind == 1 { CTL_BOLD } else { CTL };
    let text = if key.is_some() { format!("{label} ") } else { label.to_owned() };
    let tw = s.width(&text, f) + key.map_or(0.0, |k| s.width(k, MONO12));
    let x = (r.center().x - s.o.x) / s.k - tw / 2.0;
    let y = (r.center().y - s.o.y) / s.k - s.dy;
    s.put(x, y, Align2::LEFT_CENTER, &text, f, ink);
    if let Some(key) = key {
        s.put(x + s.width(&text, f), y, Align2::LEFT_CENTER, key, MONO12, ink);
    }
    response.clicked()
}

fn cta(ui: &Ui, s: &Sheet, [x, y, w]: [f32; 3], title: &str, sub: &str, key: &str) -> bool {
    let r = s.r(x, y, w, 72.0);
    let response = button(ui, r, title);
    let glow = egui::epaint::Shadow {
        offset: [0, 0],
        blur: (14.0 * s.k) as u8,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(252, 238, 10, 115),
    };
    s.painter.add(glow.as_shape(r, 0.0));
    let fill = if response.hovered() { Color32::from_rgb(0xff, 0xf5, 0x5a) } else { ATTENTION };
    s.plate(r, fill, Color32::TRANSPARENT, 8.0);
    s.put(x + 24.0, y + 26.0, Align2::LEFT_CENTER, title, CTA, ON_ATTENTION);
    s.put(x + 24.0, y + 48.0, Align2::LEFT_CENTER, sub, BODY, ON_ATTENTION);
    s.put(x + w - 24.0, y + 36.0, Align2::RIGHT_CENTER, key, font(18.0, "mono", 0.0, 1.2), ON_ATTENTION);
    response.clicked()
}

/// What sits on the well whichever way it shows: where the app runs, the
/// plan/city toggle, the legend and the feedback bar.
#[allow(clippy::too_many_arguments)]
fn overlays(ui: &mut Ui, s0: &Sheet, vs: &mut ViewState, it: &Iterate, [wx, wy, ww, wh]: [f32; 4], entered: f64, now: f64, acts: &mut Vec<Act>) {
    let s = s0.rising(entered, now, 1);
    let x = wx + 16.0;
    let y = wy + 16.0;
    // Where it runs.
    let (glyph, label, ink) = match (it.status.port, it.status.live) {
        (Some(port), true) => ("●", format!("LIVE :{port}"), OK),
        (Some(port), false) => ("○", format!("DOWN :{port}"), INK_DIM),
        (None, _) => ("○", "NO LAUNCH.JSON".to_owned(), INK_DIM),
    };
    let head = format!("{glyph} {label}");
    let sub = ".claude/launch.json";
    let w1 = s.width(&head, MONO_BOLD12) + 12.0 + s.width(sub, MONO11) + 14.0;
    let open = format!("OPEN :{}", it.status.port.unwrap_or(0));
    let w2 = s.width(&open, CTL) + 32.0;
    let badge = s.r(x, y, w1 + w2, 44.0);
    ui.interact(badge, Id::new("iterate badge"), egui::Sense::hover());
    s.plate(badge, PANEL, LINE, 0.0);
    s.put(x + 14.0, y + 22.0, Align2::LEFT_CENTER, &head, MONO_BOLD12, ink);
    s.put(x + 14.0 + s.width(&head, MONO_BOLD12) + 12.0, y + 22.0, Align2::LEFT_CENTER, sub, MONO11, INK_DIM);
    let br = s.r(x + w1, y, w2, 44.0);
    let r = button(ui, br, "Open in browser");
    s.painter.vline(br.min.x, br.y_range(), Stroke::new(1.0, LINE));
    s.put(x + w1 + w2 / 2.0, y + 22.0, Align2::CENTER_CENTER, &open, CTL, if r.hovered() { ATTENTION } else { FLOW });
    if r.clicked() {
        acts.push(Act::Open);
    }

    // Plan or city.
    let seg = ["PLAN", "CITY"];
    let sw: Vec<f32> = seg.iter().map(|t| s.width(t, CTL) + 36.0).collect();
    let tx = wx + ww / 2.0 - (sw[0] + sw[1] + s.width("V", MONO12) + 24.0) / 2.0;
    let total = s.r(tx, y, sw[0] + sw[1], 44.0);
    ui.interact(total, Id::new("iterate seg"), egui::Sense::hover());
    s.plate(total, PANEL, LINE, 0.0);
    let mut sx = tx;
    for (i, t) in seg.iter().enumerate() {
        let r = s.r(sx, y, sw[i], 44.0);
        let on = (i == 1) == vs.city;
        let response = button(ui, r, &format!("Show the {}", t.to_lowercase()));
        if on {
            s.plate(r, FLOW, FLOW, 0.0);
        } else if response.hovered() {
            s.plate(r, PANEL_RAISED, Color32::TRANSPARENT, 0.0);
        }
        s.put(sx + sw[i] / 2.0, y + 22.0, Align2::CENTER_CENTER, t, CTL, if on { ON_ATTENTION } else { INK_DIM });
        if response.clicked() && !on {
            acts.push(Act::Swap);
        }
        sx += sw[i];
    }
    s.put(sx + 12.0, y + 22.0, Align2::LEFT_CENTER, "V", MONO12, INK_DIM);

    // Legend.
    let of = match it.viewing {
        Some(k) => format!("changed in round {}", k + 1),
        None => "changed in the open round".to_owned(),
    };
    let lw = 46.0 + s.width(&of, MONO11).max(s.width("error in the run log", MONO11)) + 14.0;
    let lx = wx + ww - 16.0 - lw;
    let legend = s.r(lx, y, lw, 74.0);
    ui.interact(legend, Id::new("iterate legend"), egui::Sense::hover());
    s.plate(legend, PANEL, LINE, 0.0);
    let dash = [s.p(lx + 14.0, y + 17.0), s.p(lx + 36.0, y + 17.0)];
    s.painter.extend(Shape::dashed_line(&dash, Stroke::new(2.0, WARN), 4.0, 3.0));
    s.put(lx + 44.0, y + 17.0, Align2::LEFT_CENTER, &of, MONO11, INK_DIM);
    s.put(lx + 25.0, y + 37.0, Align2::CENTER_CENTER, "!", font(11.0, "mono-bold", 0.0, 1.3), DANGER_TEXT);
    s.put(lx + 44.0, y + 37.0, Align2::LEFT_CENTER, "error in the run log", MONO11, INK_DIM);
    s.painter.circle_filled(s.p(lx + 25.0, y + 57.0), 5.0 * s.k, SESSION_1);
    s.put(lx + 44.0, y + 57.0, Align2::LEFT_CENTER, "the session", MONO11, INK_DIM);

    // Feedback bar.
    let s = s0.rising(entered, now, 2);
    let open_round = it.rounds.len();
    let (bx, bw) = (wx + 16.0, ww - 32.0);
    let bh = 72.0;
    let by = wy + wh - 16.0 - bh;
    if let Some((text, at)) = &it.toast
        && at.elapsed().as_secs_f32() < 8.0
    {
        let t = ease(at.elapsed().as_secs_f32() / 0.3);
        let mut ts = s.clone();
        ts.painter.multiply_opacity(t);
        ts.dy += 8.0 * (1.0 - t);
        ts.plate(ts.r(bx, by - 40.0, bw, 36.0), PANEL_RAISED, LINE, 0.0);
        let clip = ts.painter.with_clip_rect(ts.r(bx + 16.0, by - 40.0, bw - 32.0, 36.0));
        let g = ts.galley(text, MONO12, FLOW, 0.0);
        clip.galley(Align2::LEFT_CENTER.anchor_size(ts.p(bx + 16.0, by - 22.0), g.size()).min, g, FLOW);
    }
    let bar = s.r(bx, by, bw, bh);
    ui.interact(bar, Id::new("iterate bar"), egui::Sense::click());
    s.plate(bar, PANEL, LINE_STRONG, 16.0);
    let cy = by + bh / 2.0;
    let eyebrow = format!("FEEDBACK · ROUND {open_round}");
    s.put(bx + 16.0, cy, Align2::LEFT_CENTER, &eyebrow, LABEL, INK_DIM);
    let mut x = bx + 24.0 + s.width(&eyebrow, LABEL);
    let draft = it.draft.as_ref();
    if let Some(anchor) = draft.and_then(|d| d.anchor.as_deref()) {
        let t = format!("@ {anchor}");
        let w = s.width(&t, MONO12) + 16.0;
        s.plate(s.r(x, cy - 13.0, w, 26.0), ATTENTION_SOFT, Color32::TRANSPARENT, 0.0);
        s.put(x + 8.0, cy, Align2::LEFT_CENTER, &t, MONO12, ATTENTION);
        x += w + 8.0;
    }
    if let Some(image) = draft.and_then(|d| d.image.as_ref()) {
        let t = format!("▣ {}", image.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let w = s.width(&t, MONO12) + 16.0;
        s.plate(s.r(x, cy - 13.0, w, 26.0), Color32::from_rgb(0x06, 0x26, 0x2a), Color32::TRANSPARENT, 0.0);
        s.put(x + 8.0, cy, Align2::LEFT_CENTER, &t, MONO12, FLOW);
        x += w + 8.0;
    }
    let pin_w = s.width("PIN P", CTL) + 30.0;
    let cap_w = s.width("CAPTURE C", CTL) + 30.0;
    let send_w = s.width("SEND Enter", CTL_BOLD) + 36.0;
    let right = bx + bw - 16.0;
    let fx1 = right - send_w - cap_w - pin_w - 24.0;
    let field = s.r(x, cy - 22.0, (fx1 - x).max(80.0), 44.0);
    s.painter.rect_filled(field, 0.0, BG_DEEP);
    s.painter.rect_stroke(field, 0.0, Stroke::new(1.0, FLOW), egui::StrokeKind::Inside);
    let hint = if vs.pin_armed {
        if vs.city { "Select a tower, then P pins it" } else { "Click a file on the plan to pin it" }
    } else {
        "What should change? Enter sends it to the session"
    };
    let edit = ui.put(
        field.shrink2(vec2(12.0 * s.k, 0.0)),
        egui::TextEdit::singleline(&mut vs.note)
            .id(Id::new("iterate-note"))
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center)
            .font(FontId::monospace(13.0 * s.k))
            .text_color(INK)
            .hint_text(egui::RichText::new(hint).color(INK_DIM).font(FontId::monospace(13.0 * s.k))),
    );
    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        acts.push(Act::Send);
    }
    let mut x = fx1 + 8.0;
    if ctl(ui, &s, s.r(x, cy - 22.0, pin_w, 44.0), "PIN", Some("P"), if vs.pin_armed { 2 } else { 0 }) {
        acts.push(Act::ArmPin);
    }
    x += pin_w + 8.0;
    if ctl(ui, &s, s.r(x, cy - 22.0, cap_w, 44.0), "CAPTURE", Some("C"), 0) {
        acts.push(Act::Capture);
    }
    x += cap_w + 8.0;
    if ctl(ui, &s, s.r(x, cy - 22.0, send_w, 44.0), "SEND", Some("Enter"), 1) {
        acts.push(Act::Send);
    }
}

fn rounds(ui: &Ui, s: &Sheet, it: &Iterate, x: f32, bottom: f32, acts: &mut Vec<Act>) {
    let (y, w) = (80.0, 436.0);
    let ph = bottom - y;
    let h = bottom + 24.0;
    let panel = s.r(x, y, w, ph);
    ui.interact(panel, Id::new("iterate rounds"), egui::Sense::click());
    s.plate(panel, PANEL, LINE, 0.0);
    let (ix, iw) = (x + 24.0, w - 48.0);
    let mut cy = y + 24.0;
    s.put(ix, cy, Align2::LEFT_TOP, "2 · ROUNDS", LABEL, INK_DIM);
    cy += 28.0;
    let open = it.rounds.len() - 1;
    let notes = it.notes_in_round();
    for (k, round) in it.rounds.iter().enumerate() {
        let is_open = k == open;
        let on = it.viewing == Some(k) || (is_open && it.viewing.is_none());
        let title = if round.note.is_empty() {
            if is_open { "Open: what the session is doing now.".to_owned() } else { "No note".to_owned() }
        } else {
            round.note.clone()
        };
        let meta = if is_open {
            format!("{notes} notes · {} files touched · live", it.status.touched.len())
        } else {
            let names: Vec<&str> = round.files.iter().take(3).map(|f| f.rsplit('/').next().unwrap_or(f)).collect();
            format!("{} files · {}", round.files.len(), names.join(", "))
        };
        let th = s.height(&title, TITLE14, iw - 66.0).min(57.0);
        let ch = (th + 40.0).max(76.0);
        let r = s.r(ix, cy, iw, ch);
        let response = button(ui, r, &format!("Round {}", k + 1));
        let (fill, edge, id_ink) = match (is_open, on) {
            (true, true) => (Color32::from_rgb(0x0b, 0x2a, 0x2e), FLOW, FLOW),
            (true, false) => (PANEL_RAISED, LINE, FLOW),
            (false, true) => (ATTENTION_SOFT, ATTENTION, ATTENTION),
            (false, false) => (PANEL_RAISED, if response.hovered() { LINE_STRONG } else { LINE }, OK),
        };
        s.plate(r, fill, edge, 8.0);
        s.put(ix + 14.0, cy + 14.0, Align2::LEFT_TOP, &format!("R{}", k + 1), MONO_BOLD15, id_ink);
        let clip = s.painter.with_clip_rect(s.r(ix + 54.0, cy + 10.0, iw - 66.0, th + 2.0));
        let g = s.galley(&title, TITLE14, INK, iw - 66.0);
        clip.galley(s.p(ix + 54.0, cy + 12.0), g, INK);
        s.put(ix + 54.0, cy + 16.0 + th, Align2::LEFT_TOP, &meta, MONO11, INK_DIM);
        if response.clicked() {
            acts.push(Act::View(if is_open || on { None } else { Some(k) }));
        }
        cy += ch + 8.0;
        if cy > h - 300.0 {
            break;
        }
    }
    if let Some(k) = it.viewing {
        let label = format!("UNDO ROUND {} AND LATER · TELLS THE SESSION", k + 1);
        if ctl(ui, s, s.r(ix, cy, iw, 44.0), &label, None, 3) {
            acts.push(Act::Undo(k));
        }
        cy += 52.0;
    }
    cy += 8.0;
    s.put(ix, cy, Align2::LEFT_TOP, "RUN LOG · .cellworks/run.log", LABEL, INK_DIM);
    cy += 26.0;
    if it.status.fires.is_empty() {
        s.plate(s.r(ix, cy, iw, 44.0), Color32::TRANSPARENT, LINE, 0.0);
        s.put(ix + 16.0, cy + 22.0, Align2::LEFT_CENTER, "✓ no file:line since the last clean build", MONO11, INK_DIM);
    } else {
        for (file, line, text) in it.status.fires.iter().take(2) {
            let th = s.height(text, BODY, iw - 32.0).min(54.0);
            let r = s.r(ix, cy, iw, th + 44.0);
            let response = button(ui, r, "Run log line");
            s.plate(r, PANEL, DANGER, 0.0);
            s.put(ix + 16.0, cy + 14.0, Align2::LEFT_TOP, &format!("{file}:{line}"), MONO_BOLD12, DANGER_TEXT);
            let clip = s.painter.with_clip_rect(r.shrink(2.0));
            let g = s.galley(text, BODY, INK, iw - 32.0);
            clip.galley(s.p(ix + 16.0, cy + 32.0), g, INK);
            if response.clicked() {
                acts.push(Act::Pin(format!("{file}:{line}")));
            }
            cy += th + 52.0;
        }
    }
    let sub = format!("{notes} notes · snapshot the worktree, open round {}", open + 2);
    if cta(ui, s, [ix, y + ph - 24.0 - 72.0, iw], &format!("CLOSE ROUND {}", open + 1), &sub, "R") {
        acts.push(Act::CloseRound);
    }
}

/// The file open from the plan, over the right of the well: the round's
/// diff, or the file when the round did not touch it. A numbered line pins
/// `file:line` for the feedback bar.
fn code_panel(ui: &mut Ui, s0: &Sheet, vs: &ViewState, it: &Iterate, [wx, wy, ww, wh]: [f32; 4], now: f64, acts: &mut Vec<Act>) {
    let Some(file) = &vs.file else {
        return;
    };
    let s = s0.rising(file.opened, now, 0);
    let w = 660.0_f32.min(ww - 32.0);
    let (x, y) = (wx + ww - 16.0 - w, wy + 104.0);
    let h = wh - 104.0 - 104.0;
    let panel = s.r(x, y, w, h);
    ui.interact(panel, Id::new("iterate code"), egui::Sense::click_and_drag());
    s.plate(panel, PANEL, LINE_STRONG, 16.0);
    // Header: where, the file, which round, the open actions.
    let (dir, name) = match file.rel.rsplit_once('/') {
        Some((d, n)) => (d.to_owned(), n.to_owned()),
        None => (".".to_owned(), file.rel.clone()),
    };
    s.put(x + 20.0, y + 18.0, Align2::LEFT_TOP, &format!("{} / {dir}", it.worktree.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()), MONO12, INK_DIM);
    s.put(x + 20.0, y + 38.0, Align2::LEFT_TOP, &name, MONO_BOLD15, INK);
    s.put(x + 20.0 + s.width(&name, MONO_BOLD15) + 12.0, y + 40.0, Align2::LEFT_TOP, &file.label, LABEL, if file.label.contains("UNCHANGED") { INK_DIM } else { WARN });
    let close_w = 44.0;
    let ed_w = s.width("OPEN IN EDITOR", CTL) + 32.0;
    let nv_w = s.width("OPEN IN NVIM", CTL_BOLD) + 32.0;
    let mut bx = x + w - 16.0 - close_w;
    if ctl(ui, &s, s.r(bx, y + 16.0, close_w, 44.0), "X", None, 0) {
        acts.push(Act::CloseFile);
    }
    bx -= ed_w + 8.0;
    if ctl(ui, &s, s.r(bx, y + 16.0, ed_w, 44.0), "OPEN IN EDITOR", None, 0) {
        acts.push(Act::Launch(false));
    }
    bx -= nv_w + 8.0;
    if ctl(ui, &s, s.r(bx, y + 16.0, nv_w, 44.0), "OPEN IN NVIM", None, 1) {
        acts.push(Act::Launch(true));
    }
    s.rule(x, x + w, y + 76.0, LINE);

    // Body: the lines, scrolled.
    let body = s.r(x + 1.0, y + 78.0, w - 2.0, h - 80.0);
    let k = s.k;
    let row_h = 22.0 * k;
    let font = FontId::monospace(13.0 * k);
    let pinned = it.draft.as_ref().and_then(|d| d.anchor.clone());
    if file.lines.is_empty() {
        s.put(x + 20.0, y + 100.0, Align2::LEFT_TOP, "Nothing to show for this file.", BODY, INK_DIM);
        return;
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
        egui::ScrollArea::vertical()
            .id_salt(("iterate code scroll", &file.rel))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for (i, line) in file.lines.iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(vec2(body.width(), row_h), egui::Sense::click());
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let p = ui.painter();
                    let anchor = line.no.map(|n| format!("{}:{n}", file.rel));
                    let is_pin = anchor.is_some() && anchor == pinned;
                    let (bg, mark_ink, ink) = match line.mark {
                        '+' => (Color32::from_rgba_unmultiplied(61, 220, 132, 30), OK, OK),
                        '-' => (DANGER_FILL, DANGER, DANGER_TEXT),
                        '…' => (Color32::TRANSPARENT, INK_DIM, INK_DIM),
                        _ => (Color32::TRANSPARENT, INK_DIM, if file.label.contains("UNCHANGED") { INK } else { INK_DIM }),
                    };
                    let bg = if is_pin {
                        ATTENTION_SOFT
                    } else if response.hovered() && line.no.is_some() {
                        PANEL_RAISED
                    } else {
                        bg
                    };
                    p.rect_filled(rect, 0.0, bg);
                    let cy = rect.center().y;
                    if let Some(n) = line.no {
                        p.text(pos2(rect.min.x + 48.0 * k, cy), Align2::RIGHT_CENTER, n.to_string(), font.clone(), INK_DIM);
                    }
                    let mark = if is_pin { ">".to_owned() } else if line.mark == '…' { String::new() } else { line.mark.to_string() };
                    p.text(pos2(rect.min.x + 60.0 * k, cy), Align2::LEFT_CENTER, mark, font.clone(), if is_pin { ATTENTION } else { mark_ink });
                    let clip = p.with_clip_rect(rect.intersect(body));
                    clip.text(pos2(rect.min.x + 80.0 * k, cy), Align2::LEFT_CENTER, &line.text, font.clone(), ink);
                    if response.hovered() && line.no.is_some() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if response.clicked()
                        && let Some(anchor) = anchor
                    {
                        acts.push(Act::Pin(anchor));
                    }
                    let _ = i;
                }
            });
    });
}

/// The review's column: what was built since the base, and the verdict.
fn review_column(ui: &Ui, s: &Sheet, review: &Review, x: f32, bottom: f32, acts: &mut Vec<Act>) -> bool {
    let (y, w) = (80.0, 436.0);
    let ph = bottom - y;
    let panel = s.r(x, y, w, ph);
    ui.interact(panel, Id::new("iterate review"), egui::Sense::click());
    s.plate(panel, PANEL, LINE, 0.0);
    let (ix, iw) = (x + 24.0, w - 48.0);
    let mut cy = y + 24.0;
    s.put(ix, cy, Align2::LEFT_TOP, &format!("5 · REVIEW · SINCE {}", review.base.to_uppercase()), LABEL, INK_DIM);
    cy += 30.0;
    s.put(ix, cy, Align2::LEFT_TOP, &format!("{} commits", review.commits.len()), TITLE14, INK);
    cy += 24.0;
    for commit in review.commits.iter().take(6) {
        let clip = s.painter.with_clip_rect(s.r(ix, cy, iw, 20.0));
        let g = s.galley(commit, MONO12, INK_DIM, 0.0);
        clip.galley(s.p(ix, cy), g, INK_DIM);
        cy += 20.0;
    }
    cy += 16.0;
    s.put(ix, cy, Align2::LEFT_TOP, &format!("{} files", review.files.len()), TITLE14, INK);
    cy += 26.0;
    for file in &review.files {
        if cy > bottom - 130.0 {
            break;
        }
        let r = s.r(ix, cy, iw, 32.0);
        let response = button(ui, r, file);
        s.plate(r, if response.hovered() { PANEL_RAISED } else { Color32::TRANSPARENT }, LINE, 0.0);
        s.put(ix + 12.0, cy + 16.0, Align2::LEFT_CENTER, &format!("~ {file}"), MONO12, WARN);
        if response.clicked() {
            acts.push(Act::OpenFile(file.clone()));
        }
        cy += 38.0;
    }
    cta(ui, s, [ix, bottom - 24.0 - 72.0, iw], "REVIEWED", "marks the case reviewed", "Enter")
}

/// Stage 5: what the rounds confirmed, the feedback log, and the hand-off.
fn freeze(ui: &Ui, s: &Sheet, it: &Iterate, entered: f64, now: f64, h: f32, factory: bool) -> bool {
    let s0 = s.rising(entered, now, 0);
    let (x, mut y, w) = (24.0, 80.0, 520.0);
    s0.put(x, y, Align2::LEFT_TOP, "3 · WHAT THE ROUNDS CONFIRMED", LABEL, INK_DIM);
    y += 28.0;
    for (k, round) in it.rounds.iter().enumerate().take(it.rounds.len() - 1) {
        let note = if round.note.is_empty() { "No note" } else { round.note.as_str() };
        let th = s0.height(note, TITLE14, w - 70.0);
        s0.plate(s0.r(x, y, w, th + 44.0), PANEL_RAISED, LINE, 0.0);
        s0.put(x + 16.0, y + 14.0, Align2::LEFT_TOP, &format!("R{}", k + 1), MONO_BOLD15, OK);
        s0.text(x + 56.0, y + 12.0, note, TITLE14, INK, w - 70.0);
        s0.put(x + 56.0, y + 16.0 + th, Align2::LEFT_TOP, &round.files.join(" · "), MONO11, INK_DIM);
        y += th + 52.0;
        if y > h - 80.0 {
            break;
        }
    }
    let s1 = s.rising(entered, now, 1);
    s1.plate(s1.r(568.0, 80.0, 624.0, h - 104.0), PANEL, LINE_STRONG, 0.0);
    s1.put(588.0, 96.0, Align2::LEFT_TOP, ".cellworks", MONO12, INK_DIM);
    s1.put(588.0, 116.0, Align2::LEFT_TOP, "feedback.log", MONO_BOLD15, INK);
    s1.rule(568.0, 1192.0, 146.0, LINE);
    let log = std::fs::read_to_string(it.worktree.join(".cellworks/feedback.log")).unwrap_or_default();
    let mut ly = 160.0;
    for (i, line) in log.lines().enumerate() {
        let ink = if line.starts_with("---") { INK_DIM } else { INK };
        s1.put(600.0, ly, Align2::RIGHT_TOP, &(i + 1).to_string(), MONO12, INK_DIM);
        ly += s1.text(612.0, ly, line, MONO12, ink, 560.0).max(18.0) + 4.0;
        if ly > h - 60.0 {
            break;
        }
    }
    let s2 = s.rising(entered, now, 2);
    let (hx, mut hy) = (1216.0, 80.0);
    s2.put(hx, hy, Align2::LEFT_TOP, "HANDS OFF", LABEL, INK_DIM);
    hy += 26.0;
    let items: [(&str, &str); 3] = if factory {
        [
            ("PROTOTYPE.MD", "/factory:freeze distills feedback.log and the rounds: what was asked, what changed, what you rejected."),
            ("PLAN", "Freeze settles the planner's questions with you, pushes the branch and labels the issue; stage 4 shows the plan."),
            ("BUILD", "The pipeline builds it from here; stage 5 reviews the change it made."),
        ]
    } else {
        [
            ("PLAN", "The session writes the steps from this prototype into the case file; stage 4 shows them on the map."),
            ("APPROVE", "You approve the plan on stage 4; the session then builds it cleanly and commits, never pushes."),
            ("REVIEW", "Stage 5 shows the built change against the base, file by file."),
        ]
    };
    for (tag, text) in items {
        let th = s2.height(text, BODY, 328.0);
        s2.plate(s2.r(hx, hy, 360.0, th + 44.0), PANEL, LINE, 0.0);
        s2.put(hx + 16.0, hy + 14.0, Align2::LEFT_TOP, tag, MONO_BOLD12, INFO);
        s2.text(hx + 16.0, hy + 32.0, text, BODY, INK, 328.0);
        hy += th + 56.0;
    }
    let (title, sub) = if factory {
        ("FREEZE", "/factory:freeze in the prototype's session")
    } else {
        ("FREEZE", "the session plans from the prototype")
    };
    cta(ui, &s2, [hx, hy + 8.0, 360.0], title, sub, "Enter")
}
