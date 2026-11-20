//! The Editing bounded context: the editor a file opens in, docked beside the
//! map or popped out into windows of its own.
//!
//! Defined by docs/domain/contexts/Editing/Editing.md. A file opens in an
//! embedded Neovim when `nvim` runs, else in a read-only view of its text.
//! The same editors stand in the city and the flat view: the app owns one
//! [`Editors`] and shows it whatever the map draws.

pub mod nvim;
pub mod tui;

use egui::{
    Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, ViewportBuilder,
    ViewportClass, ViewportId, pos2, vec2,
};
use nvim::Nvim;
use std::path::{Path, PathBuf};
use tui::Tui;

/// The share of the map the editor covers until it is made full-panel.
const OVERLAY_SHARE: f32 = 0.82;
/// How long the editor takes to grow open or to fill the panel, in seconds.
const GROW: f32 = 0.22;
/// The size of the code's font: read comfortably at a glance.
const CODE_SIZE: f32 = 13.5;

/// What a pane shows.
enum Body {
    /// The editor starts once the pane knows its size.
    Starting,
    Nvim(Box<Nvim>),
    /// Another terminal editor, chosen in the settings.
    Tui(Box<Tui>),
    /// The file's lines, read only; `jump` scrolls to the line once.
    Text {
        lines: Vec<String>,
        jump: bool,
    },
    Failed(String),
}

/// One open file.
struct Pane {
    id: u64,
    path: PathBuf,
    title: String,
    line: usize,
    body: Body,
    /// Drawn last frame as a window inside the app rather than its own.
    embedded: bool,
    /// What the header says of the file: its language, lines, functions.
    detail: String,
    /// The terminal editor it runs in instead of Neovim, if any.
    program: Option<String>,
    config: Option<PathBuf>,
}

/// Which pane has the keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Dock,
    Popped(u64),
}

/// Where a pane is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Over the map, filling its panel or not.
    Docked { full: bool },
    /// In a window of its own.
    Popped,
}

/// What a pane asked for while it was drawn.
#[derive(Default)]
struct Asked {
    full: bool,
    focus: bool,
    blur: bool,
    pop_out: bool,
    dock: bool,
    close: bool,
}

/// A pane as the app and its tests read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub path: PathBuf,
    pub line: usize,
    pub popped: bool,
    pub neovim: bool,
}

/// The open editors: one docked beside the map, any number popped out.
pub struct Editors {
    dock: Option<Pane>,
    popped: Vec<Pane>,
    focus: Option<Slot>,
    /// Whether `nvim` runs here; probed on the first open.
    neovim: Option<bool>,
    /// The terminal editor to run instead of Neovim, as set.
    program: Option<String>,
    /// The Neovim configuration to use instead of the user's own, as set.
    config: Option<PathBuf>,
    next: u64,
    scrolled: f32,
    /// The docked editor fills the map's panel rather than floating over it.
    full: bool,
    /// Where the docked editor was drawn last frame.
    drawn: Option<Rect>,
}

impl Default for Editors {
    fn default() -> Self {
        Self::new()
    }
}

impl Editors {
    /// Editors that use Neovim wherever it runs.
    pub fn new() -> Self {
        Editors {
            dock: None,
            popped: Vec::new(),
            focus: None,
            neovim: None,
            program: None,
            config: None,
            next: 0,
            scrolled: 0.0,
            full: false,
            drawn: None,
        }
    }

    /// Editors that only ever show a file's text, for tests and machines
    /// without Neovim.
    pub fn read_only() -> Self {
        Editors {
            neovim: Some(false),
            ..Self::new()
        }
    }

    /// Runs `program` (a terminal editor and its flags) for the files opened
    /// from now on instead of Neovim; Neovim reads `config` instead of the
    /// user's own when one is given. `None` for both: Neovim as configured.
    pub fn configure(&mut self, program: Option<String>, config: Option<PathBuf>) {
        self.program = program.filter(|p| !p.trim().is_empty());
        self.config = config;
    }

