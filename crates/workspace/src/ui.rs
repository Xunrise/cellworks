//! The launcher view: the header, the centred card (search row, location chip
//! row and the project list) and the preview pane of STORY-082's keyboard-
//! first palette, which replaces STORY-078's three-column layout.
//!
//! The only module of this crate that depends on egui, and the only one that
//! holds a word the engineer reads. Everything that stays within the
//! launcher — the search text, the picked location, the selection, arrow-key
//! movement — is applied directly to the [`Launcher`] passed in; only opening
//! a project or changing what Navigation tracks reaches beyond this view, as
//! a [`LauncherAction`]. Tab (scope cycling) and Esc (clear search) are routed
//! in `src/app.rs`'s key block, next to Enter and the arrows (STORY-082
//! Decision 5); this module only draws and reports clicks.

use crate::Sources;
use crate::launcher::{HostState, Launcher, ProjectKind, Sort, match_range};
use crate::project::{Project, ProjectId};
use crate::universe::{Notice, NoticeKind, ScanState, Universe};
use crate::words::Words;

// STORY-083: the Cellworks Netrunner tokens (design requirement 1). Only
// attention yellow glows; every other state colour stays paired with a glyph
// or a word, never carrying meaning alone (`state_glyph` already does this
// and keeps doing it below).
/// The background behind the card: near-black.
const BG: egui::Color32 = egui::Color32::from_rgb(0x07, 0x09, 0x0c);
/// The background's faint 100 px grid.
const GRID_LINE: egui::Color32 = egui::Color32::from_rgb(0x0e, 0x14, 0x1b);
/// A panel's own surface: the card, the header, the footer, the preview pane.
const PANEL: egui::Color32 = egui::Color32::from_rgb(0x0d, 0x11, 0x17);
/// A deep well sunk into a panel: the search field.
const WELL: egui::Color32 = egui::Color32::from_rgb(0x04, 0x05, 0x07);
/// A quiet dividing line.
const LINE: egui::Color32 = egui::Color32::from_rgb(0x1e, 0x27, 0x31);
/// A brighter line: a focused edge.
const LINE_BRIGHT: egui::Color32 = egui::Color32::from_rgb(0x34, 0x50, 0x6a);
/// Primary text.
const INK: egui::Color32 = egui::Color32::from_rgb(0xd7, 0xe2, 0xea);
/// Secondary text.
const DIM_INK: egui::Color32 = egui::Color32::from_rgb(0x7d, 0x8f, 0xa0);
/// The only colour that glows (requirement 1).
const YELLOW: egui::Color32 = egui::Color32::from_rgb(0xfc, 0xee, 0x0a);
/// Yellow's soft fill, behind a picked chip or a selected row.
const YELLOW_SOFT: egui::Color32 = egui::Color32::from_rgb(0x3a, 0x36, 0x08);
/// Focus and the mode badge.
const CYAN: egui::Color32 = egui::Color32::from_rgb(0x5e, 0xf6, 0xff);
/// A connected host's glyph.
const OK_GREEN: egui::Color32 = egui::Color32::from_rgb(0x3d, 0xdc, 0x84);
/// The ICE/offline card and a notice.
const WARN_ORANGE: egui::Color32 = egui::Color32::from_rgb(0xff, 0x9f, 0x1c);

/// Which pair of opposite corners [`chamfer`] cuts (design requirement 3).
pub(crate) enum Corners {
    TopRightBottomLeft,
    TopLeftBottomRight,
}

/// `rect`'s outline as a chamfered hexagon, going clockwise from `rect`'s
/// top-left: the named pair of opposite corners is each replaced by two
/// points `cut` away along its two adjacent edges; the other pair is kept
/// exactly (design requirement 3: the card is cut top-right/bottom-left, the
/// CTA and the ICE card top-left/bottom-right, chips either pair).
pub(crate) fn chamfer(rect: egui::Rect, cut: f32, corners: Corners) -> [egui::Pos2; 6] {
    match corners {
        Corners::TopRightBottomLeft => [
            rect.left_top(),
            egui::pos2(rect.right() - cut, rect.top()),
            egui::pos2(rect.right(), rect.top() + cut),
            rect.right_bottom(),
            egui::pos2(rect.left() + cut, rect.bottom()),
            egui::pos2(rect.left(), rect.bottom() - cut),
        ],
        Corners::TopLeftBottomRight => [
            egui::pos2(rect.left(), rect.top() + cut),
            egui::pos2(rect.left() + cut, rect.top()),
            rect.right_top(),
            egui::pos2(rect.right(), rect.bottom() - cut),
            egui::pos2(rect.right() - cut, rect.bottom()),
            rect.left_bottom(),
        ],
    }
}

/// How long the card, the list and the preview pane take to rise in, once
/// their own delay has passed (design requirement 5: "550 ms").
const RISE_DURATION: f32 = 0.550;

/// How far below its settled position the card, the list or the preview pane
/// starts (design requirement 5: "18 px").
const RISE_START_OFFSET: f32 = 18.0;

/// How long the enabled CTA's glow takes to breathe through one full cycle
/// (design requirement 5: "1.6 s").
const BREATH_PERIOD: f32 = 1.6;

/// `elapsed` seconds after the view was first shown, offset by `delay`, as
/// `(y_offset_px, opacity)`: `(18.0, 0.0)` at or before the delay, eased to
/// `(0.0, 1.0)` by 550 ms after it (design requirement 5), or always settled
/// when `reduced` is true (Decision 7). The ease-out approximates
/// `cubic-bezier(.2,.8,.2,1)` with `1.0 - (1.0 - t).powi(3)` on the
/// delay-normalised progress `t`, applied the same way to the offset and the
/// opacity so both settle together.
pub(crate) fn rise(elapsed: f32, delay: f32, reduced: bool) -> (f32, f32) {
    if reduced {
        return (0.0, 1.0);
    }
    let t = ((elapsed - delay) / RISE_DURATION).clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t).powi(3);
    (RISE_START_OFFSET * (1.0 - eased), eased)
}

/// The enabled CTA's glow-alpha multiplier `elapsed` seconds after the view
/// was first shown: a sine wave over [`BREATH_PERIOD`], oscillating smoothly
/// and never negative (design requirement 5: "the enabled CTA glow breathes
/// over 1.6 s"), or constant when `reduced` is true (Decision 7).
pub(crate) fn breath(elapsed: f32, reduced: bool) -> f32 {
    if reduced {
        return 1.0;
    }
    let phase = (elapsed / BREATH_PERIOD) * std::f32::consts::TAU;
    0.65 + 0.35 * phase.sin()
}

/// How long since `id` was first drawn in this `ui`, keeping the first time
/// seen in egui's own temp memory (Decision 7) rather than a field on
/// [`Launcher`] — the view needs no state of its own to animate.
fn first_shown(ui: &egui::Ui, id: egui::Id, now: f32) -> f32 {
    now - ui
        .ctx()
        .data_mut(|data| *data.get_temp_mut_or_insert_with(id, || now))
}

/// The `egui::Id` [`first_shown`] is read with for the whole launcher view:
/// one shared clock for the card's, the list's and the preview pane's rise.
const LAUNCHER_SHOWN_ID: &str = "cellworks-launcher-shown";

/// Installs Chakra Petch and JetBrains Mono as named families (design
/// requirement 2, Decision 5), *adding* them beside egui's own
/// `Proportional`/`Monospace` families rather than replacing either — so a
/// label that never asks for either named family keeps looking exactly as it
/// did. Not called from this module: `src/app.rs` calls it once at start-up,
/// after `set_theme` (Order of work step 3).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "chakra-petch".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/chakra-petch/ChakraPetch-SemiBold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains-mono".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"
        ))),
    );
    fonts.families.insert(
        egui::FontFamily::Name("chakra-petch".into()),
        vec!["chakra-petch".to_owned()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("jetbrains-mono".into()),
        vec!["jetbrains-mono".to_owned()],
    );
    ctx.set_fonts(fonts);
}

/// Chakra Petch, for a label — or egui's own `Proportional` face when the
/// family is not bound (requirement 2's "If a family is not bound, the view
/// uses egui's default faces"): every acceptance test builds a bare
/// `egui::Context` that never calls [`install_fonts`], so this fallback is
/// what keeps every one of them from panicking on a missing family.
fn chakra_petch(ui: &egui::Ui) -> egui::FontFamily {
    let family = egui::FontFamily::Name("chakra-petch".into());
    if ui.fonts(|fonts| fonts.families().contains(&family)) {
        family
    } else {
        egui::FontFamily::Proportional
    }
}

/// JetBrains Mono, for a path, a branch, a key or a count — or egui's own
/// `Monospace` face when the family is not bound, for the same reason as
/// [`chakra_petch`].
fn jetbrains_mono(ui: &egui::Ui) -> egui::FontFamily {
    let family = egui::FontFamily::Name("jetbrains-mono".into());
    if ui.fonts(|fonts| fonts.families().contains(&family)) {
        family
    } else {
        egui::FontFamily::Monospace
    }
}

/// Shown from the first frame until the first result arrives, so a slow walk
/// never flashes an empty launcher on the way.
const SCANNING: &str = "Scanning for projects…";

/// Shown when the scan ended without reporting, rather than leaving
/// "Scanning" on the screen for ever.
const FAILED: &str = "Could not finish scanning for projects.";

/// The card's footer hints (STORY-082 requirement 9), as one accessible
/// string; the footer draws each key in its own key box.
const HINTS_NAV: &str = "↑ ↓ j k move   Enter open   Tab scope   ⇧Tab sort   : search";

/// The same footer while the search field has focus.
const HINTS_CMD: &str = "↑ ↓ move   Enter open   Tab scope   ⇧Tab sort   Esc nav";

