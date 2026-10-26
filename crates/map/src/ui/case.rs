//! PROTOTYPE: the case mode for any Jira issue. Opening a ticket takes over
//! the window and walks through three stages: the problem in plain words,
//! the proposed fix placed on a map of the code base and the outside
//! systems, and the evidence for one step (what changes, the lines on disk,
//! the risks). The ticket comes from Jira; everything beyond it comes from
//! the case file a Claude Code session writes (see `casefile`), which is
//! watched and shown as soon as it appears. Layout and colours follow the
//! approved mockup, drawn on a 1600 x 1000 sheet; everything here is laid
//! out in those units and scaled to the window.

use std::f32::consts::{PI, TAU};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use egui::epaint::Shadow;

use super::casefile::{
    self, CaseFile, FileSpot, Kind, LaneKind, Level, Loaded, Mark, Spot, Status, System, When,
    expand,
};
use super::jira::{Jira, SignIn};
use crate::ProjectMap;
use crate::ui::Spot as MapSpot;
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, FontFamily, FontId, Galley, Id, Painter, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

const BG: Color32 = Color32::from_rgb(0x07, 0x09, 0x0c);
const BG_DEEP: Color32 = Color32::from_rgb(0x04, 0x05, 0x07);
const PANEL: Color32 = Color32::from_rgb(0x0d, 0x11, 0x17);
const PANEL_RAISED: Color32 = Color32::from_rgb(0x14, 0x1a, 0x22);
const LINE: Color32 = Color32::from_rgb(0x1e, 0x27, 0x31);
const LINE_STRONG: Color32 = Color32::from_rgb(0x34, 0x50, 0x6a);
const INK: Color32 = Color32::from_rgb(0xd7, 0xe2, 0xea);
const INK_DIM: Color32 = Color32::from_rgb(0x7d, 0x8f, 0xa0);
const ATTENTION: Color32 = Color32::from_rgb(0xfc, 0xee, 0x0a);
const ATTENTION_SOFT: Color32 = Color32::from_rgb(0x3a, 0x36, 0x08);
const ON_ATTENTION: Color32 = BG;
const FLOW: Color32 = Color32::from_rgb(0x5e, 0xf6, 0xff);
const LANE_CALLS: Color32 = Color32::from_rgb(0xb2, 0x6b, 0xff);
const OK: Color32 = Color32::from_rgb(0x3d, 0xdc, 0x84);
const INFO: Color32 = Color32::from_rgb(0x4a, 0xa8, 0xff);
const WARN: Color32 = Color32::from_rgb(0xff, 0x9f, 0x1c);
const DANGER: Color32 = Color32::from_rgb(0xff, 0x3b, 0x5c);
const DANGER_TEXT: Color32 = Color32::from_rgb(0xff, 0x6f, 0x86);
const DANGER_FILL: Color32 = Color32::from_rgb(0x3a, 0x0b, 0x16);
const ADD_FILL: Color32 = Color32::from_rgba_premultiplied(7, 26, 16, 31);
const TERRAIN: Color32 = Color32::from_rgb(0x0e, 0x13, 0x18);
const TERRAIN_EDGE: Color32 = Color32::from_rgb(0x1b, 0x23, 0x2c);
const TERRAIN_INK: Color32 = Color32::from_rgb(0x5f, 0x71, 0x83);
const ZONE_FOREIGN: Color32 = Color32::from_rgb(0x2c, 0x3b, 0x4a);

/// Registers the case's typefaces, Chakra Petch for text and labels and
/// JetBrains Mono for code, ahead of egui's defaults, which stay as the
/// fallback for glyphs these fonts lack. The files live in `assets/fonts/`
/// at the root of the repository, with their licences. Besides the regular
/// families, the named families `semibold`, `bold` and `mono-bold` carry
/// the heavier weights.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 5] = [
        (
            "ChakraPetch-Regular",
            include_bytes!("../../../../assets/fonts/ChakraPetch-Regular.ttf"),
        ),
        (
            "ChakraPetch-SemiBold",
            include_bytes!("../../../../assets/fonts/ChakraPetch-SemiBold.ttf"),
        ),
        (
            "ChakraPetch-Bold",
            include_bytes!("../../../../assets/fonts/ChakraPetch-Bold.ttf"),
        ),
        (
            "JetBrainsMono-Regular",
            include_bytes!("../../../../assets/fonts/JetBrainsMono-Regular.ttf"),
        ),
        (
            "JetBrainsMono-Bold",
            include_bytes!("../../../../assets/fonts/JetBrainsMono-Bold.ttf"),
        ),
    ];
    for (name, bytes) in faces {
        fonts.font_data.insert(
            name.to_owned(),
            Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    let sans = fonts.families[&FontFamily::Proportional].clone();
    let mono = fonts.families[&FontFamily::Monospace].clone();
    let first = |face: &str, rest: &[String]| {
        std::iter::once(face.to_owned())
            .chain(rest.iter().cloned())
            .collect::<Vec<_>>()
    };
    let families = [
        (
            FontFamily::Proportional,
            first("ChakraPetch-Regular", &sans),
        ),
        (
            FontFamily::Name("semibold".into()),
            first("ChakraPetch-SemiBold", &sans),
        ),
        (
            FontFamily::Name("bold".into()),
            first("ChakraPetch-Bold", &sans),
        ),
        (FontFamily::Monospace, first("JetBrainsMono-Regular", &mono)),
        (
            FontFamily::Name("mono-bold".into()),
            first("JetBrainsMono-Bold", &mono),
        ),
    ];
    fonts.families.extend(families);
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- the case on the map

#[derive(Clone, Copy)]
enum Glyph {
    Padlock,
    Key,
    Hex,
    Person,
    Db,
    Doc,
}

fn glyph_of(kind: Kind) -> Glyph {
    match kind {
        Kind::Vault => Glyph::Padlock,
        Kind::Key => Glyph::Key,
        Kind::Workload | Kind::Service | Kind::Queue => Glyph::Hex,
        Kind::Role => Glyph::Person,
        Kind::Database => Glyph::Db,
        Kind::Doc => Glyph::Doc,
    }
}

/// A file or an outside system of the case, placed in the map's 1000 x 820
/// units.
struct Place {
    spot: Spot,
    glyph: Glyph,
    x: f32,
    y: f32,
    label: String,
    outside: bool,
    left: bool,
    /// A role that logs in as the administrator.
    root: bool,
    /// Bit i: step i names it.
    steps: u32,
}

/// A folder of a repository that holds files of the case.
struct District {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: String,
    steps: u32,
}

fn steps_of(case: &CaseFile, spot: &Spot) -> u32 {
    case.steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.spots.iter().any(|s| s.same(spot)))
        .fold(0, |mask, (index, _)| mask | 1 << index)
}

/// Lays the case out: its files across the top, one district per folder
/// (at most two levels deep), and its outside systems below, in columns by
/// kind: secrets, workloads, roles, databases.
fn layout(case: &CaseFile) -> (Vec<Place>, Vec<District>) {
    let mut files: Vec<FileSpot> = Vec::new();
    let named = case
        .steps
        .iter()
        .flat_map(|step| step.spots.iter())
        .chain(case.lanes.iter().flat_map(|lane| [&lane.from, &lane.to]));
    for spot in named {
        if let Spot::File(file) = spot
            && !files.iter().any(|f| f.absolute() == file.absolute())
        {
            files.push(file.clone());
        }
    }
    let folder = |file: &FileSpot| -> (String, String) {
        let parent: Vec<String> = Path::new(&file.path)
            .parent()
            .map(|p| {
                p.components()
                    .take(2)
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        let label = if parent.is_empty() {
            expand(&file.repo_root)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            parent.join("/")
        };
        (file.repo_root.clone(), label)
    };
    let mut folders: Vec<(String, String)> = Vec::new();
    for file in &files {
        let key = folder(file);
        if !folders.contains(&key) {
            folders.push(key);
        }
    }

    let mut places = Vec::new();
    let mut districts = Vec::new();
    let n = folders.len().max(1);
    let columns = n.min(3);
    let rows = n.div_ceil(columns);
    let w = (920.0 - 20.0 * (columns - 1) as f32) / columns as f32;
    let h = (360.0 - 20.0 * (rows - 1) as f32) / rows as f32;
    let name = |file: &FileSpot| {
        let path = Path::new(&file.path);
        let base = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let twins = files
            .iter()
            .filter(|f| Path::new(&f.path).file_name() == path.file_name())
            .count();
        match path.parent().and_then(Path::file_name) {
            Some(parent) if twins > 1 => format!("{base} · {}", parent.to_string_lossy()),
            _ => base,
        }
    };
    for (index, key) in folders.iter().enumerate() {
        let (dx, dy) = (
            40.0 + (index % columns) as f32 * (w + 20.0),
            60.0 + (index / columns) as f32 * (h + 20.0),
        );
        let inside: Vec<&FileSpot> = files.iter().filter(|f| folder(f) == *key).collect();
        let spacing = ((h - 70.0) / (inside.len().max(2) - 1) as f32).min(44.0);
        let mut steps = 0;
        for (row, file) in inside.into_iter().enumerate() {
            let spot = Spot::File(file.clone());
            let mask = steps_of(case, &spot);
            steps |= mask;
            places.push(Place {
                glyph: Glyph::Doc,
                x: dx + 26.0,
                y: dy + 52.0 + row as f32 * spacing,
                label: name(file),
                outside: false,
                left: false,
                root: false,
                steps: mask,
                spot,
            });
        }
        districts.push(District {
            x: dx,
            y: dy,
            w,
            h,
            label: key.1.clone(),
            steps,
        });
    }

    let column = |kind: Kind| match kind {
        Kind::Vault | Kind::Key | Kind::Doc => 0,
        Kind::Workload | Kind::Service | Kind::Queue => 1,
        Kind::Role => 2,
        Kind::Database => 3,
    };
    for (c, x) in [80.0, 400.0, 640.0, 900.0].into_iter().enumerate() {
        let systems: Vec<&System> = case
            .systems
            .iter()
            .filter(|s| column(s.kind) == c)
            .collect();
        let spacing = (190.0 / (systems.len().max(2) - 1) as f32).min(80.0);
        let top = 615.0 - spacing * (systems.len().max(1) - 1) as f32 / 2.0;
        for (row, system) in systems.into_iter().enumerate() {
            let spot = Spot::System(system.id.clone());
            let root = system.kind == Kind::Role
                && case.lanes.iter().any(|lane| {
                    lane.kind == LaneKind::Root && (lane.from.same(&spot) || lane.to.same(&spot))
                });
            places.push(Place {
                glyph: glyph_of(system.kind),
                x,
                y: top + row as f32 * spacing,
                label: system.label.clone(),
                outside: true,
                left: c == 3,
                root,
                steps: steps_of(case, &spot),
                spot,
            });
        }
    }
    (places, districts)
}

// ---------------------------------------------------------------- state

/// One open case: the stage shown, the step inspected, the animations under
/// way, and the case file once there is one.
pub struct Case {
    /// The issue key, e.g. "SHOP-42".
    pub key: String,
    stage: u8,
    step: usize,
    after: bool,
    hover: Option<usize>,
    launched: Option<(String, f64)>,
    /// When the current stage was entered; its panels rise from then.
    entered: f64,
    /// The map frame (x, y, w, h, opacity) the last move started from, and
    /// when it started.
    from: [f32; 5],
    moved: f64,
    /// The case file and what was read for it; the last one that read.
    loaded: Option<Loaded>,
    /// Why the case file on disk does not read, when it does not.
    broken: Option<String>,
    /// When the case file was last looked at.
    checked: f64,
    places: Vec<Place>,
    districts: Vec<District>,
    /// Stage 1 shows the ticket as Jira has it instead of the case.
    raw: bool,
    /// What the last lifecycle button ran, as shown to the user.
    investigating: Option<String>,
    /// The last lifecycle act started from here.
    launched_act: Option<Act>,
    /// The note APPROVE writes with the approval.
    note: String,
    /// The folder an investigation starts in: the common parent of the
    /// projects Cellworks knows, set by the app.
    pub workspace: Option<PathBuf>,
    /// The repository shown on stages 2 and 3, by its index in `repos`.
    repo: usize,
    /// Whether the outside-systems panel is open.
    diagram: bool,
    /// The camera's glide onto the case, while it lasts.
    aim: Option<Aim>,
    /// Where the case's chrome stood on screen last frame.
    chrome: Vec<Rect>,
}

impl Case {
    pub fn new(key: &str, now: f64) -> Case {
        let mut case = Case {
            key: key.to_owned(),
            stage: 1,
            step: 0,
            after: false,
            hover: None,
            launched: None,
            entered: now,
            from: frame_of(1),
            moved: now - 1.0,
            loaded: None,
            broken: None,
            checked: now,
            places: Vec::new(),
            districts: Vec::new(),
            raw: false,
            investigating: None,
            launched_act: None,
            note: String::new(),
            workspace: None,
            repo: 0,
            diagram: true,
            aim: None,
            chrome: Vec::new(),
        };
        case.read();
        case
    }

    /// Whether there is a case to show beyond the ticket.
    fn ready(&self) -> bool {
        self.loaded
            .as_ref()
            .is_some_and(|loaded| !loaded.case.steps.is_empty())
    }

    /// Reads the case file again when it changed, once a second. A case
    /// that arrives rises in like a stage just entered.
    fn refresh(&mut self, now: f64) {
        if now - self.checked < 1.0 {
            return;
        }
        self.checked = now;
        if casefile::modified(&self.key) != self.loaded.as_ref().map(|l| l.modified) {
            let arrived = self.loaded.is_none();
            self.read();
            if arrived && self.loaded.is_some() {
                self.entered = now;
            }
        }
    }

    fn read(&mut self) {
        match casefile::load(&self.key) {
            None => {
                self.loaded = None;
                self.broken = None;
            }
            Some(Err(error)) => self.broken = Some(error),
            Some(Ok(loaded)) => {
                (self.places, self.districts) = layout(&loaded.case);
                self.step = self.step.min(loaded.case.steps.len().saturating_sub(1));
                self.loaded = Some(loaded);
                self.broken = None;
            }
        }
    }

    fn go(&mut self, stage: u8, now: f64) {
        if stage == self.stage || (stage > 1 && !self.ready()) {
            return;
        }
        self.from = self.frame(now);
        self.moved = now;
        self.entered = now;
        self.stage = stage;
        self.hover = None;
        // The map shows the repository of the step in focus.
        if let Some(repo) = self.repo_of(self.step) {
            self.repo = repo;
        }
    }

    fn pick(&mut self, step: usize, now: f64) {
        self.step = step;
        self.launched = None;
        if let Some(repo) = self.repo_of(step) {
            self.repo = repo;
        }
        self.go(3, now);
    }

    /// The map frame now, between where it was and where the stage puts it.
    fn frame(&self, now: f64) -> [f32; 5] {
        let t = ease(((now - self.moved) / 0.7) as f32);
        let to = frame_of(self.stage);
        std::array::from_fn(|i| self.from[i] + (to[i] - self.from[i]) * t)
    }
}

/// Where the map frame stands in each stage, in sheet units, and how
/// visible it is.
fn frame_of(stage: u8) -> [f32; 5] {
    match stage {
        1 => [0.0, 0.0, 1600.0, 1000.0, 0.16],
        2 => [24.0, 80.0, 1092.0, 896.0, 1.0],
        _ => [1216.0, 80.0, 360.0, 230.0, 1.0],
    }
}

/// The CSS curve cubic-bezier(.2, .8, .2, 1) at `t` in 0..1.
fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let bezier = |a: f32, b: f32, s: f32| {
        3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s
    };
    let (mut low, mut high) = (0.0, 1.0);
    for _ in 0..24 {
        let mid = (low + high) / 2.0;
        if bezier(0.2, 0.2, mid) < t {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(0.8, 1.0, (low + high) / 2.0)
}

// ---------------------------------------------------------------- drawing

#[derive(Clone, Copy)]
struct Font {
    size: f32,
    family: &'static str,
    /// Extra letter spacing, in em.
    spacing: f32,
    /// Line height, as a multiple of the size.
    height: f32,
}

const fn font(size: f32, family: &'static str, spacing: f32, height: f32) -> Font {
    Font {
        size,
        family,
        spacing,
        height,
    }
}

const LABEL: Font = font(12.0, "sans", 0.16, 1.25);
const MONO11: Font = font(11.0, "mono", 0.0, 1.3);
const MONO12: Font = font(12.0, "mono", 0.0, 1.4);

fn family(name: &str) -> FontFamily {
    match name {
        "sans" => FontFamily::Proportional,
        "mono" => FontFamily::Monospace,
        other => FontFamily::Name(other.into()),
    }
}

/// The sheet the mockup is drawn on, scaled by `k` and placed at `o`; `dy`
/// lifts a panel that is still rising into place.
#[derive(Clone)]
struct Sheet {
    painter: Painter,
    o: Pos2,
    k: f32,
    dy: f32,
}

impl Sheet {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.o + vec2(x, y + self.dy) * self.k
    }

    fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), vec2(w, h) * self.k)
    }

    /// The same sheet, for the `index`-th panel of a stage entered at
    /// `entered`: it fades in and rises 18 units over 0.55 s, 90 ms after
    /// the panel before it.
    fn rising(&self, entered: f64, now: f64, index: usize) -> Sheet {
        let t = ease(((now - entered - 0.09 * index as f64) / 0.55) as f32);
        let mut sheet = self.clone();
        sheet.painter.multiply_opacity(t);
        sheet.dy = 18.0 * (1.0 - t);
        sheet
    }

    fn galley(&self, text: &str, font: Font, colour: Color32, wrap: f32) -> Arc<Galley> {
        let size = font.size * self.k;
        let mut job = LayoutJob::single_section(
            text.to_owned(),
            TextFormat {
                font_id: FontId::new(size, family(font.family)),
                color: colour,
                extra_letter_spacing: font.spacing * size,
                line_height: Some(font.height * size),
                ..Default::default()
            },
        );
        job.wrap.max_width = if wrap > 0.0 {
            wrap * self.k
        } else {
            f32::INFINITY
        };
        self.painter.layout_job(job)
    }

    fn width(&self, text: &str, font: Font) -> f32 {
        self.galley(text, font, INK, 0.0).size().x / self.k
    }

    fn height(&self, text: &str, font: Font, wrap: f32) -> f32 {
        self.galley(text, font, INK, wrap).size().y / self.k
    }

    /// Writes `text` with its top left at (`x`, `y`), wrapped at `wrap`
    /// units (none when 0), and returns its height.
    fn text(&self, x: f32, y: f32, text: &str, font: Font, colour: Color32, wrap: f32) -> f32 {
        let galley = self.galley(text, font, colour, wrap);
        let height = galley.size().y / self.k;
        self.painter.galley(self.p(x, y), galley, colour);
        height
    }

    /// Like [`Sheet::text`], but the text can be selected and copied, and
    /// the web addresses in it open in the browser.
    #[allow(clippy::too_many_arguments)]
    fn pick(
        &self,
        ui: &Ui,
        x: f32,
        y: f32,
        text: &str,
        font: Font,
        colour: Color32,
        wrap: f32,
    ) -> f32 {
        self.pick_at(
            ui,
            (x, y, Align2::LEFT_TOP),
            text,
            (font, colour, wrap),
            None,
        )
    }

    /// `text` placed by `align` around (`x`, `y`), selectable; its links are
    /// the web addresses in it, or its first `head` bytes when that names one. Answers
    /// its height.
    fn pick_at(
        &self,
        ui: &Ui,
        (x, y, align): (f32, f32, Align2),
        text: &str,
        (font, colour, wrap): (Font, Color32, f32),
        head: Option<(usize, &str)>,
    ) -> f32 {
        let fade = self.painter.opacity();
        let size = font.size * self.k;
        let format = |colour: Color32, link: bool| TextFormat {
            font_id: FontId::new(size, family(font.family)),
            color: colour.gamma_multiply(fade),
            extra_letter_spacing: font.spacing * size,
            line_height: Some(font.height * size),
            underline: if link {
                Stroke::new(1.0, FLOW.gamma_multiply(fade))
            } else {
                Stroke::NONE
            },
            ..Default::default()
        };
        let links: Vec<(std::ops::Range<usize>, String)> = match head {
            Some((len, url)) => vec![(0..len.min(text.len()), url.to_owned())],
            None => urls(text)
                .into_iter()
                .map(|range| (range.clone(), text[range].to_owned()))
                .collect(),
        };
        let mut job = LayoutJob::default();
        let mut at = 0;
        for (range, _) in &links {
            job.append(&text[at..range.start], 0.0, format(colour, false));
            job.append(&text[range.clone()], 0.0, format(FLOW, true));
            at = range.end;
        }
        job.append(&text[at..], 0.0, format(colour, false));
        job.wrap.max_width = if wrap > 0.0 {
            wrap * self.k
        } else {
            f32::INFINITY
        };
        let galley = self.painter.layout_job(job);
        let size = galley.size();
        let rect = align.anchor_size(self.p(x, y), size);
        let key = (text, (x * 4.0) as i32, (y * 4.0) as i32);
        if !ui.is_rect_visible(rect) {
            self.painter.galley(rect.min, galley, colour);
            return size.y / self.k;
        }
        let response = ui.interact(
            rect,
            ui.id().with(("case text", key)),
            Sense::click_and_drag(),
        );
        for (range, url) in &links {
            let from = text[..range.start].chars().count();
            let to = from + text[range.clone()].chars().count();
            let cursor = |index| galley.pos_from_cursor(egui::text::CCursor::new(index));
            let span = cursor(from).union(cursor(to)).translate(rect.min.to_vec2());
            let link = ui.interact(span, ui.id().with(("case link", key, from)), Sense::click());
            if link.hovered() {
                ui.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if link.clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
        }
        // After the links, so their cursor is the one that shows.
        egui::text_selection::LabelSelectionState::label_text_selection(
            ui,
            &response,
            rect.min,
            galley,
            colour,
            Stroke::NONE,
        );
        size.y / self.k
    }

    /// Writes one line of `text` placed by `align` around (`x`, `y`).
    fn put(&self, x: f32, y: f32, align: Align2, text: &str, font: Font, colour: Color32) {
        let galley = self.galley(text, font, colour, 0.0);
        let at = align.anchor_size(self.p(x, y), galley.size()).min;
        self.painter.galley(at, galley, colour);
    }

    /// A panel or button: filled, edged, with its top-left and bottom-right
    /// corners cut by `cut` units.
    fn plate(&self, rect: Rect, fill: Color32, edge: Color32, cut: f32) {
        if cut <= 0.0 {
            self.painter.rect_filled(rect, 0.0, fill);
            if edge != Color32::TRANSPARENT {
                self.painter
                    .rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            }
            return;
        }
        let c = cut * self.k;
        let r = rect.shrink(0.5);
        let points = vec![
            pos2(r.min.x + c, r.min.y),
            pos2(r.max.x, r.min.y),
            pos2(r.max.x, r.max.y - c),
            pos2(r.max.x - c, r.max.y),
            pos2(r.min.x, r.max.y),
            pos2(r.min.x, r.min.y + c),
        ];
        let stroke = if edge == Color32::TRANSPARENT {
            Stroke::NONE
        } else {
            Stroke::new(1.0, edge)
        };
        self.painter
            .add(Shape::convex_polygon(points, fill, stroke));
    }

    fn rule(&self, x0: f32, x1: f32, y: f32, colour: Color32) {
        self.painter.hline(
            self.p(x0, y).x..=self.p(x1, y).x,
            self.p(x0, y).y,
            Stroke::new(1.0, colour),
        );
    }
}

/// The web addresses in `text`, as byte ranges.
fn urls(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(start) = text[at..].find("http").map(|i| i + at) {
        let rest = &text[start..];
        if !(rest.starts_with("http://") || rest.starts_with("https://")) {
            at = start + 4;
            continue;
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || "<>\"'|[]".contains(c))
            .unwrap_or(rest.len());
        let end = rest[..end]
            .trim_end_matches(['.', ',', ';', ':', '!', '?', ')'])
            .len();
        found.push(start..start + end);
        at = start + end.max(4);
    }
    found
}

/// The style for selectable text: the selection in the flow colour.
fn selecting(style: &mut egui::Style) {
    style.visuals.selection.bg_fill = FLOW.gamma_multiply(0.3);
    style.visuals.selection.stroke = Stroke::new(1.0, FLOW);
}

/// A clickable region named `label` for the accessibility tree.
fn button(ui: &Ui, rect: Rect, label: &str) -> Response {
    let response = ui.interact(rect, Id::new(("case", label)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    response
}

/// The ticket picker: my open tickets from Jira and a field for any key.
/// Answers the key to open a case for.
pub fn picker(ui: &mut Ui, jira: &mut Jira, typed: &mut String) -> Option<String> {
    jira.poll();
    let mut open = None;
    ui.horizontal(|ui| {
        let field = ui.add(
            egui::TextEdit::singleline(typed)
                .hint_text("any key, e.g. SHOP-42")
                .desired_width(180.0),
        );
        let entered = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if (ui.button("Open").clicked() || entered) && !typed.trim().is_empty() {
            open = Some(typed.trim().to_uppercase());
        }
    });
    ui.separator();
    let fetching = jira.is_fetching(super::jira::MINE);
    let failure = jira.failure(super::jira::MINE).map(str::to_owned);
    let state = jira.sign_in.clone();
    let mine = jira.mine().cloned();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("MY OPEN TICKETS").small().strong());
        let line = match (&mine, fetching) {
            (_, true) => "syncing…".to_owned(),
            (Some((_, synced)), _) => synced.label(),
            (None, _) => "not synced".to_owned(),
        };
        ui.label(egui::RichText::new(line).small().weak());
    });
    match state {
        SignIn::Needed | SignIn::Failed(_) => {
            if ui.button("Sign in to Jira").clicked() {
                jira.start_sign_in();
            }
        }
        SignIn::Pending => {
            ui.label(egui::RichText::new("sign-in pending in the browser").weak());
        }
        _ => {}
    }
    if let Some(reason) = failure {
        ui.label(egui::RichText::new(format!("sync failed: {reason}")).weak());
    }
    egui::ScrollArea::vertical()
        .max_height(420.0)
        .show(ui, |ui| {
            for row in mine.map(|(rows, _)| rows).unwrap_or_default() {
                let status = super::casefile::status_of(&row.key);
                ui.horizontal(|ui| {
                    let (mark, colour) = match status {
                        Some(Status::Fixed) => ("FIXED", OK),
                        Some(status) => (status.name(), ATTENTION),
                        None => ("NEW", INK_DIM),
                    };
                    ui.add_sized(
                        [72.0, 16.0],
                        egui::Label::new(egui::RichText::new(mark).small().strong().color(colour)),
                    )
                    .on_hover_text(if status.is_some() {
                        "where the case stands"
                    } else {
                        "not ingested yet"
                    });
                    let label = format!("{}  {}", row.key, row.summary);
                    if ui
                        .add(egui::Button::selectable(false, label).wrap())
                        .on_hover_text(format!("{} · updated {}", row.status, row.updated))
                        .clicked()
                    {
                        open = Some(row.key.clone());
                    }
                });
            }
        });
    open
}

