//! The focus of the flat view: a folder or a file double-clicked on the
//! floor, alone in the middle with what it is connected to on a ring around
//! it. Imports, calls and the bounded contexts it borders each run as their
//! own lanes and can be switched off; a double-click on the ring focuses
//! there, and Esc steps back out, to the floor in the end.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use super::{DIM, INK, NIGHT, PANEL, kind_colour};
use crate::graph::{District, Group, ItemKind, Node, ProjectMap};
use crate::lanes::Network;

/// What `path` imports, and what imports it.
const USES: Color32 = Color32::from_rgb(0x5e, 0xf6, 0xd8);
const USED_BY: Color32 = Color32::from_rgb(0xff, 0x9f, 0x1c);
/// Calls either way.
const CALLS: Color32 = Color32::from_rgb(0xb2, 0x6b, 0xff);
/// The bounded contexts it belongs to or borders.
const CONTEXT: Color32 = Color32::from_rgb(0xfc, 0xee, 0x0a);
/// The ground a card stands on.
const CARD: Color32 = Color32::from_rgb(0x14, 0x1a, 0x22);

/// The most files drawn in the middle and the most neighbours on the ring.
const MEMBERS: usize = 48;
const RING: usize = 36;
/// How long the ring takes to open, in seconds.
const OPEN: f32 = 0.35;

/// Which lanes the focus draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Links {
    pub imports: bool,
    pub calls: bool,
    pub context: bool,
}

impl Default for Links {
    fn default() -> Self {
        Links {
            imports: true,
            calls: true,
            context: true,
        }
    }
}

/// The focus: what is focused, the foci it was reached through, and the
/// lanes drawn. Nothing is focused while the stack is empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Focus {
    stack: Vec<PathBuf>,
    pub links: Links,
}

impl Focus {
    /// What is focused, if anything.
    pub fn path(&self) -> Option<&Path> {
        self.stack.last().map(PathBuf::as_path)
    }

    /// Focuses `path`, keeping what was focused to step back to.
    pub fn enter(&mut self, path: PathBuf) {
        if self.path() != Some(path.as_path()) {
            self.stack.push(path);
        }
    }

    /// Steps back to the focus before; false when nothing was focused.
    pub fn back(&mut self) -> bool {
        self.stack.pop().is_some()
    }

    /// Leaves the focus altogether.
    pub fn leave(&mut self) {
        self.stack.clear();
    }
}

/// What the focus reported as drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FocusDrawn {
    /// A file or folder clicked: selected.
    pub clicked: Option<PathBuf>,
    /// A file in the middle double-clicked: entered, as on the floor.
    pub entered: Option<PathBuf>,
    /// A neighbour double-clicked: focused next.
    pub refocus: Option<PathBuf>,
    /// The names drawn on the ring, for tests.
    pub ring: Vec<String>,
}

/// One kind of connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Uses,
    UsedBy,
    Calls,
    Context,
}

impl Kind {
    fn colour(self) -> Color32 {
        match self {
            Kind::Uses => USES,
            Kind::UsedBy => USED_BY,
            Kind::Calls => CALLS,
            Kind::Context => CONTEXT,
        }
    }

    /// Whether its goods flow out of the focus, towards the ring.
    fn outwards(self) -> bool {
        matches!(self, Kind::UsedBy | Kind::Calls)
    }
}

/// A neighbour on the ring: a file, a folder or a bounded context.
struct Neighbour {
    label: String,
    /// Where a double-click focuses.
    path: PathBuf,
    /// A bounded context rather than code.
    cell: bool,
}

