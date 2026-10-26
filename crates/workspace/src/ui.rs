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
use crate::launcher::{HostState, Launcher, ProjectKind, match_range};
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

/// The card's footer hints (STORY-082 requirement 9).
const HINTS: &str = "↑ ↓ move   Enter open   Tab scope   Esc clear search";

/// The centred card's max width (STORY-082 requirement 1): wide enough for
/// the mockup's layout, narrow enough to stay one card on a wider window.
const CARD_WIDTH: f32 = 920.0;

/// The preview pane's width, to the right of the list (STORY-082
/// requirement 7).
const PREVIEW_WIDTH: f32 = 300.0;

/// The `egui::Id` the search field is given, so [`search_focused`] can check
/// the same id `search_row` builds its `TextEdit` with.
const SEARCH_ID: &str = "cellworks-launcher-search";

/// Whether the launcher's search field currently has keyboard focus, so the
/// caller can suppress the map's letter shortcuts while a project name is
/// typed into it. The field is focused from the first frame (requirement 2),
/// so this is the launcher's ordinary state, not a corner of it — `src/app.rs`
/// reads the keys before this module draws, so only the id tells it apart.
pub fn search_focused(ui: &egui::Ui) -> bool {
    ui.memory(|memory| memory.has_focus(egui::Id::new(SEARCH_ID)))
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

/// Draws the launcher: the header above the card, the centred card (search
/// row, location chip row and the project list), the preview pane to its
/// right, the footer and the notices below the card. Mutates `launcher`
/// directly for anything that stays within it (search, pick, selection by
/// click or arrow key); returns `Some` only when the engineer asked to open a
/// project.
pub fn launcher(
    ui: &mut egui::Ui,
    universe: &Universe,
    launcher: &mut Launcher,
) -> Option<LauncherAction> {
    paint_background(ui);
    let mut action = None;

    // Decision 7: the view treats `animation_time == 0.0` as reduced motion
    // rather than threading a flag of its own — `src/app.rs` zeroes it
    // globally once, at start-up, when the system asks for reduced motion.
    let reduced = ui.style().animation_time == 0.0;
    let now = ui.input(|input| input.time) as f32;
    // One shared clock for the card's, the list's and the preview pane's
    // rise (design requirement 5's "+90 ms", "+180 ms" stagger), read once
    // here rather than separately by each.
    let elapsed = first_shown(ui, egui::Id::new(LAUNCHER_SHOWN_ID), now);

    egui::Panel::top("launcher-header")
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(16, 12)),
        )
        .show(ui, |ui| header(ui, launcher));
    egui::Panel::bottom("launcher-footer")
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(16, 8)),
        )
        .show(ui, |ui| footer(ui, launcher));
    // The preview pane keeps its width while nothing is selected: if it
    // appeared only on selection, the click that selects a project would
    // narrow the list, a group heading could wrap, and the row would move
    // from under the second click of a double-click.
    let selected = launcher.selected_project().cloned();
    let opened = egui::Panel::right("launcher-inspector")
        .exact_size(PREVIEW_WIDTH)
        .resizable(false)
        .frame(
            egui::Frame::NONE
                .fill(PANEL)
                .inner_margin(egui::Margin::symmetric(16, 16)),
        )
        .show(ui, |ui| {
            selected.as_ref().and_then(|project| {
                // The preview pane rises in last, +180 ms (design
                // requirement 5).
                let (offset, opacity) = rise(elapsed, 0.18, reduced);
                ui.with_visual_transform(
                    egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset)),
                    |ui| {
                        ui.multiply_opacity(opacity);
                        inspector(ui, launcher, project, elapsed, reduced)
                    },
                )
                .inner
            })
        })
        .inner;
    if opened.is_some() {
        action = opened;
    }

    let central = egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            // `vertical_centered` would centre on a negative offset, pushing
            // the card half off-screen to the left, if the window is
            // narrower than `CARD_WIDTH` once the preview pane has taken its
            // share — so the cap is never wider than what is actually here.
            let width = CARD_WIDTH.min(ui.available_width());
            let card_action = ui
                .vertical_centered(|ui| {
                    ui.set_max_width(width);
                    // The card rises in with no delay of its own (design
                    // requirement 5).
                    let (offset, opacity) = rise(elapsed, 0.0, reduced);
                    ui.with_visual_transform(
                        egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset)),
                        |ui| {
                            ui.multiply_opacity(opacity);
                            card(ui, universe, launcher, elapsed, reduced)
                        },
                    )
                    .inner
                })
                .inner;
            // Below the card, not inside the scrollable list, so a notice is
            // never scrolled out of sight (requirement 11).
            let words = launcher.words();
            for notice in universe.notices() {
                notice_label(ui, words, notice);
            }
            card_action
        })
        .inner;
    if action.is_none() {
        action = central;
    }

    // Decision 8: a scheduled repaint, never an immediate one, so a headless
    // harness's single `run()` call settles — and never scheduled at all
    // when reduced, so a reduced-motion harness settles with nothing left to
    // repaint. While the card, the list or the preview pane (the preview's
    // +180 ms delay is the largest) is still rising, or the CTA is shown and
    // breathing, another frame is due in about 1/30 s.
    if !reduced {
        let still_rising = elapsed < 0.18 + RISE_DURATION;
        let cta_breathing = selected
            .as_ref()
            .is_some_and(|project| launcher.can_open(&project.id));
        if still_rising || cta_breathing {
            ui.ctx().request_repaint_after_secs(1.0 / 30.0);
        }
    }

    action
}