/// `text`, cut with an ellipsis to `max` units at `font`.
fn elide(s: &Sheet, text: &str, font: Font, max: f32) -> String {
    if s.width(text, font) <= max {
        return text.to_owned();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut = format!("{}…", chars.iter().collect::<String>().trim_end());
        if s.width(&cut, font) <= max {
            return cut;
        }
    }
    String::new()
}

/// Draws the case over `ui`, on top of the live map when there is one, and
/// answers what it was asked: false once it is closed.
pub fn show(ui: &mut Ui, case: &mut Case, jira: &mut Jira, live_map: Option<Live<'_>>) -> bool {
    jira.poll();
    let ctx = ui.ctx().clone();
    let now = ctx.input(|input| input.time);
    case.refresh(now);
    let (one, two, three, enter, esc) = ctx.input(|input| {
        (
            input.key_pressed(egui::Key::Num1),
            input.key_pressed(egui::Key::Num2),
            input.key_pressed(egui::Key::Num3),
            input.key_pressed(egui::Key::Enter),
            input.key_pressed(egui::Key::Escape),
        )
    });
    let live_map = live_map.filter(|_| case.ready());
    if esc {
        // On the map, Esc leaves the case and the map stays.
        if case.stage == 1 || (case.stage == 2 && live_map.is_some()) {
            return false;
        }
        case.go(case.stage - 1, now);
    } else if one {
        case.go(1, now);
    } else if two {
        case.go(2, now);
    } else if three {
        case.go(3, now);
    } else if enter {
        match case.stage {
            1 => case.go(2, now),
            2 => case.pick(case.hover.unwrap_or(0), now),
            _ => {}
        }
    }

    let screen = ctx.content_rect();
    let (o, k) = sheet_frame(screen);
    let painter = Painter::new(ctx.clone(), ui.layer_id(), screen);
    let sheet = Sheet {
        painter,
        o,
        k,
        dy: 0.0,
    };

    // The chrome takes the pointer from the map below it; registered first,
    // so the case's own buttons on it still win.
    case.chrome.clear();
    let top = Rect::from_min_max(screen.min, pos2(screen.max.x, sheet.p(0.0, 64.0).y));
    block(ui, case, top, 0);
    match (case.stage, &live_map) {
        (2, Some(_)) => {
            let [x, y, w, h] = MAP_AREA;
            let dh = case.diagram_h();
            for (salt, [bx, by, bw, bh]) in [
                [1140.0, 80.0, 436.0, 896.0],
                [x, y, 330.0, 82.0],
                [x + w - 260.0, y, 260.0, 102.0],
                [x, y + h - dh, w, dh],
            ]
            .into_iter()
            .enumerate()
            {
                block(ui, case, sheet.r(bx, by, bw, bh), salt + 1);
            }
            // The map shows only in its area; its edges and labels stay out
            // of the gaps between the panels.
            for rect in around(screen, top, sheet.r(x, y, w, h)) {
                sheet.painter.rect_filled(rect, 0.0, BG);
            }
        }
        (3, Some(_)) => {
            // Everything but the window onto the map.
            let hole = sheet.r(HOLE[0], HOLE[1], HOLE[2], HOLE[3]);
            for (salt, rect) in around(screen, top, hole).into_iter().enumerate() {
                sheet.painter.rect_filled(rect, 0.0, BG);
                block(ui, case, rect, salt + 10);
            }
        }
        _ => {
            block(ui, case, screen, 20);
            // Stage 1 over whatever lies behind, dimmed; without a map the
            // later stages on a schematic.
            let fill = if case.stage == 1 {
                BG.gamma_multiply(0.86)
            } else {
                BG
            };
            sheet.painter.rect_filled(screen, 0.0, fill);
        }
    }

    match (case.stage, &live_map) {
        (1, _) => scrolled(ui, &sheet, case, jira, now),
        (_, Some(map)) => {
            live(ui, &sheet, case, now, map);
            if case.stage == 2 {
                fix(ui, &sheet, case, now);
            } else {
                evidence(ui, &sheet, case, now);
            }
        }
        (2, None) => {
            schematic(ui, &sheet, case, now);
            fix(ui, &sheet, case, now);
        }
        (_, None) => {
            schematic(ui, &sheet, case, now);
            evidence(ui, &sheet, case, now);
        }
    }
    match top_bar(ui, &sheet, case, jira, live_map.is_some()) {
        Some(0) => return false,
        Some(stage) => case.go(stage, now),
        None => {}
    }
    ctx.request_repaint();
    true
}