    /// Opens `path` at `line` (from 1) in the docked editor, which takes the
    /// keyboard. A file already popped out is only brought to its line.
    pub fn open(
        &mut self,
        path: &Path,
        line: usize,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) {
        let line = line.max(1);
        if let Some(pane) = self.popped.iter_mut().find(|pane| pane.path == path) {
            pane.go_to(line);
            self.focus = Some(Slot::Popped(pane.id));
            return;
        }
        self.focus = Some(Slot::Dock);
        if let Some(pane) = self.dock.as_mut().filter(|pane| pane.path == path) {
            pane.go_to(line);
            return;
        }
        let neovim = *self.neovim.get_or_insert_with(nvim::available);
        let body = if neovim || self.program.is_some() {
            Body::Starting
        } else {
            text_of(path)
        };
        self.next += 1;
        self.dock = Some(Pane {
            id: self.next,
            path: path.to_path_buf(),
            title: title.into(),
            line,
            body,
            embedded: true,
            detail: detail.into(),
            program: self.program.clone(),
            config: self.config.clone(),
        });
    }

    /// Pops the docked editor out into a window of its own, where it stays
    /// while other files open in the dock.
    pub fn pop_out(&mut self) {
        if let Some(pane) = self.dock.take() {
            if self.focus == Some(Slot::Dock) {
                self.focus = Some(Slot::Popped(pane.id));
            }
            self.popped.push(pane);
        }
    }

    /// Closes the docked editor.
    pub fn close_dock(&mut self) {
        self.dock = None;
        if self.focus == Some(Slot::Dock) {
            self.focus = None;
        }
    }

    /// Every open pane, the docked one first.
    pub fn shown(&self) -> Vec<Shown> {
        let shown = |pane: &Pane, popped| Shown {
            path: pane.path.clone(),
            line: pane.line,
            popped,
            neovim: matches!(pane.body, Body::Nvim(_))
                || matches!(pane.body, Body::Starting) && pane.program.is_none(),
        };
        self.dock
            .iter()
            .map(|pane| shown(pane, false))
            .chain(self.popped.iter().map(|pane| shown(pane, true)))
            .collect()
    }

    /// Whether an editor drawn inside the app's window has the keyboard, so
    /// the map's own keys stand back. A pane in a window of its own gets its
    /// keys from that window and never takes the map's.
    pub fn has_keyboard(&self) -> bool {
        match self.focus {
            Some(Slot::Dock) => self.dock.is_some(),
            Some(Slot::Popped(id)) => self
                .popped
                .iter()
                .any(|pane| pane.id == id && pane.embedded),
            None => false,
        }
    }

    /// Whether the docked editor covers `point` on screen, so the map beneath
    /// neither zooms nor pans there.
    pub fn covers(&self, point: egui::Pos2) -> bool {
        self.dock.is_some() && self.drawn.is_some_and(|rect| rect.contains(point))
    }

    /// Whether the docked editor fills the map's panel.
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Gives the keyboard back to the map.
    pub fn blur(&mut self) {
        self.focus = None;
    }