/// The background behind the header, the footer, the preview pane and the
/// floating card: near-black with a faint 100 px grid (design requirement
/// 1). The rest of the app keeps its current visuals — only this view paints
/// it.
fn paint_background(ui: &egui::Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, BG);
    let step = 100.0;
    let mut x = rect.left();
    while x <= rect.right() {
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, GRID_LINE),
        );
        x += step;
    }
    let mut y = rect.top();
    while y <= rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, GRID_LINE),
        );
        y += step;
    }
}

/// "Cellworks" (Decision 4: one string, in or out of theme words), the
/// eyebrow line, the project and host counts and the disabled add-project
/// button (STORY-082 requirement 1, STORY-083 requirements 1, 2 and 7/8).
fn header(ui: &mut egui::Ui, launcher: &Launcher) {
    let words = launcher.words();
    ui.vertical(|ui| {
        if !words.header_eyebrow.is_empty() {
            ui.label(
                egui::RichText::new(words.header_eyebrow)
                    .font(egui::FontId::new(11.0, chakra_petch(ui)))
                    .color(CYAN),
            );
        }
        ui.horizontal(|ui| {
            ui.label(cellworks_title(ui));
            let chips = launcher.location_chips();
            let projects = chips.first().map_or(0, |chip| chip.count);
            // Every chip but "All" and "This Mac" is a remote host.
            let hosts = chips.len().saturating_sub(2);
            ui.label(
                egui::RichText::new(words.counts(projects, hosts))
                    .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
                    .color(DIM_INK),
            );
            add_project_button(ui, words.add_project_button);
        });
    });
}

/// "CELLWORKS", with "WORKS" in attention yellow and "CELL" in ink (the
/// confirmed canvas look) — one string either way theme words turn, so it is
/// built here rather than read from [`Words`] (Decision 4).
fn cellworks_title(ui: &egui::Ui) -> egui::text::LayoutJob {
    let font = egui::FontId::new(20.0, chakra_petch(ui));
    let mut job = egui::text::LayoutJob::default();
    job.append(
        "CELL",
        0.0,
        egui::TextFormat {
            font_id: font.clone(),
            color: INK,
            ..Default::default()
        },
    );
    job.append(
        "WORKS",
        0.0,
        egui::TextFormat {
            font_id: font,
            color: YELLOW,
            ..Default::default()
        },
    );
    job
}