/// The four rectangles of `screen` below `top` that frame `hole`.
fn around(screen: Rect, top: Rect, hole: Rect) -> [Rect; 4] {
    [
        Rect::from_min_max(
            pos2(screen.min.x, top.max.y),
            pos2(hole.min.x, screen.max.y),
        ),
        Rect::from_min_max(pos2(hole.max.x, top.max.y), screen.max),
        Rect::from_min_max(pos2(hole.min.x, top.max.y), pos2(hole.max.x, hole.min.y)),
        Rect::from_min_max(pos2(hole.min.x, hole.max.y), pos2(hole.max.x, screen.max.y)),
    ]
}

/// The case's top bar; answers the stage asked for, 0 to leave the case.
fn top_bar(ui: &Ui, s: &Sheet, case: &mut Case, jira: &mut Jira, live: bool) -> Option<u8> {
    let mut go = None;
    let screen = ui.ctx().content_rect();
    let bar = Rect::from_min_max(screen.min, pos2(screen.max.x, s.p(0.0, 64.0).y));
    s.painter.rect_filled(bar, 0.0, PANEL);
    s.painter
        .hline(bar.x_range(), bar.max.y - 0.5, Stroke::new(1.0, LINE));

    let tag = font(12.0, "bold", 0.14, 1.25);
    let tag_w = s.width("CASE", tag) + 20.0;
    s.painter
        .rect_filled(s.r(24.0, 21.0, tag_w, 22.0), 0.0, ATTENTION_SOFT);
    s.put(
        24.0 + tag_w / 2.0,
        32.0,
        Align2::CENTER_CENTER,
        "CASE",
        tag,
        ATTENTION,
    );
    let mut style = (*ui.style().clone()).clone();
    selecting(&mut style);
    let text_ui = Ui::new(
        ui.ctx().clone(),
        ui.id().with("case top text"),
        egui::UiBuilder::new()
            .layer_id(ui.layer_id())
            .max_rect(bar)
            .style(Arc::new(style)),
    );
    let ui_text = &text_ui;
    let key = font(14.0, "mono-bold", 0.0, 1.25);
    let key_x = 24.0 + tag_w + 12.0;
    let issue_url = jira.issue(&case.key).map(|(issue, _)| issue.url.clone());
    s.pick_at(
        ui_text,
        (key_x, 25.0, Align2::LEFT_CENTER),
        &case.key,
        (key, INK, 0.0),
        issue_url.as_deref().map(|url| (case.key.len(), url)),
    );
    let title_x = key_x + s.width(&case.key, key) + 12.0;
    // The summary and how fresh it is, from Jira; up to where the rail
    // starts.
    let fetching = jira.is_fetching(&case.key);
    let failure = jira.failure(&case.key).map(str::to_owned);
    let (summary, synced) = match jira.issue(&case.key) {
        Some((issue, synced)) => (issue.summary.clone(), synced.label()),
        None => (String::new(), "not synced".to_owned()),
    };
    let title = font(14.0, "sans", 0.0, 1.25);
    s.pick_at(
        ui_text,
        (title_x, 25.0, Align2::LEFT_CENTER),
        &elide(s, &summary, title, 440.0 - title_x),
        (title, INK_DIM, 0.0),
        None,
    );
    let state = match (&jira.sign_in, fetching, failure) {
        (SignIn::Pending, _, _) => "sign-in pending in the browser".to_owned(),
        (SignIn::Needed, _, _) => "sign-in needed".to_owned(),
        (SignIn::Failed(reason), _, _) => format!("sign-in failed: {reason}"),
        (_, true, _) => "syncing…".to_owned(),
        (_, _, Some(reason)) => format!("sync failed: {reason}"),
        _ => String::new(),
    };
    let line = if state.is_empty() {
        format!("from Jira · {synced}")
    } else {
        format!("from Jira · {synced} · {state}")
    };
    s.pick_at(
        ui_text,
        (key_x, 46.0, Align2::LEFT_CENTER),
        &elide(s, &line, MONO11, 440.0 - key_x),
        (MONO11, INK_DIM, 0.0),
        None,
    );

    let label = font(12.0, "semibold", 0.12, 1.25);
    let number = font(12.0, "mono", 0.0, 1.25);
    let rail = [(1u8, "PROBLEM"), (2, "FIX IN PLACE"), (3, "EVIDENCE")];
    let widths: Vec<f32> = rail
        .iter()
        .map(|(n, name)| 24.0 + s.width(&n.to_string(), number) + 8.0 + s.width(name, label))
        .collect();
    let mut x = 456.0;
    let ready = case.ready();
    for ((n, name), w) in rail.iter().zip(&widths) {
        let current = *n == case.stage;
        let rect = s.r(x, 10.0, *w, 44.0);
        let mut s = s.clone();
        if *n > 1 && !ready {
            s.painter.multiply_opacity(0.35);
        }
        let response = button(ui, rect, &format!("{n} {name}"));
        let (fill, edge, ink) = if current {
            (ATTENTION, ATTENTION, ON_ATTENTION)
        } else if response.hovered() {
            (PANEL_RAISED, LINE_STRONG, INK)
        } else {
            (
                Color32::TRANSPARENT,
                LINE,
                if *n < case.stage { INK } else { INK_DIM },
            )
        };
        s.plate(rect, fill, edge, 8.0);
        s.put(
            x + 12.0,
            32.0,
            Align2::LEFT_CENTER,
            &n.to_string(),
            number,
            ink,
        );
        let nx = x + 12.0 + s.width(&n.to_string(), number) + 8.0;
        s.put(nx, 32.0, Align2::LEFT_CENTER, name, label, ink);
        if response.clicked() {
            go = Some(*n);
        }
        x += w + 4.0;
    }

    let mut right = 1576.0;
    if case.stage > 1 {
        let back = font(12.0, "sans", 0.12, 1.25);
        let w = s.width("BACK", back) + 32.0;
        let rect = s.r(right - w, 10.0, w, 44.0);
        let response = button(ui, rect, "BACK");
        let edge = if response.hovered() {
            LINE_STRONG
        } else {
            LINE
        };
        s.plate(rect, Color32::TRANSPARENT, edge, 0.0);
        s.put(
            right - w / 2.0,
            32.0,
            Align2::CENTER_CENTER,
            "BACK",
            back,
            INK,
        );
        if response.clicked() {
            go = Some(if case.stage == 2 && live {
                0
            } else {
                case.stage - 1
            });
        }
        right -= w + 16.0;
    }
    if matches!(jira.sign_in, SignIn::Needed | SignIn::Failed(_)) {
        let w = s.width("SIGN IN TO JIRA", LABEL) + 32.0;
        let rect = s.r(right - w, 10.0, w, 44.0);
        let response = button(ui, rect, "SIGN IN TO JIRA");
        let fill = if response.hovered() {
            PANEL_RAISED
        } else {
            Color32::TRANSPARENT
        };
        s.plate(rect, fill, ATTENTION, 8.0);
        s.put(
            right - w / 2.0,
            32.0,
            Align2::CENTER_CENTER,
            "SIGN IN TO JIRA",
            LABEL,
            ATTENTION,
        );
        if response.clicked() {
            jira.start_sign_in();
        }
        right -= w + 16.0;
    }
    // More than one repository: one map at a time, picked here.
    let repos: Vec<String> = case
        .loaded
        .as_ref()
        .map(|loaded| loaded.case.repos.iter().map(|r| r.name.clone()).collect())
        .unwrap_or_default();
    if case.stage > 1 && repos.len() > 1 {
        let chip = font(11.0, "mono", 0.0, 1.25);
        for (index, name) in repos.iter().enumerate().rev() {
            let w = s.width(name, chip) + 24.0;
            let rect = s.r(right - w, 18.0, w, 28.0);
            let response = button(ui, rect, &format!("REPO {name}"));
            let on = index == case.repo;
            let (fill, edge, ink) = if on {
                (ATTENTION_SOFT, ATTENTION, ATTENTION)
            } else if response.hovered() {
                (PANEL_RAISED, LINE_STRONG, INK)
            } else {
                (Color32::TRANSPARENT, LINE, INK_DIM)
            };
            s.plate(rect, fill, edge, 6.0);
            s.put(
                right - w / 2.0,
                32.0,
                Align2::CENTER_CENTER,
                name,
                chip,
                ink,
            );
            if response.clicked() {
                case.repo = index;
            }
            right -= w + 6.0;
        }
        s.put(
            right - 4.0,
            32.0,
            Align2::RIGHT_CENTER,
            "REPO",
            MONO11,
            INK_DIM,
        );
        right -= 34.0;
    }
    lifecycle(ui, s, case, right);
    go
}

/// The case's life on the top bar, right-aligned to `right`: the flavour,
/// then INGEST › PLAN › APPROVE › FIX, the done ones ticked, the running
/// one pulsing and the next one the button that does it.
fn lifecycle(ui: &Ui, s: &Sheet, case: &mut Case, right: f32) {
    let now = ui.ctx().input(|input| input.time);
    let pip = font(11.0, "mono-bold", 0.06, 1.25);
    let next_font = font(12.0, "bold", 0.12, 1.25);
    let acts: Vec<(Act, Standing, String)> = Act::ALL
        .iter()
        .map(|&act| {
            let standing = case.standing(act);
            let text = match standing {
                Standing::Done => format!("✓ {}", act.name()),
                Standing::Running => format!("{}…", act.name()),
                _ => act.name().to_owned(),
            };
            (act, standing, text)
        })
        .collect();
    let widths: Vec<f32> = acts
        .iter()
        .map(|(_, standing, text)| match standing {
            Standing::Next => s.width(text, next_font) + 32.0,
            _ => s.width(text, pip) + 16.0,
        })
        .collect();
    let arrow = 18.0;
    let total = widths.iter().sum::<f32>() + arrow * 3.0;
    let mut x = right - total;
    let flavour = case.flavour().name();
    let chip = font(10.0, "mono-bold", 0.1, 1.25);
    let fw = s.width(flavour, chip) + 14.0;
    s.plate(
        s.r(x - fw - 12.0, 22.0, fw, 20.0),
        PANEL_RAISED,
        LINE_STRONG,
        4.0,
    );
    s.put(
        x - 12.0 - fw / 2.0,
        32.0,
        Align2::CENTER_CENTER,
        flavour,
        chip,
        INK_DIM,
    );
    let mut clicked = None;
    for (index, ((act, standing, text), w)) in acts.iter().zip(&widths).enumerate() {
        let rect = s.r(x, 14.0, *w, 36.0);
        match standing {
            Standing::Next => {
                let response = button(ui, rect, act.name());
                let glow = if response.hovered() { 150 } else { 100 };
                s.painter.add(
                    Shadow {
                        offset: [0, 0],
                        blur: (12.0 * s.k).round() as u8,
                        spread: (1.0 * s.k).round() as u8,
                        color: Color32::from_rgba_unmultiplied(252, 238, 10, glow),
                    }
                    .as_shape(rect, 0.0),
                );
                s.plate(rect, ATTENTION, Color32::TRANSPARENT, 6.0);
                s.put(
                    x + w / 2.0,
                    32.0,
                    Align2::CENTER_CENTER,
                    text,
                    next_font,
                    ON_ATTENTION,
                );
                if response.clicked() {
                    clicked = Some(*act);
                }
            }
            Standing::Running => {
                let pulse = 0.625 + 0.275 * (TAU * (now as f32) / 1.6).cos();
                s.plate(rect, PANEL, ATTENTION.gamma_multiply(pulse), 6.0);
                s.put(
                    x + w / 2.0,
                    32.0,
                    Align2::CENTER_CENTER,
                    text,
                    pip,
                    ATTENTION,
                );
            }
            Standing::Done => {
                s.put(x + w / 2.0, 32.0, Align2::CENTER_CENTER, text, pip, OK);
            }
            Standing::Blocked(why) => {
                let _ = button(ui, rect, act.name()).on_hover_text(*why);
                s.plate(rect, Color32::TRANSPARENT, LINE, 6.0);
                s.put(x + w / 2.0, 32.0, Align2::CENTER_CENTER, text, pip, INK_DIM);
            }
            Standing::Later => {
                s.put(
                    x + w / 2.0,
                    32.0,
                    Align2::CENTER_CENTER,
                    text,
                    pip,
                    INK_DIM.gamma_multiply(0.6),
                );
            }
        }
        x += w;
        if index + 1 < acts.len() {
            s.put(
                x + arrow / 2.0,
                32.0,
                Align2::CENTER_CENTER,
                "›",
                pip,
                LINE_STRONG,
            );
            x += arrow;
        }
    }
    if let Some(act) = clicked {
        case.act(act, now);
    }
}

// ---------------------------------------------------------------- the live map

/// What the app drew this frame under the case: the opened project's map and
/// where its towers went on screen.
pub struct Live<'a> {
    pub map: &'a ProjectMap,
    pub towers: &'a [crate::ui::DrawnTower],
}

/// Where the 1600 x 1000 sheet sits on `screen`, and its scale.
fn sheet_frame(screen: Rect) -> (Pos2, f32) {
    let k = (screen.width() / 1600.0).min(screen.height() / 1000.0);
    (screen.center() - vec2(800.0, 500.0) * k, k)
}

/// The part of stage 2's sheet the map shows through: left of the steps,
/// below the toggle, above the outside-systems panel.
const MAP_AREA: [f32; 4] = [24.0, 80.0, 1092.0, 896.0];
/// Stage 3's window onto the map.
const HOLE: [f32; 4] = [1216.0, 80.0, 360.0, 230.0];

impl Case {
    /// The folder whose map the case wants open, on stages 2 and 3: the
    /// shown repository's worktree, else its root.
    pub fn wants(&self) -> Option<PathBuf> {
        if self.stage < 2 || !self.ready() {
            return None;
        }
        let loaded = self.loaded.as_ref()?;
        loaded.case.repos.get(self.repo).map(casefile::Repo::base)
    }

    /// Whether the case's chrome covers `point` on screen, as drawn last
    /// frame: the map does not zoom under it. Stage 1 covers the whole map.
    pub fn covers(&self, point: Pos2) -> bool {
        self.stage == 1 || self.chrome.iter().any(|rect| rect.contains(point))
    }

    /// The repository a step's files live in, by its index in `repos`.
    fn repo_of(&self, step: usize) -> Option<usize> {
        let case = &self.loaded.as_ref()?.case;
        let name = case
            .steps
            .get(step)?
            .spots
            .iter()
            .find_map(|spot| match spot {
                Spot::File(file) => file.repo.clone(),
                Spot::System(_) => None,
            })?;
        case.repos.iter().position(|repo| repo.name == name)
    }

    /// Per place, the path it has on `map` when the map holds it.
    fn in_city(&self, map: &ProjectMap) -> Vec<Option<PathBuf>> {
        self.places
            .iter()
            .map(|place| match &place.spot {
                Spot::File(file) => {
                    let path = file.absolute();
                    let path = path.canonicalize().unwrap_or(path);
                    (path.starts_with(&map.root) && !map.items_under(&path).is_empty())
                        .then_some(path)
                }
                Spot::System(_) => None,
            })
            .collect()
    }

    /// The step in focus: hovered on stage 2, open on stage 3.
    fn focus(&self) -> Option<usize> {
        match self.stage {
            2 => self.hover,
            3 => Some(self.step),
            _ => None,
        }
    }

    /// What the map lights on stages 2 and 3: the case's buildings, the
    /// focused step's in full. Everything else recedes.
    pub fn spots(&self, map: &ProjectMap) -> Option<Vec<MapSpot>> {
        if self.stage < 2 || !self.ready() {
            return None;
        }
        let focus = self.focus();
        let shows = |steps: u32| focus.is_none_or(|f| steps & (1 << f) != 0);
        Some(
            self.places
                .iter()
                .zip(self.in_city(map))
                .filter_map(|(place, path)| {
                    Some(MapSpot {
                        path: path?,
                        light: if shows(place.steps) {
                            1.0
                        } else {
                            super::ticket::RECEDES * 2.0
                        },
                        pulse: shows(place.steps),
                    })
                })
                .collect(),
        )
    }