// STORY-083: the launcher's metrics, in points, taken from the approved mock.
/// The header bar's height.
const HEADER_HEIGHT: f32 = 48.0;
/// The header's left and right padding.
const HEADER_PADDING: f32 = 24.0;
/// The status bar's height, along the window's bottom edge.
const STATUS_HEIGHT: f32 = 28.0;
/// The gap between the header and the card.
const CARD_TOP_GAP: f32 = 40.0;
/// The centred card's width (STORY-082 requirement 1): wide enough for the
/// mockup's layout, narrow enough to stay one card on a wider window.
const CARD_WIDTH: f32 = 920.0;
/// The gap between the card and each notice below it.
const NOTICE_GAP: f32 = 12.0;
/// One notice's height below the card.
const NOTICE_HEIGHT: f32 = 38.0;
/// The search well's margin inside the card.
const SEARCH_MARGIN: f32 = 12.0;
/// The search well's height.
const SEARCH_HEIGHT: f32 = 56.0;
/// The search well's inner left and right padding.
const SEARCH_PADDING: f32 = 14.0;
/// The chip row's padding around its chips.
const CHIP_ROW_PADDING: f32 = 12.0;
/// One chip's height.
const CHIP_HEIGHT: f32 = 30.0;
/// One chip's left and right padding.
const CHIP_PADDING: f32 = 12.0;
/// The gap between two chips, and between the parts of one chip.
const CHIP_GAP: f32 = 8.0;
/// The body's height: the list beside the preview pane.
const BODY_HEIGHT: f32 = 446.0;
/// The smallest body a short window shrinks it to: one row, which the list
/// scrolls through.
const BODY_MIN_HEIGHT: f32 = LIST_TOP_PADDING + ROW_HEIGHT;
/// The preview pane's width, to the right of the list (STORY-082
/// requirement 7).
const PREVIEW_WIDTH: f32 = 300.0;
/// The preview pane's padding.
const PREVIEW_PADDING: f32 = 20.0;
/// The vertical gap between the preview pane's blocks.
const PREVIEW_GAP: f32 = 16.0;
/// The width of the preview pane's PATH/BRANCH/FILES term column.
const TERM_COLUMN: f32 = 64.0;
/// The gap between two PATH/BRANCH/FILES rows.
const TERM_ROW_GAP: f32 = 10.0;
/// The list's padding above its first row.
const LIST_TOP_PADDING: f32 = 8.0;
/// One project row's height.
const ROW_HEIGHT: f32 = 34.0;
/// A row's and a group heading's left and right padding.
const ROW_PADDING: f32 = 16.0;
/// The name column's width.
const NAME_COLUMN: f32 = 190.0;
/// The branch column's width, right-aligned.
const BRANCH_COLUMN: f32 = 140.0;
/// The gap between two columns of a row.
const COLUMN_GAP: f32 = 12.0;
/// The space above a group heading's text, and below it.
const HEADING_SPACE: (f32, f32) = (12.0, 6.0);
/// The in-card footer's height.
const FOOT_HEIGHT: f32 = 36.0;
/// The CTA's height (requirement 4).
const CTA_HEIGHT: f32 = 72.0;
/// The sync control's height.
const SYNC_HEIGHT: f32 = 44.0;
/// A header control's height ("Add project").
const CONTROL_HEIGHT: f32 = 32.0;
/// The card's chamfer.
const CARD_CUT: f32 = 12.0;
/// The CTA's and the ICE card's chamfer.
const CTA_CUT: f32 = 10.0;
/// A chip's and a control's chamfer.
const CONTROL_CUT: f32 = 8.0;
/// A host state glyph's diameter.
const GLYPH_SIZE: f32 = 8.0;

/// Where [`card`] leaves how many project rows the list's viewport shows, for
/// [`list_page_rows`].
const PAGE_ROWS_ID: &str = "cellworks-launcher-page-rows";

/// How many project rows fit in the list as last drawn, at least one: what
/// Ctrl+D and Ctrl+U move the selection by (STORY-083 requirement 14).
pub fn list_page_rows(ui: &egui::Ui) -> usize {
    ui.data(|data| data.get_temp::<usize>(egui::Id::new(PAGE_ROWS_ID)))
        .unwrap_or(1)
}

/// The `egui::Id` the search field is given, so [`search_focused`] can check
/// the same id `search_row` builds its `TextEdit` with.
const SEARCH_ID: &str = "cellworks-launcher-search";

/// Whether the launcher's search field currently has keyboard focus, so the
/// caller can suppress the map's letter shortcuts while a project name is
/// typed into it. That focus is the launcher's CMD mode (STORY-083
/// requirement 8); without it the launcher is in NAV. `src/app.rs` reads the
/// keys before this module draws, so only the id tells it apart.
pub fn search_focused(ui: &egui::Ui) -> bool {
    ui.memory(|memory| memory.has_focus(egui::Id::new(SEARCH_ID)))
}

/// Whether the search field had focus at the end of the previous frame: egui
/// drops the focus on an Escape before the app reads the key, so this tells
/// an Esc that leaves CMD from an Esc pressed in NAV.
pub fn search_was_focused(ui: &egui::Ui) -> bool {
    ui.memory(|memory| memory.had_focus_last_frame(egui::Id::new(SEARCH_ID)))
}

/// A click the launcher reports upward: everything else (search text, the
/// picked location, the selection, arrow-key movement) is applied directly
/// to the `Launcher` passed in, because only opening a project or changing
/// what Navigation tracks reaches beyond this view.
#[derive(Clone, Debug, PartialEq)]
pub enum LauncherAction {
    /// The engineer selected this project, by a click or the arrow keys.
    Selected(Project),
    /// The engineer opened this project: double-clicked its name, or pressed
    /// Enter or "Open" while it was selected. Always a local project — the
    /// view never emits this for a remote one.
    Opened(Project),
}

/// `text` in the mock's type: `size` points of `family` in `color`, tracked
/// by `tracking` em (the mock's `letter-spacing`).
fn styled(
    text: &str,
    size: f32,
    family: egui::FontFamily,
    color: egui::Color32,
    tracking: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::new(size, family),
            color,
            extra_letter_spacing: size * tracking,
            ..Default::default()
        },
    );
    job
}

/// `job`, cut to one row of at most `width` points with an ellipsis.
fn one_row(mut job: egui::text::LayoutJob, width: f32) -> egui::text::LayoutJob {
    job.wrap = egui::text::TextWrapping {
        max_width: width.max(0.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    job
}

/// Lays `job` out.
fn galley(ui: &egui::Ui, job: egui::text::LayoutJob) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout_job(job)
}

/// Gives `response` an accessibility node of `typ` that reads `label`: the
/// words themselves, while the screen shows them upper-cased and tracked
/// (requirement 10 matches the words case-insensitively).
fn labelled(response: &egui::Response, typ: egui::WidgetType, label: &str) {
    let enabled = response.enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(typ, enabled, label));
}

/// Paints `galley` at `pos` as a label that reads `label`, with its own
/// node under `id`.
fn text_at(
    ui: &egui::Ui,
    id: egui::Id,
    pos: egui::Pos2,
    galley: std::sync::Arc<egui::Galley>,
    label: &str,
) -> egui::Response {
    let response = ui.interact(
        egui::Rect::from_min_size(pos, galley.size()),
        id,
        egui::Sense::hover(),
    );
    ui.painter().galley(pos, galley, INK);
    labelled(&response, egui::WidgetType::Label, label);
    response
}

/// Allocates `galley` in `ui`'s flow and paints it as a label that reads
/// `label`.
fn text_in_flow(
    ui: &mut egui::Ui,
    galley: std::sync::Arc<egui::Galley>,
    label: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    ui.painter().galley(rect.min, galley, INK);
    labelled(&response, egui::WidgetType::Label, label);
    response
}

/// `galley` at the left of `rect`, centred vertically.
fn left_centred(rect: egui::Rect, x: f32, galley: &egui::Galley) -> egui::Pos2 {
    egui::pos2(x, rect.center().y - galley.size().y / 2.0)
}

/// A key cap: `galley` in a 1 px box, its left edge at `x` and centred on
/// `centre_y`. Returns the box.
fn key_cap(
    ui: &egui::Ui,
    x: f32,
    centre_y: f32,
    galley: std::sync::Arc<egui::Galley>,
) -> egui::Rect {
    let size = galley.size() + egui::vec2(10.0, 4.0);
    let rect = egui::Rect::from_min_size(egui::pos2(x, centre_y - size.y / 2.0), size);
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, LINE_BRIGHT),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.min + egui::vec2(5.0, 2.0), galley, DIM_INK);
    rect
}

/// Paints key caps and their words left to right from `x`, centred on
/// `centre_y`, `gap` apart; returns where the last one ends.
fn key_hints(ui: &egui::Ui, mut x: f32, centre_y: f32, hints: &[(&str, &str)], gap: f32) -> f32 {
    let mono = jetbrains_mono(ui);
    for (index, (key, word)) in hints.iter().enumerate() {
        if index > 0 {
            x += gap;
        }
        let cap = key_cap(
            ui,
            x,
            centre_y,
            galley(ui, styled(key, 11.0, mono.clone(), DIM_INK, 0.0)),
        );
        let word = galley(ui, styled(word, 11.0, mono.clone(), DIM_INK, 0.0));
        let pos = egui::pos2(cap.right() + 6.0, centre_y - word.size().y / 2.0);
        x = pos.x + word.size().x;
        ui.painter().galley(pos, word, DIM_INK);
    }
    x
}

/// How wide [`key_hints`] draws `hints`.
fn key_hints_width(ui: &egui::Ui, hints: &[(&str, &str)], gap: f32) -> f32 {
    let mono = jetbrains_mono(ui);
    let mut width = 0.0;
    for (index, (key, word)) in hints.iter().enumerate() {
        if index > 0 {
            width += gap;
        }
        let key = galley(ui, styled(key, 11.0, mono.clone(), DIM_INK, 0.0));
        let word = galley(ui, styled(word, 11.0, mono.clone(), DIM_INK, 0.0));
        width += key.size().x + 10.0 + 6.0 + word.size().x;
    }
    width
}