    /// Draws the docked editor over the map: across most of `room`, the map
    /// beside the Explorer, or all of `panel`, the map's whole panel, once
    /// made full-panel (⌘F). Each popped-out editor stands in its own window.
    /// Call after the map is drawn.
    pub fn show(&mut self, ui: &mut egui::Ui, room: Rect, panel: Rect) {
        self.drawn = None;
        if let Some(pane) = self.dock.as_mut() {
            if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                self.full = !self.full;
            }
            let ctx = ui.ctx().clone();
            // It grows out of the middle of the map when opened, and eases
            // between floating and filling the panel.
            let grown =
                ctx.animate_bool_with_time(egui::Id::new(("editor grown", pane.id)), true, GROW);
            let filled = ctx.animate_bool_with_time(egui::Id::new("editor full"), self.full, GROW);
            let floating = Rect::from_center_size(
                room.center(),
                (room.size() * OVERLAY_SHARE)
                    .max(vec2(360.0, 240.0))
                    .min(room.size()),
            );
            let target = lerp_rect(floating, panel, filled);
            let rect = lerp_rect(
                Rect::from_center_size(target.center(), target.size() * 0.6),
                target,
                ease(grown),
            );
            let focused = self.focus == Some(Slot::Dock);
            let full = self.full;
            let mut scrolled = self.scrolled;
            let asked = egui::Area::new(egui::Id::new("editor overlay"))
                .order(egui::Order::Middle)
                .fixed_pos(rect.min)
                .constrain(false)
                .show(&ctx, |ui| {
                    ui.set_min_size(rect.size());
                    ui.set_max_size(rect.size());
                    // Nothing in the overlay reaches the map beneath.
                    ui.interact(
                        rect,
                        ui.id().with("editor backdrop"),
                        Sense::click_and_drag(),
                    );
                    let radius = if full { 0.0 } else { 8.0 };
                    ui.painter()
                        .rect_filled(rect, radius, ui.visuals().extreme_bg_color);
                    let mut asked = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                            ui.set_clip_rect(rect);
                            pane.show(ui, focused, Place::Docked { full }, &mut scrolled)
                        })
                        .inner;
                    let edge = if focused {
                        ui.visuals().selection.stroke
                    } else {
                        ui.visuals().widgets.noninteractive.bg_stroke
                    };
                    ui.painter()
                        .rect_stroke(rect, radius, edge, StrokeKind::Outside);
                    if grown < 1.0 || (filled > 0.0 && filled < 1.0) {
                        ui.ctx().request_repaint();
                    }
                    let pressed_outside = ui.input(|i| {
                        i.pointer.any_pressed()
                            && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p))
                    });
                    if pressed_outside {
                        asked.blur = true;
                    }
                    asked
                })
                .inner;
            self.scrolled = scrolled;
            self.drawn = Some(rect);
            if asked.full {
                self.full = !self.full;
            }
            self.answer(Slot::Dock, asked);
        }
        let ctx = ui.ctx().clone();
        let mut answers = Vec::new();
        for pane in &mut self.popped {
            let id = pane.id;
            let focused = self.focus == Some(Slot::Popped(id));
            let mut scrolled = 0.0;
            let builder = ViewportBuilder::default()
                .with_title(format!("{} — Cellworks", pane.title))
                .with_inner_size([720.0, 820.0]);
            let asked = ctx.show_viewport_immediate(
                ViewportId::from_hash_of(("cellworks editor", id)),
                builder,
                |ui, class| {
                    pane.embedded = class == ViewportClass::EmbeddedWindow;
                    // A window of its own has the keyboard whenever it has
                    // the system's focus.
                    let focused = if pane.embedded {
                        focused
                    } else {
                        ui.input(|i| i.viewport().focused.unwrap_or(false))
                    };
                    let mut asked = egui::CentralPanel::default()
                        .frame(egui::Frame::central_panel(ui.style()).inner_margin(0.0))
                        .show(ui, |ui| {
                            pane.show(ui, focused, Place::Popped, &mut scrolled)
                        })
                        .inner;
                    if ui.input(|i| i.viewport().close_requested()) {
                        asked.close = true;
                    }
                    asked
                },
            );
            answers.push((id, asked));
        }
        for (id, asked) in answers {
            self.answer(Slot::Popped(id), asked);
        }
    }

    fn answer(&mut self, slot: Slot, asked: Asked) {
        if asked.focus {
            self.focus = Some(slot);
        } else if asked.blur && self.focus == Some(slot) {
            self.focus = None;
        }
        match slot {
            Slot::Dock => {
                if asked.close {
                    self.close_dock();
                } else if asked.pop_out {
                    self.pop_out();
                }
            }
            Slot::Popped(id) => {
                let Some(index) = self.popped.iter().position(|pane| pane.id == id) else {
                    return;
                };
                if asked.close {
                    self.popped.remove(index);
                    if self.focus == Some(slot) {
                        self.focus = None;
                    }
                } else if asked.dock {
                    let mut pane = self.popped.remove(index);
                    pane.embedded = true;
                    if let Some(old) = self.dock.replace(pane) {
                        self.popped.push(old);
                    }
                    self.focus = Some(Slot::Dock);
                }
            }
        }
    }
}

/// The read-only body of `path`.
fn text_of(path: &Path) -> Body {
    match std::fs::read_to_string(path) {
        Ok(text) => Body::Text {
            lines: text.lines().map(str::to_owned).collect(),
            jump: true,
        },
        Err(error) => Body::Failed(format!("{}: {error}", path.display())),
    }
}