    /// Where the camera should be this frame while it glides onto the case's
    /// buildings: on entering stage 2 all of them, framed in the map area; on
    /// stage 3 the step's, framed in its window. None once it has arrived, so
    /// the map is free to pan and zoom.
    pub fn camera(
        &mut self,
        map: &ProjectMap,
        visible: Option<Rect>,
        screen: Rect,
        now: f64,
    ) -> Option<Rect> {
        if self.stage < 2 || !self.ready() {
            self.aim = None;
            return None;
        }
        let focus = (self.stage == 3).then_some(self.step);
        let extent = map.extent;
        let key = (
            self.stage,
            focus.unwrap_or(usize::MAX),
            self.repo,
            self.diagram,
            [extent.min.x, extent.min.y, extent.max.x, extent.max.y].map(|v| v.round() as i32),
        );
        if self.aim.as_ref().is_none_or(|aim| aim.key != key) {
            let area = self
                .places
                .iter()
                .zip(self.in_city(map))
                .filter(|(place, _)| focus.is_none_or(|f| place.steps & (1 << f) != 0))
                .filter_map(|(_, path)| path)
                .flat_map(|path| map.items_under(&path))
                .map(|item| item.area)
                .fold(Rect::NOTHING, Rect::union);
            let area = if area.is_positive() {
                area.expand(area.size().max_elem() * 0.15 + 2.0)
            } else {
                extent
            };
            let [x, y, w, h] = if self.stage == 3 {
                HOLE
            } else {
                let [x, y, w, h] = MAP_AREA;
                // Clear of the toggle above and the outside systems below.
                [x, y + 90.0, w, h - 90.0 - self.diagram_h()]
            };
            let (o, k) = sheet_frame(screen);
            let target = Rect::from_min_size(o + vec2(x, y) * k, vec2(w, h) * k);
            let scale = (target.width() / area.width().max(1.0))
                .min(target.height() / area.height().max(1.0));
            let side = screen.width().min(screen.height()) / scale;
            let centre = area.center() - (target.center() - screen.center()) / scale;
            let to = Rect::from_center_size(centre, vec2(side, side));
            let whole =
                Rect::from_center_size(extent.center(), vec2(1.0, 1.0) * extent.size().max_elem());
            let from = visible.filter(|v| v.intersects(extent)).unwrap_or(whole);
            self.aim = Some(Aim {
                key,
                from,
                to,
                at: now,
                done: false,
            });
        }
        let aim = self.aim.as_mut()?;
        if aim.done {
            return None;
        }
        let t = ease(((now - aim.at) / 0.7) as f32);
        aim.done = t >= 1.0;
        Some(Rect::from_min_max(
            aim.from.min + (aim.to.min - aim.from.min) * t,
            aim.from.max + (aim.to.max - aim.from.max) * t,
        ))
    }

    fn diagram_h(&self) -> f32 {
        if self.diagram { 300.0 } else { 34.0 }
    }
}

/// The camera's glide onto the case: what it is aimed at (stage, step,
/// repository, panel, map), from where, to where, since when.
struct Aim {
    key: (u8, usize, usize, bool, [i32; 4]),
    from: Rect,
    to: Rect,
    at: f64,
    done: bool,
}

/// Takes the pointer over `rect` from the map below, and remembers it so
/// the map does not zoom under it either.
fn block(ui: &Ui, case: &mut Case, rect: Rect, salt: usize) {
    ui.interact(
        rect,
        Id::new(("case chrome", salt)),
        Sense::click_and_drag(),
    );
    case.chrome.push(rect);
}

/// The case over the live map: lanes from its buildings to the outside
/// systems docked below, a pulsing ring and a name on each building, and the
/// TODAY / AFTER toggle. On stage 3 only the ring, in the step's window.
fn live(ui: &mut Ui, s: &Sheet, case: &mut Case, now: f64, live: &Live<'_>) {
    let focus = case.focus();
    let after = case.stage == 3 || case.after;
    let shows = |steps: u32| focus.is_none_or(|f| steps & (1 << f) != 0);
    let ctx = ui.ctx().clone();
    let fade = |key: (&str, usize), target: f32| {
        ctx.animate_value_with_time(Id::new(("case fade", key.0, key.1)), target, 0.4)
    };
    let in_city = case.in_city(live.map);
    let [x, y, w, h] = if case.stage == 3 { HOLE } else { MAP_AREA };
    let rect = s.r(x, y, w, h);
    let diagram_rect = s.r(x, y + h - case.diagram_h(), w, case.diagram_h());
    let city_rect = if case.stage == 3 {
        rect
    } else {
        Rect::from_min_max(rect.min, pos2(rect.max.x, diagram_rect.min.y))
    };

    // Where each place stands on screen: its buildings, or its glyph.
    let mut at: Vec<Option<Pos2>> = vec![None; case.places.len()];
    let mut grounds: Vec<Option<Rect>> = vec![None; case.places.len()];
    for tower in live.towers {
        for (index, path) in in_city.iter().enumerate() {
            if let Some(path) = path
                && tower.path.starts_with(path)
            {
                grounds[index] =
                    Some(grounds[index].map_or(tower.outline, |r| r.union(tower.outline)));
            }
        }
    }
    for (index, ground) in grounds.iter().enumerate() {
        if let Some(ground) = ground
            && city_rect.contains(ground.center())
        {
            at[index] = Some(ground.center());
        }
    }

    let p = s
        .painter
        .with_clip_rect(rect.intersect(s.painter.clip_rect()));
    let pulse = 0.625 + 0.275 * (TAU * (now as f32) / 1.6).cos();
    let on_city = p.with_clip_rect(city_rect.intersect(p.clip_rect()));

    // The outside systems, docked below the map; collapsible.
    let outside: Vec<usize> = (0..case.places.len())
        .filter(|&i| in_city[i].is_none())
        .collect();
    let column = |place: &Place| match place.glyph {
        Glyph::Doc => 0,
        Glyph::Padlock | Glyph::Key => 1,
        Glyph::Hex => 2,
        Glyph::Person => 3,
        Glyph::Db => 4,
    };
    let m = (diagram_rect.width() / 1000.0).min((diagram_rect.height() - 30.0 * s.k) / 270.0);
    let origin = pos2(
        diagram_rect.center().x - 500.0 * m,
        diagram_rect.min.y + 30.0 * s.k - 20.0 * m,
    );
    let v = |x: f32, y: f32| origin + vec2(x, y) * m;
    if case.stage == 2 {
        let a = s.rising(case.entered, now, 2);
        a.painter.rect_filled(diagram_rect, 0.0, BG_DEEP);
        a.painter.line_segment(
            [diagram_rect.left_top(), diagram_rect.right_top()],
            Stroke::new(1.0, LINE_STRONG),
        );
        let header = Rect::from_min_size(diagram_rect.min, vec2(diagram_rect.width(), 34.0 * s.k));
        let toggle = button(ui, header, "OUTSIDE SYSTEMS");
        if toggle.clicked() {
            case.diagram = !case.diagram;
        }
        let caption = if case.diagram {
            "OUTSIDE SYSTEMS  ▾"
        } else {
            "OUTSIDE SYSTEMS  ▸"
        };
        a.put(
            x + 14.0,
            y + h - case.diagram_h() + 17.0,
            Align2::LEFT_CENTER,
            caption,
            MONO12,
            if toggle.hovered() { INK } else { TERRAIN_INK },
        );
        if case.diagram {
            for (c, cx) in [30.0, 230.0, 440.0, 610.0, 975.0].into_iter().enumerate() {
                let here: Vec<usize> = outside
                    .iter()
                    .copied()
                    .filter(|&i| column(&case.places[i]) == c)
                    .collect();
                let spacing = (220.0 / (here.len().max(2) - 1) as f32).min(52.0);
                let top = 150.0 - spacing * (here.len().max(1) - 1) as f32 / 2.0;
                for (row, index) in here.into_iter().enumerate() {
                    at[index] = Some(v(cx, top + row as f32 * spacing));
                }
            }
        }

        // The lanes, today's or after the fix, from the buildings where they
        // stand now.
        let lanes = case
            .loaded
            .as_ref()
            .map(|loaded| loaded.case.lanes.clone())
            .unwrap_or_default();
        let index_of = |spot: &Spot| case.places.iter().position(|p| p.spot.same(spot));
        for (index, lane) in lanes.iter().enumerate() {
            if lane.when != When::Both && (lane.when == When::After) != after {
                continue;
            }
            let (Some(a), Some(b)) = (index_of(&lane.from), index_of(&lane.to)) else {
                continue;
            };
            let (Some(pa), Some(pb)) = (at[a], at[b]) else {
                continue;
            };
            let steps = case.places[a].steps | case.places[b].steps;
            let o = fade(("lane", index), if shows(steps) { 0.95 } else { 0.12 });
            let d = pb - pa;
            let curve = if d.y.abs() > d.x.abs() {
                let my = (pa.y + pb.y) / 2.0;
                [pa, pos2(pa.x, my), pos2(pb.x, my), pb]
            } else {
                let mx = (pa.x + pb.x) / 2.0;
                [pa, pos2(mx, pa.y), pos2(mx, pb.y), pb]
            };
            let points: Vec<Pos2> = (0..=32)
                .map(|i| {
                    let t = i as f32 / 32.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    (0..4).fold(Pos2::ZERO, |sum, j| sum + curve[j].to_vec2() * w[j])
                })
                .collect();
            let (colour, width, dashed) = match lane.kind {
                LaneKind::Root => (DANGER, 2.0, false),
                LaneKind::OwnRole => (FLOW, 2.0, false),
                LaneKind::DefinedBy => (LANE_CALLS, 1.6, true),
                LaneKind::Reads => (INK_DIM, 1.2, true),
            };
            let stroke = Stroke::new(width, colour.gamma_multiply(o));
            if dashed {
                p.extend(Shape::dashed_line(&points, stroke, 5.0, 4.0));
            } else {
                p.add(Shape::line(points, stroke));
            }
        }

        // The glyphs of the diagram.
        for &index in &outside {
            let Some(c) = at[index] else { continue };
            let place = &case.places[index];
            let hit = focus.is_some_and(|f| place.steps & (1 << f) != 0);
            let o = fade(("spot", index), if shows(place.steps) { 1.0 } else { 0.22 });
            let mut stroke = if matches!(place.spot, Spot::File(_)) {
                INK
            } else {
                INFO
            };
            if place.root {
                stroke = if after { INK_DIM } else { DANGER };
            } else if matches!(place.glyph, Glyph::Person) {
                stroke = if after { OK } else { INK_DIM };
            }
            if hit {
                stroke = ATTENTION;
                p.circle_stroke(
                    c,
                    17.0 * m,
                    Stroke::new(2.0 * m, ATTENTION.gamma_multiply(pulse * o)),
                );
            }
            p.rect_filled(
                Rect::from_center_size(c, vec2(26.0, 26.0) * m),
                0.0,
                BG_DEEP.gamma_multiply(o),
            );
            glyph(
                &p,
                place.glyph,
                c,
                m * 0.9,
                Stroke::new(1.6 * m, stroke.gamma_multiply(o)),
            );
            let ink = if hit { INK } else { INK_DIM }.gamma_multiply(o);
            let (dx, align) = if column(place) == 4 {
                (-18.0, Align2::RIGHT_CENTER)
            } else {
                (18.0, Align2::LEFT_CENTER)
            };
            let galley = p.layout_no_wrap(
                place.label.clone(),
                FontId::new(11.0 * m.max(0.8), family("mono")),
                ink,
            );
            let tag = align.anchor_size(c + vec2(dx * m, 0.0), galley.size());
            p.rect_filled(
                tag.expand2(vec2(4.0, 1.0) * m),
                0.0,
                BG_DEEP.gamma_multiply(0.9),
            );
            p.galley(tag.min, galley, ink);
        }
    }

    // The lit buildings: a pulsing ring and, on stage 2, their name; a tag
    // that would cover an earlier one moves below it.
    let mut tags: Vec<Rect> = Vec::new();
    for (index, ground) in grounds.iter().enumerate() {
        let Some(ground) = ground else { continue };
        let place = &case.places[index];
        let o = fade(("spot", index), if shows(place.steps) { 1.0 } else { 0.0 });
        if o < 0.02 {
            continue;
        }
        let radius = ground.size().max_elem() / 2.0 + 8.0;
        on_city.circle_stroke(
            ground.center(),
            radius,
            Stroke::new(2.0, ATTENTION.gamma_multiply(pulse * o)),
        );
        if case.stage == 2 {
            let galley = on_city.layout_no_wrap(
                place.label.clone(),
                FontId::new(11.0, family("mono")),
                INK.gamma_multiply(o),
            );
            let mut tag = ground.center() + vec2(radius + 6.0, -radius * 0.5);
            while let Some(taken) = tags
                .iter()
                .find(|taken| taken.intersects(Rect::from_min_size(tag, galley.size()).expand(3.0)))
            {
                tag.y = taken.max.y + 5.0;
            }
            let back = Rect::from_min_size(tag, galley.size()).expand2(vec2(5.0, 2.0));
            tags.push(back);
            on_city.rect_filled(back, 0.0, BG_DEEP.gamma_multiply(0.85 * o));
            on_city.rect_stroke(
                back,
                0.0,
                Stroke::new(1.0, ATTENTION.gamma_multiply(o)),
                StrokeKind::Outside,
            );
            on_city.galley(tag, galley, INK);
        }
    }
    if case.stage == 3 {
        p.rect_stroke(rect, 0.0, Stroke::new(1.0, LINE_STRONG), StrokeKind::Inside);
    }
    frame_overlay(ui, s, case, now, [x, y, w]);
}