/// Draws the launcher: the header bar, the centred card (search row, location
/// chip row, the project list beside the preview pane, and the key-hint
/// footer), the notices below the card and the status bar. Mutates `launcher`
/// directly for anything that stays within it (search, pick, selection by
/// click or arrow key); returns `Some` only when the engineer selected or
/// asked to open a project.
pub fn launcher(
    ui: &mut egui::Ui,
    universe: &Universe,
    launcher: &mut Launcher,
) -> Option<LauncherAction> {
    paint_background(ui);
    let full = ui.max_rect();

    // Decision 7: the view treats `animation_time == 0.0` as reduced motion
    // rather than threading a flag of its own — `src/app.rs` zeroes it
    // globally once, at start-up, when the system asks for reduced motion.
    let reduced = ui.style().animation_time == 0.0;
    let now = ui.input(|input| input.time) as f32;
    // One shared clock for the card's, the list's and the preview pane's
    // rise (design requirement 5's "+90 ms", "+180 ms" stagger), read once
    // here rather than separately by each.
    let elapsed = first_shown(ui, egui::Id::new(LAUNCHER_SHOWN_ID), now);

    let header_rect = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), HEADER_HEIGHT));
    header(ui, header_rect, launcher);

    let notices = universe.notices();
    let width = CARD_WIDTH.min(full.width() - 2.0 * SEARCH_MARGIN).max(0.0);
    let fixed = SEARCH_MARGIN + SEARCH_HEIGHT + chip_row_height() + FOOT_HEIGHT;
    let room = full.height()
        - HEADER_HEIGHT
        - STATUS_HEIGHT
        - CARD_TOP_GAP
        - fixed
        - notices.len() as f32 * (NOTICE_GAP + NOTICE_HEIGHT)
        - NOTICE_GAP;
    let body_height = BODY_HEIGHT.min(room).max(BODY_MIN_HEIGHT);
    let card_rect = egui::Rect::from_min_size(
        egui::pos2(
            full.center().x - width / 2.0,
            header_rect.bottom() + CARD_TOP_GAP,
        ),
        egui::vec2(width, fixed + body_height),
    );

    // The card rises in with no delay of its own (design requirement 5).
    let (offset, opacity) = rise(elapsed, 0.0, reduced);
    let action = ui
        .scope_builder(egui::UiBuilder::new().max_rect(card_rect), |ui| {
            ui.with_visual_transform(
                egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset)),
                |ui| {
                    ui.multiply_opacity(opacity);
                    card(
                        ui,
                        card_rect,
                        body_height,
                        universe,
                        launcher,
                        elapsed,
                        reduced,
                    )
                },
            )
            .inner
        })
        .inner;

    // Below the card, not inside the scrollable list, so a notice is never
    // scrolled out of sight (requirement 11).
    let mut top = card_rect.bottom() + NOTICE_GAP;
    for notice in notices {
        let rect = egui::Rect::from_min_size(
            egui::pos2(card_rect.left(), top),
            egui::vec2(width, NOTICE_HEIGHT),
        );
        notice_label(ui, rect, launcher.words(), notice);
        top += NOTICE_HEIGHT + NOTICE_GAP;
    }

    let status_rect = egui::Rect::from_min_max(
        egui::pos2(full.left(), full.bottom() - STATUS_HEIGHT),
        full.max,
    );
    status_bar(ui, status_rect, launcher);
    ui.advance_cursor_after_rect(full);

    // Decision 8: a scheduled repaint, never an immediate one, so a headless
    // harness's single `run()` call settles — and never scheduled at all
    // when reduced, so a reduced-motion harness settles with nothing left to
    // repaint. While the card, the list or the preview pane (the preview's
    // +180 ms delay is the largest) is still rising, or the CTA is shown and
    // breathing, another frame is due in about 1/30 s.
    if !reduced {
        let still_rising = elapsed < 0.18 + RISE_DURATION;
        let cta_breathing = launcher
            .selected_project()
            .is_some_and(|project| launcher.can_open(&project.id));
        if still_rising || cta_breathing {
            ui.ctx().request_repaint_after_secs(1.0 / 30.0);
        }
    }

    action
}

/// The background behind the whole view: near-black with a faint 100 px grid
/// (design requirement 1), painted on the whole window, below everything
/// this view draws. The rest of the app keeps its current visuals — only this
/// view paints it.
fn paint_background(ui: &egui::Ui) {
    let rect = ui.ctx().content_rect();
    let painter = ui.ctx().layer_painter(ui.layer_id());
    painter.rect_filled(rect, 0.0, BG);
    let step = 100.0;
    let stroke = egui::Stroke::new(1.0, GRID_LINE);
    let mut x = rect.left();
    while x <= rect.right() {
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            stroke,
        );
        x += step;
    }
    let mut y = rect.top();
    while y <= rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            stroke,
        );
        y += step;
    }
}

/// The 48 px header bar: "CELLWORKS" (Decision 4: one string, in or out of
/// theme words) and the eyebrow on the left; the project and host counts and
/// the disabled add-project control on the right (STORY-082 requirement 1,
/// STORY-083 requirements 1, 2 and 7/8).
fn header(ui: &mut egui::Ui, rect: egui::Rect, launcher: &Launcher) {
    let words = launcher.words();
    ui.painter().rect_filled(rect, 0.0, BG.gamma_multiply(0.85));
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        egui::Stroke::new(1.0, LINE),
    );
    let inner = rect.shrink2(egui::vec2(HEADER_PADDING, 0.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let title = galley(ui, cellworks_title(ui));
            text_in_flow(ui, title, "CELLWORKS");
            if !words.header_eyebrow.is_empty() {
                let eyebrow = galley(
                    ui,
                    styled(words.header_eyebrow, 11.0, chakra_petch(ui), DIM_INK, 0.16),
                );
                text_in_flow(ui, eyebrow, words.header_eyebrow);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                add_project_button(ui, words.add_project_button);
                let chips = launcher.location_chips();
                let projects = chips.first().map_or(0, |chip| chip.count);
                // Every chip but "All" and "This Mac" is a remote host.
                let hosts = chips.len().saturating_sub(2);
                let counts = words.counts(projects, hosts);
                let counts_galley =
                    galley(ui, styled(&counts, 12.0, jetbrains_mono(ui), DIM_INK, 0.0));
                text_in_flow(ui, counts_galley, &counts);
            });
        },
    );
}

/// "CELLWORKS", with "WORKS" in attention yellow and "CELL" in ink (the
/// confirmed canvas look) — one string either way theme words turn, so it is
/// built here rather than read from [`Words`] (Decision 4).
fn cellworks_title(ui: &egui::Ui) -> egui::text::LayoutJob {
    let mut job = styled("CELL", 15.0, chakra_petch(ui), INK, 0.18);
    job.append(
        "WORKS",
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::new(15.0, chakra_petch(ui)),
            color: YELLOW,
            extra_letter_spacing: 15.0 * 0.18,
            ..Default::default()
        },
    );
    job
}

/// The status bar along the window's bottom edge: the cyan mode badge — "NAV"
/// and the status text, or "CMD" and the live query while searching
/// (STORY-083 requirement 4) — and the key hints on the right.
fn status_bar(ui: &egui::Ui, rect: egui::Rect, launcher: &Launcher) {
    let words = launcher.words();
    ui.painter().rect_filled(rect, 0.0, PANEL);
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        egui::Stroke::new(1.0, LINE),
    );
    let searching = search_focused(ui);
    let (mode, text, color) = if searching {
        ("CMD", format!(":{}", launcher.search_text()), INK)
    } else {
        ("NAV", words.status_text.to_owned(), DIM_INK)
    };
    let mode_galley = galley(ui, styled(mode, 11.0, chakra_petch(ui), BG, 0.16));
    let badge = egui::Rect::from_min_size(
        rect.min,
        egui::vec2(mode_galley.size().x + 24.0, rect.height()),
    );
    ui.painter().rect_filled(badge, 0.0, CYAN);
    let id = egui::Id::new("cellworks-launcher-status");
    text_at(
        ui,
        id.with("mode"),
        left_centred(badge, badge.left() + 12.0, &mode_galley),
        mode_galley,
        mode,
    );
    let text_galley = galley(ui, styled(&text, 11.0, jetbrains_mono(ui), color, 0.0));
    text_at(
        ui,
        id.with("text"),
        left_centred(rect, badge.right() + 16.0, &text_galley),
        text_galley,
        &text,
    );
    let hints: &[(&str, &str)] = if searching {
        &[("⏎", "open"), ("Esc", "nav"), ("⌘K", "nav")]
    } else {
        &[("j/k", "move"), (":", "search"), ("⏎", "open")]
    };
    let width = key_hints_width(ui, hints, 16.0);
    key_hints(
        ui,
        rect.right() - 16.0 - width,
        rect.center().y,
        hints,
        16.0,
    );
}

/// The chip row's height: one chip and its padding.
fn chip_row_height() -> f32 {
    CHIP_HEIGHT + 2.0 * CHIP_ROW_PADDING
}