/// The status bar's cyan mode badge — "NAV" and the status text, or "CMD" and
/// the live query while searching (STORY-083 requirement 4) — followed by
/// STORY-082's own key hints (Decision 4: `HINTS` is one string).
fn footer(ui: &mut egui::Ui, launcher: &Launcher) {
    let words = launcher.words();
    ui.horizontal(|ui| {
        let (mode, text) = if launcher.search_text().is_empty() {
            ("NAV", words.status_text.to_owned())
        } else {
            ("CMD", launcher.search_text().to_owned())
        };
        ui.label(
            egui::RichText::new(mode)
                .font(egui::FontId::new(11.0, chakra_petch(ui)))
                .color(CYAN),
        );
        ui.label(egui::RichText::new(text).color(DIM_INK));
        ui.label(
            egui::RichText::new(HINTS)
                .font(egui::FontId::new(12.0, jetbrains_mono(ui)))
                .color(DIM_INK),
        );
    });
}

/// The card's content: the search row, the location chip row and the
/// scrollable list (STORY-082 requirements 2 to 6, 10 and 11), behind a
/// chamfered panel cut top-right and bottom-left (STORY-083 requirement 3).
/// The list rises in +90 ms after the card itself (design requirement 5);
/// `elapsed` and `reduced` are [`launcher`]'s own shared clock and
/// reduced-motion reading, passed down rather than read again here.
fn card(
    ui: &mut egui::Ui,
    universe: &Universe,
    launcher: &mut Launcher,
    elapsed: f32,
    reduced: bool,
) -> Option<LauncherAction> {
    let background = ui.painter().add(egui::Shape::Noop);
    search_row(ui, launcher);
    chip_row(ui, launcher);
    let (offset, opacity) = rise(elapsed, 0.09, reduced);
    let action = ui
        .with_visual_transform(
            egui::emath::TSTransform::from_translation(egui::vec2(0.0, offset)),
            |ui| {
                ui.multiply_opacity(opacity);
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| list(ui, universe, launcher))
                    .inner
            },
        )
        .inner;
    let points = chamfer(ui.min_rect(), 12.0, Corners::TopRightBottomLeft);
    ui.painter().set(
        background,
        egui::Shape::convex_polygon(points.to_vec(), PANEL, egui::Stroke::new(1.0, LINE)),
    );
    action
}

/// The search field, focused on the first frame and again on ⌘K; while
/// searching, the match count and the "Esc clear" hint (STORY-082
/// requirement 2). A deep well with a cyan edge when focused, a cyan ":"
/// prompt before the text, and a yellow caret (STORY-083 requirements 4 and
/// 5) that blinks or not exactly as egui's native text cursor already does —
/// `src/app.rs` makes that reduced-motion-aware globally, so only its colour
/// is set here, scoped to this field alone.
fn search_row(ui: &mut egui::Ui, launcher: &mut Launcher) {
    let words = launcher.words();
    let search_id = egui::Id::new(SEARCH_ID);
    let focused = search_focused(ui);
    egui::Frame::NONE
        .fill(WELL)
        .stroke(egui::Stroke::new(1.0, if focused { CYAN } else { LINE }))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.visuals_mut().text_cursor.stroke.color = YELLOW;
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(":")
                        .font(egui::FontId::new(14.0, jetbrains_mono(ui)))
                        .color(CYAN),
                );
                let mut text = launcher.search_text().to_owned();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        // Tab scopes the list (requirement 4), so the field
                        // keeps the focus on it: egui decides the focus
                        // direction from the raw input before the key block
                        // runs, and `consume_key` there cannot take that
                        // decision back. `lock_focus` only tells egui's
                        // focus pass to leave this widget alone — a
                        // single-line field inserts no tab character, and
                        // the key block has consumed the key before the
                        // field reads its events anyway.
                        .lock_focus(true)
                        .id(search_id)
                        .frame(egui::Frame::NONE)
                        .font(egui::FontId::new(14.0, jetbrains_mono(ui)))
                        .text_color(INK)
                        .hint_text(words.search_placeholder),
                );
                if response.changed() {
                    launcher.search(text);
                }
                if !launcher.search_text().is_empty() {
                    ui.label(
                        egui::RichText::new(format!("{} matches", launcher.match_count()))
                            .color(CYAN),
                    );
                    ui.label(egui::RichText::new("Esc clear").color(DIM_INK));
                }
            });
        });
    if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::K)) {
        launcher.request_focus();
    }
    if launcher.take_focus_request() {
        ui.memory_mut(|memory| memory.request_focus(search_id));
    }
}