fn schematic(ui: &Ui, s: &Sheet, case: &mut Case, now: f64) {
    let [x, y, w, h, opacity] = case.frame(now);
    let rect = s.r(x, y, w, h);
    let mut p = s
        .painter
        .with_clip_rect(rect.intersect(s.painter.clip_rect()));
    p.multiply_opacity(opacity);
    p.rect_filled(rect, 0.0, BG_DEEP);
    p.rect_stroke(rect, 0.0, Stroke::new(1.0, LINE), StrokeKind::Inside);

    // The map's own sheet: 1000 x 820, centred in the frame.
    let m = (rect.width() / 1000.0).min(rect.height() / 820.0);
    let origin = rect.center() - vec2(500.0, 410.0) * m;
    let v = |x: f32, y: f32| origin + vec2(x, y) * m;
    let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(v(x, y), vec2(w, h) * m);
    let text = |at: Pos2,
                align: Align2,
                text: &str,
                size: f32,
                family_name: &str,
                spacing: f32,
                colour: Color32| {
        let mut job = LayoutJob::single_section(
            text.to_owned(),
            TextFormat {
                font_id: FontId::new(size * m, family(family_name)),
                color: colour,
                extra_letter_spacing: spacing * m,
                ..Default::default()
            },
        );
        job.wrap.max_width = f32::INFINITY;
        let galley = p.layout_job(job);
        let at = align.anchor_size(at, galley.size()).min;
        p.galley(at, galley, colour);
    };

    let focus = match case.stage {
        2 => case.hover,
        3 => Some(case.step),
        _ => None,
    };
    let after = case.stage == 3 || case.after;
    let shows = |steps: u32| focus.is_none_or(|f| steps & (1 << f) != 0);
    let fade = |key: (&str, usize), target: f32| {
        ui.ctx()
            .animate_value_with_time(Id::new(("case fade", key.0, key.1)), target, 0.4)
    };

    // The code base: a terrain of blocks, with one district per folder that
    // holds files of the case.
    p.rect_filled(r(40.0, 60.0, 920.0, 360.0), 0.0, TERRAIN);
    p.rect_stroke(
        r(40.0, 60.0, 920.0, 360.0),
        0.0,
        Stroke::new(1.0, TERRAIN_EDGE),
        StrokeKind::Inside,
    );
    let blocks = if case.stage == 3 { 0.5 } else { 0.8 };
    for row in 0..8 {
        for column in 0..20 {
            let (bx, by) = (60.0 + column as f32 * 44.0, 100.0 + row as f32 * 38.0);
            let covered = case.districts.iter().any(|d| {
                bx + 34.0 > d.x - 10.0
                    && bx < d.x + d.w + 10.0
                    && by + 26.0 > d.y - 10.0
                    && by < d.y + d.h + 10.0
            });
            if !covered && bx + 34.0 < 950.0 {
                p.rect_filled(
                    r(bx, by, 34.0, 26.0),
                    0.0,
                    TERRAIN_EDGE.gamma_multiply(blocks),
                );
            }
        }
    }
    for (index, district) in case.districts.iter().enumerate() {
        let lit = focus.is_some_and(|f| district.steps & (1 << f) != 0);
        let o = fade(
            ("district", index),
            if shows(district.steps) { 1.0 } else { 0.35 },
        );
        let area = r(district.x, district.y, district.w, district.h);
        p.rect_filled(area, 0.0, PANEL.gamma_multiply(o));
        let (edge, width) = if lit {
            (ATTENTION, 2.0)
        } else {
            (LINE_STRONG, 1.0)
        };
        p.rect_stroke(
            area,
            0.0,
            Stroke::new(width * m, edge.gamma_multiply(o)),
            StrokeKind::Inside,
        );
        text(
            v(district.x + 8.0, district.y + 22.0),
            Align2::LEFT_BOTTOM,
            &district.label,
            11.0,
            "mono",
            0.0,
            INK.gamma_multiply(o),
        );
    }

    let zone = [
        v(40.0, 465.0),
        v(960.0, 465.0),
        v(960.0, 800.0),
        v(40.0, 800.0),
        v(40.0, 465.0),
    ];
    p.extend(Shape::dashed_line(
        &zone,
        Stroke::new(1.0, ZONE_FOREIGN),
        6.0 * m,
        5.0 * m,
    ));
    text(
        v(52.0, 492.0),
        Align2::LEFT_BOTTOM,
        "OUTSIDE SYSTEMS · STUBS",
        13.0,
        "sans",
        2.0,
        TERRAIN_INK,
    );

    let lanes = case
        .loaded
        .as_ref()
        .map(|loaded| loaded.case.lanes.clone())
        .unwrap_or_default();
    let place = |spot: &Spot| case.places.iter().find(|p| p.spot.same(spot));
    for (index, lane) in lanes.iter().enumerate() {
        if lane.when != When::Both && (lane.when == When::After) != after {
            continue;
        }
        let (Some(a), Some(b)) = (place(&lane.from), place(&lane.to)) else {
            continue;
        };
        let steps = a.steps | b.steps;
        let o = fade(("lane", index), if shows(steps) { 0.95 } else { 0.12 });
        let (x1, x2) = (a.x + 14.0, b.x - 14.0);
        let mx = (x1 + x2) / 2.0;
        let curve = [v(x1, a.y), v(mx, a.y), v(mx, b.y), v(x2, b.y)];
        let points: Vec<Pos2> = (0..=32)
            .map(|i| {
                let t = i as f32 / 32.0;
                let u = 1.0 - t;
                let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                (0..4).fold(Pos2::ZERO, |sum, j| sum + curve[j].to_vec2() * w[j])
            })
            .collect();
        let (colour, width, dashed) = match lane.kind {
            LaneKind::Root => (DANGER, 2.0, false),
            LaneKind::OwnRole => (FLOW, 2.0, false),
            LaneKind::DefinedBy => (LANE_CALLS, 2.0, true),
            LaneKind::Reads => (INK_DIM, 1.2, true),
        };
        let stroke = Stroke::new(width * m, colour.gamma_multiply(o));
        if dashed {
            p.extend(Shape::dashed_line(&points, stroke, 5.0 * m, 4.0 * m));
        } else {
            p.add(Shape::line(points, stroke));
        }
    }

    let pulse = 0.625 + 0.275 * (TAU * (now as f32) / 1.6).cos();
    for (index, spot) in case.places.iter().enumerate() {
        let hit = focus.is_some_and(|f| spot.steps & (1 << f) != 0);
        let o = fade(("spot", index), if shows(spot.steps) { 1.0 } else { 0.22 });
        let mut stroke = if spot.outside { INFO } else { INK };
        if spot.root {
            stroke = if after { INK_DIM } else { DANGER };
        } else if matches!(spot.glyph, Glyph::Person) {
            stroke = if after { OK } else { INK_DIM };
        }
        if hit {
            stroke = ATTENTION;
        }
        let at = v(spot.x, spot.y);
        if hit {
            p.circle_stroke(
                at,
                19.0 * m,
                Stroke::new(2.0 * m, ATTENTION.gamma_multiply(pulse * o)),
            );
        }
        p.rect_filled(
            Rect::from_center_size(at, vec2(28.0, 28.0) * m),
            0.0,
            BG_DEEP.gamma_multiply(o),
        );
        glyph(
            &p,
            spot.glyph,
            at,
            m,
            Stroke::new(1.7 * m, stroke.gamma_multiply(o)),
        );
        let ink = if hit { INK } else { INK_DIM }.gamma_multiply(o);
        let (lx, align) = if spot.left {
            (-18.0, Align2::RIGHT_CENTER)
        } else {
            (18.0, Align2::LEFT_CENTER)
        };
        text(
            v(spot.x + lx, spot.y),
            align,
            &spot.label,
            11.0,
            "mono",
            0.0,
            ink,
        );
    }

    frame_overlay(ui, s, case, now, [x, y, w]);
}

/// What sits on the map frame itself, in sheet units: the TODAY / AFTER
/// toggle and the legend in stage 2, the caption in stage 3.
fn frame_overlay(ui: &Ui, s: &Sheet, case: &mut Case, now: f64, [x, y, w]: [f32; 3]) {
    if case.stage == 2 {
        let s = s.rising(case.entered, now, 1);
        let toggle = font(12.0, "semibold", 0.12, 1.25);
        let mut bx = x + 17.0;
        let widths = ["TODAY", "AFTER THE FIX"].map(|t| s.width(t, toggle) + 36.0);
        s.plate(
            s.r(x + 16.0, y + 16.0, widths[0] + widths[1] + 2.0, 46.0),
            PANEL,
            LINE,
            0.0,
        );
        for (index, name) in ["TODAY", "AFTER THE FIX"].into_iter().enumerate() {
            let on = (index == 1) == case.after;
            let rect = s.r(bx, y + 17.0, widths[index], 44.0);
            let response = button(ui, rect, name);
            let fill = match (on, index) {
                (true, 0) => DANGER,
                (true, _) => FLOW,
                _ => Color32::TRANSPARENT,
            };
            s.painter.rect_filled(rect, 0.0, fill);
            let ink = if on { ON_ATTENTION } else { INK_DIM };
            s.put(
                bx + widths[index] / 2.0,
                y + 39.0,
                Align2::CENTER_CENTER,
                name,
                toggle,
                ink,
            );
            if response.clicked() {
                case.after = index == 1;
            }
            bx += widths[index];
        }

        let rows = [
            ("logs in as root", DANGER, false),
            ("logs in as own role", FLOW, false),
            ("defined by", LANE_CALLS, true),
        ];
        let text_w = rows
            .iter()
            .map(|(t, _, _)| s.width(t, MONO11))
            .fold(0.0, f32::max);
        let lw = 28.0 + 30.0 + text_w;
        let lx = x + w - 16.0 - lw;
        s.plate(s.r(lx, y + 16.0, lw, 76.0), PANEL, LINE, 0.0);
        for (index, (name, colour, dashed)) in rows.into_iter().enumerate() {
            let cy = y + 33.0 + index as f32 * 20.0;
            let line = [s.p(lx + 14.0, cy), s.p(lx + 36.0, cy)];
            if dashed {
                s.painter.extend(Shape::dashed_line(
                    &line,
                    Stroke::new(2.0, colour),
                    4.0,
                    3.0,
                ));
            } else {
                s.painter.line_segment(line, Stroke::new(2.0, colour));
            }
            s.put(lx + 44.0, cy, Align2::LEFT_CENTER, name, MONO11, INK_DIM);
        }
    } else if case.stage == 3 {
        let caption = font(11.0, "sans", 0.14, 1.25);
        let cw = s.width("WHERE THIS STEP LIVES", caption) + 16.0;
        s.painter
            .rect_filled(s.r(x + 1.0, y + 1.0, cw, 22.0), 0.0, PANEL);
        s.put(
            x + 9.0,
            y + 12.0,
            Align2::LEFT_CENTER,
            "WHERE THIS STEP LIVES",
            caption,
            INK_DIM,
        );
    }
}

/// Draws the glyph `kind` around `c`, its 28 x 28 units scaled by `m`.
fn glyph(p: &Painter, kind: Glyph, c: Pos2, m: f32, stroke: Stroke) {
    let pt = |x: f32, y: f32| c + vec2(x, y) * m;
    let arc = |cx: f32, cy: f32, rx: f32, ry: f32, from: f32, to: f32| -> Vec<Pos2> {
        (0..=24)
            .map(|i| {
                let a = from + (to - from) * i as f32 / 24.0;
                pt(cx + rx * a.cos(), cy + ry * a.sin())
            })
            .collect()
    };
    let line = |points: Vec<Pos2>| {
        p.add(Shape::line(points, stroke));
    };
    let closed = |points: Vec<Pos2>| {
        p.add(Shape::closed_line(points, stroke));
    };
    match kind {
        Glyph::Padlock => {
            closed(vec![
                pt(-9.0, -2.0),
                pt(9.0, -2.0),
                pt(9.0, 12.0),
                pt(-9.0, 12.0),
            ]);
            let mut shackle = vec![pt(-6.0, -2.0)];
            shackle.extend(arc(0.0, -7.0, 6.0, 6.0, PI, TAU));
            shackle.push(pt(6.0, -2.0));
            line(shackle);
        }
        Glyph::Key => {
            closed(arc(-6.0, 0.0, 5.0, 5.0, 0.0, TAU));
            line(vec![pt(-1.0, 0.0), pt(11.0, 0.0)]);
            line(vec![pt(7.0, 0.0), pt(7.0, 4.0)]);
            line(vec![pt(10.0, 0.0), pt(10.0, 3.0)]);
        }
        Glyph::Hex => closed(
            [
                (10.0, 0.0),
                (5.0, 8.7),
                (-5.0, 8.7),
                (-10.0, 0.0),
                (-5.0, -8.7),
                (5.0, -8.7),
            ]
            .map(|(x, y)| pt(x, y))
            .to_vec(),
        ),
        Glyph::Person => {
            closed(arc(0.0, -5.0, 4.5, 4.5, 0.0, TAU));
            line(arc(0.0, 10.0, 9.0, 8.0, PI, TAU));
        }
        Glyph::Db => {
            closed(arc(0.0, -7.0, 9.0, 3.5, 0.0, TAU));
            line(vec![pt(-9.0, -7.0), pt(-9.0, 7.0)]);
            line(vec![pt(9.0, -7.0), pt(9.0, 7.0)]);
            line(arc(0.0, 7.0, 9.0, 3.5, 0.0, PI));
        }
        Glyph::Doc => {
            closed(vec![
                pt(-7.0, -10.0),
                pt(4.0, -10.0),
                pt(8.0, -6.0),
                pt(8.0, 10.0),
                pt(-7.0, 10.0),
            ]);
            line(vec![pt(-3.0, -2.0), pt(4.0, -2.0)]);
            line(vec![pt(-3.0, 3.0), pt(4.0, 3.0)]);
        }
    }
}

// ---------------------------------------------------------------- stage 1

/// Stage 1 below the top bar, scrolling when it runs past the window.
fn scrolled(ui: &mut Ui, s: &Sheet, case: &mut Case, jira: &mut Jira, now: f64) {
    let screen = ui.ctx().content_rect();
    let rect = Rect::from_min_max(pos2(screen.min.x, s.p(0.0, 64.0).y), screen.max);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(("case stage 1", &case.key)),
    );
    let style = child.style_mut();
    style.spacing.scroll = egui::style::ScrollStyle::thin();
    let widgets = &mut style.visuals.widgets;
    widgets.inactive.bg_fill = LINE_STRONG;
    widgets.inactive.fg_stroke.color = LINE_STRONG;
    widgets.hovered.bg_fill = INK_DIM;
    widgets.hovered.fg_stroke.color = INK_DIM;
    widgets.active.bg_fill = ATTENTION;
    widgets.active.fg_stroke.color = ATTENTION;
    style.visuals.extreme_bg_color = BG_DEEP;
    selecting(style);
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut child, |ui| {
            let mut sheet = s.clone();
            sheet.painter = ui.painter().clone();
            sheet.o.y = ui.max_rect().min.y - 64.0 * s.k;
            let bottom = problem(ui, &sheet, case, jira, now);
            ui.allocate_space(vec2(ui.available_width(), (bottom + 40.0 - 64.0) * s.k));
        });
}