/// The card: the search well, the location chip row, the list beside the
/// preview pane, and the key-hint footer (STORY-082 requirements 2 to 11),
/// on a chamfered panel cut top-right and bottom-left (STORY-083 requirement
/// 3). The list rises in +90 ms after the card itself and the preview pane
/// +180 ms (design requirement 5); `elapsed` and `reduced` are [`launcher`]'s
/// own shared clock and reduced-motion reading, passed down rather than read
/// again here.
fn card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    body_height: f32,
    universe: &Universe,
    launcher: &mut Launcher,
    elapsed: f32,
    reduced: bool,
) -> Option<LauncherAction> {
    let points = chamfer(rect, CARD_CUT, Corners::TopRightBottomLeft);
    for (expand, alpha) in [(6.0, 0.10), (3.0, 0.18)] {
        ui.painter().add(egui::Shape::convex_polygon(
            chamfer(
                rect.expand(expand),
                CARD_CUT + expand,
                Corners::TopRightBottomLeft,
            )
            .to_vec(),
            egui::Color32::BLACK.gamma_multiply(alpha),
            egui::Stroke::NONE,
        ));
    }
    ui.painter().add(egui::Shape::convex_polygon(
        points.to_vec(),
        PANEL,
        egui::Stroke::new(1.0, CYAN.gamma_multiply(0.18)),
    ));

    let search_rect = egui::Rect::from_min_max(
        rect.min + egui::vec2(SEARCH_MARGIN, SEARCH_MARGIN),
        egui::pos2(
            rect.right() - SEARCH_MARGIN,
            rect.top() + SEARCH_MARGIN + SEARCH_HEIGHT,
        ),
    );
    search_row(ui, search_rect, launcher);

    let chips_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left(), search_rect.bottom()),
        egui::vec2(rect.width(), chip_row_height()),
    );
    chip_row(ui, chips_rect, launcher);
    ui.painter().hline(
        rect.x_range(),
        chips_rect.bottom() - 0.5,
        egui::Stroke::new(1.0, LINE),
    );

    let body = egui::Rect::from_min_size(
        egui::pos2(rect.left(), chips_rect.bottom()),
        egui::vec2(rect.width(), body_height),
    );
    let preview_width = PREVIEW_WIDTH.min(body.width() / 2.0);
    let aside = egui::Rect::from_min_max(
        egui::pos2(body.right() - preview_width, body.top()),
        body.max,
    );
    let list_rect = egui::Rect::from_min_max(body.min, egui::pos2(aside.left(), body.bottom()));
    let page_rows = (((list_rect.height() - LIST_TOP_PADDING) / ROW_HEIGHT) as usize).max(1);
    ui.data_mut(|data| data.insert_temp(egui::Id::new(PAGE_ROWS_ID), page_rows));
    let fade = egui::Rect::from_min_max(
        aside.min,
        egui::pos2(aside.right(), aside.top() + aside.height() * 0.4),
    );
    let mut mesh = egui::Mesh::default();
    let top = CYAN.gamma_multiply(0.03);
    mesh.colored_vertex(fade.left_top(), top);
    mesh.colored_vertex(fade.right_top(), top);
    mesh.colored_vertex(fade.right_bottom(), egui::Color32::TRANSPARENT);
    mesh.colored_vertex(fade.left_bottom(), egui::Color32::TRANSPARENT);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(mesh);
    ui.painter().vline(
        aside.left() + 0.5,
        aside.y_range(),
        egui::Stroke::new(1.0, LINE),
    );

    let mut action = None;
    // The preview pane is drawn before the list, so the list's row is the
    // last of a project's name in tree order, as it was when the preview
    // was a panel of its own. It keeps its width while nothing is selected:
    // if it appeared only on selection, the click that selects a project
    // would narrow the list and move the row from under a double-click.
    if let Some(project) = launcher.selected_project().cloned() {
        // The preview pane rises in last, +180 ms (design requirement 5).
        let (offset, opacity) = rise(elapsed, 0.18, reduced);
        action = ui
            .scope_builder(egui::UiBuilder::new().max_rect(aside), |ui| {
                ui.with_visual_transform(
                    egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset)),
                    |ui| {
                        ui.multiply_opacity(opacity);
                        inspector(ui, aside, launcher, &project, elapsed, reduced)
                    },
                )
                .inner
            })
            .inner;
    }

    let (offset, opacity) = rise(elapsed, 0.09, reduced);
    let listed = ui
        .scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
            ui.with_visual_transform(
                egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset)),
                |ui| {
                    ui.multiply_opacity(opacity);
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.add_space(LIST_TOP_PADDING);
                            list(ui, universe, launcher)
                        })
                        .inner
                },
            )
            .inner
        })
        .inner;
    if listed.is_some() {
        action = listed;
    }

    let foot = egui::Rect::from_min_max(egui::pos2(rect.left(), body.bottom()), rect.max);
    card_footer(ui, foot);
    action
}

/// The card's own key-hint footer (STORY-082 requirement 9).
fn card_footer(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        egui::Stroke::new(1.0, LINE),
    );
    let response = ui.interact(
        rect,
        egui::Id::new("cellworks-launcher-hints"),
        egui::Sense::hover(),
    );
    let cmd = search_focused(ui);
    labelled(
        &response,
        egui::WidgetType::Label,
        if cmd { HINTS_CMD } else { HINTS_NAV },
    );
    let centre_y = rect.center().y;
    key_hints(
        ui,
        rect.left() + ROW_PADDING,
        centre_y,
        if cmd {
            &[
                ("↑↓", "move"),
                ("⏎", "open"),
                ("Tab", "scope"),
                ("⇧Tab", "sort"),
            ]
        } else {
            &[
                ("↑↓/jk", "move"),
                ("⏎", "open"),
                ("Tab", "scope"),
                ("⇧Tab", "sort"),
            ]
        },
        18.0,
    );
    let right: &[(&str, &str)] = if cmd {
        &[("Esc", "nav")]
    } else {
        &[(":", "search")]
    };
    let width = key_hints_width(ui, right, 18.0);
    key_hints(
        ui,
        rect.right() - ROW_PADDING - width,
        centre_y,
        right,
        18.0,
    );
}

/// The search well, focused in CMD mode only (⌘K, ":", "/" or a click; the
/// keys are read in `src/app.rs`); while text is typed, the match count and
/// the "Esc clear" key (STORY-082 requirement 2, STORY-083 requirement 8). A deep well with a cyan, glowing edge when focused, a cyan ":" prompt
/// before the text, and a yellow caret (STORY-083 requirements 4 and 5) that
/// blinks or not exactly as egui's native text cursor already does —
/// `src/app.rs` makes that reduced-motion-aware globally, so only its colour
/// is set here, scoped to this field alone.
fn search_row(ui: &mut egui::Ui, rect: egui::Rect, launcher: &mut Launcher) {
    let words = launcher.words();
    let search_id = egui::Id::new(SEARCH_ID);
    let focused = search_focused(ui);
    let mono = jetbrains_mono(ui);
    let painter = ui.painter();
    if focused {
        // epaint has no blur: a falloff of 1 px rings stands in for the
        // mock's 14 px glow.
        for step in 0..14 {
            let fade = 1.0 - step as f32 / 14.0;
            painter.rect_stroke(
                rect.expand(step as f32 + 0.5),
                0.0,
                egui::Stroke::new(1.0, CYAN.gamma_multiply(0.12 * fade * fade)),
                egui::StrokeKind::Middle,
            );
        }
    }
    painter.rect_filled(rect, 0.0, WELL);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, if focused { CYAN } else { LINE_BRIGHT }),
        egui::StrokeKind::Inside,
    );

    let prompt = galley(ui, styled(":", 16.0, mono.clone(), CYAN, 0.0));
    let prompt_pos = left_centred(rect, rect.left() + SEARCH_PADDING, &prompt);
    let text_left = prompt_pos.x + prompt.size().x + 12.0;
    ui.painter().galley(prompt_pos, prompt, CYAN);

    // The right-hand side is laid out first, so the field knows its width;
    // it is painted after the field, so the field stays first in tree order.
    let searching = !launcher.search_text().is_empty();
    let right = rect.right() - SEARCH_PADDING;
    let (escape, matches) = if searching {
        let escape_word = if focused { "Esc nav" } else { "Esc clear" };
        let escape = galley(ui, styled(escape_word, 11.0, mono.clone(), DIM_INK, 0.0));
        let matches = format!("{} matches", launcher.match_count());
        let matches_galley = galley(ui, styled(&matches, 12.0, mono.clone(), CYAN, 0.0));
        (Some(escape), Some((matches, matches_galley)))
    } else {
        (None, None)
    };
    let shortcut = galley(ui, styled("⌘K", 11.0, mono.clone(), DIM_INK, 0.0));
    let right_width = match (&escape, &matches) {
        (Some(escape), Some((_, matches))) => escape.size().x + 10.0 + 12.0 + matches.size().x,
        _ => shortcut.size().x + 10.0,
    };

    let font = egui::FontId::new(15.0, mono.clone());
    let row_height = ui.fonts_mut(|fonts| fonts.row_height(&font));
    let field = egui::Rect::from_min_max(
        egui::pos2(text_left, rect.center().y - row_height / 2.0),
        egui::pos2(
            right - right_width - 12.0,
            rect.center().y + row_height / 2.0,
        ),
    );
    let mut text = launcher.search_text().to_owned();
    ui.visuals_mut().text_cursor.stroke.color = YELLOW;
    let response = ui.put(
        field,
        egui::TextEdit::singleline(&mut text)
            // Tab scopes the list (requirement 4), so the field keeps the
            // focus on it: egui decides the focus direction from the raw
            // input before the key block runs, and `consume_key` there
            // cannot take that decision back. `lock_focus` only tells egui's
            // focus pass to leave this widget alone — a single-line field
            // inserts no tab character, and the key block has consumed the
            // key before the field reads its events anyway.
            .lock_focus(true)
            .id(search_id)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(field.width())
            .font(font)
            .text_color(INK)
            .hint_text(
                egui::RichText::new(words.search_placeholder)
                    .font(egui::FontId::new(15.0, mono))
                    .color(DIM_INK),
            ),
    );
    if response.changed() {
        launcher.search(text);
    }

    let centre_y = rect.center().y;
    match (escape, matches) {
        (Some(escape), Some((matches, matches_galley))) => {
            let escape_x = right - escape.size().x - 10.0;
            let cap = key_cap(ui, escape_x, centre_y, escape);
            let response = ui.interact(cap, search_id.with("escape"), egui::Sense::hover());
            labelled(
                &response,
                egui::WidgetType::Label,
                if focused { "Esc nav" } else { "Esc clear" },
            );
            let pos = egui::pos2(
                escape_x - 12.0 - matches_galley.size().x,
                centre_y - matches_galley.size().y / 2.0,
            );
            text_at(ui, search_id.with("matches"), pos, matches_galley, &matches);
        }
        _ => {
            key_cap(ui, right - shortcut.size().x - 10.0, centre_y, shortcut);
        }
    }

    match launcher.take_focus_request() {
        Some(true) => ui.memory_mut(|memory| memory.request_focus(search_id)),
        Some(false) => ui.memory_mut(|memory| memory.surrender_focus(search_id)),
        None => {}
    }
}