impl Pane {
    fn go_to(&mut self, line: usize) {
        self.line = line;
        match &mut self.body {
            Body::Nvim(nvim) => nvim.input(&format!("<Esc>{line}Gzz")),
            // A terminal editor keeps its place: there is no common way to
            // move it.
            Body::Text { jump, .. } => *jump = true,
            _ => {}
        }
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        focused: bool,
        place: Place,
        scrolled: &mut f32,
    ) -> Asked {
        let mut asked = Asked::default();
        self.header(ui, focused, place, &mut asked);
        let area = ui.available_rect_before_wrap();
        let response = ui.interact(area, ui.id().with(("editor body", self.id)), Sense::click());
        ui.advance_cursor_after_rect(area);
        if response.clicked() || response.secondary_clicked() {
            asked.focus = true;
        } else if place == Place::Popped
            && ui.input(|i| i.pointer.any_pressed())
            && !response.hovered()
        {
            asked.blur = true;
        }
        let font = FontId::monospace(CODE_SIZE);
        match &mut self.body {
            Body::Starting => {
                let (cols, rows) = grid(ui, area, &font);
                let ctx = ui.ctx().clone();
                let repaint = move || ctx.request_repaint();
                self.body = match &self.program {
                    Some(program) => {
                        match Tui::spawn(program, &self.path, self.line, cols, rows, repaint) {
                            Ok(tui) => Body::Tui(Box::new(tui)),
                            Err(error) => Body::Failed(error),
                        }
                    }
                    None => match Nvim::spawn(
                        &self.path,
                        self.config.as_deref(),
                        self.line,
                        cols,
                        rows,
                        repaint,
                    ) {
                        Ok(nvim) => Body::Nvim(Box::new(nvim)),
                        // Neovim would not start: the text is read instead.
                        Err(_) => text_of(&self.path),
                    },
                };
            }
            Body::Tui(tui) => {
                if !tui.alive() {
                    asked.close = true;
                    return asked;
                }
                let (cols, rows) = grid(ui, area, &font);
                tui.resize(cols, rows);
                tui.pump();
                if focused {
                    for event in ui.input(|i| i.events.clone()) {
                        let bytes = tui::key_bytes(&event);
                        if !bytes.is_empty() {
                            tui.send(&bytes);
                        }
                    }
                }
                let (cw, lh) = cell(ui, &font);
                tui.paint(ui, area, &font, cw, lh, focused);
            }
            Body::Nvim(nvim) => {
                if !nvim.alive() {
                    // `:q` in the editor closes its pane.
                    asked.close = true;
                    return asked;
                }
                let (cols, rows) = grid(ui, area, &font);
                nvim.resize(cols, rows);
                nvim.pump();
                let (cw, lh) = cell(ui, &font);
                let at = |p: egui::Pos2| {
                    (
                        ((p.y - area.min.y) / lh).max(0.0) as usize,
                        ((p.x - area.min.x) / cw).max(0.0) as usize,
                    )
                };
                if let Some(p) = response
                    .interact_pointer_pos()
                    .filter(|_| response.clicked())
                {
                    let (row, col) = at(p);
                    nvim.mouse("left", "press", row, col);
                    nvim.mouse("left", "release", row, col);
                }
                if let Some(p) = response.hover_pos() {
                    let delta = ui.input(|i| i.smooth_scroll_delta.y);
                    *scrolled += delta;
                    let (row, col) = at(p);
                    while scrolled.abs() >= lh {
                        let way = if *scrolled > 0.0 { "up" } else { "down" };
                        nvim.mouse("wheel", way, row, col);
                        *scrolled -= lh * scrolled.signum();
                    }
                    // The map under the pointer does not also scroll.
                    ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
                }
                if focused {
                    asked.blur |= keys(ui, nvim);
                    if let Some((row, _)) = Some(nvim.cursor) {
                        self.line = row + 1;
                    }
                }
                paint(ui, nvim, area, &font, cw, lh, focused);
            }
            Body::Text { lines, jump } => {
                text(ui, area, lines, self.line, jump, &font);
                if focused && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    asked.blur = true;
                }
            }
            Body::Failed(error) => {
                ui.painter().text(
                    area.min + vec2(16.0, 16.0),
                    Align2::LEFT_TOP,
                    error.as_str(),
                    FontId::proportional(13.0),
                    ui.visuals().error_fg_color,
                );
            }
        }
        asked
    }

    fn header(&self, ui: &mut egui::Ui, focused: bool, place: Place, asked: &mut Asked) {
        // The amber of the map's selection: the file's name reads first.
        const AMBER: Color32 = Color32::from_rgb(0xff, 0xb3, 0x47);
        egui::Frame::new()
            .fill(Color32::from_rgb(0x16, 0x14, 0x10))
            .inner_margin(egui::Margin::symmetric(12, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let colour = if focused { AMBER } else { AMBER.gamma_multiply(0.75) };
                    ui.add(
                        egui::Label::new(
                            RichText::new(&self.title).monospace().size(13.0).color(colour),
                        )
                        .truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("close").clicked() {
                            asked.close = true;
                        }
                        match place {
                            Place::Docked { full } => {
                                let label = if full { "exit full panel" } else { "full panel" };
                                if ui.small_button(label).on_hover_text("⌘F").clicked() {
                                    asked.full = true;
                                }
                                if ui
                                    .small_button("pop out")
                                    .on_hover_text(
                                        "Into a window of its own; it stays open while you navigate",
                                    )
                                    .clicked()
                                {
                                    asked.pop_out = true;
                                }
                            }
                            Place::Popped => {
                                if ui
                                    .small_button("dock")
                                    .on_hover_text("Back over the map")
                                    .clicked()
                                {
                                    asked.dock = true;
                                }
                            }
                        }
                        let mode = match &self.body {
                            Body::Nvim(nvim) => format!("nvim · {}", nvim.mode),
                            Body::Tui(_) | Body::Starting => self
                                .program
                                .as_deref()
                                .and_then(|p| p.split_whitespace().next())
                                .unwrap_or("nvim")
                                .to_owned(),
                            Body::Text { .. } => "read only · nvim not found".to_owned(),
                            Body::Failed(_) => "unreadable".to_owned(),
                        };
                        ui.label(RichText::new(mode).monospace().size(11.0).weak());
                        if !self.detail.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&self.detail).monospace().size(11.0).weak(),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            });
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min + (b.min - a.min) * t, a.max + (b.max - a.max) * t)
}