/// Draws stage 1 and answers where it ends, in sheet units.
fn problem(ui: &Ui, s: &Sheet, case: &mut Case, jira: &mut Jira, now: f64) -> f32 {
    let (x, top, gap) = (180.0, 104.0, 24.0);
    let left_w = (1240.0 - gap) * 1.25 / 2.25;
    let right_w = 1240.0 - gap - left_w;
    let rx = x + left_w + gap;
    let issue = jira
        .issue(&case.key)
        .map(|(issue, synced)| (issue.clone(), synced.label()));
    let file = case.loaded.as_ref().map(|loaded| loaded.case.clone());
    let raw = case.raw || file.is_none();

    // Measure first: the left panel stretches to the taller column.
    let inner = left_w - 64.0;
    let h1 = font(34.0, "bold", 0.0, 1.15);
    let body = font(17.0, "sans", 0.0, 1.6);
    let title = font(14.0, "semibold", 0.0, 1.35);
    let provenance = MONO11;
    let card_text = inner - 32.0 - 44.0 - 14.0;

    // The left column: the case in plain words, or the ticket as written.
    let (label, origin, headline, paragraph, cards): (
        _,
        _,
        String,
        String,
        Vec<(String, String, String)>,
    ) =
        if raw {
            let (summary, description, origin) = match &issue {
                Some((issue, synced)) => (
                    issue.summary.clone(),
                    issue.description.clone(),
                    format!("from Jira · {synced} · {}", issue.url),
                ),
                None => (
                    case.key.clone(),
                    "The ticket has not been read from Jira yet.".to_owned(),
                    "from Jira · not synced".to_owned(),
                ),
            };
            // Jira's wiki markup, reduced to what reads as plain text.
            let description = description
                .trim()
                .replace("**", "")
                .replace("\\[", "[")
                .replace("\\]", "]");
            let meta = issue
                .as_ref()
                .map(|(issue, _)| {
                    let mut meta = vec![
                        (
                            "TYPE".to_owned(),
                            issue.kind.clone(),
                            format!("{} · {}", issue.status, issue.priority),
                        ),
                        (
                            "WHO".to_owned(),
                            issue.assignee.clone().unwrap_or("unassigned".into()),
                            format!(
                                "reported by {}",
                                issue.reporter.clone().unwrap_or("nobody".into())
                            ),
                        ),
                    ];
                    if let Some((how, key, summary)) = issue.links.first() {
                        meta.push(("LINK".to_owned(), format!("{how} {key}"), summary.clone()));
                    }
                    meta
                })
                .unwrap_or_default();
            (
                "1 · THE TICKET, AS WRITTEN IN JIRA",
                origin,
                summary,
                description,
                meta,
            )
        } else {
            let file = file.as_ref().expect("a case file when not raw");
            (
                "1 · THE PROBLEM, IN PLAIN WORDS",
                format!("from case by Claude, {}", file.written),
                file.problem.headline.clone(),
                file.problem.plain.clone(),
                file.problem
                    .problems
                    .iter()
                    .map(|f| (f.id.clone(), f.title.clone(), f.place.clone()))
                    .chain(file.open_questions.iter().enumerate().map(|(i, q)| {
                        (format!("Q{}", i + 1), q.clone(), "open question".to_owned())
                    }))
                    .collect(),
            )
        };
    let h1 = if raw {
        font(24.0, "bold", 0.0, 1.2)
    } else {
        h1
    };
    let body = if raw {
        font(14.0, "sans", 0.0, 1.5)
    } else {
        body
    };
    // The origin wraps beside the toggle; in case mode Jira's own summary
    // follows it, in full.
    let origin_w = if file.is_some() {
        inner - s.width("RAW TICKET", LABEL) - 44.0
    } else {
        inner
    };
    let jira_line = match (&issue, raw) {
        (Some((issue, _)), false) => Some(format!("JIRA  {}", issue.summary)),
        _ => None,
    };
    let jira_font = font(13.0, "sans", 0.0, 1.4);
    let issue_url = issue.as_ref().map(|(issue, _)| issue.url.clone());
    // The linked issue's page, next to this issue's.
    let linked_url = issue.as_ref().and_then(|(issue, _)| {
        let (_, key, _) = issue.links.first()?;
        let (site, _) = issue.url.rsplit_once("/browse/")?;
        Some(format!("{site}/browse/{key}"))
    });
    let jira_h = jira_line
        .as_ref()
        .map(|line| s.height(line, jira_font, origin_w) + 6.0)
        .unwrap_or(0.0);
    let card_hs: Vec<f32> = cards
        .iter()
        .map(|(_, t, w)| {
            28.0 + s.height(t, title, card_text) + 4.0 + s.height(w, MONO12, card_text)
        })
        .collect();
    let left_h = 64.0
        + s.height("X", LABEL, 0.0)
        + 6.0
        + s.height(&origin, provenance, origin_w)
        + jira_h
        + 20.0
        + s.height(&headline, h1, inner)
        + 20.0
        + s.height(&paragraph, body, inner)
        + 20.0
        + card_hs.iter().sum::<f32>()
        + 10.0 * card_hs.len().saturating_sub(1) as f32;

    // The right column: today, done when, and the way on.
    let r_inner = right_w - 56.0;
    let crit = font(15.0, "sans", 0.0, 1.45);
    let note = font(12.0, "sans", 0.0, 1.4);
    let chain = file
        .as_ref()
        .map(|f| f.problem.today_chain.clone())
        .unwrap_or_default();
    let item_w = ((r_inner + 34.0) / chain.len().max(1) as f32 - 34.0).min(86.0);
    let strip_h = 36.0
        + 6.0
        + chain
            .iter()
            .map(|link| s.height(&link.label, MONO11, item_w + 14.0))
            .fold(0.0, f32::max);
    let today_h = if chain.is_empty() {
        0.0
    } else {
        56.0 + s.height("X", LABEL, 0.0) + 14.0 + strip_h
    };
    let notes = file
        .as_ref()
        .map(|f| f.problem.criteria_notes.clone())
        .unwrap_or_default();
    let jira_criteria = issue
        .as_ref()
        .map(|(issue, _)| issue.criteria.clone())
        .unwrap_or_default();
    // Jira's criteria, with the case's notes under them when they pair up;
    // the case's own when Jira has none.
    let (criteria, sub, crit_origin): (Vec<String>, Vec<String>, String) =
        if !jira_criteria.is_empty() {
            let sub = if notes.len() == jira_criteria.len() {
                notes.clone()
            } else {
                Vec::new()
            };
            (jira_criteria, sub, "from Jira".to_owned())
        } else if !notes.is_empty() {
            (
                notes.clone(),
                Vec::new(),
                format!(
                    "from case by Claude, {}",
                    file.as_ref().map(|f| f.written.as_str()).unwrap_or("")
                ),
            )
        } else {
            (Vec::new(), Vec::new(), "none in Jira".to_owned())
        };
    let crit_hs: Vec<f32> = criteria
        .iter()
        .enumerate()
        .map(|(i, c)| {
            s.height(c, crit, r_inner - 26.0)
                + sub
                    .get(i)
                    .map(|n| 4.0 + s.height(n, note, r_inner - 26.0))
                    .unwrap_or(0.0)
        })
        .collect();
    let done_h = 56.0
        + s.height("X", LABEL, 0.0)
        + 6.0
        + s.height("X", provenance, 0.0)
        + 12.0 * criteria.len().max(1) as f32
        + crit_hs.iter().sum::<f32>();
    let ready = case.ready();
    let fix_h = claude_fix(ui, s, case, [rx, top, right_w], now, false);
    let today_gap = if today_h > 0.0 { today_h + 24.0 } else { 0.0 };
    let cta_h = if ready { 72.0 + 24.0 } else { 0.0 };
    let right_h = fix_h + 24.0 + today_gap + done_h + cta_h;
    let row_h = left_h.max(right_h);

    // The left panel.
    let a = s.rising(case.entered, now, 0);
    a.plate(a.r(x, top, left_w, row_h), PANEL, LINE, 16.0);
    let lx = x + 32.0;
    let mut y = top + 32.0;
    y += a.pick(ui, lx, y, label, LABEL, INK_DIM, 0.0) + 6.0;
    y += a.pick(ui, lx, y, &origin, provenance, INK_DIM, origin_w);
    if let Some(line) = &jira_line {
        y += 6.0
            + a.pick_at(
                ui,
                (lx, y + 6.0, Align2::LEFT_TOP),
                line,
                (jira_font, INK_DIM, origin_w),
                issue_url.as_deref().map(|url| (4, url)),
            );
    }
    y += 20.0;
    if file.is_some() {
        let toggle = if case.raw { "THE CASE" } else { "RAW TICKET" };
        let tw = a.width(toggle, LABEL) + 28.0;
        let rect = a.r(x + left_w - 32.0 - tw, top + 26.0, tw, 34.0);
        let response = button(ui, rect, toggle);
        let edge = if response.hovered() {
            INK_DIM
        } else {
            LINE_STRONG
        };
        a.plate(rect, Color32::TRANSPARENT, edge, 6.0);
        a.put(
            x + left_w - 32.0 - tw / 2.0,
            top + 43.0,
            Align2::CENTER_CENTER,
            toggle,
            LABEL,
            INK,
        );
        if response.clicked() {
            case.raw = !case.raw;
        }
    }
    y += a.pick(ui, lx, y, &headline, h1, INK, inner) + 20.0;
    y += a.pick(
        ui,
        lx,
        y,
        &paragraph,
        body,
        if raw { INK_DIM } else { INK },
        inner,
    ) + 20.0;
    for ((id, t, place), h) in cards.iter().zip(&card_hs) {
        a.plate(a.r(lx, y, inner, *h), PANEL_RAISED, LINE, 0.0);
        a.pick(
            ui,
            lx + 16.0,
            y + 14.0,
            id,
            font(14.0, "mono-bold", 0.0, 1.35),
            if raw {
                INK_DIM
            } else if id.starts_with('Q') {
                WARN
            } else {
                DANGER_TEXT
            },
            0.0,
        );
        let tx = lx + 16.0 + 44.0 + 14.0;
        let to = (raw && id == "LINK")
            .then_some(linked_url.as_deref())
            .flatten()
            .map(|url| (t.len(), url));
        let th = a.pick_at(
            ui,
            (tx, y + 14.0, Align2::LEFT_TOP),
            t,
            (title, INK, card_text),
            to,
        );
        a.pick(
            ui,
            tx,
            y + 14.0 + th + 4.0,
            place,
            MONO12,
            INK_DIM,
            card_text,
        );
        y += h + 10.0;
    }

    // Claude's fix, then today as a strip of glyphs.
    claude_fix(ui, s, case, [rx, top, right_w], now, true);
    let b = s.rising(case.entered, now, 1);
    let ty0 = top + fix_h + 24.0;
    if !chain.is_empty() {
        let top = ty0;
        b.plate(b.r(rx, top, right_w, today_h), PANEL, LINE, 16.0);
        let mut y = top + 28.0;
        y += b.pick(
            ui,
            rx + 28.0,
            y,
            "TODAY · HOW THE APPS REACH THE DATABASE",
            LABEL,
            INK_DIM,
            0.0,
        ) + 14.0;
        let mut sx = rx + 28.0;
        for (index, link) in chain.iter().enumerate() {
            let (stroke, ink) = if link.danger {
                (DANGER, DANGER_TEXT)
            } else {
                (INFO, INK_DIM)
            };
            let centre = b.p(sx + item_w / 2.0, y + 18.0);
            glyph(
                &b.painter,
                glyph_of(link.kind),
                centre,
                36.0 / 28.0 * b.k,
                Stroke::new(1.7 * 36.0 / 28.0 * b.k, stroke),
            );
            let galley = b.galley(&link.label, MONO11, ink, item_w + 14.0);
            let mut job = (*galley.job).clone();
            job.halign = egui::Align::Center;
            let galley = b.painter.layout_job(job);
            b.painter
                .galley(b.p(sx + item_w / 2.0, y + 42.0), galley, ink);
            sx += item_w;
            if index + 1 < chain.len() {
                b.painter
                    .rect_filled(b.r(sx + 8.0, y + 23.0, 18.0, 2.0), 0.0, DANGER);
                sx += 34.0;
            }
        }
    }

    // Done when, and the way on.
    let c = s.rising(case.entered, now, 2);
    let dy0 = ty0 + today_gap;
    c.plate(c.r(rx, dy0, right_w, done_h), PANEL, LINE, 16.0);
    let mut y = dy0 + 28.0;
    y += c.pick(ui, rx + 28.0, y, "DONE WHEN", LABEL, INK_DIM, 0.0) + 6.0;
    y += c.pick(ui, rx + 28.0, y, &crit_origin, provenance, INK_DIM, 0.0) + 12.0;
    for (index, (criterion, h)) in criteria.iter().zip(&crit_hs).enumerate() {
        c.painter.rect_stroke(
            c.r(rx + 28.0, y + 3.0, 14.0, 14.0),
            0.0,
            Stroke::new(1.5, INK_DIM),
            StrokeKind::Inside,
        );
        let ch = c.pick(ui, rx + 54.0, y, criterion, crit, INK, r_inner - 26.0);
        if let Some(n) = sub.get(index) {
            c.pick(
                ui,
                rx + 54.0,
                y + ch + 4.0,
                n,
                note,
                INK_DIM,
                r_inner - 26.0,
            );
        }
        y += h + 12.0;
    }

    let cta_y = dy0 + done_h + 24.0;
    let cta = c.r(rx, cta_y, right_w, 72.0);
    let cy = cta_y + 36.0;
    let bottom = (top + row_h).max(dy0 + done_h + cta_h);
    if !ready {
        return bottom;
    }
    let response = button(ui, cta, "SEE THE PROPOSED FIX");
    let glow = if response.hovered() { 150 } else { 115 };
    c.painter.add(
        Shadow {
            offset: [0, 0],
            blur: (14.0 * c.k).round() as u8,
            spread: (2.0 * c.k).round() as u8,
            color: Color32::from_rgba_unmultiplied(252, 238, 10, glow),
        }
        .as_shape(cta, 0.0),
    );
    c.plate(cta, ATTENTION, Color32::TRANSPARENT, 8.0);
    let steps = file.as_ref().map(|f| f.steps.len()).unwrap_or(0);
    c.put(
        rx + 28.0,
        cy - 1.0,
        Align2::LEFT_BOTTOM,
        "SEE THE PROPOSED FIX",
        font(16.0, "bold", 0.14, 1.25),
        ON_ATTENTION,
    );
    c.put(
        rx + 28.0,
        cy + 1.0,
        Align2::LEFT_TOP,
        &format!("{steps} steps, placed on the map of the code base"),
        font(13.0, "sans", 0.0, 1.25),
        ON_ATTENTION,
    );
    c.put(
        rx + right_w - 28.0,
        cy,
        Align2::RIGHT_CENTER,
        "Enter",
        font(18.0, "mono", 0.0, 1.25),
        ON_ATTENTION,
    );
    if response.clicked() {
        case.go(2, now);
    }
    bottom
}

/// The right column's head on stage 1: Claude's fix in short, or, until
/// there is one, why not and the button that gets it. Answers its height;
/// draws only when `draw`.
fn claude_fix(
    ui: &Ui,
    s: &Sheet,
    case: &mut Case,
    [rx, top, w]: [f32; 3],
    now: f64,
    draw: bool,
) -> f32 {
    let inner = w - 56.0;
    let note = font(13.0, "sans", 0.0, 1.45);
    let big = font(20.0, "semibold", 0.0, 1.3);
    let approach_font = font(16.0, "sans", 0.0, 1.5);
    let step_font = font(14.0, "sans", 0.0, 1.4);
    let status = case.status();
    let file = case.loaded.as_ref().map(|l| l.case.clone());
    let (worktree, branch) = case.worktree().unwrap_or_default();
    let flavour = case.flavour();
    let who = match flavour {
        Flavour::Claude => "a Claude Code session with the ticket-plan skill",
        Flavour::Jarvis => "a Claude Code session running jarvis plan-story",
        Flavour::Factory => "a Claude Code session running factory:design",
    };
    let state = match status {
        None => "not ingested yet".to_owned(),
        Some(Status::Ingested) => "ingested · not planned yet".to_owned(),
        Some(Status::Planning) => format!("planning · {}", flavour.name().to_lowercase()),
        Some(Status::Planned) => "planned · waiting for your approval".to_owned(),
        Some(Status::Approved) => "approved · not fixed yet".to_owned(),
        Some(Status::Fixing) => format!("fixing on {branch}"),
        Some(Status::Fixed) => format!("fixed on {branch} · committed, not pushed"),
    };
    let planned = status.is_some_and(|s| s >= Status::Planned);
    // A panel's title, its body and the act it offers.
    type Locked<'a> = Option<(String, String, Option<(Act, &'a str)>)>;
    let locked: Locked = match (&case.broken, status) {
        (Some(error), _) => Some((
            "The case file does not read".to_owned(),
            error.clone(),
            None,
        )),
        (None, None) => Some((
            "Not ingested yet".to_owned(),
            format!(
                "INGEST starts a Claude Code session that reads the ticket and the repositories and writes the problem, in plain words, to {}. It plans nothing.",
                casefile::path(&case.key).display()
            ),
            Some((Act::Ingest, "a session reads the ticket and the code")),
        )),
        (None, Some(Status::Ingested)) => Some((
            "Not planned yet".to_owned(),
            format!(
                "PLAN starts {who}. It creates the worktree {worktree} on branch {branch} and plans the fix there. The steps and the map unlock when the plan is written."
            ),
            Some((Act::Plan, "worktree, branch, approach and steps")),
        )),
        (None, Some(Status::Planning)) => Some((
            "Planning…".to_owned(),
            format!(
                "In the worktree {worktree}, on branch {branch}. The plan shows here as soon as the session writes it."
            ),
            Some((Act::Plan, "")),
        )),
        _ => None,
    };
    let shown = case.investigating.clone();
    let steps: Vec<(String, String)> = file
        .as_ref()
        .map(|f| {
            f.steps
                .iter()
                .map(|st| (st.id.clone(), st.title.clone()))
                .collect()
        })
        .unwrap_or_default();
    let approach = file
        .as_ref()
        .map(|f| f.approach.clone())
        .unwrap_or_default();
    let step_w = inner - 44.0;

    let mut h = 28.0 + s.height("X", LABEL, 0.0) + 6.0 + s.height(&state, MONO11, inner) + 18.0;
    match (&locked, planned) {
        (Some((title, text, act)), _) => {
            h += s.height(title, big, inner) + 10.0 + s.height(text, note, inner) + 18.0;
            if act.is_some() {
                h += 72.0;
                if let Some(shown) = &shown {
                    h += 10.0 + s.height(shown, MONO11, inner);
                }
            }
        }
        (None, _) => {
            h += s.height(&approach, approach_font, inner) + 16.0;
            for (_, title) in &steps {
                h += s.height(title, step_font, step_w) + 8.0;
            }
        }
    }
    h += 10.0;
    if !draw {
        return h;
    }

    let a = s.rising(case.entered, now, 1);
    let edge = if planned { LINE_STRONG } else { LINE };
    a.plate(a.r(rx, top, w, h), PANEL, edge, 16.0);
    let lx = rx + 28.0;
    let mut y = top + 28.0;
    y += a.pick(ui, lx, y, "CLAUDE'S FIX", LABEL, ATTENTION, 0.0) + 6.0;
    y += a.pick(ui, lx, y, &state, MONO11, INK_DIM, inner) + 18.0;
    match locked {
        Some((title, text, act)) => {
            y += a.pick(ui, lx, y, &title, big, INK, inner) + 10.0;
            y += a.pick(ui, lx, y, &text, note, INK_DIM, inner) + 18.0;
            if let Some((act, sub)) = act {
                act_cta(ui, &a, case, act, [rx + 28.0, y, inner], sub, now);
                y += 72.0;
                if let Some(shown) = &shown {
                    a.pick(ui, lx, y + 10.0, shown, MONO11, INK_DIM, inner);
                }
            }
        }
        None => {
            y += a.pick(ui, lx, y, &approach, approach_font, INK, inner) + 16.0;
            let id_font = font(13.0, "mono-bold", 0.0, 1.4);
            for (id, title) in &steps {
                a.pick(ui, lx, y, id, id_font, ATTENTION, 0.0);
                y += a.pick(ui, lx + 44.0, y, title, step_font, INK, step_w) + 8.0;
            }
        }
    }
    h
}