/// The location chip row: "All", "This Mac", then one chip per remote host
/// (STORY-082 requirement 3), replacing STORY-078's left locations column.
/// Clicking a chip picks it; Tab cycles it (`src/app.rs`'s key block, not
/// here). Chamfered, upper-case and tracked (STORY-083 requirement 3); the
/// picked chip gets a soft-yellow fill with yellow text and edge
/// (requirement 4). Each chip's name, state word and count are separate
/// labels that read the words themselves (ASSUMED (Q-1)).
fn chip_row(ui: &mut egui::Ui, rect: egui::Rect, launcher: &mut Launcher) {
    let picked = launcher.picked().clone();
    let words = launcher.words();
    let sans = chakra_petch(ui);
    let mono = jetbrains_mono(ui);
    let id = egui::Id::new("cellworks-launcher-chips");
    let mut x = rect.left() + CHIP_ROW_PADDING;
    let top = rect.top() + CHIP_ROW_PADDING;
    let mut clicked = None;
    for (index, chip) in launcher.location_chips().into_iter().enumerate() {
        let is_picked = chip.pick == picked;
        let offline = chip.state == Some(HostState::Offline);
        let text_color = match (is_picked, offline) {
            (true, _) => YELLOW,
            (false, true) => DIM_INK,
            (false, false) => INK,
        };
        let count_color = if is_picked { YELLOW } else { DIM_INK };
        let name = galley(
            ui,
            styled(
                &chip.label.to_uppercase(),
                11.0,
                sans.clone(),
                text_color,
                0.12,
            ),
        );
        let state_word = chip.state.map(|state| {
            let word = words.host_state_word(state);
            (
                word,
                galley(
                    ui,
                    styled(&word.to_uppercase(), 11.0, sans.clone(), text_color, 0.12),
                ),
            )
        });
        let count_text = chip.count.to_string();
        let count = galley(
            ui,
            styled(&count_text, 11.0, mono.clone(), count_color, 0.0),
        );

        let mut width = 2.0 * CHIP_PADDING + name.size().x + CHIP_GAP + count.size().x;
        if chip.state.is_some() {
            width += GLYPH_SIZE + CHIP_GAP;
        }
        if let Some((_, state)) = &state_word {
            width += 4.0 + state.size().x;
        }
        let chip_rect =
            egui::Rect::from_min_size(egui::pos2(x, top), egui::vec2(width, CHIP_HEIGHT));
        let (fill, stroke) = if is_picked {
            (YELLOW_SOFT, egui::Stroke::new(1.0, YELLOW))
        } else {
            (egui::Color32::TRANSPARENT, egui::Stroke::new(1.0, LINE))
        };
        ui.painter().add(egui::Shape::convex_polygon(
            chamfer(chip_rect, CONTROL_CUT, Corners::TopLeftBottomRight).to_vec(),
            fill,
            stroke,
        ));

        // The whole chip is the click target; its node reads the location's
        // name, so a test or a screen reader finds the chip by it.
        let chip_id = id.with(index);
        let response = ui.interact(chip_rect, chip_id, egui::Sense::click());
        labelled(&response, egui::WidgetType::Label, &chip.label);
        if response.clicked() {
            clicked = Some(chip.pick.clone());
        }

        let mut cursor = chip_rect.left() + CHIP_PADDING;
        if let Some(state) = chip.state {
            state_glyph(
                ui,
                egui::pos2(cursor + GLYPH_SIZE / 2.0, chip_rect.center().y),
                state,
            );
            cursor += GLYPH_SIZE + CHIP_GAP;
        }
        let name_pos = left_centred(chip_rect, cursor, &name);
        cursor += name.size().x;
        ui.painter().galley(name_pos, name, text_color);
        if let Some((word, state)) = state_word {
            cursor += 4.0;
            let pos = left_centred(chip_rect, cursor, &state);
            cursor += state.size().x;
            text_at(ui, chip_id.with("state"), pos, state, word);
        }
        cursor += CHIP_GAP;
        let pos = left_centred(chip_rect, cursor, &count);
        text_at(ui, chip_id.with("count"), pos, count, &count_text);
        x = chip_rect.right() + CHIP_GAP;
    }
    // The sort chip after the locations: same chip, no count (STORY-083
    // requirement 15). Its text is the same with theme words on or off.
    let sort_text = launcher.sort().label();
    let sort = galley(ui, styled(&sort_text.to_uppercase(), 11.0, sans, INK, 0.12));
    let chip_rect = egui::Rect::from_min_size(
        egui::pos2(x, top),
        egui::vec2(2.0 * CHIP_PADDING + sort.size().x, CHIP_HEIGHT),
    );
    ui.painter().add(egui::Shape::convex_polygon(
        chamfer(chip_rect, CONTROL_CUT, Corners::TopLeftBottomRight).to_vec(),
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, LINE),
    ));
    let sort_id = id.with("sort");
    let response = ui.interact(chip_rect, sort_id, egui::Sense::click());
    labelled(&response, egui::WidgetType::Label, sort_text);
    let sort_clicked = response.clicked();
    let pos = left_centred(chip_rect, chip_rect.left() + CHIP_PADDING, &sort);
    ui.painter().galley(pos, sort, INK);

    if let Some(pick) = clicked {
        launcher.pick(pick);
    }
    if sort_clicked {
        launcher.cycle_sort();
    }
}

/// A host's state glyph centred on `centre`, painted beside the word rather
/// than written into it (ASSUMED (Q-1)): a filled dot for a connected host, a
/// ring around a dot for one still being checked, an empty ring for an
/// offline one. The shape differs as well as the colour, and the word beside
/// it names the state, so the glyph never carries the meaning alone.
/// Connected is ok green, checking is cyan, offline is dim ink (STORY-083's
/// confirmed behaviour).
fn state_glyph(ui: &egui::Ui, centre: egui::Pos2, state: HostState) {
    let radius = GLYPH_SIZE / 2.0;
    let color = match state {
        HostState::Connected => OK_GREEN,
        HostState::Checking => CYAN,
        HostState::Offline => DIM_INK,
    };
    let stroke = egui::Stroke::new(1.5, color);
    let painter = ui.painter();
    match state {
        HostState::Connected => {
            painter.circle_filled(centre, radius, color);
        }
        HostState::Checking => {
            painter.circle_stroke(centre, radius, stroke);
            painter.circle_filled(centre, radius * 0.4, color);
        }
        HostState::Offline => {
            painter.circle_stroke(centre, radius - 0.5, stroke);
        }
    }
}

/// The recent rows, then the scanning/failed/empty-state/grouped list
/// (STORY-078's states, kept per STORY-082 requirement 11).
fn list(ui: &mut egui::Ui, universe: &Universe, launcher: &mut Launcher) -> Option<LauncherAction> {
    let mut action = None;
    let words = launcher.words();
    let selected = launcher.selected().cloned();
    let mut take = |click: Option<RowClick>, project: &Project, launcher: &mut Launcher| match click
    {
        Some(RowClick::Selected) => {
            launcher.select(Some(project.id.clone()));
            action = Some(LauncherAction::Selected(project.clone()));
        }
        Some(RowClick::Opened) => {
            action = Some(LauncherAction::Opened(project.clone()));
        }
        None => {}
    };

    let recent: Vec<Project> = launcher.recent_row().into_iter().cloned().collect();
    if !recent.is_empty() {
        group_heading_label(ui, words.recent_heading, None);
        for project in &recent {
            // A recent row reads "recent: <name>", so it is told apart from
            // the same project's row in its group. It looks like a project
            // row but stays what STORY-078 made it, a line to read: it never
            // shows the selection and a click on it does nothing, so the
            // project's row in its group stays the one selected row.
            let label = format!("recent: {}", project.name);
            let _ = project_row(ui, launcher, project, None, &label);
        }
    }

    match universe.scan() {
        ScanState::Scanning => {
            list_message(ui, SCANNING);
        }
        ScanState::Failed => {
            list_message(ui, FAILED);
        }
        ScanState::Reported(projects) if projects.is_empty() => {
            list_message(ui, &empty_state(universe.sources()));
        }
        ScanState::Reported(_) => {
            let groups: Vec<(String, Vec<Project>)> = launcher
                .groups()
                .into_iter()
                .map(|(label, projects)| (label, projects.into_iter().cloned().collect()))
                .collect();
            if groups.is_empty() && !launcher.search_text().is_empty() {
                list_message(
                    ui,
                    &format!("No project matches \"{}\".", launcher.search_text()),
                );
                ui.horizontal(|ui| {
                    ui.add_space(ROW_PADDING);
                    add_project_button(ui, words.add_project_button);
                });
            } else {
                for (label, projects) in &groups {
                    // One group per location, named by `Launcher::groups`
                    // (requirement 5): "This Mac" once for every local
                    // project, then a group per host — restated in theme
                    // words through `group_heading`, since `groups()` itself
                    // is unchanged (design Order of work step 2).
                    let state = host_state(launcher, label);
                    group_heading_label(ui, &group_heading(launcher, label), state);
                    let mut folder: Option<String> = None;
                    for project in projects {
                        // In dir sort the local group names each folder once,
                        // where the parent changes (requirement 16).
                        if launcher.sort() == Sort::Dir
                            && let Some(parent) = launcher.folder_of(project)
                            && folder.as_ref() != Some(&parent)
                        {
                            folder_subheading(ui, &parent);
                            folder = Some(parent);
                        }
                        let click =
                            project_row(ui, launcher, project, selected.as_ref(), &project.name);
                        take(click, project, launcher);
                    }
                }
            }
        }
    }

    action
}