/// The location chip row: "All", "This Mac", then one chip per remote host
/// (STORY-082 requirement 3), replacing STORY-078's left locations column.
/// Clicking a chip picks it; Tab cycles it (`src/app.rs`'s key block, not
/// here). Chamfered pills (STORY-083 requirement 3); the picked chip gets a
/// soft-yellow fill with yellow text and edge (requirement 4).
fn chip_row(ui: &mut egui::Ui, launcher: &mut Launcher) {
    let picked = launcher.picked().clone();
    let words = launcher.words();
    ui.horizontal(|ui| {
        for chip in launcher.location_chips() {
            let is_picked = chip.pick == picked;
            let text_color = if is_picked { YELLOW } else { INK };
            let dim_color = if is_picked { YELLOW } else { DIM_INK };
            let background = ui.painter().add(egui::Shape::Noop);
            let group = ui.horizontal(|ui| {
                // The state's glyph and word together (requirement 3's "a
                // glyph paired with a word, never colour alone"): the glyph
                // is painted, the word is the only text, which is what
                // ASSUMED (Q-1) requires a label to carry.
                if let Some(state) = chip.state {
                    state_glyph(ui, state);
                }
                let name = ui
                    .label(
                        egui::RichText::new(chip.label.as_str())
                            .font(egui::FontId::new(13.0, chakra_petch(ui)))
                            .color(text_color),
                    )
                    .interact(egui::Sense::click());
                if let Some(state) = chip.state {
                    ui.label(egui::RichText::new(words.host_state_word(state)).color(dim_color));
                }
                ui.label(
                    egui::RichText::new(chip.count.to_string())
                        .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
                        .color(dim_color),
                );
                name
            });
            let rect = group.response.rect.expand(6.0);
            let points = chamfer(rect, 8.0, Corners::TopLeftBottomRight);
            let (fill, stroke) = if is_picked {
                (YELLOW_SOFT, egui::Stroke::new(1.0, YELLOW))
            } else {
                (egui::Color32::TRANSPARENT, egui::Stroke::new(1.0, LINE))
            };
            ui.painter().set(
                background,
                egui::Shape::convex_polygon(points.to_vec(), fill, stroke),
            );
            if group.inner.clicked() {
                launcher.pick(chip.pick.clone());
            }
            ui.add_space(4.0);
        }
    });
}

/// A host chip's state glyph, painted beside the word rather than written
/// into it (ASSUMED (Q-1)): a filled dot for a connected host, a ring around
/// a dot for one still being checked, an empty ring for an offline one. The
/// shape differs as well as the colour, and the word beside it names the
/// state, so the glyph never carries the meaning alone. Connected is ok
/// green, checking is cyan, offline is dim ink (STORY-083's confirmed
/// behaviour).
fn state_glyph(ui: &mut egui::Ui, state: HostState) {
    let size = ui.text_style_height(&egui::TextStyle::Body) * 0.5;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let centre = rect.center();
    let radius = size / 2.0;
    let color = match state {
        HostState::Connected => OK_GREEN,
        HostState::Checking => CYAN,
        HostState::Offline => DIM_INK,
    };
    let stroke = egui::Stroke::new(1.0, color);
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
            painter.circle_stroke(centre, radius, stroke);
        }
    }
}