/// The big button for a lifecycle act at sheet (x, y), `w` wide, 72 high:
/// lit when it is the next act, pulsing while its session runs, greyed
/// with the reason when it cannot run from here.
fn act_cta(
    ui: &Ui,
    c: &Sheet,
    case: &mut Case,
    act: Act,
    [x, y, w]: [f32; 3],
    sub: &str,
    now: f64,
) {
    let rect = c.r(x, y, w, 72.0);
    let cy = y + 36.0;
    let label = match act {
        Act::Ingest => "INGEST THE TICKET",
        Act::Plan => "PLAN THE FIX",
        Act::Approve => "APPROVE THE PLAN",
        Act::Fix => "FIX IT",
    };
    let head = font(16.0, "bold", 0.14, 1.25);
    let small = font(13.0, "sans", 0.0, 1.25);
    match case.standing(act) {
        Standing::Next => {
            let response = button(ui, rect, label);
            let glow = if response.hovered() { 150 } else { 115 };
            c.painter.add(
                Shadow {
                    offset: [0, 0],
                    blur: (14.0 * c.k).round() as u8,
                    spread: (2.0 * c.k).round() as u8,
                    color: Color32::from_rgba_unmultiplied(252, 238, 10, glow),
                }
                .as_shape(rect, 0.0),
            );
            c.plate(rect, ATTENTION, Color32::TRANSPARENT, 8.0);
            c.put(
                x + 24.0,
                cy - 1.0,
                Align2::LEFT_BOTTOM,
                label,
                head,
                ON_ATTENTION,
            );
            c.put(
                x + 24.0,
                cy + 1.0,
                Align2::LEFT_TOP,
                sub,
                small,
                ON_ATTENTION,
            );
            c.put(
                x + w - 24.0,
                cy,
                Align2::RIGHT_CENTER,
                case.flavour().name(),
                font(11.0, "mono-bold", 0.1, 1.25),
                ON_ATTENTION,
            );
            if response.clicked() {
                case.act(act, now);
            }
        }
        Standing::Running => {
            let pulse = 0.625 + 0.275 * (TAU * (now as f32) / 1.6).cos();
            c.plate(rect, PANEL, ATTENTION.gamma_multiply(pulse), 8.0);
            let running = match act {
                Act::Ingest => "INGESTING…",
                Act::Plan => "PLANNING…",
                Act::Approve => "APPROVING…",
                Act::Fix => "FIXING…",
            };
            c.put(
                x + 24.0,
                cy - 1.0,
                Align2::LEFT_BOTTOM,
                running,
                head,
                ATTENTION,
            );
            c.put(
                x + 24.0,
                cy + 1.0,
                Align2::LEFT_TOP,
                "a Claude Code session is on it",
                small,
                INK_DIM,
            );
        }
        Standing::Blocked(why) => {
            c.plate(rect, Color32::TRANSPARENT, LINE_STRONG, 8.0);
            c.put(
                x + 24.0,
                cy - 1.0,
                Align2::LEFT_BOTTOM,
                label,
                head,
                INK_DIM,
            );
            c.put(x + 24.0, cy + 1.0, Align2::LEFT_TOP, why, small, INK_DIM);
        }
        Standing::Done | Standing::Later => {}
    }
}

// ---------------------------------------------------------------- stage 2

fn fix(ui: &mut Ui, s: &Sheet, case: &mut Case, now: f64) {
    let Some(file) = case.loaded.as_ref().map(|l| l.case.clone()) else {
        return;
    };
    let a = s.rising(case.entered, now, 0);
    let (x, top, w, h) = (1140.0, 80.0, 436.0, 896.0);
    a.plate(a.r(x, top, w, h), PANEL, LINE, 0.0);
    let lx = x + 24.0;
    let inner = w - 48.0;
    let mut y = top + 24.0;
    y += a.text(lx, y, "2 · CLAUDE'S FIX", LABEL, ATTENTION, 0.0) + 6.0;
    let flavour = case.flavour().name().to_lowercase();
    let (_, branch) = case.worktree().unwrap_or_default();
    let state = match file.status {
        Status::Planned => format!("planned by {flavour} on {branch} · waiting for your approval"),
        Status::Approved => format!("approved · {branch}"),
        Status::Fixing => format!("fixing on {branch}"),
        Status::Fixed => format!("fixed on {branch} · committed, not pushed"),
        _ => branch.to_string(),
    };
    y += a.text(lx, y, &state, MONO11, INK_DIM, inner) + 14.0;
    y += a.text(
        lx,
        y,
        &file.approach,
        font(16.0, "sans", 0.0, 1.5),
        INK,
        inner,
    ) + 14.0;

    let title = font(15.0, "semibold", 0.0, 1.3);
    let summary = font(13.0, "sans", 0.0, 1.4);
    let text_w = inner - 32.0 - 40.0 - 12.0;
    let mut hovered = None;
    for (index, step) in file.steps.iter().enumerate() {
        let content = a.height(&step.title, title, text_w)
            + 4.0
            + a.height(&step.summary, summary, text_w)
            + 4.0
            + a.height(&step.place, MONO11, text_w);
        let bh = (28.0 + content).max(88.0);
        let rect = a.r(lx, y, inner, bh);
        let response = button(ui, rect, &step.id);
        if response.hovered() {
            hovered = Some(index);
        }
        let on = case.hover == Some(index);
        let (fill, edge, id_ink) = if on {
            (ATTENTION_SOFT, ATTENTION, ATTENTION)
        } else {
            (PANEL_RAISED, LINE, INK_DIM)
        };
        a.plate(rect, fill, edge, 8.0);
        a.text(
            lx + 16.0,
            y + 14.0,
            &step.id,
            font(15.0, "mono-bold", 0.0, 1.3),
            id_ink,
            0.0,
        );
        let tx = lx + 16.0 + 40.0 + 12.0;
        let mut ty = y + 14.0;
        ty += a.text(tx, ty, &step.title, title, INK, text_w) + 4.0;
        ty += a.text(tx, ty, &step.summary, summary, INK_DIM, text_w) + 4.0;
        a.text(tx, ty, &step.place, MONO11, INK_DIM, text_w);
        if response.clicked() {
            case.pick(index, now);
            return;
        }
        y += bh + 10.0;
    }
    case.hover = hovered;
    let hint = if file.status >= Status::Fixing {
        "Hover a step to see where it lands. Click it to see the diff the fix made."
    } else {
        "Hover a step to see where it lands. Click it to inspect the planned change."
    };
    // The gate at the foot: approve the plan, then fix it.
    let gate = match file.status {
        Status::Planned => Some(Act::Approve),
        Status::Approved | Status::Fixing => Some(Act::Fix),
        _ => None,
    };
    let mut bottom = top + h - 24.0;
    if let Some(act) = gate {
        bottom -= 72.0;
        act_cta(
            ui,
            &a,
            case,
            act,
            [lx, bottom, inner],
            match act {
                Act::Approve => "writes the approval into the case file",
                _ => "implements the plan, commits locally",
            },
            now,
        );
        if act == Act::Approve {
            bottom -= 44.0;
            let field = a.r(lx, bottom, inner, 34.0);
            ui.put(
                field,
                egui::TextEdit::singleline(&mut case.note)
                    .hint_text("note with the approval (optional)")
                    .font(FontId::new(13.0 * a.k, family("sans")))
                    .text_color(INK)
                    .background_color(BG_DEEP)
                    .margin(vec2(10.0, 8.0) * a.k),
            );
        } else if let Some(shown) = &case.investigating {
            let sh = a.height(shown, MONO11, inner);
            bottom -= sh + 10.0;
            a.text(lx, bottom, shown, MONO11, INK_DIM, inner);
        }
        bottom -= 14.0;
    }
    let hint_h = a.height(hint, MONO12, inner);
    a.text(lx, bottom - hint_h, hint, MONO12, INK_DIM, inner);
}

// ---------------------------------------------------------------- stage 3

fn evidence(ui: &Ui, s: &Sheet, case: &mut Case, now: f64) {
    let Some(steps) = case.loaded.as_ref().map(|l| l.case.steps.clone()) else {
        return;
    };
    let step = &steps[case.step];

    // The step, and what it changes.
    let a = s.rising(case.entered, now, 0);
    let (x, w) = (24.0, 330.0);
    let mut y = 80.0;
    y += a.text(x, y, "3 · THE EVIDENCE", LABEL, INK_DIM, 0.0) + 16.0;
    let n = steps.len() as f32;
    let tab_w = (w - 6.0 * (n - 1.0)) / n;
    for (index, other) in steps.iter().enumerate() {
        let rect = a.r(x + index as f32 * (tab_w + 6.0), y, tab_w, 44.0);
        let response = button(ui, rect, &other.id);
        let on = index == case.step;
        let (fill, edge, ink) = if on {
            (ATTENTION_SOFT, ATTENTION, ATTENTION)
        } else if response.hovered() {
            (PANEL_RAISED, LINE_STRONG, INK)
        } else {
            (PANEL_RAISED, LINE, INK_DIM)
        };
        a.plate(rect, fill, edge, 8.0);
        a.painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            &other.id,
            FontId::new(14.0 * a.k, family("mono-bold")),
            ink,
        );
        if response.clicked() && !on {
            case.step = index;
            case.launched = None;
        }
    }
    y += 44.0 + 16.0;
    let title = font(20.0, "bold", 0.0, 1.2);
    let summary = font(14.0, "sans", 0.0, 1.5);
    let card_h = 40.0
        + s.height(&step.title, title, w - 40.0)
        + 8.0
        + s.height(&step.summary, summary, w - 40.0);
    a.plate(a.r(x, y, w, card_h), PANEL, LINE, 0.0);
    let th = a.text(x + 20.0, y + 20.0, &step.title, title, INK, w - 40.0);
    a.text(
        x + 20.0,
        y + 20.0 + th + 8.0,
        &step.summary,
        summary,
        INK_DIM,
        w - 40.0,
    );
    y += card_h + 16.0;
    y += a.text(x, y, "WHAT CHANGES", LABEL, INK_DIM, 0.0) + 16.0;
    let what = font(13.0, "semibold", 0.0, 1.3);
    let inner = w - 32.0;
    for change in &step.changes {
        let name = &change.what;
        let before = format!("- {}", change.before);
        let after = format!("+ {}", change.after);
        let h = 28.0
            + s.height(name, what, inner)
            + 8.0
            + s.height(&before, MONO12, inner)
            + 8.0
            + s.height(&after, MONO12, inner);
        a.plate(a.r(x, y, w, h), PANEL, LINE, 0.0);
        let mut ty = y + 14.0;
        ty += a.text(x + 16.0, ty, name, what, INK, inner) + 8.0;
        ty += a.text(x + 16.0, ty, &before, MONO12, DANGER_TEXT, inner) + 8.0;
        a.text(x + 16.0, ty, &after, MONO12, OK, inner);
        y += h + 16.0;
    }

    code(ui, &s.rising(case.entered, now, 1), case, now);

    // Risks and open questions.
    let c = s.rising(case.entered, now, 2);
    let (x, w) = (1216.0, 360.0);
    let mut y = 330.0;
    y += c.text(x, y, "RISKS AND OPEN QUESTIONS", LABEL, INK_DIM, 0.0) + 12.0;
    let body = font(13.0, "sans", 0.0, 1.45);
    for risk in &step.risks {
        let (tag, text) = (&risk.tag, &risk.text);
        let tag_font = font(11.0, "mono-bold", 0.0, 1.3);
        let h = 28.0 + c.height(tag, tag_font, 0.0) + 6.0 + c.height(text, body, w - 32.0);
        let (ink, edge) = match risk.level {
            Level::Danger => (DANGER_TEXT, DANGER),
            Level::Warn => (WARN, LINE),
            Level::Info => (INFO, LINE),
        };
        c.plate(c.r(x, y, w, h), PANEL, edge, 0.0);
        let th = c.text(x + 16.0, y + 14.0, tag, tag_font, ink, 0.0);
        c.text(x + 16.0, y + 14.0 + th + 6.0, text, body, INK, w - 32.0);
        y += h + 12.0;
    }
}