/// A line of text in the list, in place of rows.
fn list_message(ui: &mut egui::Ui, text: &str) {
    ui.add_space(HEADING_SPACE.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(ROW_PADDING);
        ui.set_max_width(ui.available_width() - ROW_PADDING);
        ui.label(egui::RichText::new(text).color(DIM_INK));
    });
    ui.add_space(HEADING_SPACE.0);
}

/// A group heading: an eyebrow, upper-case and tracked, with a host's state
/// glyph before it and a rule running to the right edge after it (STORY-083
/// requirement 4). Its node reads `text` itself.
fn group_heading_label(ui: &mut egui::Ui, text: &str, state: Option<HostState>) {
    let eyebrow = galley(
        ui,
        styled(&text.to_uppercase(), 11.0, chakra_petch(ui), DIM_INK, 0.16),
    );
    let height = HEADING_SPACE.0 + eyebrow.size().y + HEADING_SPACE.1;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    labelled(&response, egui::WidgetType::Label, text);
    let centre_y = rect.top() + HEADING_SPACE.0 + eyebrow.size().y / 2.0;
    let mut x = rect.left() + ROW_PADDING;
    if let Some(state) = state {
        state_glyph(ui, egui::pos2(x + GLYPH_SIZE / 2.0, centre_y), state);
        x += GLYPH_SIZE + CHIP_GAP;
    }
    let width = eyebrow.size().x;
    ui.painter().galley(
        egui::pos2(x, rect.top() + HEADING_SPACE.0),
        eyebrow,
        DIM_INK,
    );
    let rule_left = x + width + CHIP_GAP;
    let rule_right = rect.right() - ROW_PADDING;
    if rule_left < rule_right {
        ui.painter().hline(
            rule_left..=rule_right,
            centre_y.round() + 0.5,
            egui::Stroke::new(1.0, LINE),
        );
    }
}

/// A folder subheading inside the local group: the folder's displayed path in
/// the dim mono hint style, its node reading the path itself.
fn folder_subheading(ui: &mut egui::Ui, text: &str) {
    let path = galley(ui, styled(text, 11.0, jetbrains_mono(ui), DIM_INK, 0.0));
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), path.size().y + HEADING_SPACE.1),
        egui::Sense::hover(),
    );
    labelled(&response, egui::WidgetType::Label, text);
    ui.painter().galley(
        egui::pos2(rect.left() + ROW_PADDING, rect.top()),
        path,
        DIM_INK,
    );
}

/// `label`'s host state, when `label` names a remote host's group.
fn host_state(launcher: &Launcher, label: &str) -> Option<HostState> {
    launcher
        .hosts()
        .into_iter()
        .find(|(host, _)| *host == label)
        .map(|(_, state)| state)
}

/// `label`, as `Launcher::groups` returns it, restated the way its group
/// heading reads (STORY-083 requirement 4): a remote host's heading adds its
/// state after its upper-cased name in theme words ("<HOST> NET · <STATE>"),
/// read from [`Words::host_group_heading`]; the local location's heading is
/// `label` itself, already the right word (`this_mac_label`) from
/// `Launcher::groups`.
fn group_heading(launcher: &Launcher, label: &str) -> String {
    match launcher
        .hosts()
        .into_iter()
        .find(|(host, _)| *host == label)
    {
        Some((host, state)) => launcher.words().host_group_heading(host, state),
        None => label.to_owned(),
    }
}

/// What a click on one project's row meant.
enum RowClick {
    Selected,
    Opened,
}

/// One project's 34 px row, in three columns: its name (underlined in
/// attention yellow where the search matched it, yellow when selected, dim
/// when its host is offline), its path and its branch label, right-aligned.
/// Selected: a soft-yellow fill and a 2 px yellow left bar across the full
/// width (STORY-083 requirement 4). The whole row is the click target; its
/// node reads `label`, and the path and branch have nodes of their own.
fn project_row(
    ui: &mut egui::Ui,
    launcher: &Launcher,
    project: &Project,
    selected: Option<&ProjectId>,
    label: &str,
) -> Option<RowClick> {
    let is_selected = selected == Some(&project.id);
    let (rect, entry) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    labelled(&entry, egui::WidgetType::Label, label);
    if is_selected {
        ui.ctx()
            .accesskit_node_builder(entry.id, |node| node.set_selected(true));
        // Keep the selection in view as the arrow keys move it: scroll once
        // each time the selected row changes.
        let shown = egui::Id::new("launcher-scrolled-to");
        if ui.data(|data| data.get_temp::<ProjectId>(shown)).as_ref() != Some(&project.id) {
            ui.data_mut(|data| data.insert_temp(shown, project.id.clone()));
            entry.scroll_to_me(None);
        }
        ui.painter().rect_filled(rect, 0.0, YELLOW_SOFT);
        ui.painter().rect_filled(
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 2.0, rect.bottom())),
            0.0,
            YELLOW,
        );
    } else if entry.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, YELLOW_SOFT.gamma_multiply(0.35));
    }

    let offline = project.id.host().is_some_and(|host| {
        launcher
            .hosts()
            .into_iter()
            .any(|(name, state)| name == host && state == HostState::Offline)
    });
    let name_color = match (is_selected, offline) {
        (true, _) => YELLOW,
        (false, true) => DIM_INK,
        (false, false) => INK,
    };
    let mono = jetbrains_mono(ui);
    let left = rect.left() + ROW_PADDING;
    let right = rect.right() - ROW_PADDING;
    let name = galley(
        ui,
        one_row(
            name_job(&project.name, launcher.search_text(), name_color, &mono),
            NAME_COLUMN,
        ),
    );
    ui.painter()
        .galley(left_centred(rect, left, &name), name, name_color);

    let branch_left = right - BRANCH_COLUMN;
    let path_left = left + NAME_COLUMN + COLUMN_GAP;
    let path_text = launcher.path_of(project);
    let path = galley(
        ui,
        one_row(
            styled(&path_text, 12.0, mono.clone(), DIM_INK, 0.0),
            branch_left - COLUMN_GAP - path_left,
        ),
    );
    text_at(
        ui,
        entry.id.with("path"),
        left_centred(rect, path_left, &path),
        path,
        &path_text,
    );
    // Only once the branch is known: an unread row says nothing, rather than
    // calling every repository a folder before it is selected.
    if let Some(branch_text) = launcher.branch_label(&project.id) {
        let branch = galley(
            ui,
            one_row(
                styled(&branch_text, 12.0, mono, DIM_INK, 0.0),
                BRANCH_COLUMN,
            ),
        );
        let x = right - branch.size().x;
        text_at(
            ui,
            entry.id.with("branch"),
            left_centred(rect, x, &branch),
            branch,
            &branch_text,
        );
    }

    // A remote project's row does not open on double-click (requirement 8);
    // a double-click there still selects it.
    if entry.double_clicked() && project.id.local_path().is_some() {
        Some(RowClick::Opened)
    } else if entry.clicked() {
        Some(RowClick::Selected)
    } else {
        None
    }
}

/// `name` in `color`, with the byte range `query` matches in it underlined in
/// attention yellow (STORY-082 requirement 6, STORY-083 requirement 4); plain
/// text when there is no match, so a path-only hit never underlines the
/// wrong fragment.
fn name_job(
    name: &str,
    query: &str,
    color: egui::Color32,
    family: &egui::FontFamily,
) -> egui::text::LayoutJob {
    let plain = egui::TextFormat {
        font_id: egui::FontId::new(13.0, family.clone()),
        color,
        ..Default::default()
    };
    let mut job = egui::text::LayoutJob::default();
    match match_range(name, query) {
        Some(range) => {
            job.append(&name[..range.start], 0.0, plain.clone());
            job.append(
                &name[range.clone()],
                0.0,
                egui::TextFormat {
                    underline: egui::Stroke::new(2.0, YELLOW),
                    ..plain.clone()
                },
            );
            job.append(&name[range.end..], 0.0, plain);
        }
        None => job.append(name, 0.0, plain),
    }
    job
}