/// The width and height of one cell of `font`.
fn cell(ui: &egui::Ui, font: &FontId) -> (f32, f32) {
    ui.fonts_mut(|fonts| (fonts.glyph_width(font, 'M'), fonts.row_height(font)))
}

/// How many columns and rows of `font` fit `area`.
fn grid(ui: &egui::Ui, area: Rect, font: &FontId) -> (usize, usize) {
    let (cw, lh) = cell(ui, font);
    (
        ((area.width() / cw) as usize).max(20),
        ((area.height() / lh) as usize).max(5),
    )
}

/// Sends this frame's keys to `nvim`; true when Esc in normal mode leaves
/// the editor, which then does not pass it on.
fn keys(ui: &egui::Ui, nvim: &mut Nvim) -> bool {
    let mut leave = false;
    for event in ui.input(|i| i.events.clone()) {
        match event {
            egui::Event::Text(text) => nvim.input(&text.replace('<', "<lt>")),
            egui::Event::Paste(text) => nvim.input(&text.replace('<', "<lt>")),
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } if nvim.mode == "normal" => leave = true,
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.mac_cmd => {
                if let Some(keys) = nvim::key_notation(key, modifiers) {
                    nvim.input(&keys);
                }
            }
            _ => {}
        }
    }
    leave
}

fn colour(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Paints Neovim's grid into `area`.
fn paint(ui: &egui::Ui, nvim: &Nvim, area: Rect, font: &FontId, cw: f32, lh: f32, focused: bool) {
    let painter = ui.painter().with_clip_rect(area);
    painter.rect_filled(area, 0.0, colour(nvim.bg));
    for (r, row) in nvim.cells.iter().enumerate() {
        let y = area.min.y + r as f32 * lh;
        let mut c = 0;
        while c < row.len() {
            let id = row[c].1;
            let start = c;
            let mut text = String::new();
            while c < row.len() && row[c].1 == id {
                text.push_str(&row[c].0);
                c += 1;
            }
            let highlight = nvim.highlights.get(&id).copied().unwrap_or_default();
            let (mut fg, mut bg) = (
                highlight.fg.unwrap_or(nvim.fg),
                highlight.bg.unwrap_or(nvim.bg),
            );
            if highlight.reverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            let x = area.min.x + start as f32 * cw;
            let width = (c - start) as f32 * cw;
            if bg != nvim.bg {
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, y), vec2(width, lh)),
                    0.0,
                    colour(bg),
                );
            }
            if !text.trim().is_empty() {
                painter.text(pos2(x, y), Align2::LEFT_TOP, text, font.clone(), colour(fg));
            }
            if highlight.underline {
                painter.hline(x..=x + width, y + lh - 1.0, Stroke::new(1.0, colour(fg)));
            }
        }
    }
    let cursor = Rect::from_min_size(
        area.min + vec2(nvim.cursor.1 as f32 * cw, nvim.cursor.0 as f32 * lh),
        vec2(if nvim.mode == "insert" { 2.0 } else { cw }, lh),
    );
    if focused {
        painter.rect_filled(cursor, 0.0, colour(nvim.fg).gamma_multiply(0.6));
    } else {
        painter.rect_stroke(
            cursor,
            0.0,
            Stroke::new(1.0, colour(nvim.fg)),
            StrokeKind::Inside,
        );
        painter.text(
            area.right_top() + vec2(-8.0, 6.0),
            Align2::RIGHT_TOP,
            "click to edit",
            FontId::monospace(11.0),
            colour(nvim.fg).gamma_multiply(0.5),
        );
    }
}