fn code(ui: &Ui, s: &Sheet, case: &mut Case, now: f64) {
    let Some(loaded) = case.loaded.as_ref() else {
        return;
    };
    let (crumb, file_name) = casefile::heading(&loaded.case.steps[case.step]);
    // The real diff once the fix is under way; the planned excerpt before.
    let crumb = match &loaded.diffs[case.step] {
        Some((_, range)) => format!("git diff {range} · {crumb}"),
        None => format!("planned · {crumb}"),
    };
    let opens = loaded.opens[case.step].clone();
    let code_lines = loaded.lines[case.step].clone();
    let (x, top, w, h) = (378.0, 80.0, 814.0, 896.0);
    s.plate(s.r(x, top, w, h), PANEL, LINE_STRONG, 0.0);

    // The header: where the file lives, and the ways to open it.
    let label = font(12.0, "bold", 0.1, 1.25);
    let nvim_w = s.width("OPEN IN NVIM", label) + 32.0;
    let editor_w = s.width("OPEN IN EDITOR", font(12.0, "semibold", 0.1, 1.25)) + 32.0;
    let crumb_w = w - 40.0 - nvim_w - editor_w - 32.0;
    let file = font(15.0, "mono-bold", 0.0, 1.3);
    let text_h = s.height(&crumb, MONO12, crumb_w) + 4.0 + s.height(&file_name, file, crumb_w);
    let head_h = text_h.max(44.0) + 32.0;
    let mid = top + head_h / 2.0;
    let ch = s.text(
        x + 20.0,
        mid - text_h / 2.0,
        &crumb,
        MONO12,
        INK_DIM,
        crumb_w,
    );
    s.text(
        x + 20.0,
        mid - text_h / 2.0 + ch + 4.0,
        &file_name,
        file,
        INK,
        crumb_w,
    );
    s.rule(x, x + w, top + head_h - 0.5, LINE);

    let ex = x + w - 20.0 - editor_w;
    let nx = ex - 16.0 - nvim_w;
    let nvim = s.r(nx, mid - 22.0, nvim_w, 44.0);
    let editor = s.r(ex, mid - 22.0, editor_w, 44.0);
    let can_open = opens.is_some();
    let mut dim = s.clone();
    if !can_open {
        dim.painter.multiply_opacity(0.35);
    }
    let nvim_response = button(ui, nvim, "OPEN IN NVIM");
    let editor_response = button(ui, editor, "OPEN IN EDITOR");
    dim.plate(nvim, ATTENTION, Color32::TRANSPARENT, 8.0);
    dim.put(
        nx + nvim_w / 2.0,
        mid,
        Align2::CENTER_CENTER,
        "OPEN IN NVIM",
        label,
        ON_ATTENTION,
    );
    let edge = if editor_response.hovered() && can_open {
        INK_DIM
    } else {
        LINE_STRONG
    };
    dim.plate(editor, Color32::TRANSPARENT, edge, 8.0);
    dim.put(
        ex + editor_w / 2.0,
        mid,
        Align2::CENTER_CENTER,
        "OPEN IN EDITOR",
        font(12.0, "semibold", 0.1, 1.25),
        INK,
    );
    match &opens {
        Some((path, line)) => {
            if nvim_response.clicked() {
                case.launched = Some((launch(true, path, *line), now));
            }
            if editor_response.clicked() {
                case.launched = Some((launch(false, path, *line), now));
            }
        }
        None => {
            nvim_response.on_hover_text("stub, nothing to open");
            editor_response.on_hover_text("stub, nothing to open");
        }
    }

    // The lines.
    let toast_h = if case.launched.is_some() { 40.0 } else { 0.0 };
    let body = s.r(x + 1.0, top + head_h, w - 2.0, h - head_h - toast_h - 1.0);
    let mut lines = s.clone();
    lines.painter = s
        .painter
        .with_clip_rect(body.intersect(s.painter.clip_rect()));
    let mono = font(13.0, "mono", 0.0, 1.25);
    let mut y = top + head_h + 12.0;
    for (number, text, mark) in &code_lines {
        let (fill, sign, sign_ink, ink) = match mark {
            Mark::Ctx | Mark::Gap => (Color32::TRANSPARENT, "", INK_DIM, INK_DIM),
            Mark::File => (PANEL_RAISED, "", INK_DIM, INK),
            Mark::Hit => (ATTENTION_SOFT, ">", ATTENTION, INK),
            Mark::Del => (DANGER_FILL, "-", DANGER, DANGER_TEXT),
            Mark::Add => (ADD_FILL, "+", OK, OK),
        };
        lines
            .painter
            .rect_filled(lines.r(x + 1.0, y, w - 2.0, 22.0), 0.0, fill);
        if *number > 0 {
            lines.put(
                x + 46.0,
                y + 11.0,
                Align2::RIGHT_CENTER,
                &number.to_string(),
                mono,
                INK_DIM,
            );
        }
        lines.put(
            x + 56.0,
            y + 11.0,
            Align2::LEFT_CENTER,
            sign,
            mono,
            sign_ink,
        );
        lines.put(x + 76.0, y + 11.0, Align2::LEFT_CENTER, text, mono, ink);
        y += 22.0;
    }

    // What was launched, rising in.
    if let Some((command, at)) = &case.launched {
        let t = ease(((now - at) / 0.3) as f32);
        let mut toast = s.clone();
        toast.painter.multiply_opacity(t);
        toast.dy += 8.0 * (1.0 - t);
        let ty = top + h - toast_h;
        toast.painter.rect_filled(
            toast.r(x + 1.0, ty, w - 2.0, toast_h - 1.0),
            0.0,
            PANEL_RAISED,
        );
        toast.rule(x, x + w, ty + 0.5, LINE);
        let galley = toast.galley(&format!("Launched: {command}"), MONO12, FLOW, 0.0);
        let mut job = (*galley.job).clone();
        job.wrap.max_width = (w - 40.0) * toast.k;
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = toast.painter.layout_job(job);
        toast
            .painter
            .galley(toast.p(x + 20.0, ty + 12.0), galley, FLOW);
    }
}

// ---------------------------------------------------------------- opening

/// Opens `path` at `line` in Neovim, in a new Terminal window, or in the
/// editor, and says what ran. With `CELLWORKS_DRY_LAUNCH` set nothing runs,
/// so the screenshot harness can press the buttons.
fn launch(nvim: bool, path: &Path, line: usize) -> String {
    let file = path.display().to_string();
    let (shown, command) = if nvim {
        let shell = format!("nvim +{line} '{file}'");
        let mut command = Command::new("osascript");
        command.args([
            "-e",
            &format!("tell application \"Terminal\" to do script \"{shell}\""),
            "-e",
            "tell application \"Terminal\" to activate",
        ]);
        (format!("nvim +{line} {file}"), command)
    } else {
        editor(&file, line)
    };
    run(shown, command)
}

/// Runs `command` in the background and says what ran, as `shown`. With
/// `CELLWORKS_DRY_LAUNCH` set nothing runs.
fn run(shown: String, mut command: Command) -> String {
    if std::env::var_os("CELLWORKS_DRY_LAUNCH").is_some() {
        return format!("{shown}   (dry run)");
    }
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
            shown
        }
        Err(error) => format!("{shown}   failed: {error}"),
    }
}

/// How a repository wants its tickets planned and fixed.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Flavour {
    /// The Factory pipeline plans and builds (`.github/workflows/sdlc.yml`).
    Factory,
    /// The jarvis plugins plan and implement stories.
    Jarvis,
    /// Plain Claude Code, with the ticket skills in this source tree.
    Claude,
}

impl Flavour {
    pub fn of(root: &Path) -> Flavour {
        if root.join(".github/workflows/sdlc.yml").is_file() {
            return Flavour::Factory;
        }
        let settings =
            std::fs::read_to_string(root.join(".claude/settings.json")).unwrap_or_default();
        if settings.contains("jarvis") || root.join(".claude/work-tracking.conf").is_file() {
            return Flavour::Jarvis;
        }
        Flavour::Claude
    }

    pub fn name(self) -> &'static str {
        match self {
            Flavour::Factory => "FACTORY",
            Flavour::Jarvis => "JARVIS",
            Flavour::Claude => "CLAUDE",
        }
    }
}

/// One step of the case's life, as the rail and the buttons name it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    Ingest,
    Plan,
    Approve,
    Fix,
}

impl Act {
    const ALL: [Act; 4] = [Act::Ingest, Act::Plan, Act::Approve, Act::Fix];

    fn name(self) -> &'static str {
        match self {
            Act::Ingest => "INGEST",
            Act::Plan => "PLAN",
            Act::Approve => "APPROVE",
            Act::Fix => "FIX",
        }
    }

    /// The status the act leads to, once done.
    fn done(self) -> Status {
        match self {
            Act::Ingest => Status::Ingested,
            Act::Plan => Status::Planned,
            Act::Approve => Status::Approved,
            Act::Fix => Status::Fixed,
        }
    }
}

/// Where a lifecycle step stands for the case.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Standing {
    Done,
    Running,
    Next,
    Blocked(&'static str),
    Later,
}

impl Case {
    /// The case's status; None before ingest wrote a case file.
    fn status(&self) -> Option<Status> {
        self.loaded.as_ref().map(|l| l.case.status)
    }

    /// The repository the fix is planned and made in: the first one.
    fn main_repo(&self) -> Option<casefile::Repo> {
        self.loaded.as_ref()?.case.repos.first().cloned()
    }

    pub fn flavour(&self) -> Flavour {
        self.main_repo()
            .map(|repo| Flavour::of(&expand(&repo.root)))
            .unwrap_or(Flavour::Claude)
    }

    fn standing(&self, act: Act) -> Standing {
        let status = self.status();
        let running = match act {
            Act::Ingest => self.launched_act == Some(Act::Ingest) && status.is_none(),
            Act::Plan => status == Some(Status::Planning),
            Act::Approve => false,
            Act::Fix => status == Some(Status::Fixing),
        };
        if running {
            return Standing::Running;
        }
        if status.is_some_and(|s| s >= act.done()) {
            return Standing::Done;
        }
        let next = match status {
            None => Act::Ingest,
            Some(Status::Ingested | Status::Planning) => Act::Plan,
            Some(Status::Planned) => Act::Approve,
            Some(_) => Act::Fix,
        };
        if act != next {
            return Standing::Later;
        }
        if act == Act::Fix && self.flavour() == Flavour::Factory {
            return Standing::Blocked("runs in the Factory pipeline");
        }
        Standing::Next
    }

    /// Does `act`: launches its session, or for APPROVE writes the case
    /// file. Only ever on the user's click.
    fn act(&mut self, act: Act, now: f64) {
        if self.standing(act) != Standing::Next {
            return;
        }
        self.launched_act = Some(act);
        let shown = match act {
            Act::Ingest => ingest(&self.key, self.workspace.as_deref()),
            Act::Plan => self.plan(),
            Act::Approve => {
                casefile::advance(&self.key, Status::Approved, self.note.trim(), |_| {})
                    .map(|()| "approved; nothing ran".to_owned())
                    .unwrap_or_else(|error| format!("could not approve: {error}"))
            }
            Act::Fix => self.fix(),
        };
        self.investigating = Some(shown);
        self.checked = now - 2.0;
        self.refresh(now);
    }

    /// The worktree and branch the fix is planned and made on: beside the
    /// repository, named from the key.
    fn worktree(&self) -> Option<(String, String)> {
        let repo = self.main_repo()?;
        if let (Some(worktree), Some(branch)) = (&repo.worktree, &repo.branch) {
            return Some((worktree.clone(), branch.clone()));
        }
        let key = self.key.to_lowercase();
        let words: Vec<String> = self
            .loaded
            .as_ref()?
            .case
            .problem
            .headline
            .to_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| w.len() > 2)
            .take(4)
            .map(str::to_owned)
            .collect();
        let slug = if words.is_empty() {
            String::new()
        } else {
            format!("-{}", words.join("-"))
        };
        Some((
            format!("{}-{key}", repo.root.trim_end_matches('/')),
            format!("fix/{key}{slug}"),
        ))
    }

    fn plan(&mut self) -> String {
        let (Some(repo), Some((worktree, branch))) = (self.main_repo(), self.worktree()) else {
            return "no repository in the case file to plan in".to_owned();
        };
        let base = repo
            .base_branch
            .clone()
            .or_else(|| current_branch(&expand(&repo.root)))
            .unwrap_or_else(|| "main".to_owned());
        let key = self.key.clone();
        let file = casefile::path(&key).display().to_string();
        let case_doc = skill("ticket-case");
        let flavour = self.flavour();
        let prompt = match flavour {
            Flavour::Claude => format!(
                "Read {} and follow it to plan {key}. Create the worktree {worktree} on a new branch {branch} from {base} and plan there. Write the plan into the case file {file} as {case_doc} says.",
                skill("ticket-plan")
            ),
            Flavour::Jarvis => format!(
                "/jarvis-execute:plan-story {key} -- plan only, in the worktree {worktree} on branch {branch} from {base}. Then write the approach and the steps into the case file {file} as {case_doc} says, and set its status to planned."
            ),
            Flavour::Factory => format!(
                "/factory:design {key} -- then write the approach and the steps into the case file {file} as {case_doc} says, with the worktree {worktree} on branch {branch} from {base}, and set its status to planned."
            ),
        };
        let written = casefile::advance(&key, Status::Planning, "", |case| {
            if let Some(main) = case.repos.first_mut() {
                main.worktree = Some(worktree.clone());
                main.branch = Some(branch.clone());
                main.base_branch = Some(base.clone());
            }
        });
        if let Err(error) = written {
            return format!("could not write the case file: {error}");
        }
        session(&expand(&repo.root), &prompt)
    }

    fn fix(&mut self) -> String {
        let Some((worktree, branch)) = self.worktree() else {
            return "no worktree in the case file to fix in".to_owned();
        };
        let key = self.key.clone();
        let file = casefile::path(&key).display().to_string();
        let case_doc = skill("ticket-case");
        let prompt = match self.flavour() {
            Flavour::Jarvis => format!(
                "/jarvis-execute:implement-story {key} -- on branch {branch}, commit locally and never push. Then set the case file {file} to fixed as {case_doc} says."
            ),
            _ => format!(
                "Read {} and follow it to fix {key} on branch {branch}, as the approved plan in the case file {file} says. Commit locally, never push. Update the case file as {case_doc} says.",
                skill("ticket-fix")
            ),
        };
        if let Err(error) = casefile::advance(&key, Status::Fixing, "", |_| {}) {
            return format!("could not write the case file: {error}");
        }
        session(&expand(&worktree), &prompt)
    }
}

/// The branch checked out in `root`.
fn current_branch(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!name.is_empty() && name != "HEAD").then_some(name)
}

/// Starts a Claude Code session that ingests `key` into a new case file,
/// in `workspace` (else the home folder). Says what ran.
fn ingest(key: &str, workspace: Option<&Path>) -> String {
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return format!("not an issue key: {key}");
    }
    let dir = workspace
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()));
    let file = casefile::path(key).display().to_string();
    let prompt = format!(
        "Read {} and follow it to ingest {key}. Write the case file to {file} as {} says.",
        skill("ticket-ingest"),
        skill("ticket-case")
    );
    session(&dir, &prompt)
}

/// Starts `claude <prompt>` in `dir`, in a new Terminal window. The session
/// is told where the skills are, so nothing has to be installed into
/// Claude's own folders. Says what ran.
fn session(dir: &Path, prompt: &str) -> String {
    let dir = dir.display().to_string();
    // Quoted once for the shell, then escaped for the AppleScript string.
    let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
    let shell = format!("cd {} && claude {}", quote(&dir), quote(prompt));
    let script = shell.replace('\\', "\\\\").replace('"', "\\\"");
    let mut command = Command::new("osascript");
    command.args([
        "-e",
        &format!("tell application \"Terminal\" to do script \"{script}\""),
        "-e",
        "tell application \"Terminal\" to activate",
    ]);
    run(format!("claude \"{prompt}\"   in {dir}"), command)
}

/// A ticket skill's SKILL.md: under `CELLWORKS_SKILLS`, else in this
/// source tree.
fn skill(name: &str) -> String {
    let root = std::env::var_os("CELLWORKS_SKILLS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.claude/skills"));
    let path = root.join(name).join("SKILL.md");
    path.canonicalize().unwrap_or(path).display().to_string()
}

/// `$VISUAL` or `$EDITOR` when it names an editor with a window of its own;
/// a terminal editor cannot open one from here, so the system's default
/// text editor opens the file instead.
fn editor(file: &str, line: usize) -> (String, Command) {
    let named = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty());
    if let Some(value) = named {
        let mut words = value.split_whitespace();
        let program = words.next().unwrap_or_default().to_owned();
        let base = Path::new(&program)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let args = match base {
            "code" | "cursor" | "codium" => Some(vec!["-g".to_owned(), format!("{file}:{line}")]),
            "subl" | "zed" => Some(vec![format!("{file}:{line}")]),
            "mate" | "bbedit" | "gvim" | "mvim" | "idea" => Some(vec![file.to_owned()]),
            _ => None,
        };
        if let Some(args) = args {
            let mut command = Command::new(&program);
            command.args(words).args(&args);
            return (format!("{value} {}", args.join(" ")), command);
        }
    }
    let mut command = Command::new("open");
    command.args(["-t", file]);
    (
        format!("open -t {file}   ($EDITOR is a terminal editor; line {line})"),
        command,
    )
}