/// The preview pane inside the card, beside the list (STORY-082 requirement
/// 7, STORY-083 requirement 4): an eyebrow, the project's name upper-case,
/// its kind and location, a PATH/BRANCH/FILES list; at the bottom the
/// orange ICE card when its host is offline, the CTA and the disabled sync
/// control. `elapsed` and `reduced` are [`launcher`]'s own shared clock and
/// reduced-motion reading, passed through only so the CTA's glow can breathe
/// by it (this pane's own rise is applied around the call to this function,
/// not inside it).
fn inspector(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    launcher: &Launcher,
    project: &Project,
    elapsed: f32,
    reduced: bool,
) -> Option<LauncherAction> {
    let words = launcher.words();
    let inner = rect.shrink(PREVIEW_PADDING);
    let sans = chakra_petch(ui);
    let mono = jetbrains_mono(ui);
    let id = egui::Id::new("cellworks-launcher-preview");
    let mut y = inner.top();

    if !words.preview_eyebrow.is_empty() {
        let eyebrow = galley(
            ui,
            styled(words.preview_eyebrow, 11.0, sans.clone(), DIM_INK, 0.16),
        );
        y += eyebrow.size().y + 6.0;
        text_at(
            ui,
            id.with("eyebrow"),
            egui::pos2(inner.left(), inner.top()),
            eyebrow,
            words.preview_eyebrow,
        );
    }
    let name = galley(
        ui,
        one_row(
            styled(&project.name.to_uppercase(), 20.0, sans.clone(), INK, 0.06),
            inner.width(),
        ),
    );
    let name_height = name.size().y;
    text_at(
        ui,
        id.with("name"),
        egui::pos2(inner.left(), y),
        name,
        &project.name,
    );
    y += name_height + 6.0;

    let kind = match launcher.kind(&project.id) {
        ProjectKind::Repository => words.local_repo_kind,
        // "folder"/"block" names both a branch-less project's row and what
        // the inspector calls it — one word, not two (Decision 1).
        ProjectKind::Folder => words.branchless,
        ProjectKind::Remote => words.remote_kind,
    };
    // The location is where the project lives — this Mac or the host's name
    // — not the host's connection state, which its chip carries.
    let kind_text = format!("{kind} · {}", launcher.location_of(&project.id));
    let kind_galley = galley(
        ui,
        one_row(
            styled(
                &kind_text,
                12.0,
                egui::FontFamily::Proportional,
                DIM_INK,
                0.0,
            ),
            inner.width(),
        ),
    );
    let kind_height = kind_galley.size().y;
    text_at(
        ui,
        id.with("kind"),
        egui::pos2(inner.left(), y),
        kind_galley,
        &kind_text,
    );
    y += kind_height + PREVIEW_GAP + 4.0;

    let files = launcher
        .details_of(&project.id)
        .and_then(|details| details.files)
        .map(|files| {
            (
                format!("{files} file{}", if files == 1 { "" } else { "s" }),
                thousands(files),
            )
        });
    let path = launcher.path_of(project);
    let branch = launcher.branch_label(&project.id);
    let terms: [(&str, Option<(String, String)>); 3] = [
        ("Path", Some((path.clone(), path))),
        ("Branch", branch.map(|branch| (branch.clone(), branch))),
        ("Files", files),
    ];
    for (term, value) in terms {
        let term_galley = galley(
            ui,
            styled(&term.to_uppercase(), 10.0, sans.clone(), DIM_INK, 0.14),
        );
        let (label, shown) = value.unwrap_or_else(|| (String::new(), "—".to_owned()));
        let value_galley = galley(
            ui,
            one_row(
                styled(&shown, 12.0, mono.clone(), INK, 0.0),
                inner.width() - TERM_COLUMN - COLUMN_GAP,
            ),
        );
        let row_height = value_galley.size().y.max(term_galley.size().y);
        let row = egui::Rect::from_min_size(
            egui::pos2(inner.left(), y),
            egui::vec2(inner.width(), row_height),
        );
        text_at(
            ui,
            id.with(term),
            left_centred(row, inner.left(), &term_galley),
            term_galley,
            term,
        );
        let value_pos = left_centred(row, inner.left() + TERM_COLUMN + COLUMN_GAP, &value_galley);
        if label.is_empty() {
            ui.painter().galley(value_pos, value_galley, INK);
        } else {
            text_at(
                ui,
                id.with((term, "value")),
                value_pos,
                value_galley,
                &label,
            );
        }
        y += row_height + TERM_ROW_GAP;
    }

    // From the bottom up: the sync control, the CTA, then the ICE card.
    let sync = egui::Rect::from_min_max(
        egui::pos2(inner.left(), inner.bottom() - SYNC_HEIGHT),
        inner.max,
    );
    let cta = egui::Rect::from_min_size(
        egui::pos2(inner.left(), sync.top() - PREVIEW_GAP - CTA_HEIGHT),
        egui::vec2(inner.width(), CTA_HEIGHT),
    );
    if let Some(host) = project.id.host()
        && launcher
            .hosts()
            .into_iter()
            .any(|(name, state)| name == host && state == HostState::Offline)
    {
        ice_card(
            ui,
            id.with("ice"),
            inner.left()..=inner.right(),
            cta.top() - PREVIEW_GAP,
            words,
            host,
        );
    }

    let can_open = launcher.can_open(&project.id);
    let label = launcher.open_label(&project.id);
    let subtitle = if label == words.connect_first {
        label.to_owned()
    } else if words.open_subtitle.is_empty() {
        String::new()
    } else {
        format!("{} {}", words.open_subtitle, project.name)
    };
    let open = cta_button(
        ui,
        id.with("open"),
        cta,
        can_open,
        words.open_action,
        &subtitle,
        label,
        elapsed,
        reduced,
    );
    let open = if can_open {
        open
    } else if project.id.host().is_some() && label == words.open_label(true) {
        // Decision 1: remote opening is not built, even on a connected host,
        // though the label alone reads as if Enter would work — keyed on the
        // project (a remote one whose label happens to be the connected
        // one), not on a hardcoded legacy string.
        open.on_disabled_hover_text("Remote projects open in a later story")
    } else {
        open
    };

    sync_control(ui, id.with("sync"), sync, words.sync_button);

    open.clicked()
        .then(|| LauncherAction::Opened(project.clone()))
}

/// `n` with a comma between each group of three digits ("2,431").
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The 72 px CTA (STORY-083 requirement 4): `title` over `subtitle` and a
/// "⏎" key, on a filled chamfered yellow block with a breathing glow (design
/// requirement 5) when the project can be opened, or an outline block in dim
/// ink when it cannot. Its node is a button that reads `label`. `elapsed` and
/// `reduced` drive [`breath`], the same way [`rise`] is driven elsewhere in
/// this view.
#[allow(clippy::too_many_arguments)]
fn cta_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    can_open: bool,
    title: &str,
    subtitle: &str,
    label: &str,
    elapsed: f32,
    reduced: bool,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            if !can_open {
                ui.disable();
            }
            ui.interact(rect, id, egui::Sense::click())
        })
        .inner;
    labelled(&response, egui::WidgetType::Button, label);

    let points = chamfer(rect, CTA_CUT, Corners::TopLeftBottomRight);
    let text_color = if can_open { BG } else { DIM_INK };
    if can_open {
        // The glow: expanded chamfer outlines of falling alpha (epaint has
        // no blur), scaled by the breath factor (Decision 6).
        let factor = breath(elapsed, reduced);
        for step in 0..18 {
            let expand = step as f32 + 0.5;
            let fade = 1.0 - step as f32 / 18.0;
            ui.painter().add(egui::Shape::closed_line(
                chamfer(
                    rect.expand(expand),
                    CTA_CUT + expand,
                    Corners::TopLeftBottomRight,
                )
                .to_vec(),
                egui::Stroke::new(1.0, YELLOW.gamma_multiply(0.45 * factor * fade * fade)),
            ));
        }
        ui.painter().add(egui::Shape::convex_polygon(
            points.to_vec(),
            YELLOW,
            egui::Stroke::NONE,
        ));
    } else {
        ui.painter().add(egui::Shape::convex_polygon(
            points.to_vec(),
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, LINE_BRIGHT),
        ));
    }

    let title = galley(
        ui,
        styled(
            &title.to_uppercase(),
            15.0,
            chakra_petch(ui),
            text_color,
            0.14,
        ),
    );
    let subtitle = (!subtitle.is_empty()).then(|| {
        galley(
            ui,
            styled(
                subtitle,
                12.0,
                egui::FontFamily::Proportional,
                text_color.gamma_multiply(0.8),
                0.0,
            ),
        )
    });
    let height = title.size().y + subtitle.as_ref().map_or(0.0, |s| s.size().y + 2.0);
    let top = rect.center().y - height / 2.0;
    let left = rect.left() + 20.0;
    let title_height = title.size().y;
    ui.painter()
        .galley(egui::pos2(left, top), title, text_color);
    if let Some(subtitle) = subtitle {
        ui.painter().galley(
            egui::pos2(left, top + title_height + 2.0),
            subtitle,
            text_color,
        );
    }
    let key = galley(ui, styled("⏎", 16.0, jetbrains_mono(ui), text_color, 0.0));
    let pos = egui::pos2(
        rect.right() - 20.0 - key.size().x,
        rect.center().y - key.size().y / 2.0,
    );
    ui.painter().galley(pos, key, text_color);
    response
}

/// The disabled 44 px sync control, an outline across the pane's width.
fn sync_control(ui: &mut egui::Ui, id: egui::Id, rect: egui::Rect, label: &str) {
    let response = ui
        .scope(|ui| {
            ui.disable();
            ui.interact(rect, id, egui::Sense::click())
        })
        .inner;
    labelled(&response, egui::WidgetType::Button, label);
    ui.painter().add(egui::Shape::convex_polygon(
        chamfer(rect, CONTROL_CUT, Corners::TopLeftBottomRight).to_vec(),
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, LINE),
    ));
    let text = galley(
        ui,
        styled(&label.to_uppercase(), 11.0, chakra_petch(ui), DIM_INK, 0.12),
    );
    let pos = rect.center() - text.size() / 2.0;
    ui.painter().galley(pos, text, DIM_INK);
}

/// The orange ICE/offline card (STORY-083 requirement 4), spanning `x` with
/// its bottom edge at `bottom`, shown above the CTA when the selected
/// project's host is offline.
fn ice_card(
    ui: &egui::Ui,
    id: egui::Id,
    x: std::ops::RangeInclusive<f32>,
    bottom: f32,
    words: &Words,
    host: &str,
) {
    let (left, right) = (*x.start(), *x.end());
    let padding = egui::vec2(14.0, 12.0);
    let heading = galley(
        ui,
        styled(words.ice_heading, 11.0, chakra_petch(ui), WARN_ORANGE, 0.16),
    );
    let body_text = words.offline_body(host);
    let mut body_job = styled(&body_text, 12.0, egui::FontFamily::Proportional, INK, 0.0);
    body_job.wrap.max_width = right - left - 2.0 * padding.x - 3.0;
    body_job.sections[0].format.line_height = Some(18.0);
    let body = galley(ui, body_job);
    let height = 2.0 * padding.y + heading.size().y + 4.0 + body.size().y;
    let rect =
        egui::Rect::from_min_max(egui::pos2(left, bottom - height), egui::pos2(right, bottom));
    ui.painter().add(egui::Shape::convex_polygon(
        chamfer(rect, CTA_CUT, Corners::TopLeftBottomRight).to_vec(),
        WARN_ORANGE.gamma_multiply(0.08),
        egui::Stroke::new(1.0, WARN_ORANGE.gamma_multiply(0.35)),
    ));
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(left, rect.top() + CTA_CUT),
            egui::pos2(left + 3.0, rect.bottom()),
        ),
        0.0,
        WARN_ORANGE,
    );
    let heading_height = heading.size().y;
    let text_left = left + 3.0 + padding.x;
    text_at(
        ui,
        id.with("heading"),
        egui::pos2(text_left, rect.top() + padding.y),
        heading,
        words.ice_heading,
    );
    text_at(
        ui,
        id.with("body"),
        egui::pos2(text_left, rect.top() + padding.y + heading_height + 4.0),
        body,
        &body_text,
    );
}