/// The file's `lines`, numbered, read only; scrolled to `line` while `jump`.
fn text(
    ui: &mut egui::Ui,
    area: Rect,
    lines: &[String],
    line: usize,
    jump: &mut bool,
    font: &FontId,
) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area));
    let (_, lh) = cell(&child, font);
    let digits = lines.len().max(1).to_string().len();
    let mut scroll = egui::ScrollArea::both().auto_shrink(false);
    if std::mem::take(jump) {
        scroll = scroll.vertical_scroll_offset(((line as f32 - 6.0).max(0.0)) * lh);
    }
    let accent = child.visuals().selection.bg_fill;
    let weak = child.visuals().weak_text_color();
    let strong = child.visuals().text_color();
    scroll.show_rows(&mut child, lh, lines.len(), |ui, rows| {
        for index in rows {
            let number = index + 1;
            let job = RichText::new(format!(
                "{number:>digits$}  {}",
                lines[index].replace('\t', "    ")
            ))
            .font(font.clone());
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), lh), Sense::hover());
            if number == line {
                ui.painter()
                    .rect_filled(rect, 0.0, accent.gamma_multiply(0.35));
            }
            let painter = ui.painter();
            let split = digits + 2;
            let text = job.text();
            let (gutter, code) = text.split_at(
                text.char_indices()
                    .nth(split)
                    .map_or(text.len(), |(i, _)| i),
            );
            painter.text(
                rect.left_top() + vec2(8.0, 0.0),
                Align2::LEFT_TOP,
                gutter,
                font.clone(),
                weak,
            );
            let (cw, _) = cell(ui, font);
            painter.text(
                rect.left_top() + vec2(8.0 + split as f32 * cw, 0.0),
                Align2::LEFT_TOP,
                code,
                font.clone(),
                strong,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.rs");
        std::fs::write(&path, text).unwrap();
        (dir, path)
    }

    #[test]
    fn a_file_opens_in_the_dock_and_another_replaces_it() {
        let (_dir, a) = file("fn a() {}\n");
        let b = a.with_file_name("b.rs");
        std::fs::write(&b, "fn b() {}\n").unwrap();
        let mut editors = Editors::read_only();
        editors.open(&a, 1, "a.rs", "");
        assert!(editors.has_keyboard());
        editors.open(&b, 3, "b.rs", "");
        let shown = editors.shown();
        assert_eq!(shown.len(), 1);
        assert_eq!((shown[0].path.as_path(), shown[0].line), (b.as_path(), 3));
        assert!(!shown[0].neovim);
    }

    #[test]
    fn a_popped_out_file_stays_open_while_the_next_opens_in_the_dock() {
        let (_dir, a) = file("fn a() {}\n");
        let b = a.with_file_name("b.rs");
        std::fs::write(&b, "fn b() {}\n").unwrap();
        let mut editors = Editors::read_only();
        editors.open(&a, 1, "a.rs", "");
        editors.pop_out();
        editors.open(&b, 1, "b.rs", "");
        let shown = editors.shown();
        assert_eq!(shown.len(), 2);
        assert_eq!(
            (shown[0].path.as_path(), shown[0].popped),
            (b.as_path(), false)
        );
        assert_eq!(
            (shown[1].path.as_path(), shown[1].popped),
            (a.as_path(), true)
        );
        // Opening the popped-out file again brings it to its line, not a copy.
        editors.open(&a, 4, "a.rs", "");
        assert_eq!(editors.shown().len(), 2);
        assert_eq!(editors.shown()[1].line, 4);
    }

    #[test]
    fn esc_in_the_read_only_view_gives_the_keyboard_back() {
        let (_dir, a) = file("one\ntwo\n");
        let mut editors = Editors::read_only();
        editors.open(&a, 2, "a.rs", "");
        let mut harness = egui_kittest::Harness::builder()
            .with_size(vec2(900.0, 600.0))
            .build_ui_state(
                |ui, editors: &mut Editors| {
                    let rect = ui.max_rect();
                    editors.show(ui, rect, rect)
                },
                editors,
            );
        harness.run();
        assert!(harness.state().has_keyboard());
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(!harness.state().has_keyboard());
    }
}