/// The files under `path` in `map`, or the file `path` itself.
fn members<'a>(map: &'a ProjectMap, path: &Path) -> Vec<&'a Node> {
    fn groups<'a>(within: &'a [Group], out: &mut Vec<&'a Node>) {
        for group in within {
            out.extend(&group.nodes);
            groups(&group.groups, out);
        }
    }
    fn district<'a>(district: &'a District, out: &mut Vec<&'a Node>) {
        out.extend(&district.nodes);
        for sub in &district.sub_districts {
            out.extend(&sub.nodes);
            groups(&sub.groups, out);
        }
    }
    let mut all = Vec::new();
    for d in &map.districts {
        district(d, &mut all);
    }
    all.retain(|node| node.path.starts_with(path));
    all.sort_by(|a, b| a.path.cmp(&b.path));
    all
}

/// The path that holds every one of `paths`.
fn common(paths: &[&Path]) -> Option<PathBuf> {
    let mut out = paths.first()?.to_path_buf();
    for path in &paths[1..] {
        while !path.starts_with(&out) {
            out = out.parent()?.to_path_buf();
        }
    }
    Some(out)
}

/// The neighbours of the focus and its links to them: for each, the member
/// (an index into `inside`, or `None` for the focus as a whole), the
/// neighbour, the kind and how many.
type Lanes = BTreeMap<(Option<usize>, usize, Kind), usize>;

fn neighbours(
    map: &ProjectMap,
    network: Option<&Network>,
    focus: &Path,
    inside: &[&Node],
    links: Links,
) -> (Vec<Neighbour>, Lanes) {
    let single = inside.len() == 1 && inside[0].path == focus;
    let file_of = |path: &Path| -> Option<usize> {
        inside.iter().position(|node| path.starts_with(&node.path))
    };
    // A file neighbour of a file is shown as itself; of a folder, as its
    // folder.
    let every = members(map, &map.root);
    let files: std::collections::HashSet<&Path> = every.iter().map(|n| n.path.as_path()).collect();
    let key = |path: &Path| -> PathBuf {
        let file = path
            .ancestors()
            .find(|a| files.contains(a))
            .unwrap_or(path)
            .to_path_buf();
        if single {
            file
        } else {
            file.parent().map(Path::to_path_buf).unwrap_or(file)
        }
    };
    let mut found: BTreeMap<PathBuf, (String, bool)> = BTreeMap::new();
    let mut raw: Vec<(Option<usize>, PathBuf, Kind)> = Vec::new();
    let mut link = |from: &Path, to: &Path, out: Kind, back: Kind| {
        let (inner, outer) = (focus_holds(focus, from), focus_holds(focus, to));
        if inner == outer {
            return;
        }
        let (member, other, kind) = if inner {
            (file_of(from), to, out)
        } else {
            (file_of(to), from, back)
        };
        raw.push((member, key(other), kind));
    };
    if let Some(network) = network {
        if links.imports {
            for flow in &network.flows {
                link(&flow.from, &flow.to, Kind::Uses, Kind::UsedBy);
            }
        }
        if links.calls {
            for call in &network.calls {
                link(&call.caller, &call.callee, Kind::Calls, Kind::Calls);
            }
        }
    }
    for (_, path, _) in &raw {
        let label = path
            .strip_prefix(&map.root)
            .unwrap_or(path)
            .display()
            .to_string();
        let label = if single { label } else { format!("{label}/") };
        found.entry(path.clone()).or_insert((label, false));
    }
    // The bounded contexts the focus lies in and those they border.
    let mut cells: Vec<(String, PathBuf)> = Vec::new();
    if links.context
        && let Some(contexts) = &map.contexts
    {
        let own: Vec<&str> = inside
            .iter()
            .filter_map(|node| contexts.context_of_file(&node.path))
            .map(|c| c.name.as_str())
            .collect();
        let mut names: Vec<&str> = own.clone();
        for road in &contexts.roads {
            if own.contains(&road.upstream.as_str()) {
                names.push(&road.downstream);
            }
            if own.contains(&road.downstream.as_str()) {
                names.push(&road.upstream);
            }
        }
        names.sort();
        names.dedup();
        for name in names {
            let files: Vec<&Path> = every
                .iter()
                .filter(|n| {
                    contexts
                        .context_of_file(&n.path)
                        .is_some_and(|c| c.name == name)
                })
                .map(|n| n.path.as_path())
                .collect();
            if let Some(home) =
                common(&files).filter(|home| !focus.starts_with(home) || own.contains(&name))
            {
                cells.push((name.to_owned(), home));
            }
        }
    }
    let mut ring: Vec<Neighbour> = found
        .into_iter()
        .map(|(path, (label, cell))| Neighbour { label, path, cell })
        .collect();
    ring.truncate(RING.saturating_sub(cells.len()));
    let code = ring.len();
    ring.extend(cells.into_iter().map(|(name, path)| Neighbour {
        label: format!("cell · {name}"),
        path,
        cell: true,
    }));
    let mut lanes = Lanes::new();
    for (member, path, kind) in raw {
        if let Some(at) = ring[..code].iter().position(|n| n.path == path) {
            *lanes.entry((member, at, kind)).or_default() += 1;
        }
    }
    for at in code..ring.len() {
        lanes.insert((None, at, Kind::Context), 1);
    }
    (ring, lanes)
}