/// The recent row, then the scanning/failed/empty-state/grouped list
/// (STORY-078's states, kept per STORY-082 requirement 11).
fn list(ui: &mut egui::Ui, universe: &Universe, launcher: &mut Launcher) -> Option<LauncherAction> {
    let mut action = None;
    let words = launcher.words();

    let recent: Vec<Project> = launcher.recent_row().into_iter().cloned().collect();
    if !recent.is_empty() {
        group_heading_label(ui, words.recent_heading);
        ui.horizontal(|ui| {
            for project in &recent {
                ui.label(format!("recent: {}", project.name));
            }
        });
    }

    match universe.scan() {
        ScanState::Scanning => {
            ui.label(SCANNING);
        }
        ScanState::Failed => {
            ui.label(FAILED);
        }
        ScanState::Reported(projects) if projects.is_empty() => {
            ui.label(empty_state(universe.sources()));
        }
        ScanState::Reported(_) => {
            let groups: Vec<(String, Vec<Project>)> = launcher
                .groups()
                .into_iter()
                .map(|(label, projects)| (label, projects.into_iter().cloned().collect()))
                .collect();
            if groups.is_empty() && !launcher.search_text().is_empty() {
                ui.label(format!(
                    "No project matches \"{}\".",
                    launcher.search_text()
                ));
                add_project_button(ui, words.add_project_button);
            } else {
                let selected = launcher.selected().cloned();
                for (label, projects) in &groups {
                    // One group per location, named by `Launcher::groups`
                    // (requirement 5): "This Mac" once for every local
                    // project, then a group per host — restated in theme
                    // words through `group_heading`, since `groups()` itself
                    // is unchanged (design Order of work step 2).
                    group_heading_label(ui, &group_heading(launcher, label));
                    for project in projects {
                        if let Some(click) = project_row(ui, launcher, project, selected.as_ref()) {
                            match click {
                                RowClick::Selected => {
                                    launcher.select(Some(project.id.clone()));
                                    action = Some(LauncherAction::Selected(project.clone()));
                                }
                                RowClick::Opened => {
                                    action = Some(LauncherAction::Opened(project.clone()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    action
}

/// A group heading, drawn as an eyebrow with a rule under it (STORY-083
/// requirement 4).
fn group_heading_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(egui::FontId::new(12.0, chakra_petch(ui)))
            .color(DIM_INK),
    );
    ui.separator();
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

/// One project's row: its name (underlined in attention yellow where the
/// search matched it, yellow when selected), its path and its branch label.
/// Selected: a soft-yellow fill and a 2 px yellow left bar (STORY-083
/// requirement 4).
fn project_row(
    ui: &mut egui::Ui,
    launcher: &Launcher,
    project: &Project,
    selected: Option<&ProjectId>,
) -> Option<RowClick> {
    let mut click = None;
    let is_selected = selected == Some(&project.id);
    let background = ui.painter().add(egui::Shape::Noop);
    let row = ui.horizontal(|ui| {
        let name_color = if is_selected { YELLOW } else { INK };
        let name = name_job(&project.name, launcher.search_text(), name_color);
        // The sense goes on the response, not the widget, so the entry stays
        // a label in the accessibility tree.
        let entry = ui.label(name).interact(egui::Sense::click());
        if is_selected {
            ui.ctx()
                .accesskit_node_builder(entry.id, |node| node.set_selected(true));
        }
        // A remote project's name does not open on double-click
        // (requirement 8); a double-click there still selects it.
        if entry.double_clicked() && project.id.local_path().is_some() {
            click = Some(RowClick::Opened);
        } else if entry.clicked() {
            click = Some(RowClick::Selected);
        }
        ui.label(
            egui::RichText::new(launcher.path_of(project))
                .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
                .color(DIM_INK),
        );
        // Only once the branch is known: an unread row says nothing, rather
        // than calling every repository a folder before it is selected.
        if let Some(branch) = launcher.branch_label(&project.id) {
            ui.label(
                egui::RichText::new(branch)
                    .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
                    .color(DIM_INK),
            );
        }
    });
    if is_selected {
        let rect = row.response.rect.expand(4.0);
        ui.painter()
            .set(background, egui::Shape::rect_filled(rect, 0.0, YELLOW_SOFT));
        let bar = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 2.0, rect.bottom()),
        );
        ui.painter().rect_filled(bar, 0.0, YELLOW);
    } else {
        ui.painter().set(background, egui::Shape::Noop);
    }
    click
}

/// `name`, with the byte range `query` matches in it underlined in attention
/// yellow (STORY-082 requirement 6, STORY-083 requirement 4); plain text in
/// `color` when there is no match, so a path-only hit never underlines the
/// wrong fragment.
fn name_job(name: &str, query: &str, color: egui::Color32) -> egui::text::LayoutJob {
    let plain = egui::TextFormat {
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
                    color: YELLOW,
                    underline: egui::Stroke::new(1.0, YELLOW),
                    ..Default::default()
                },
            );
            job.append(&name[range.end..], 0.0, plain);
        }
        None => job.append(name, 0.0, plain),
    }
    job
}

/// `project`'s name, kind and location, path, branch, file count (when
/// known), a CTA and a disabled sync button (STORY-082 requirement 7,
/// STORY-083 requirement 4): an eyebrow above the name, and an orange
/// ICE/offline card below the CTA when the project's host is offline.
/// `elapsed` and `reduced` are [`launcher`]'s own shared clock and
/// reduced-motion reading, passed through only so the CTA's glow can breathe
/// by it (this panel's own rise is applied around the call to this function,
/// not inside it).
fn inspector(
    ui: &mut egui::Ui,
    launcher: &Launcher,
    project: &Project,
    elapsed: f32,
    reduced: bool,
) -> Option<LauncherAction> {
    let mut action = None;
    let words = launcher.words();

    if !words.preview_eyebrow.is_empty() {
        ui.label(
            egui::RichText::new(words.preview_eyebrow)
                .font(egui::FontId::new(11.0, chakra_petch(ui)))
                .color(CYAN),
        );
    }
    ui.label(
        egui::RichText::new(project.name.as_str())
            .font(egui::FontId::new(18.0, chakra_petch(ui)))
            .color(INK),
    );
    let kind = match launcher.kind(&project.id) {
        ProjectKind::Repository => words.local_repo_kind,
        // "folder"/"block" names both a branch-less project's row and what
        // the inspector calls it — one word, not two (Decision 1).
        ProjectKind::Folder => words.branchless,
        ProjectKind::Remote => words.remote_kind,
    };
    // The location is where the project lives — this Mac or the host's name
    // — not the host's connection state, which its chip carries.
    ui.label(
        egui::RichText::new(format!("{kind} · {}", launcher.location_of(&project.id)))
            .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
            .color(DIM_INK),
    );
    ui.label(
        egui::RichText::new(launcher.path_of(project))
            .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
            .color(DIM_INK),
    );
    if let Some(branch) = launcher.branch_label(&project.id) {
        ui.label(
            egui::RichText::new(branch)
                .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
                .color(DIM_INK),
        );
    }
    if let Some(details) = launcher.details_of(&project.id)
        && let Some(files) = details.files
    {
        ui.label(
            egui::RichText::new(format!("{files} file{}", if files == 1 { "" } else { "s" }))
                .font(egui::FontId::new(13.0, jetbrains_mono(ui)))
                .color(DIM_INK),
        );
    }

    ui.add_space(8.0);
    let can_open = launcher.can_open(&project.id);
    let label = launcher.open_label(&project.id);
    let open = cta_button(ui, can_open, label, elapsed, reduced);
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
    if open.clicked() {
        action = Some(LauncherAction::Opened(project.clone()));
    }

    ui.add_enabled(false, egui::Button::new(words.sync_button));

    if let Some(host) = project.id.host()
        && launcher
            .hosts()
            .into_iter()
            .any(|(name, state)| name == host && state == HostState::Offline)
    {
        ice_card(ui, words, host);
    }

    action
}

/// The 72 px CTA (STORY-083 requirement 4): a filled chamfered yellow block
/// with a breathing glow (design requirement 5) when the project can be
/// opened, or an outline block when it cannot. `elapsed` and `reduced` drive
/// [`breath`], the same way [`rise`] is driven elsewhere in this view.
fn cta_button(
    ui: &mut egui::Ui,
    can_open: bool,
    label: &str,
    elapsed: f32,
    reduced: bool,
) -> egui::Response {
    let text_color = if can_open { BG } else { INK };
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add_enabled(
        can_open,
        egui::Button::new(
            egui::RichText::new(label)
                .font(egui::FontId::new(16.0, chakra_petch(ui)))
                .color(text_color),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(ui.available_width(), 72.0)),
    );
    let rect = response.rect;
    let points = chamfer(rect, 10.0, Corners::TopLeftBottomRight);
    let (fill, stroke) = if can_open {
        (YELLOW, egui::Stroke::NONE)
    } else {
        (
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, LINE_BRIGHT),
        )
    };
    ui.painter().set(
        background,
        egui::Shape::convex_polygon(points.to_vec(), fill, stroke),
    );
    if can_open {
        // The glow: three expanded chamfer outlines of falling alpha
        // (epaint has no blur), scaled by the breath factor (Decision 6).
        let factor = breath(elapsed, reduced);
        for (expand, alpha) in [(3.0, 90u8), (6.0, 55u8), (9.0, 25u8)] {
            let glow_points = chamfer(
                rect.expand(expand),
                10.0 + expand,
                Corners::TopLeftBottomRight,
            );
            let alpha = (f32::from(alpha) * factor).clamp(0.0, 255.0) as u8;
            ui.painter().add(egui::Shape::closed_line(
                glow_points.to_vec(),
                egui::Stroke::new(1.5, YELLOW.gamma_multiply_u8(alpha)),
            ));
        }
    }
    response
}

/// The orange ICE/offline card (STORY-083 requirement 4), shown under the CTA
/// when the selected project's host is offline.
fn ice_card(ui: &mut egui::Ui, words: &Words, host: &str) {
    ui.add_space(8.0);
    let background = ui.painter().add(egui::Shape::Noop);
    let content = ui
        .vertical(|ui| {
            ui.label(
                egui::RichText::new(words.ice_heading)
                    .font(egui::FontId::new(13.0, chakra_petch(ui)))
                    .color(WARN_ORANGE),
            );
            ui.label(egui::RichText::new(words.offline_body(host)).color(INK));
        })
        .response;
    let rect = content.rect.expand(8.0);
    let points = chamfer(rect, 10.0, Corners::TopLeftBottomRight);
    ui.painter().set(
        background,
        egui::Shape::convex_polygon(
            points.to_vec(),
            WARN_ORANGE.gamma_multiply(0.12),
            egui::Stroke::new(1.0, WARN_ORANGE),
        ),
    );
}

/// The disabled add-project button (requirement 1); ⌘N does nothing yet.
fn add_project_button(ui: &mut egui::Ui, label: &str) {
    ui.add_enabled(false, egui::Button::new(label))
        .on_disabled_hover_text("Coming soon");
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

/// A notice below the card, with an orange ▲ (STORY-083 requirement 4).
fn notice_label(ui: &mut egui::Ui, words: &Words, notice: &Notice) {
    ui.label(egui::RichText::new(format!("▲ {}", notice_text(words, notice))).color(WARN_ORANGE));
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