/// The disabled add-project control (requirement 1); ⌘N does nothing yet.
/// Drawn upper-case with its shortcut in a key box; its node is a button
/// that reads `label` itself.
fn add_project_button(ui: &mut egui::Ui, label: &str) {
    let (word, key) = label.rsplit_once(' ').unwrap_or((label, ""));
    let text = galley(
        ui,
        styled(&word.to_uppercase(), 11.0, chakra_petch(ui), DIM_INK, 0.1),
    );
    let key =
        (!key.is_empty()).then(|| galley(ui, styled(key, 11.0, jetbrains_mono(ui), DIM_INK, 0.0)));
    let width =
        12.0 + text.size().x + key.as_ref().map_or(0.0, |key| 8.0 + key.size().x + 10.0) + 12.0;
    let rect = ui
        .add_enabled_ui(false, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, CONTROL_HEIGHT), egui::Sense::click());
            labelled(&response, egui::WidgetType::Button, label);
            response.on_disabled_hover_text("Coming soon");
            rect
        })
        .inner;
    // Painted outside the disabled scope: the mock draws the control at full
    // strength, already dim ink on a strong line, not faded a second time.
    ui.painter().add(egui::Shape::convex_polygon(
        chamfer(rect, CONTROL_CUT, Corners::TopLeftBottomRight).to_vec(),
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, LINE_BRIGHT),
    ));
    let text_pos = left_centred(rect, rect.left() + 12.0, &text);
    let text_right = text_pos.x + text.size().x;
    ui.painter().galley(text_pos, text, DIM_INK);
    if let Some(key) = key {
        key_cap(ui, text_right + 8.0, rect.center().y, key);
    }
}

/// What an empty launcher tells the engineer.
///
/// The settings file is named because, until EPIC-008 brings the interface
/// for adding a folder, hand-editing it is the only way to add one. Product
/// owner, 2026-10-08.
fn empty_state(sources: Option<&Sources>) -> String {
    match sources {
        Some(sources) => format!(
            "No projects yet. Put a git repository in {}, or add a folder's path to {}.",
            sources.projects_folder.display(),
            sources.settings_file.display(),
        ),
        // There is no projects folder and no settings file to name: the home
        // directory this machine would take them from is unset.
        None => "No projects yet. Cellworks cannot find your home directory, \
                 so it has no projects folder to scan."
            .to_owned(),
    }
}

/// A failure, written beside the folder or the file it belongs to. Only a
/// skipped host's wording moves with the theme words (design Decision 4);
/// the other three stay one string either way.
fn notice_text(words: &Words, notice: &Notice) -> String {
    let path = notice.path.display();
    let reason = &notice.reason;

    match notice.kind {
        NoticeKind::ProjectsFolderUnreadable => format!("Could not read {path}: {reason}"),
        NoticeKind::AddedFolderSkipped => format!("Skipped the added folder {path}: {reason}"),
        NoticeKind::SettingsUnreadable => {
            format!("Could not read the settings file {path}: {reason}")
        }
        NoticeKind::HostSkipped => words.notice(&path.to_string(), reason),
    }
}

/// A notice below the card, a full card's width: a panel with an orange
/// left bar, an orange ▲ and the text in ink (STORY-083 requirement 4). Its
/// node reads "▲ " and the text.
fn notice_label(ui: &mut egui::Ui, rect: egui::Rect, words: &Words, notice: &Notice) {
    let text = notice_text(words, notice);
    ui.painter().rect_filled(rect, 0.0, PANEL);
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, LINE),
        egui::StrokeKind::Inside,
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 3.0, rect.bottom())),
        0.0,
        WARN_ORANGE,
    );
    let mut job = styled("▲", 12.0, egui::FontFamily::Proportional, WARN_ORANGE, 0.0);
    job.append(
        &text,
        10.0,
        egui::TextFormat {
            font_id: egui::FontId::new(12.0, egui::FontFamily::Proportional),
            color: INK,
            ..Default::default()
        },
    );
    let galley = galley(ui, one_row(job, rect.width() - 2.0 * SEARCH_PADDING));
    let id = egui::Id::new("cellworks-launcher-notice").with(&text);
    let pos = left_centred(rect, rect.left() + SEARCH_PADDING, &galley);
    text_at(ui, id, pos, galley, &format!("▲ {text}"));
}

// STORY-083 below: tests for the Netrunner look's pure geometry and motion
// helpers (design Order of work 2 and 3) — `chamfer`, `rise` and `breath`,
// defined above, and `Corners`.

#[cfg(test)]
mod tests {
    use super::*;

    /// `chamfer`'s six points stay on or inside the rect they cut, whichever
    /// pair of opposite corners is named.
    fn assert_points_stay_inside(rect: egui::Rect, points: &[egui::Pos2; 6]) {
        for point in points {
            assert!(
                point.x >= rect.min.x - f32::EPSILON && point.x <= rect.max.x + f32::EPSILON,
                "{point:?} is within {rect:?} on x"
            );
            assert!(
                point.y >= rect.min.y - f32::EPSILON && point.y <= rect.max.y + f32::EPSILON,
                "{point:?} is within {rect:?} on y"
            );
        }
    }

    /// Chamfering the top-right and bottom-left corners (requirement 3's card
    /// cut): the top-left and bottom-right corners of the rect are kept
    /// exactly, and neither cut corner appears — each is replaced by two
    /// points offset by `cut` along its two edges.
    #[test]
    fn chamfer_cuts_the_named_corners_and_keeps_the_other_two() {
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0));
        let cut = 10.0;

        let points = chamfer(rect, cut, Corners::TopRightBottomLeft);

        assert_points_stay_inside(rect, &points);
        assert!(
            points.contains(&rect.left_top()),
            "the uncut top-left corner is kept exactly: {points:?}"
        );
        assert!(
            points.contains(&rect.right_bottom()),
            "the uncut bottom-right corner is kept exactly: {points:?}"
        );
        assert!(
            !points.contains(&rect.right_top()),
            "the cut top-right corner never appears as a point: {points:?}"
        );
        assert!(
            !points.contains(&rect.left_bottom()),
            "the cut bottom-left corner never appears as a point: {points:?}"
        );
    }

    /// The other named pair (the CTA and ICE card's cut, requirement 3):
    /// top-right and bottom-left are kept, top-left and bottom-right are cut.
    #[test]
    fn chamfer_cuts_the_other_named_pair_of_corners() {
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0));
        let cut = 10.0;

        let points = chamfer(rect, cut, Corners::TopLeftBottomRight);

        assert_points_stay_inside(rect, &points);
        assert!(
            points.contains(&rect.right_top()),
            "the uncut top-right corner is kept exactly: {points:?}"
        );
        assert!(
            points.contains(&rect.left_bottom()),
            "the uncut bottom-left corner is kept exactly: {points:?}"
        );
        assert!(
            !points.contains(&rect.left_top()),
            "the cut top-left corner never appears as a point: {points:?}"
        );
        assert!(
            !points.contains(&rect.right_bottom()),
            "the cut bottom-right corner never appears as a point: {points:?}"
        );
    }

    /// Design requirement 5 / Decision 7: with reduced motion, the card is
    /// always fully risen and opaque, whatever `elapsed` or `delay` are.
    #[test]
    fn rise_is_settled_at_once_when_reduced() {
        assert_eq!(rise(0.0, 0.0, true), (0.0, 1.0));
        assert_eq!(rise(10.0, 0.5, true), (0.0, 1.0));
    }

    /// Design requirement 5: the card starts 18 px low and invisible, and is
    /// fully risen 550 ms after its delay; in between it has moved and faded
    /// in only partway.
    #[test]
    fn rise_starts_low_and_settles_550ms_after_its_delay() {
        let delay = 0.09; // the list's +90 ms stagger.

        assert_eq!(
            rise(0.0, delay, false),
            (18.0, 0.0),
            "before the delay, not yet risen"
        );
        assert_eq!(
            rise(delay, delay, false),
            (18.0, 0.0),
            "exactly at the delay, still the start state"
        );
        assert_eq!(
            rise(delay + 0.550, delay, false),
            (0.0, 1.0),
            "550ms after the delay, settled"
        );
        assert_eq!(
            rise(delay + 10.0, delay, false),
            (0.0, 1.0),
            "well past settling, stays settled"
        );

        let (offset, opacity) = rise(delay + 0.275, delay, false);
        assert!(
            offset > 0.0 && offset < 18.0,
            "midway through the rise, partly risen: {offset}"
        );
        assert!(
            opacity > 0.0 && opacity < 1.0,
            "midway through the fade-in, partly visible: {opacity}"
        );
    }

    /// Design Decision 7: with reduced motion, the CTA's glow never
    /// breathes — the multiplier is constant, whatever `elapsed` is.
    #[test]
    fn breath_is_constant_when_reduced() {
        let value = breath(0.0, true);

        assert_eq!(breath(0.4, true), value);
        assert_eq!(breath(5.0, true), value);
    }

    /// Design requirement 5: the enabled CTA's glow breathes over 1.6 s —
    /// periodic with that period, and actually varying within it rather than
    /// a constant multiplier.
    #[test]
    fn breath_is_periodic_over_1_6_seconds_and_varies_within_it() {
        let t = 0.2;

        assert!(
            (breath(t, false) - breath(t + 1.6, false)).abs() < 1e-4,
            "one period later, the same phase: {} vs {}",
            breath(t, false),
            breath(t + 1.6, false)
        );
        assert_ne!(
            breath(0.0, false),
            breath(0.4, false),
            "the glow actually varies over the period, not a constant"
        );
    }
}