/// Whether `path` lies in what is focused.
fn focus_holds(focus: &Path, path: &Path) -> bool {
    path.starts_with(focus)
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The point where the line from `rect`'s centre towards `to` leaves it.
fn edge(rect: Rect, to: Pos2) -> Pos2 {
    let d = to - rect.center();
    if d.length() < 1e-3 {
        return rect.center();
    }
    let sx = if d.x.abs() > 1e-3 {
        rect.width() / 2.0 / d.x.abs()
    } else {
        f32::INFINITY
    };
    let sy = if d.y.abs() > 1e-3 {
        rect.height() / 2.0 / d.y.abs()
    } else {
        f32::INFINITY
    };
    rect.center() + d * sx.min(sy)
}

/// A card: a filled rectangle with `title` and `note`, in `accent`.
fn card(
    painter: &egui::Painter,
    rect: Rect,
    title: &str,
    note: &str,
    accent: Color32,
    light: f32,
    hot: bool,
) {
    painter.rect_filled(rect, 6.0, CARD.gamma_multiply(light.max(0.6)));
    painter.rect_filled(
        Rect::from_min_size(rect.min, vec2(rect.width(), 3.0)),
        egui::CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        accent.gamma_multiply(light),
    );
    let stroke = if hot {
        Stroke::new(2.0, INK.gamma_multiply(light))
    } else {
        Stroke::new(1.0, accent.gamma_multiply(0.45 * light))
    };
    painter.rect_stroke(rect, 6.0, stroke, StrokeKind::Inside);
    let size = (rect.height() * 0.28).clamp(11.0, 14.0);
    let room = rect.width() - 16.0;
    painter.text(
        rect.left_top() + vec2(8.0, 7.0),
        Align2::LEFT_TOP,
        cut(title, size, room),
        FontId::monospace(size),
        INK.gamma_multiply(light),
    );
    if !note.is_empty() {
        painter.text(
            rect.left_bottom() + vec2(8.0, -6.0),
            Align2::LEFT_BOTTOM,
            cut(note, 10.5, room),
            FontId::monospace(10.5),
            DIM.gamma_multiply(light),
        );
    }
}

/// `text` cut to `room` points of monospace at `size`, keeping its end,
/// which names the file.
fn cut(text: &str, size: f32, room: f32) -> String {
    let keep = (room / (size * 0.62)).max(4.0) as usize;
    let count = text.chars().count();
    if count <= keep {
        text.to_owned()
    } else {
        std::iter::once('…')
            .chain(text.chars().skip(count - keep + 1))
            .collect()
    }
}

/// Draws the focus over `panel`, on top of the floor it hides.
pub fn focus(
    ui: &mut egui::Ui,
    map: &ProjectMap,
    network: Option<&Network>,
    focus: &mut Focus,
    selected: Option<&Path>,
    panel: Rect,
) -> FocusDrawn {
    let mut drawn = FocusDrawn::default();
    let Some(path) = focus.path().map(Path::to_path_buf) else {
        return drawn;
    };
    let id = ui.id().with("focus");
    // Nothing reaches the floor beneath.
    ui.interact(panel, id.with("backdrop"), Sense::click());
    let painter = ui.painter().with_clip_rect(panel);
    painter.rect_filled(panel, 0.0, NIGHT.gamma_multiply(0.96));
    let now = ui.input(|i| i.time) as f32;
    let opened = ui.ctx().animate_bool_with_time(id.with(&path), true, OPEN);
    if opened < 1.0 {
        ui.ctx().request_repaint();
    }
    let t = ease(opened);

    let inside = members(map, &path);
    let single = inside.len() == 1 && inside[0].path == path;
    let (ring, lanes) = neighbours(map, network, &path, &inside, focus.links);

    // The header: what is focused, the lanes it shows, how to leave.
    let bar = Rect::from_min_size(panel.min, vec2(panel.width(), 34.0));
    painter.rect_filled(bar, 0.0, PANEL);
    let name = path
        .strip_prefix(&map.root)
        .unwrap_or(&path)
        .display()
        .to_string();
    let name = if single { name } else { format!("{name}/") };
    painter.text(
        bar.left_center() + vec2(14.0, 0.0),
        Align2::LEFT_CENTER,
        format!("FOCUS  {name}"),
        FontId::monospace(13.0),
        CONTEXT,
    );
    let mut x = bar.right() - 12.0;
    let hint = "double-click a neighbour to focus it · Esc back";
    let hint_w = hint.chars().count() as f32 * 11.0 * 0.62;
    painter.text(
        pos2(x, bar.center().y),
        Align2::RIGHT_CENTER,
        hint,
        FontId::monospace(11.0),
        DIM,
    );
    x -= hint_w + 18.0;
    for (label, on, colour, which) in [
        ("context", focus.links.context, CONTEXT, 2),
        ("calls", focus.links.calls, CALLS, 1),
        ("imports", focus.links.imports, USES, 0),
    ] {
        let w = label.len() as f32 * 7.2 + 18.0;
        let chip = Rect::from_min_size(pos2(x - w, bar.center().y - 10.0), vec2(w, 20.0));
        x -= w + 6.0;
        let response = ui
            .interact(chip, id.with(("chip", which)), Sense::click())
            .on_hover_text(format!("Show or hide the {label} lanes"));
        if on {
            painter.rect_filled(chip, 4.0, colour.gamma_multiply(0.18));
        }
        painter.rect_stroke(
            chip,
            4.0,
            Stroke::new(1.0, colour.gamma_multiply(if on { 0.9 } else { 0.35 })),
            StrokeKind::Inside,
        );
        painter.text(
            chip.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::monospace(11.0),
            if on { colour } else { DIM },
        );
        if response.clicked() {
            match which {
                0 => focus.links.imports = !focus.links.imports,
                1 => focus.links.calls = !focus.links.calls,
                _ => focus.links.context = !focus.links.context,
            }
        }
    }

    // The middle: the file read in full, or the folder's files as cards.
    let room = Rect::from_min_max(pos2(panel.left(), bar.bottom()), panel.max);
    let centre = room.center();
    let shown = &inside[..inside.len().min(MEMBERS)];
    let more = inside.len() - shown.len();
    let (cell, cols) = if single {
        (vec2(300.0, 150.0), 1)
    } else {
        let n = shown.len() + usize::from(more > 0);
        let cols = ((n as f32 * 1.8).sqrt().ceil() as usize).clamp(1, 8);
        (vec2(168.0, 52.0), cols)
    };
    let rows = (shown.len() + usize::from(more > 0)).div_ceil(cols).max(1);
    let gap = 10.0;
    let block = vec2(
        cols as f32 * (cell.x + gap) - gap,
        rows as f32 * (cell.y + gap) - gap,
    );
    let middle = Rect::from_center_size(centre, block);
    if !single {
        painter.rect_stroke(
            middle.expand(18.0),
            12.0,
            Stroke::new(1.0, CONTEXT.gamma_multiply(0.25)),
            StrokeKind::Outside,
        );
    }
    let cards: Vec<Rect> = (0..shown.len())
        .map(|i| {
            let (c, r) = (i % cols, i / cols);
            Rect::from_min_size(
                middle.min + vec2(c as f32 * (cell.x + gap), r as f32 * (cell.y + gap)),
                cell,
            )
        })
        .collect();

    // The ring: around the middle, filling the panel, opening outwards.
    let n = ring.len().max(1) as f32;
    let radius = vec2(
        (room.width() / 2.0 - 104.0).max(middle.width() / 2.0 + 140.0),
        (room.height() / 2.0 - 36.0).max(middle.height() / 2.0 + 70.0),
    );
    let satellites: Vec<Rect> = (0..ring.len())
        .map(|k| {
            let a = k as f32 / n * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let at = centre + vec2(a.cos() * radius.x, a.sin() * radius.y) * t;
            let rect = Rect::from_center_size(at, vec2(176.0, 38.0));
            // Kept inside the room, however small.
            let shift = vec2(
                (room.left() + 6.0 - rect.left()).max(0.0)
                    - (rect.right() - room.right() + 6.0).max(0.0),
                (room.top() + 6.0 - rect.top()).max(0.0)
                    - (rect.bottom() - room.bottom() + 6.0).max(0.0),
            );
            rect.translate(shift)
        })
        .collect();

    // The lanes first, under the cards: a faint band and goods moving along it.
    for (&(member, at, kind), &count) in &lanes {
        let from_rect = member.and_then(|m| cards.get(m).copied()).unwrap_or(middle);
        let to_rect = satellites[at];
        let (a, b) = (
            edge(from_rect, to_rect.center()),
            edge(to_rect, from_rect.center()),
        );
        let width = (2.0 + (count as f32 + 1.0).log2() * 2.0).min(9.0);
        let colour = kind.colour();
        painter.line_segment([a, b], Stroke::new(width, colour.gamma_multiply(0.13 * t)));
        let (start, end) = if kind.outwards() { (a, b) } else { (b, a) };
        let length = (end - start).length();
        if length > 1.0 {
            let mut d = (now * 55.0) % 36.0;
            while d < length {
                let p = start + (end - start) * (d / length);
                painter.circle_filled(p, 2.2, colour.gamma_multiply(0.7 * t));
                d += 36.0;
            }
        }
    }
    if !lanes.is_empty() {
        ui.ctx().request_repaint();
    }

    // The middle's cards.
    for (node, rect) in shown.iter().zip(&cards) {
        let response = ui.interact(*rect, id.with(("member", &node.path)), Sense::click());
        let lines = node
            .items
            .iter()
            .filter_map(|i| i.floors.last())
            .map(|f| f.number)
            .max()
            .unwrap_or(0);
        let functions = node
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Function)
            .count();
        let accent = kind_colour(node.kind).unwrap_or(USES);
        let hot = response.hovered() || selected == Some(node.path.as_path());
        if single {
            let note = format!(
                "{lines} lines · {} items · {functions} fn",
                node.items.len()
            );
            card(&painter, *rect, &node.name, &note, accent, 1.0, hot);
            let named = node.items.iter().filter(|i| i.kind != ItemKind::Uses);
            for (row, item) in named.take(5).enumerate() {
                painter.text(
                    rect.left_top() + vec2(14.0, 32.0 + row as f32 * 17.0),
                    Align2::LEFT_TOP,
                    cut(&item.name, 12.0, rect.width() - 28.0),
                    FontId::monospace(12.0),
                    DIM,
                );
            }
        } else {
            card(
                &painter,
                *rect,
                &node.name,
                &format!("{lines} lines · {functions} fn"),
                accent,
                1.0,
                hot,
            );
        }
        if response.double_clicked() {
            drawn.entered = Some(node.path.clone());
        } else if response.clicked() {
            drawn.clicked = Some(node.path.clone());
        }
        let label = response.on_hover_text(format!("{} — double-click to open", node.name));
        ui.ctx()
            .accesskit_node_builder(label.id, |b| b.set_label(node.name.clone()));
    }
    if more > 0 {
        let i = shown.len();
        let rect = Rect::from_min_size(
            middle.min
                + vec2(
                    (i % cols) as f32 * (cell.x + gap),
                    (i / cols) as f32 * (cell.y + gap),
                ),
            cell,
        );
        card(
            &painter,
            rect,
            &format!("+{more} more"),
            "",
            DIM,
            0.7,
            false,
        );
    }

    // The ring's cards.
    for (neighbour, rect) in ring.iter().zip(&satellites) {
        let response = ui.interact(
            *rect,
            id.with(("ring", &neighbour.path, neighbour.cell)),
            Sense::click(),
        );
        let kinds: Vec<Kind> = lanes
            .iter()
            .filter(|((_, at, _), _)| {
                ring[*at].path == neighbour.path && ring[*at].cell == neighbour.cell
            })
            .map(|((_, _, kind), _)| *kind)
            .collect();
        let links: usize = lanes
            .iter()
            .filter(|((_, at, _), _)| {
                ring[*at].path == neighbour.path && ring[*at].cell == neighbour.cell
            })
            .map(|(_, n)| n)
            .sum();
        let accent = kinds.first().map_or(DIM, |k| k.colour());
        let note = if neighbour.cell {
            "bounded context".to_owned()
        } else {
            format!("{links} link{}", if links == 1 { "" } else { "s" })
        };
        card(
            &painter,
            *rect,
            &neighbour.label,
            &note,
            accent,
            t,
            response.hovered(),
        );
        if response.double_clicked() {
            drawn.refocus = Some(neighbour.path.clone());
        } else if response.clicked() && !neighbour.cell {
            drawn.clicked = Some(neighbour.path.clone());
        }
        let label =
            response.on_hover_text(format!("{} — double-click to focus it", neighbour.label));
        ui.ctx()
            .accesskit_node_builder(label.id, |b| b.set_label(neighbour.label.clone()));
        drawn.ring.push(neighbour.label.clone());
    }
    if ring.is_empty() {
        painter.text(
            pos2(centre.x, middle.bottom() + 44.0),
            Align2::CENTER_TOP,
            "no lanes to anything outside",
            FontId::monospace(12.0),
            DIM,
        );
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_focus_steps_back_the_way_it_came() {
        let mut focus = Focus::default();
        assert_eq!(focus.path(), None);
        focus.enter(PathBuf::from("/p/a"));
        focus.enter(PathBuf::from("/p/b"));
        focus.enter(PathBuf::from("/p/b"));
        assert_eq!(focus.path(), Some(Path::new("/p/b")));
        assert!(focus.back());
        assert_eq!(focus.path(), Some(Path::new("/p/a")));
        assert!(focus.back());
        assert!(!focus.back());
    }

    #[test]
    fn a_long_name_keeps_its_end() {
        assert_eq!(cut("crates/map/src/ui/flat.rs", 10.0, 62.0), "…i/flat.rs");
        assert_eq!(cut("flat.rs", 10.0, 200.0), "flat.rs");
    }

    #[test]
    fn a_lane_leaves_a_card_at_its_edge() {
        let rect = Rect::from_center_size(Pos2::ZERO, vec2(100.0, 50.0));
        assert_eq!(edge(rect, pos2(500.0, 0.0)), pos2(50.0, 0.0));
        assert_eq!(edge(rect, pos2(0.0, -500.0)), pos2(0.0, -25.0));
    }
}
