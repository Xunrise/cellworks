//! Painting one project map: the ground of its districts and blocks, seen
//! isometrically, and the city of towers standing on it.
//!
//! The only module of this crate that touches an egui widget. Every name is a
//! `Label` placed at the front of the element it names rather than painted
//! text, so the frame's accessibility tree carries one entry per district,
//! sub-district, group and node. A click on a name is reported back; the
//! selected element is outlined and its label marked selected, so the
//! selection is visible both on screen and in the accessibility tree.

use std::path::{Path, PathBuf};

use egui::{Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};

mod buildings;
mod explain;
mod lanes;
mod why;

pub use buildings::{DrawnBridge, DrawnBuilding, bridge_card};
pub use explain::{banner, caption, caption_text, lane_toggles, legend, legend_text, lens_bar};
pub use lanes::{DrawnLane, colour_of};
pub use why::{NO_CONTEXT, NO_STORY, why_panel};

use crate::Framing;
use crate::city::{self, iso, iso_bounds};
use crate::graph::{Declares, DistrictKind, Group, Item, ItemKind, Node, NodeKind, ProjectMap};
use crate::layout::TITLE;
use crate::view::{Pivot, View, fit, turn_at};
use crate::{Bridge, Form, LaneId, LaneKind, Lanes, Lens, Trim};

/// One frame of the map as drawn.
pub struct Drawn {
    /// The path of the element clicked, if any: by its name, or a tower by
    /// any of its lines, or a block by its ground.
    pub clicked: Option<PathBuf>,
    /// Whether empty ground was clicked: between blocks, or off the city.
    pub ground_clicked: bool,
    /// The tower double-clicked, to step inside.
    pub entered: Option<PathBuf>,
    /// The tower one of whose lines is under the pointer.
    pub hovered: Option<PathBuf>,
    /// Where the map was seen from.
    pub view: View,
    /// The towers drawn, back to front.
    pub towers: Vec<DrawnTower>,
    /// The buildings drawn, by their bases, back to front.
    pub buildings: Vec<DrawnBuilding>,
    /// The skybridges drawn.
    pub bridges: Vec<DrawnBridge>,
    /// The cables drawn.
    pub cables: Vec<Cable>,
    /// The skybridge under the pointer, if any.
    pub bridge_hovered: Option<crate::Bridge>,
    /// How far the ground was dragged this frame, at the layout's scale.
    pub dragged: Vec2,
    /// The space the map was drawn in. Names placed past its edge make the
    /// `Ui` around it grow, so what comes after asks this instead.
    pub panel: Rect,
    /// Every district drawn, with the outline it was drawn in.
    pub districts: Vec<(PathBuf, Outline)>,
    /// In the folders lens, every folder drawn by its outline, and whether
    /// its name is written on it.
    pub folders: Vec<(PathBuf, bool)>,
    /// Every block, folder and district drawn by its outline; the outline
    /// itself is the network's, not copied every frame.
    pub outlines: Vec<PathBuf>,
    /// Every lane drawn.
    pub lanes: Vec<DrawnLane>,
    /// The lane clicked, if any.
    pub lane_clicked: Option<LaneClick>,
    /// The lane under the pointer, if any.
    pub lane_hovered: Option<LaneId>,
    /// In the custom lens: the district or block dragged, and how far this
    /// frame, at the layout's scale; and whether the drag ended.
    pub moved: Option<(PathBuf, Vec2)>,
    pub move_ended: bool,
}

/// The outline a district is drawn in: each lens has its own, so the view
/// says which lens is on before a name is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    /// A round plaza with a dashed edge: the folders lens, the tests and the
    /// outskirts.
    Plaza,
    /// A square step: the imports lens.
    Step,
    /// A hexagon: the calls lens.
    Hexagon,
    /// A double ring: the co-change lens.
    Orbit,
    /// A cell's membrane, a thick ring with a thin one inside: the cells
    /// lens.
    Membrane,
    /// A round plaza with a solid edge: the engineer's own, the custom lens.
    Own,
}

impl Outline {
    /// The outline of a district of `kind` in `lens`.
    pub fn of(lens: Lens, kind: DistrictKind) -> Outline {
        match (lens, kind) {
            (Lens::Custom, _) => Outline::Own,
            (_, DistrictKind::Context | DistrictKind::NoContext) => Outline::Membrane,
            (_, DistrictKind::Folder | DistrictKind::Tests | DistrictKind::Outskirts) => {
                Outline::Plaza
            }
            (Lens::Imports, _) => Outline::Step,
            (Lens::Calls, _) => Outline::Hexagon,
            (Lens::CoChange, _) => Outline::Orbit,
            (Lens::Folders | Lens::Cells, _) => Outline::Plaza,
        }
    }
}

/// One tower as drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnTower {
    pub path: PathBuf,
    pub colour: Color32,
    /// Where each floor went on screen, the ground floor first; far out, where
    /// the floors are no longer told apart, and for a terrace or a monument
    /// until close enough to read its lines, one span up the tower's middle.
    pub floors: Vec<Span>,
    /// The screen rectangle its building covers.
    pub outline: Rect,
    /// Whether it was drawn as glass, standing in front of the tower pointed
    /// at or selected.
    pub glass: bool,
    /// How brightly it was drawn: 1 in full, less behind glass or fog.
    pub light: f32,
    /// Whether it hangs in the cellar.
    pub cellar: bool,
}

/// One line on screen: from the point of its first character to the point of
/// its last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub start: Pos2,
    pub end: Pos2,
}

impl Span {
    /// How far `at` is from the line.
    pub fn distance(&self, at: Pos2) -> f32 {
        let along = self.end - self.start;
        let t =
            ((at - self.start).dot(along) / along.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
        (self.start + along * t - at).length()
    }
}

/// The night the city stands in, the ground of its blocks, the edges of
/// folders and the colour of names.
const NIGHT: Color32 = Color32::from_rgb(0x04, 0x06, 0x0a);
const GROUND: Color32 = Color32::from_rgb(0x0b, 0x10, 0x16);
const EDGE: Color32 = Color32::from_rgb(0x1e, 0x27, 0x31);
const DIM: Color32 = Color32::from_rgb(0x7d, 0x8f, 0xa0);
const INK: Color32 = Color32::from_rgb(0xd7, 0xe2, 0xea);
const PANEL: Color32 = Color32::from_rgb(0x0d, 0x11, 0x17);
const LINE_STRONG: Color32 = Color32::from_rgb(0x34, 0x50, 0x6a);
const FLOW: Color32 = Color32::from_rgb(0x5e, 0xf6, 0xff);
const ATTENTION_SOFT: Color32 = Color32::from_rgb(0x3a, 0x36, 0x08);

/// The colours of the crowns, one per interface, and the fill of a crown.
const CROWN_HUES: [Color32; 5] = [
    Color32::from_rgb(0x5e, 0xf6, 0xff),
    Color32::from_rgb(0xb2, 0x6b, 0xff),
    Color32::from_rgb(0xff, 0xb3, 0x5c),
    Color32::from_rgb(0x4a, 0xa8, 0xff),
    Color32::from_rgb(0xff, 0x5c, 0xa8),
];
const CROWN_FILL: Color32 = Color32::from_rgb(0x0b, 0x2a, 0x2e);

/// The colours the floors of the selected tower are tinted by, one per pull
/// request in the order the why panel lists them; a group beyond the palette
/// starts again at its first colour (STORY-019).
const PR_HUES: [Color32; 6] = [
    Color32::from_rgb(0x5e, 0xf6, 0xff),
    Color32::from_rgb(0xff, 0xb3, 0x5c),
    Color32::from_rgb(0xb2, 0x6b, 0xff),
    Color32::from_rgb(0x7a, 0xe5, 0x82),
    Color32::from_rgb(0xff, 0x5c, 0xa8),
    Color32::from_rgb(0x4a, 0xa8, 0xff),
];

/// How much of its light a tower keeps while another one is explained: the
/// city steps back so the colours of the explained tower's floors read
/// (STORY-019).
const UNEXPLAINED: f32 = 0.4;

/// The paving of a forecourt, and the colour of a mast's head.
const PAVING: Color32 = Color32::from_rgb(0x14, 0x1a, 0x22);
const CABLE: Color32 = Color32::from_rgb(0xb2, 0x6b, 0xff);

/// The colours of the buildings of files that are not code, by kind; a
/// contract, an example of secrets and an image are drawn in [`FLOW`], a
/// build file in [`DIM`]; and the fill of a sealed vault.
const DOCUMENT: Color32 = Color32::from_rgb(0x4a, 0xa8, 0xff);
const CONFIGURATION: Color32 = Color32::from_rgb(0xff, 0xb3, 0x5c);
const SPECIFICATION: Color32 = Color32::from_rgb(0xb2, 0x6b, 0xff);
const INFRASTRUCTURE: Color32 = Color32::from_rgb(0xff, 0x9f, 0x1c);
const SECRETS: Color32 = Color32::from_rgb(0xff, 0x3b, 0x5c);
const GENERATED: Color32 = Color32::from_rgb(0x2c, 0x3b, 0x4a);
const SCHEMA: Color32 = Color32::from_rgb(0x34, 0x50, 0x6a);
const VAULT: Color32 = Color32::from_rgb(0x16, 0x05, 0x0a);

/// The colour the buildings of a node of `kind` are drawn in; `None` for
/// code, which keeps its district's.
fn kind_colour(kind: NodeKind) -> Option<Color32> {
    match kind {
        NodeKind::Code => None,
        NodeKind::Document => Some(DOCUMENT),
        NodeKind::Configuration => Some(CONFIGURATION),
        NodeKind::Specification => Some(SPECIFICATION),
        NodeKind::Infrastructure => Some(INFRASTRUCTURE),
        NodeKind::Build => Some(DIM),
        NodeKind::Contract | NodeKind::SecretsExample | NodeKind::Image => Some(FLOW),
        NodeKind::Secrets => Some(SECRETS),
        NodeKind::Generated => Some(GENERATED),
        NodeKind::Schema => Some(SCHEMA),
    }
}

/// The colours of the districts, in the order the map lists them; an eighth
/// district takes the first colour again.
const DISTRICT_COLOURS: [Color32; 7] = [
    Color32::from_rgb(0x5e, 0xf6, 0xff),
    Color32::from_rgb(0xff, 0x5c, 0xa8),
    Color32::from_rgb(0x9c, 0xff, 0x57),
    Color32::from_rgb(0xff, 0x8a, 0x3d),
    Color32::from_rgb(0xb2, 0x6b, 0xff),
    Color32::from_rgb(0xff, 0xd1, 0x66),
    Color32::from_rgb(0x3d, 0xff, 0xc8),
];

/// Tests are grey towers, whatever their district.
const TEST_COLOUR: Color32 = Color32::from_rgb(0x56, 0x67, 0x7a);

/// How much of the city outside the folder view still shows.
const OUTSIDE: f32 = 0.35;

/// How opaque a building's faces are, its side's colour, and how bright its
/// edges glow.
const SOLID: f32 = 0.92;
const SIDE: Color32 = Color32::from_rgb(0x07, 0x0a, 0x0e);
const EDGE_LIGHT: f32 = 0.35;

/// How wide a floor's stroke is far out.
const FAR_STROKE: f32 = 1.4;

/// How much of the cellar shows outside the underground view, and how much of
/// the city above shows inside it.
const CELLAR: f32 = 0.2;
const ABOVE_UNDERGROUND: f32 = 0.15;

/// How much of a tower shows while a turned camera faces another facade:
/// next to nothing in front of the facade, less the further behind it.
fn fog(item: &Item, pivot: Pivot) -> f32 {
    let here = item.area.center();
    let in_front = if pivot.along_x {
        here.y - pivot.ground.y
    } else {
        here.x - pivot.ground.x
    };
    if in_front > 0.5 {
        FOGGED_FRONT
    } else if in_front < -0.5 {
        (1.0 + in_front / FOG_DEPTH).max(FOGGED_BEHIND)
    } else {
        1.0
    }
}

/// Fog: what stands in front of a faced facade keeps this much, what stands
/// behind fades over this depth down to the least it keeps.
const FOGGED_FRONT: f32 = 0.06;
const FOG_DEPTH: f32 = 25.0;
const FOGGED_BEHIND: f32 = 0.12;

/// How bright a line glows, a line that calls something, and a tower pointed
/// at or selected; glass lets a tenth through.
const LINE: f32 = 0.45;
const CALLING_LINE: f32 = 0.85;
const FOCUSED: f32 = 1.3;
const GLASS: f32 = 0.1;

/// From this scale on a token is drawn as long as it is, so a tower reads as
/// a wall of code.
const DASHES_FROM: f32 = 1.2;

/// From this font size on, the floors close up are written as their lines.
const LETTERS_FROM: f32 = 7.0;

/// The colour of a road between cells.
const ROAD: Color32 = Color32::from_rgb(38, 46, 60);

/// The colour of a warning: an import cycle.
const WARNING: Color32 = Color32::from_rgb(255, 150, 60);

/// The colour of whatever needs the engineer: selection, the pointer.
const ATTENTION: Color32 = Color32::from_rgb(0xfc, 0xee, 0x0a);

/// `colour` at `bright`ness, added to what lies beneath rather than painted
/// over it, so points glow and crowded points glow brighter.
fn glow(colour: Color32, bright: f32) -> Color32 {
    let [r, g, b, _] = colour.to_array();
    let scaled = |channel: u8| (f32::from(channel) * bright).min(255.0) as u8;
    Color32::from_rgba_premultiplied(scaled(r), scaled(g), scaled(b), 0)
}

/// A small card beside the pointer naming the tower under it: its name, its
/// file and how many lines it has.
fn tip(ui: &egui::Ui, at: Pos2, item: &Item) {
    let painter = ui.painter();
    let file = item
        .path
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let lines = item.floors.len();
    let unit = if lines == 1 { "line" } else { "lines" };
    let name = painter.layout_no_wrap(item.name.clone(), egui::FontId::proportional(13.0), INK);
    let meta = painter.layout_no_wrap(
        format!("{file} · {lines} {unit}"),
        egui::FontId::monospace(11.0),
        DIM,
    );
    let corner = at + vec2(14.0, 14.0);
    let size = vec2(
        name.size().x.max(meta.size().x),
        name.size().y + 2.0 + meta.size().y,
    );
    let card = Rect::from_min_size(corner, size).expand2(vec2(10.0, 6.0));
    painter.rect(
        card,
        0.0,
        PANEL,
        Stroke::new(1.0, LINE_STRONG),
        StrokeKind::Inside,
    );
    painter.galley(corner, name.clone(), INK);
    painter.galley(corner + vec2(0.0, name.size().y + 2.0), meta, DIM);
}

/// Four corner brackets around `rect`.
fn brackets(shapes: &mut Vec<Shape>, rect: Rect, colour: Color32) {
    let stroke = Stroke::new(2.0, colour);
    let arm = 10.0_f32.min(rect.width() / 3.0).min(rect.height() / 3.0);
    for (corner, dx, dy) in [
        (rect.left_top(), 1.0, 1.0),
        (rect.right_top(), -1.0, 1.0),
        (rect.left_bottom(), 1.0, -1.0),
        (rect.right_bottom(), -1.0, -1.0),
    ] {
        let line = vec![
            corner + vec2(dx * arm, 0.0),
            corner,
            corner + vec2(0.0, dy * arm),
        ];
        shapes.push(Shape::line(line, stroke));
    }
}

/// How far from a skybridge's top, on screen, a pointer still points at it.
const BRIDGE_REACH: f32 = 6.0;

/// How far from a line's points, on screen, a pointer still hits the line.
const PICK_REACH: f32 = 3.0;

/// Below this scale a floor is one point at its indentation; above it, one
/// point per token.
const TOKENS_FROM: f32 = 0.8;

/// Below this scale a tower's floors are no longer told apart: it is drawn
/// as its box with a lit front, and picked by its middle. Its building keeps
/// its form, spire and beacon; doors, courtyard, forecourt, masts, cables,
/// crowns and the wires and pipes on the ground are left out with the
/// floors, and a skybridge is drawn in one piece.
const BOXES_BELOW: f32 = 0.5;

/// How brightly a far-out tower's front is lit, against a line's.
const BOX_FACE: f32 = 0.6;

/// How much wider than its name a folder outline is on screen before the name
/// is written on it.
const NAME_ROOM: f32 = 1.5;

/// The size of a folder outline's name.
const FOLDER_NAME: f32 = 11.0;

/// The colour of the district at `index` in the map's list.
fn district_colour(index: usize) -> Color32 {
    DISTRICT_COLOURS[index % DISTRICT_COLOURS.len()]
}

/// What one frame of the map is drawn with: where it goes and what is
/// selected. The clicked element is written back to `clicked`; names and
/// towers are collected while the ground is drawn, and drawn over it.
struct Painter<'a> {
    view: View,
    selected: Option<&'a Path>,
    /// The folder whose folder view is drawn; everything outside it is not.
    folder_view: Option<&'a Path>,
    clicked: Option<PathBuf>,
    /// The colour of the district being drawn, and the centre of its plaza
    /// when the district itself is shown.
    colour: Color32,
    plaza: Option<Pos2>,
    labels: Vec<Labelled<'a>>,
    towers: Vec<Standing<'a>>,
    /// The ground of every block drawn, the node it stands for, and how high
    /// it is raised.
    blocks: Vec<(Rect, &'a Path, f32)>,
    /// The facade a turned camera faces, and how far it has turned.
    facing: Option<(Pivot, f32)>,
    /// Whether the view is underground: the cellar within reach.
    underground: bool,
    /// The outline of the district being drawn.
    outline: Outline,
    /// For every item, the district whose colour it keeps (its crate).
    homes: &'a std::collections::BTreeMap<PathBuf, usize>,
    /// How high the district being drawn is raised.
    lift: f32,
    /// The blocks holding the files of an import cycle.
    cycles: &'a std::collections::BTreeSet<PathBuf>,
    /// Whether the city is being arranged (the custom lens): a district's
    /// name then moves the district when dragged.
    arranging: bool,
    /// The district whose name was dragged this frame, how far on screen,
    /// and whether the drag ended.
    name_dragged: Option<(PathBuf, Vec2, bool)>,
    /// Whether folders are drawn by their outlines (the folders lens): an
    /// outline inside the outline of the folder holding it, blocks without
    /// avenues.
    folders: bool,
    /// Every folder outline named this frame, and whether its name is
    /// written.
    folder_names: Vec<(PathBuf, bool)>,
    /// The outline of every block, folder and district, once worked out.
    outlines: Option<&'a std::collections::HashMap<PathBuf, Vec<Pos2>>>,
    /// The blocks, folders and districts drawn by their outlines this frame.
    outlines_drawn: Vec<PathBuf>,
    /// What each building shows of itself and where each item stands in
    /// one, once worked out.
    buildings: Option<&'a crate::Buildings>,
    /// The skybridges between the buildings, once worked out.
    bridges: &'a [Bridge],
    /// Whether the cables from the masts are shown.
    cables: bool,
    /// The cables drawn this frame.
    cables_drawn: Vec<Cable>,
    /// The skybridge pointed at this frame, if any.
    bridge_hovered: Option<crate::Bridge>,
    /// The billboard of each image on a block of its own: the node, its
    /// ground and how high it is raised.
    billboards: Vec<(&'a Path, Rect, f32)>,
    /// The palette index of each floor of the selected tower, while one is
    /// explained; every other tower is dimmed then (STORY-019).
    tint: Option<&'a [usize]>,
}

/// A name to put on the map: the ground it names, how it is written, the
/// name, the path it stands for, and how high that ground is raised.
type Labelled<'a> = (Rect, Naming, &'a str, &'a Path, f32);

/// How a name is written: a district's in its colour along its front,
/// always; a folder outline's on its front once the outline has room for
/// it; a block with code on it just below its front edge, quietly and only
/// close enough to read. Folders below a district drawn without an outline,
/// and blocks without code, are named for the accessibility tree only.
#[derive(Clone, Copy)]
enum Naming {
    District(Color32),
    FolderOutline,
    Folder,
    /// A block, and whether it has code on it.
    Block(bool),
}

/// Below this scale only district names are written; the others are still
/// there for the accessibility tree, in no colour.
const NAMES_FROM: f32 = 0.9;

/// What is drawn of a map and how: the view options of one frame.
#[derive(Clone, Copy, Debug)]
pub struct ViewOptions<'a> {
    /// The part of the map to fit into the space left.
    pub framing: Framing,
    /// The element outlined as selected.
    pub selected: Option<&'a Path>,
    /// The folder whose folder view is drawn; everything outside it faded.
    pub folder_view: Option<&'a Path>,
    /// The facade the camera turns towards, close enough (see [`turn_at`]).
    pub pivot: Option<Pivot>,
    /// Whether the cellar is in reach rather than faint.
    pub underground: bool,
    /// The lanes shown.
    pub lanes: Lanes,
    /// The lane picked, kept lit with what it carries.
    pub picked: Option<&'a LaneId>,
    /// The map's lanes, worked out once for it; no lane is drawn without.
    pub network: Option<&'a crate::Network>,
    /// Whether a cable runs from each mast to the building it calls.
    pub cables: bool,
    /// The selected tower's floors tinted by the pull request that last
    /// wrote each: one index into the palette per floor, in source order, as
    /// [`crate::Why::floors`] gives them. Every other tower is dimmed while
    /// it is set; `None` leaves the city as it was (STORY-019).
    pub tint: Option<&'a [usize]>,
}

/// A click on a lane: the lane, and with Cmd held the tower at its far end
/// from the click, to travel to.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneClick {
    pub lane: LaneId,
    pub travel: Option<PathBuf>,
}

/// Draws `map` as `options` say, fitted to the space left in `ui`, and
/// reports what was clicked this frame: a name, a tower by any line of it, a
/// block, or empty ground. Draws nothing for an empty map.
pub fn project(ui: &mut egui::Ui, map: &ProjectMap, options: ViewOptions<'_>) -> Drawn {
    let ViewOptions {
        framing,
        selected,
        folder_view,
        pivot,
        underground,
        lanes: shown,
        picked,
        network,
        cables,
        tint,
    } = options;
    let panel = ui.available_rect_before_wrap();
    let view = fit(framing, panel);
    let turn = turn_at(view.scale());
    let view = match pivot {
        Some(pivot) if turn > 0.0 => view.turned(turn, pivot, panel.center()),
        _ => view,
    };
    ui.painter().rect_filled(panel, 0.0, NIGHT);
    let mut painter = Painter {
        view,
        selected,
        folder_view,
        clicked: None,
        colour: DISTRICT_COLOURS[0],
        plaza: None,
        labels: Vec::new(),
        towers: Vec::new(),
        blocks: Vec::new(),
        facing: pivot.filter(|_| turn > 0.0).map(|pivot| (pivot, turn)),
        underground,
        outline: Outline::Plaza,
        homes: &map.homes,
        lift: 0.0,
        cycles: &map.cycles,
        arranging: map.lens == Lens::Custom,
        name_dragged: None,
        folders: map.lens == Lens::Folders,
        folder_names: Vec::new(),
        outlines: network.map(|network| &network.buildings.outlines),
        outlines_drawn: Vec::new(),
        buildings: network.map(|network| &network.buildings),
        bridges: network.map_or(&[], |network| &network.buildings.bridges),
        cables,
        cables_drawn: Vec::new(),
        bridge_hovered: None,
        billboards: Vec::new(),
        tint,
    };
    orbits(ui, map, &view);
    let mut outlines = Vec::new();
    // Back to front, so a raised district hides what stands behind it.
    let mut order: Vec<usize> = (0..map.districts.len()).collect();
    order.sort_by(|a, b| {
        let depth = |d: usize| view.to_screen(map.districts[d].area.center(), 0.0).y;
        depth(*a).total_cmp(&depth(*b)).then(a.cmp(b))
    });
    for index in order {
        let district = &map.districts[index];
        if district.elevation > 0.0 {
            platform(
                ui,
                &view,
                district.area,
                district.elevation,
                district_colour(index),
            );
        }
        painter.lift = district.elevation;
        painter.colour = district_colour(index);
        painter.plaza = None;
        painter.outline = Outline::of(map.lens, district.kind);
        outlines.push((district.path.clone(), painter.outline));
        let (area, name, path) = (district.area, &district.name, &district.path);
        let shown = painter.ground(ui, area, name, path, false, true);
        for sub in &district.sub_districts {
            let shown = painter.ground(ui, sub.area, &sub.name, &sub.path, shown, false);
            for group in &sub.groups {
                painter.group_with_children(ui, group, shown);
            }
            painter.nodes(ui, &sub.nodes, shown);
        }
        painter.nodes(ui, &district.nodes, shown);
    }
    roads(ui, map, &view);
    // The quiet names of folders and blocks lie under the ground's sense, so
    // a click on a tower standing over a name reaches the tower; the names of
    // districts, and of folder outlines written on them, lie over it.
    let (districts, quiet): (Vec<_>, Vec<_>) = std::mem::take(&mut painter.labels)
        .into_iter()
        .partition(|(area, naming, name, path, lift)| match naming {
            Naming::District(_) => true,
            Naming::FolderOutline => painter.outline_has_room(*area, path, name, *lift),
            Naming::Folder | Naming::Block(_) => false,
        });
    for (area, naming, name, path, lift) in quiet {
        painter.label(ui, area, naming, name, path, lift);
    }
    let ground = ui.interact(panel, ui.id().with("ground"), Sense::click_and_drag());
    let mut dragged = view.ground_delta(ground.drag_delta());
    // In the custom lens a drag that starts on a block moves the block, one
    // that starts on a district's open ground the district; elsewhere it
    // pans. What it moves is kept from frame to frame.
    let mut moved = None;
    let mut move_ended = false;
    if map.lens == Lens::Custom {
        let held = ui.id().with("custom move");
        if ground.drag_started()
            && let Some(at) = ui
                .input(|input| input.pointer.press_origin())
                .or(ground.interact_pointer_pos())
        {
            // Where the button went down: the drag only starts once the
            // pointer has moved, by then perhaps off a small block.
            let block = painter
                .blocks
                .iter()
                .find(|(area, _, lift)| area.contains(view.to_ground_at(at, *lift)))
                .map(|(_, path, _)| path.to_path_buf());
            let district = || {
                map.at(view.to_ground(at))
                    .first()
                    .map(|path| path.to_path_buf())
            };
            let target: Option<PathBuf> = block.or_else(district);
            ui.data_mut(|data| data.insert_temp(held, target));
            // The drag starts once the pointer has moved a little: the move
            // counts from where the button went down.
            if let Some(now) = ground.interact_pointer_pos() {
                dragged = view.ground_delta(now - at);
            }
        }
        if let Some(path) = ui
            .data(|data| data.get_temp::<Option<PathBuf>>(held))
            .flatten()
        {
            if dragged != Vec2::ZERO {
                moved = Some((path, dragged));
                dragged = Vec2::ZERO;
            }
            if ground.drag_stopped() {
                ui.data_mut(|data| data.remove::<Option<PathBuf>>(held));
                move_ended = true;
            }
        }
    }
    let pointer = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|pointer| panel.contains(*pointer));
    let time = ui.input(|input| input.time) as f32;
    let focus = lanes::Focus {
        shown,
        selected,
        picked,
        pointer,
        network,
    };
    let mut frame = lanes::frame(map, &view, &focus);
    let mut shapes = Vec::new();
    lanes::draw(&mut frame, LaneKind::Imports, &view, time, &mut shapes);
    ui.painter().extend(shapes);
    let (towers, buildings, bridges, hovered) = painter.towers(ui, pointer);
    let mut shapes = Vec::new();
    lanes::draw(&mut frame, LaneKind::Calls, &view, time, &mut shapes);
    ui.painter().extend(shapes);
    lanes::card(ui, &frame, picked, pointer);
    if shown.imports || shown.calls {
        // Traffic keeps moving.
        ui.ctx().request_repaint();
    }
    let mut entered = None;
    let mut ground_clicked = false;
    let mut lane_clicked = None;
    // A click on a lane picks it; with Cmd it travels to the far end.
    if ground.clicked()
        && let (Some(at), Some(lane)) = (
            ground.interact_pointer_pos(),
            frame.hovered.map(|i| &frame.lanes[i]),
        )
    {
        let command = ui.input(|input| {
            input.modifiers.command
                || input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::PointerButton { modifiers, .. } if modifiers.command))
        });
        let (start, end) = (
            lane.screen[0],
            *lane.screen.last().unwrap_or(&lane.screen[0]),
        );
        let far = if (start - at).length() > (end - at).length() {
            &lane.ends[0]
        } else {
            &lane.ends[1]
        };
        lane_clicked = Some(LaneClick {
            lane: lane.id.clone(),
            travel: command.then(|| far.clone()),
        });
    } else if (ground.clicked() || ground.double_clicked())
        && let Some(at) = ground.interact_pointer_pos()
    {
        let tower = pick(&towers, at, painter.underground);
        if ground.double_clicked() {
            entered = tower.clone();
        }
        let block = painter
            .blocks
            .iter()
            .find(|(area, _, lift)| area.contains(view.to_ground_at(at, *lift)))
            .map(|(_, path, _)| path.to_path_buf());
        // In the folders lens a click on a folder's open ground selects the
        // innermost folder holding it; a district's open ground is empty
        // ground.
        let folder = || {
            let chain = map.at(view.to_ground(at));
            match chain.as_slice() {
                [_district, .., folder] if painter.folders => Some(folder.to_path_buf()),
                _ => None,
            }
        };
        match tower.or(block).or_else(folder) {
            Some(path) => painter.clicked = Some(path),
            None => ground_clicked = true,
        }
    }
    for (area, naming, name, path, lift) in districts {
        painter.label(ui, area, naming, name, path, lift);
    }
    if let Some((path, by, ended)) = painter.name_dragged.take() {
        if by != Vec2::ZERO {
            moved = Some((path, view.ground_delta(by)));
        }
        move_ended |= ended;
    }
    Drawn {
        clicked: painter.clicked,
        ground_clicked,
        entered,
        hovered,
        view,
        towers,
        buildings,
        bridges,
        cables: painter.cables_drawn,
        bridge_hovered: painter.bridge_hovered,
        dragged,
        panel,
        districts: outlines,
        folders: painter.folder_names,
        outlines: painter.outlines_drawn,
        lane_hovered: frame.hovered.map(|i| frame.lanes[i].id.clone()),
        lanes: frame.lanes,
        lane_clicked,
        moved,
        move_ended,
    }
}

/// A district raised by `e`: a solid box from its ground down to its
/// footprint, the two sides facing the viewer dark and edged in `colour`,
/// with an opaque top for what stands on it.
fn platform(ui: &egui::Ui, view: &View, area: Rect, e: f32, colour: Color32) {
    let corners = [
        area.left_top(),
        area.right_top(),
        area.right_bottom(),
        area.left_bottom(),
    ];
    let top = |p: Pos2| view.to_screen(p, e);
    let foot = |p: Pos2| view.to_screen(p, 0.0);
    let edge = Stroke::new(1.0, colour.gamma_multiply(0.45));
    // The sides facing the viewer are the ones whose ground edge is nearest
    // the bottom of the screen; drawn back to front.
    let mut sides: Vec<(Pos2, Pos2)> = (0..4).map(|k| (corners[k], corners[(k + 1) % 4])).collect();
    sides.sort_by(|a, b| {
        let near = |(p, q): &(Pos2, Pos2)| foot(*p).y + foot(*q).y;
        near(a).total_cmp(&near(b))
    });
    for (k, (a, b)) in sides.iter().enumerate().skip(2) {
        let fill = if k == 2 { SIDE } else { GROUND };
        let face = vec![top(*a), top(*b), foot(*b), foot(*a)];
        ui.painter().add(Shape::convex_polygon(face, fill, edge));
        for step in 1..6 {
            let p = *a + (*b - *a) * (step as f32 / 6.0);
            let pillar = Stroke::new(1.0, colour.gamma_multiply(0.12));
            ui.painter().line_segment([top(p), foot(p)], pillar);
        }
    }
    let roof: Vec<Pos2> = corners.iter().map(|p| top(*p)).collect();
    ui.painter()
        .add(Shape::convex_polygon(roof, NIGHT, Stroke::NONE));
}

/// The cells lens's roads: from the upstream cell's membrane to the
/// downstream one's, named by the relationship's type, an arrow pointing
/// downstream.
fn roads(ui: &egui::Ui, map: &ProjectMap, view: &View) {
    for (up, down, kind) in &map.roads {
        let (a, b) = (map.districts[*up].area, map.districts[*down].area);
        let dir = (b.center() - a.center()).normalized();
        let start = a.center() + dir * round(a).width() / 2.0;
        let end = b.center() - dir * round(b).width() / 2.0;
        let line = [view.to_screen(start, 0.0), view.to_screen(end, 0.0)];
        ui.painter().line_segment(line, Stroke::new(9.0, NIGHT));
        ui.painter().line_segment(line, Stroke::new(6.0, ROAD));
        ui.painter()
            .extend(Shape::dashed_line(&line, Stroke::new(1.0, DIM), 6.0, 6.0));
        let back = (line[0] - line[1]).normalized() * 12.0;
        let side = vec2(-back.y, back.x) * 0.5;
        let tip = line[1];
        let arrow = vec![tip, tip + back + side, tip + back - side];
        ui.painter()
            .add(Shape::convex_polygon(arrow, DIM, Stroke::NONE));
        let middle = line[0] + (line[1] - line[0]) * 0.5;
        let galley =
            ui.painter()
                .layout_no_wrap(kind.clone(), egui::FontId::proportional(11.0), INK);
        let label = Rect::from_center_size(middle, galley.size() + vec2(10.0, 4.0));
        ui.painter().rect_filled(label, 3.0, PANEL);
        ui.painter().galley(label.min + vec2(5.0, 2.0), galley, INK);
    }
}

/// The co-change lens's orbits: a faint dashed circle round its hub through
/// every other district it arranged.
fn orbits(ui: &egui::Ui, map: &ProjectMap, view: &View) {
    let Some(hub) = map.hub.and_then(|hub| map.districts.get(hub)) else {
        return;
    };
    let centre = hub.area.center();
    for district in &map.districts {
        if district.path == hub.path || district.kind != DistrictKind::Cluster {
            continue;
        }
        let distance = (district.area.center() - centre).length();
        let mut orbit = view.ring(
            Rect::from_center_size(centre, Vec2::splat(2.0 * distance)),
            0.0,
        );
        orbit.push(orbit[0]);
        ui.painter().extend(Shape::dashed_line(
            &orbit,
            Stroke::new(1.0, DIM.gamma_multiply(0.25)),
            2.0,
            8.0,
        ));
    }
}

/// The tower one of whose lines is under the screen point `at`, the nearest
/// first, the cellar left out unless `underground`: a line is hit only where it has characters, so a tower is reached
/// through the gaps of the towers in front of it. Far out a tower's one span
/// runs up its middle.
fn pick(towers: &[DrawnTower], at: Pos2, underground: bool) -> Option<PathBuf> {
    towers
        .iter()
        .rev()
        // The cellar is out of reach unless the view is underground.
        .filter(|tower| underground || !tower.cellar)
        .find(|tower| {
            tower
                .floors
                .iter()
                .any(|floor| floor.distance(at) <= PICK_REACH)
        })
        .map(|tower| tower.path.clone())
}

impl<'a> Painter<'a> {
    fn group_with_children(&mut self, ui: &mut egui::Ui, group: &'a Group, inside: bool) {
        let shown = self.ground(ui, group.area, &group.name, &group.path, inside, false);
        for inner in &group.groups {
            self.group_with_children(ui, inner, shown);
        }
        self.nodes(ui, &group.nodes, shown);
    }

    /// Draws the folder at `path`, in full when it is shown: when its parent
    /// is shown (`inside`), it is the folder view, or there is no folder view.
    /// Outside the folder view the city stays, faded, and its names are not
    /// put on the map. A district is a plaza: a round ground faintly tinted in
    /// its colour with a dashed edge; in the folders lens its ground follows
    /// its outline. In the folders lens a deeper folder is a finer dashed
    /// outline in that colour; elsewhere it has no ground of its own, only
    /// its name. Returns whether it is shown, which is whether what
    /// it holds is shown.
    fn ground(
        &mut self,
        ui: &egui::Ui,
        area: Rect,
        name: &'a str,
        path: &'a Path,
        inside: bool,
        district: bool,
    ) -> bool {
        let shown = inside || self.folder_view.is_none_or(|folder| folder == path);
        let fade = if shown { 1.0 } else { OUTSIDE };
        let by_outline = self.folders && (!district || self.outline == Outline::Plaza);
        if by_outline && self.outline_of(path).is_some() {
            self.outlines_drawn.push(path.to_path_buf());
        }
        if district {
            let ring = match self.outline {
                Outline::Plaza if self.folders => self.round(area, path, self.lift),
                Outline::Plaza | Outline::Orbit | Outline::Membrane | Outline::Own => {
                    self.view.ring(round(area), self.lift)
                }
                Outline::Step => self.view.corners(area, self.lift),
                Outline::Hexagon => self.view.polygon(round(area), 6, self.lift),
            };
            let fill = self.colour.gamma_multiply(0.03 * fade);
            ui.painter()
                .add(Shape::convex_polygon(ring.clone(), fill, Stroke::NONE));
            let mut closed = ring;
            closed.push(closed[0]);
            let edge = Stroke::new(1.0, self.colour.gamma_multiply(0.25 * fade));
            let solid = Stroke::new(1.5, self.colour.gamma_multiply(0.4 * fade));
            match self.outline {
                Outline::Plaza => ui
                    .painter()
                    .extend(Shape::dashed_line(&closed, edge, 6.0, 6.0)),
                Outline::Step | Outline::Hexagon | Outline::Own => {
                    ui.painter().add(Shape::line(closed, solid));
                }
                Outline::Membrane => {
                    let membrane = Stroke::new(6.0, self.colour.gamma_multiply(0.12 * fade));
                    ui.painter().add(Shape::line(closed.clone(), membrane));
                    let ring = Stroke::new(1.8, self.colour.gamma_multiply(0.5 * fade));
                    ui.painter().add(Shape::line(closed, ring));
                    let inner = round(area).scale_from_center(0.94);
                    let mut inner = self.view.ring(inner, self.lift);
                    inner.push(inner[0]);
                    let thin = Stroke::new(1.0, self.colour.gamma_multiply(0.25 * fade));
                    ui.painter().add(Shape::line(inner, thin));
                }
                Outline::Orbit => {
                    ui.painter().add(Shape::line(closed, solid));
                    let inner = round(area).scale_from_center(0.88);
                    let mut inner = self.view.ring(inner, self.lift);
                    inner.push(inner[0]);
                    ui.painter()
                        .extend(Shape::dashed_line(&inner, edge, 3.0, 5.0));
                }
            }
            self.plaza = Some(area.center());
            if !shown {
                let front = Rect::from_points(&self.round(area, path, self.lift)).center_bottom();
                ui.painter().text(
                    front,
                    egui::Align2::CENTER_BOTTOM,
                    name,
                    egui::FontId::proportional(13.0),
                    self.colour.gamma_multiply(fade),
                );
            }
        }
        if !district && self.folders {
            let mut outline = self.round(area, path, self.lift);
            outline.push(outline[0]);
            let edge = Stroke::new(1.0, self.colour.gamma_multiply(0.3 * fade));
            ui.painter()
                .extend(Shape::dashed_line(&outline, edge, 4.0, 4.0));
        }
        if !shown {
            return false;
        }
        let naming = match (district, self.folders) {
            (true, _) => Naming::District(self.colour),
            (false, true) => Naming::FolderOutline,
            (false, false) => Naming::Folder,
        };
        self.labels.push((area, naming, name, path, self.lift));
        true
    }

    /// Draws the blocks of `nodes` and queues their towers; faded, unnamed
    /// and not to be clicked when they lie outside the folder view.
    fn nodes(&mut self, ui: &egui::Ui, nodes: &'a [Node], shown: bool) {
        let fade = if shown { 1.0 } else { OUTSIDE };
        for node in nodes {
            // In the folders lens the outlines say which folder a block is in.
            if let Some(plaza) = self.plaza.filter(|_| !self.folders) {
                let avenue = [
                    self.view.to_screen(plaza, self.lift),
                    self.view.to_screen(node.area.center(), self.lift),
                ];
                let stroke = Stroke::new(1.0, self.colour.gamma_multiply(0.2 * fade));
                ui.painter().line_segment(avenue, stroke);
            }
            // A block without code is ground, not a place to build on; a
            // block of only tests lies in the cellar, outlined.
            let cellar = !node.items.is_empty() && node.items.iter().all(city::in_cellar);
            let (fill, edge) = if cellar {
                (Color32::TRANSPARENT, EDGE.gamma_multiply(0.4))
            } else if node.items.is_empty() {
                (GROUND.gamma_multiply(0.4), EDGE.gamma_multiply(0.6))
            } else {
                (GROUND.gamma_multiply(0.85), self.colour.gamma_multiply(0.3))
            };
            let outline = self.outline_of(&node.path);
            ui.painter().add(Shape::convex_polygon(
                self.plotted(outline, node.area, self.lift),
                fill.gamma_multiply(fade),
                Stroke::new(1.0, edge.gamma_multiply(fade)),
            ));
            if outline.is_some() {
                self.outlines_drawn.push(node.path.clone());
            }
            if self.cycles.contains(&node.path) {
                // An import cycle: a warning, dashed and glowing, and said.
                let mut edge = self.view.corners(node.area.expand(2.0), self.lift);
                edge.push(edge[0]);
                ui.painter().add(Shape::line(
                    edge.clone(),
                    Stroke::new(4.0, WARNING.gamma_multiply(0.15 * fade)),
                ));
                ui.painter().extend(Shape::dashed_line(
                    &edge,
                    Stroke::new(1.5, WARNING.gamma_multiply(0.9 * fade)),
                    5.0,
                    3.0,
                ));
                ui.painter().text(
                    self.view.to_screen(node.area.left_bottom(), self.lift),
                    egui::Align2::RIGHT_TOP,
                    "import cycle",
                    egui::FontId::proportional(11.0),
                    WARNING.gamma_multiply(fade),
                );
            }
            if shown {
                self.labels.push((
                    node.area,
                    Naming::Block(!node.items.is_empty()),
                    &node.name,
                    &node.path,
                    self.lift,
                ));
                if !cellar || self.underground {
                    self.blocks.push((node.area, &node.path, self.lift));
                }
            }
            let kind = kind_colour(node.kind);
            let mut home = home_of(self.homes);
            // Where the items of the block stand, looked up once a block.
            let stands = self
                .buildings
                .and_then(|buildings| Some((buildings, buildings.stands.get(&node.path)?)));
            for (index, item) in node.items.iter().enumerate() {
                let colour = match (item.kind, kind) {
                    (ItemKind::Test, _) => TEST_COLOUR,
                    (_, Some(kind)) => kind,
                    _ => home(item).map_or(self.colour, district_colour),
                };
                let stand = stands.and_then(|(buildings, stands)| {
                    let stand = (*stands.get(index)?)?;
                    let base = node.items.get(stand.base)?;
                    Some((stand, base, buildings.trims.get(stand.trim)?))
                });
                self.towers.push(Standing {
                    item,
                    colour: colour.gamma_multiply(fade),
                    base: stand.map_or(item, |(_, base, _)| base),
                    is_base: stand.is_none_or(|(stand, _, _)| stand.base == index),
                    top: stand.is_some_and(|(stand, _, _)| stand.top),
                    trim: stand.map(|(_, _, trim)| trim),
                    order: (0.0, 0.0),
                });
            }
            // What lies on the ground between the buildings of a kind: the
            // wires between cabinets, a customs house's border and roads.
            let colour = kind.unwrap_or(self.colour).gamma_multiply(fade);
            let mut ground = Vec::new();
            match node.kind {
                // Far out the wires are left out with the floors.
                NodeKind::Configuration if self.view.scale() >= BOXES_BELOW => {
                    let cabinets = node
                        .items
                        .iter()
                        .filter(|i| i.declaration.part_of.is_none());
                    let centres: Vec<Pos2> = cabinets.map(|i| i.area.center()).collect();
                    for pair in centres.windows(2) {
                        buildings::wire(
                            &self.view,
                            &mut ground,
                            (pair[0], self.lift),
                            (pair[1], self.lift),
                            colour,
                        );
                    }
                }
                NodeKind::Contract => {
                    for item in &node.items {
                        buildings::border(&self.view, &mut ground, item.area, self.lift, colour);
                    }
                }
                NodeKind::Image if node.items.is_empty() => {
                    self.billboards.push((&node.path, node.area, self.lift));
                }
                _ => {}
            }
            ui.painter().extend(ground);
        }
    }

    /// Draws the queued towers, the farthest first so nearer towers stand in
    /// front of them and the tiers of a building above it, and reports each
    /// tower and building as drawn, with the tower one of whose lines is
    /// under `pointer`. Towers in front of the tower pointed at, or else the
    /// selected one, turn to glass where they would hide it, but not the
    /// tiers of its own building. Far out a floor is one point at its
    /// indentation; close enough, one point per token. Skybridges are drawn
    /// in pieces among the towers, each at its own depth, so a tower in
    /// front of a piece hides it; so are the cables, when shown.
    fn towers(
        &mut self,
        ui: &egui::Ui,
        pointer: Option<Pos2>,
    ) -> (
        Vec<DrawnTower>,
        Vec<DrawnBuilding>,
        Vec<DrawnBridge>,
        Option<PathBuf>,
    ) {
        let view = self.view;
        let depth = |item: &Item| depth(&view, item);
        // The items of a building are drawn together at the depth of its
        // base, so what it stands on goes first.
        let mut towers = std::mem::take(&mut self.towers);
        for tower in &mut towers {
            tower.order = (depth(tower.base), depth(tower.item));
        }
        towers.sort_by(|one, other| {
            (one.order.0.total_cmp(&other.order.0))
                .then(one.order.1.total_cmp(&other.order.1))
                .then(one.item.base.total_cmp(&other.item.base))
        });
        let scale = self.view.scale();
        let boxes = scale < BOXES_BELOW;
        let tokens = scale >= TOKENS_FROM;
        let point = if tokens {
            (scale * 0.32).clamp(1.0, 3.0)
        } else {
            1.8
        };
        // Close enough, a floor is its line, written along the wall with one
        // character per column, so each letter stands where its point stood.
        let advance = ui.fonts_mut(|fonts| fonts.glyph_width(&egui::FontId::monospace(1.0), '0'));
        let font = city::COLUMN * scale / advance.max(f32::EPSILON);
        let letters = (font >= LETTERS_FROM).then_some(font);
        // Only code and the bays of a library show their lines from afar;
        // other buildings only close enough to read them, a vault never.
        let shows_floors = |tower: &Standing| match tower.form() {
            Form::Tower | Form::Skyscraper | Form::Library => true,
            Form::Vault { .. } => false,
            _ => letters.is_some(),
        };
        let mut drawn: Vec<DrawnTower> = towers
            .iter()
            .map(|tower| (tower, tower.item))
            .map(|(tower, item)| DrawnTower {
                path: item.path.clone(),
                colour: tower.colour,
                floors: if boxes || !shows_floors(tower) {
                    vec![self.middle(item)]
                } else {
                    (0..item.floors.len())
                        .map(|index| {
                            let z = city::floor_z(item, index);
                            let (start, end) = city::floor_span(item, index);
                            Span {
                                start: self.view.to_screen(start, z),
                                end: self.view.to_screen(end, z),
                            }
                        })
                        .collect()
                },
                outline: Rect::from_points(&self.solid(item).concat()),
                glass: false,
                light: 1.0,
                cellar: city::in_cellar(item),
            })
            .collect();
        let hovered = pointer.and_then(|at| pick(&drawn, at, self.underground));
        let focus = hovered.as_deref().or(self.selected);
        if let Some(index) = drawn
            .iter()
            .position(|tower| Some(tower.path.as_path()) == focus)
        {
            let hidden = drawn[index].outline;
            let own = towers[index].base;
            for (standing, tower) in towers.iter().zip(&mut drawn).skip(index + 1) {
                tower.glass = tower.outline.intersects(hidden) && !std::ptr::eq(standing.base, own);
            }
        }
        let mut built = Vec::new();

        // Far out a building keeps its form, spire and beacon; its doors,
        // courtyard, forecourt, masts, cables and crowns are left out, and a
        // skybridge is drawn in one piece.
        let details = !boxes;
        // The skybridges between buildings, cut into pieces, the farthest
        // first.
        let bridges = self.bridges;
        let mut pieces: Vec<Piece> = bridges
            .iter()
            .enumerate()
            .flat_map(|(index, bridge)| {
                let ends = ((bridge.start, bridge.z), (bridge.end, bridge.z));
                let cut = if details {
                    crate::buildings::pieces(ends.0, ends.1)
                } else {
                    vec![[ends.0, ends.1]]
                };
                cut.into_iter()
                    .map(move |ends| Piece::of(&view, Overlay::Bridge(index), ends))
            })
            .collect();
        // How high the masts of `item` stand: on its roof, or in the cellar
        // hanging from its bottom.
        let mast_z = |item: &Item, hangs: bool| {
            if hangs {
                item.base - city::reach(item)
            } else {
                city::height(item)
            }
        };
        // The cables from the tips of the masts shown to the roofs of the
        // buildings they call; with a selection, only its own.
        let mut cables: Vec<Cable> = Vec::new();
        for tower in towers.iter().filter(|_| self.cables && details) {
            let (item, base) = (tower.item, &tower.base.path);
            let hangs = city::in_cellar(item);
            let Some(trim) = tower.trim.filter(|_| {
                hangs == self.underground
                    && tower.top
                    && self
                        .selected
                        .is_none_or(|selected| base.starts_with(selected))
            }) else {
                continue;
            };
            let n = trim.masts.len();
            for (i, mast) in trim.masts.iter().enumerate() {
                let tip = crate::buildings::mast(item.area, mast_z(item, hangs), i, n, hangs);
                let overlay = Overlay::Cable(cables.len());
                cables.push((base.clone(), mast.target.clone()));
                pieces.extend(
                    crate::buildings::pieces(tip, mast.roof)
                        .into_iter()
                        .map(|ends| Piece::of(&view, overlay, ends)),
                );
            }
        }
        // The billboards of the images, each where its block is.
        let billboards = std::mem::take(&mut self.billboards);
        for (index, (_, area, lift)) in billboards.iter().enumerate() {
            let ends = [(area.min, *lift), (area.max, *lift)];
            pieces.push(Piece::of(&view, Overlay::Billboard(index), ends));
        }
        pieces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        // The pieces of each skybridge drawn: how many towers were drawn
        // before it, and its middle on screen.
        let mut laid: Vec<Vec<(usize, Pos2)>> = vec![Vec::new(); bridges.len()];
        let mut next = pieces.iter().peekable();
        let above = if self.underground {
            ABOVE_UNDERGROUND
        } else {
            1.0
        };
        let mut lay = |piece: &Piece, before: usize, shapes: &mut Vec<Shape>| {
            piece.draw(&view, shapes, above);
            if let Overlay::Bridge(index) = piece.overlay {
                laid[index].push((before, piece.middle));
            }
        };

        let turn = self.facing.map_or(0.0, |(_, turn)| turn);
        let clip = ui.painter().clip_rect();
        let mut shapes = Vec::new();
        // The buildings whose ground works, a hall or a plinth, are drawn.
        let mut founded = std::collections::HashSet::new();
        for (index, (standing, tower)) in towers.iter().zip(drawn.iter_mut()).enumerate() {
            let item = standing.item;
            while let Some(piece) = next.next_if(|piece| piece.depth < depth(item)) {
                lay(piece, index, &mut shapes);
            }
            let focused = Some(item.path.as_path()) == focus;
            let mut lit = if focused { FOCUSED } else { 1.0 };
            // Written walls crowd each other: the one being read stands out.
            if letters.is_some() && focus.is_some() && !focused {
                lit *= 0.5;
            }
            if tower.glass {
                lit *= GLASS;
            }
            // While one tower is explained, the rest of the city steps back
            // so the colours of its floors read.
            if self.tint.is_some() && Some(item.path.as_path()) != self.selected {
                lit *= UNEXPLAINED;
            }
            if let Some((pivot, turn)) = self.facing {
                lit *= 1.0 + (fog(item, pivot) - 1.0) * turn;
            }
            // The cellar shows faintly; the underground view brings it
            // forward and the city above steps back.
            lit *= match (city::in_cellar(item), self.underground) {
                (true, false) => CELLAR,
                (false, true) => ABOVE_UNDERGROUND,
                _ => 1.0,
            };
            tower.light = lit;
            let (base, trim, is_base) = (standing.base, standing.trim, standing.is_base);
            let spire = item.declaration.declares == Declares::EntryPoint;
            let form = standing.form();
            // Masts stand above the ground in the city, and hang from a
            // tower in the cellar only underground.
            let hangs = city::in_cellar(item);
            let masts = trim
                .filter(|_| details && hangs == self.underground)
                .map_or(0, |trim| trim.masts.len());
            let forecourt = trim
                .filter(|_| details)
                .and_then(|trim| crate::buildings::forecourt(trim.callers));
            let courtyard = details && trim.is_some_and(|trim| trim.courtyard);
            let doors = trim
                .filter(|_| details)
                .map_or(0, |trim| trim.doors.min(buildings::DOORS));
            let crowns = trim
                .filter(|trim| details && standing.top && !trim.crowns.is_empty())
                .map(|trim| &trim.crowns);
            if let Some(trim) = trim.filter(|_| is_base) {
                let houses = match form {
                    Form::Terrace => crate::buildings::houses(item).to_vec(),
                    _ => Vec::new(),
                };
                built.push(DrawnBuilding {
                    path: item.path.clone(),
                    form,
                    tiers: trim.tiers.clone(),
                    crowns: if details {
                        trim.crowns.clone()
                    } else {
                        Vec::new()
                    },
                    houses,
                    spire,
                    doors,
                    courtyard,
                    forecourt: forecourt.is_some(),
                    masts: vec![hangs; masts],
                });
            }
            if !clip.intersects(tower.outline) {
                continue;
            }
            let colour = tower.colour;
            // On the ground around its base: a forecourt in front and a
            // courtyard behind.
            if is_base {
                if let Some(deep) = forecourt {
                    buildings::forecourt(&view, &mut shapes, item, deep, colour, lit);
                }
                if courtyard {
                    buildings::courtyard(&view, &mut shapes, item, lit);
                }
            }
            // What a building stands on goes with its first item drawn.
            if let Some(ground) = trim
                .and_then(|trim| trim.foundation)
                .filter(|_| founded.insert(&base.path))
            {
                match form {
                    Form::Library => {
                        buildings::library_hall(&view, &mut shapes, ground, item.base, colour, lit);
                    }
                    _ => buildings::plinth(&view, &mut shapes, ground, item.base, colour, lit),
                }
            }
            match form {
                Form::Terrace if is_base => {
                    let houses = crate::buildings::houses(item);
                    buildings::terrace(&view, &mut shapes, item, houses, colour, lit);
                }
                Form::Monument if is_base => {
                    buildings::monument(&view, &mut shapes, item, colour, lit)
                }
                Form::Vault { glass } => buildings::vault(&view, &mut shapes, item, glass, lit),
                Form::Ghost | Form::Slab => {
                    buildings::outline(&view, &mut shapes, item, form, colour, lit)
                }
                _ => self.body(&mut shapes, item, colour, lit, turn),
            }
            match form {
                Form::Depot => buildings::depot(&view, &mut shapes, item, colour, lit),
                Form::CustomsHouse => {
                    buildings::upper_storey(&view, &mut shapes, item, colour, lit)
                }
                Form::Plant if is_base && details => {
                    buildings::pipes(&view, &mut shapes, item, colour, lit)
                }
                _ => {}
            }
            // The top of its building wears the crowns; an entry point its
            // spire.
            if let Some(crowns) = crowns {
                let roof = city::height(item);
                buildings::crowns(&view, &mut shapes, item.area, roof, crowns, lit);
            }
            if spire {
                buildings::spire(&view, &mut shapes, item, lit);
            }
            if is_base && doors > 0 {
                buildings::doors(&view, &mut shapes, item, doors, lit);
            }
            if masts > 0 && standing.top {
                let z = mast_z(item, hangs);
                buildings::masts(&view, &mut shapes, item.area, z, masts, hangs, colour, lit);
            }
            if !shows_floors(standing) {
                continue;
            }
            if boxes {
                // Its lines, too close to tell apart, light its front.
                let [front, _, _] = self.solid(item);
                let face = glow(tower.colour, LINE * BOX_FACE * lit);
                shapes.push(Shape::convex_polygon(front, face, Stroke::NONE));
                continue;
            }
            // Explained, a tower wears the colour of the pull request that
            // last wrote each floor instead of its district's.
            let tinted = self
                .tint
                .filter(|_| Some(item.path.as_path()) == self.selected);
            for (index, floor) in item.floors.iter().enumerate() {
                let z = city::floor_z(item, index);
                let bright = if floor.calls.is_empty() {
                    LINE
                } else {
                    CALLING_LINE
                };
                let colour = match tinted.and_then(|tint| tint.get(index)) {
                    Some(group) => PR_HUES[group % PR_HUES.len()],
                    None => tower.colour,
                };
                let fill = glow(colour, bright * lit);
                if let Some(font) = letters {
                    shapes.push(self.written(ui, item, index, font, colour, lit));
                    continue;
                }
                if scale >= DASHES_FROM {
                    for (start, end) in city::token_spans(item, index) {
                        let line = [self.view.to_screen(start, z), self.view.to_screen(end, z)];
                        shapes.push(Shape::line_segment(line, Stroke::new(point, fill)));
                    }
                    continue;
                }
                if tokens {
                    for ground in city::token_points(item, index) {
                        let at = self.view.to_screen(ground, z);
                        let square = Rect::from_center_size(at, vec2(point, point));
                        shapes.push(Shape::rect_filled(square, 0.0, fill));
                    }
                } else {
                    // Far out a floor is one stroke as wide as its line, so a
                    // tower keeps its width.
                    let (start, end) = city::floor_span(item, index);
                    let line = [self.view.to_screen(start, z), self.view.to_screen(end, z)];
                    shapes.push(Shape::line_segment(line, Stroke::new(FAR_STROKE, fill)));
                }
            }
        }
        for piece in next {
            lay(piece, drawn.len(), &mut shapes);
        }
        built.extend(billboards.iter().map(|(path, _, _)| DrawnBuilding {
            path: path.to_path_buf(),
            form: Form::Billboard,
            tiers: Vec::new(),
            crowns: Vec::new(),
            houses: Vec::new(),
            spire: false,
            doors: 0,
            courtyard: false,
            forecourt: false,
            masts: Vec::new(),
        }));
        // A skybridge pointed at, where no tower is, is lit and says what it
        // joins.
        let over_bridge = pointer.filter(|_| hovered.is_none()).and_then(|at| {
            bridges.iter().find(|bridge| {
                let top = bridge.z + buildings::BRIDGE_DEPTH / 2.0;
                let span = Span {
                    start: self.view.to_screen(bridge.start, top),
                    end: self.view.to_screen(bridge.end, top),
                };
                span.distance(at) <= BRIDGE_REACH
            })
        });
        if let Some(bridge) = over_bridge {
            buildings::light_bridge(&self.view, &mut shapes, bridge);
        }
        self.bridge_hovered = over_bridge.cloned();
        let bridges = bridges
            .iter()
            .zip(laid)
            .map(|(bridge, pieces)| DrawnBridge {
                from: bridge.from.clone(),
                to: bridge.to.clone(),
                z: bridge.z,
                pieces,
            })
            .collect();
        // Brackets at the corners of the selected tower, and fainter ones
        // around the one pointed at.
        for (tower, colour) in [
            (self.selected, Color32::WHITE.gamma_multiply(0.85)),
            (hovered.as_deref(), ATTENTION.gamma_multiply(0.45)),
        ] {
            if let Some(tower) = tower.and_then(|path| drawn.iter().find(|t| t.path == path)) {
                brackets(&mut shapes, tower.outline.expand(4.0), colour);
            }
        }
        ui.painter().extend(shapes);
        if let (Some(at), Some(tower)) = (
            pointer,
            towers
                .iter()
                .find(|tower| Some(tower.item.path.as_path()) == hovered.as_deref()),
        ) {
            tip(ui, at, tower.item);
        }
        if let (Some(at), Some(bridge)) = (pointer, over_bridge) {
            buildings::bridge_tip(ui, at, bridge);
        }
        self.cables_drawn = cables;
        (drawn, built, bridges, hovered)
    }

    /// The corners of `item`'s box on screen: its front face, and the side
    /// at the end of its lines and its roof, each as four corners. A box in
    /// the cellar hangs from the ground, its "roof" at its bottom.
    fn solid(&self, item: &Item) -> [Vec<Pos2>; 3] {
        let length = item.area.width().max(item.area.height());
        let ground = item.base;
        let height = if city::in_cellar(item) {
            item.base - city::reach(item)
        } else {
            city::height(item)
        };
        let at = |point: Pos2, z: f32| self.view.to_screen(point, z);
        let (front_start, front_end) = (city::front(item, 0.0), city::front(item, length));
        let (back_start, back_end) = (city::back(item, 0.0), city::back(item, length));
        [
            vec![
                at(front_start, ground),
                at(front_end, ground),
                at(front_end, height),
                at(front_start, height),
            ],
            vec![
                at(front_end, ground),
                at(back_end, ground),
                at(back_end, height),
                at(front_end, height),
            ],
            vec![
                at(front_start, height),
                at(front_end, height),
                at(back_end, height),
                at(back_start, height),
            ],
        ]
    }

    /// The span up the middle of `item`'s front, from its foot to its roof.
    fn middle(&self, item: &Item) -> Span {
        let [front, _, _] = self.solid(item);
        Span {
            start: front[0] + (front[1] - front[0]) / 2.0,
            end: front[3] + (front[2] - front[3]) / 2.0,
        }
    }

    /// Draws `item`'s box: a dark front face, a darker side and a roof, edged
    /// in `colour`, so a nearer building hides the ones behind it. `lit`
    /// thins it out with the tower; the side and roof fade as the camera
    /// turns `turn` of the way to face a facade.
    fn body(&self, shapes: &mut Vec<Shape>, item: &Item, colour: Color32, lit: f32, turn: f32) {
        let [front, side, roof] = self.solid(item);
        let opacity = (lit.min(1.0) * SOLID).clamp(0.0, SOLID);
        let edge = colour.gamma_multiply((lit * EDGE_LIGHT).min(1.0));
        let body = 1.0 - turn;
        if body > 0.0 {
            let side_fill = SIDE.gamma_multiply(opacity * body);
            shapes.push(Shape::convex_polygon(side.clone(), side_fill, Stroke::NONE));
            let roof_fill = GROUND.gamma_multiply(opacity * body);
            let roof_edge = Stroke::new(1.0, edge.gamma_multiply(body));
            shapes.push(Shape::convex_polygon(roof, roof_fill, roof_edge));
            shapes.push(Shape::line_segment([side[1], side[2]], roof_edge));
        }
        let face = GROUND.gamma_multiply(opacity);
        shapes.push(Shape::convex_polygon(front.clone(), face, Stroke::NONE));
        let edge = Stroke::new(1.0, edge);
        shapes.push(Shape::line_segment([front[0], front[3]], edge));
        shapes.push(Shape::line_segment([front[1], front[2]], edge));
    }

    /// Floor `index` of `item` as its line of code, written along the wall at
    /// `font` size from the wall's start, a character per column, cut off at
    /// the wall's end.
    fn written(
        &self,
        ui: &egui::Ui,
        item: &Item,
        index: usize,
        font: f32,
        colour: Color32,
        lit: f32,
    ) -> Shape {
        let floor = &item.floors[index];
        let z = city::floor_z(item, index);
        let start = self.view.to_screen(city::front(item, 0.0), z);
        let end = self.view.to_screen(city::front(item, city::WIDEST), z);
        let length = item.area.width().max(item.area.height());
        let columns = (length / city::COLUMN).round() as usize;
        let text: String = floor
            .text
            .replace('\t', "    ")
            .chars()
            .take(columns)
            .collect();
        let bright = if floor.calls.is_empty() { 0.75 } else { 1.0 };
        let galley = ui.painter().layout_no_wrap(
            text,
            egui::FontId::monospace(font),
            colour.gamma_multiply((bright * lit).min(1.0)),
        );
        let angle = (end - start).angle();
        // The galley hangs from its top left corner; lift it so the letters
        // stand on the floor's line.
        let lift = egui::emath::Rot2::from_angle(angle) * vec2(0.0, -galley.size().y * 0.8);
        egui::epaint::TextShape::new(start + lift, galley, colour)
            .with_angle(angle)
            .into()
    }

    /// Whether the outline of the folder at `path` on `area`, raised by
    /// `lift`, is wide enough on screen for `name` to be written on it.
    fn outline_has_room(&self, area: Rect, path: &Path, name: &str, lift: f32) -> bool {
        let bounds = Rect::from_points(&self.round(area, path, lift));
        bounds.width() > NAME_ROOM * name_width(name, FOLDER_NAME)
    }

    /// The outline of the element at `path`, round what it holds, if the
    /// network is worked out.
    fn outline_of(&self, path: &Path) -> Option<&'a [Pos2]> {
        let outline = self.outlines?.get(path)?;
        (outline.len() >= 3).then_some(outline.as_slice())
    }

    /// On screen, raised by `lift`: in the folders lens the outline of the
    /// folder or district at `path` on `area`, and until the network is
    /// worked out, and in every other lens, the circle around `area`.
    fn round(&self, area: Rect, path: &Path, lift: f32) -> Vec<Pos2> {
        match self.outline_of(path).filter(|_| self.folders) {
            Some(outline) => outline
                .iter()
                .map(|p| self.view.to_screen(*p, lift))
                .collect(),
            None => self.view.ring(round(area), lift),
        }
    }

    /// On screen, raised by `lift`: the outline of the block at `path` on
    /// `area`, or until the network is worked out its rectangle.
    fn plot(&self, area: Rect, path: &Path, lift: f32) -> Vec<Pos2> {
        self.plotted(self.outline_of(path), area, lift)
    }

    /// On screen, raised by `lift`: `outline`, or without one the rectangle
    /// `area`.
    fn plotted(&self, outline: Option<&[Pos2]>, area: Rect, lift: f32) -> Vec<Pos2> {
        match outline {
            Some(outline) => outline
                .iter()
                .map(|p| self.view.to_screen(*p, lift))
                .collect(),
            None => self.view.corners(area, lift),
        }
    }

    /// Writes `name` on the ground `area` as `naming` says, outlines that
    /// ground when `path` is selected, and notes a click on the name.
    fn label(
        &mut self,
        ui: &mut egui::Ui,
        area: Rect,
        naming: Naming,
        name: &str,
        path: &Path,
        lift: f32,
    ) {
        let bounds = match naming {
            Naming::District(_) | Naming::FolderOutline => {
                Rect::from_points(&self.round(area, path, lift))
            }
            Naming::Folder | Naming::Block(_) => Rect::from_points(&self.view.corners(area, lift)),
        };
        let strip = TITLE.min(bounds.height());
        let front = Rect::from_min_max(pos2(bounds.min.x, bounds.max.y - strip), bounds.max);
        let quiet = if self.view.scale() >= NAMES_FROM {
            DIM
        } else {
            Color32::TRANSPARENT
        };
        let (at, text) = match naming {
            Naming::District(colour) => {
                // Just as wide as the name, so it neither truncates under a
                // small plaza nor covers what stands beside it under a
                // large one.
                let size = if self.view.scale() > 1.0 { 15.0 } else { 12.0 };
                let width = name_width(name, size);
                let at = Rect::from_center_size(front.center(), vec2(width, front.height()));
                (at, RichText::new(name).strong().size(size).color(colour))
            }
            // On the front of its outline, once the outline has room for it;
            // until then named for the accessibility tree, and clickable,
            // only.
            Naming::FolderOutline => {
                let width = name_width(name, FOLDER_NAME);
                let written = self.outline_has_room(area, path, name, lift);
                self.folder_names.push((path.to_path_buf(), written));
                let colour = if written {
                    INK.gamma_multiply(0.75)
                } else {
                    Color32::TRANSPARENT
                };
                let at = Rect::from_center_size(
                    pos2(bounds.center().x, bounds.max.y - FOLDER_NAME),
                    vec2(width, FOLDER_NAME + 6.0),
                );
                (at, RichText::new(name).size(FOLDER_NAME).color(colour))
            }
            // Folders below a district and blocks without code are ground,
            // not landmarks: named for the accessibility tree only.
            Naming::Folder | Naming::Block(false) => (
                front,
                RichText::new(name)
                    .monospace()
                    .size(11.0)
                    .color(Color32::TRANSPARENT),
            ),
            Naming::Block(true) => {
                let front = self.view.to_screen(pos2(area.center().x, area.max.y), lift);
                let below = Rect::from_min_size(
                    pos2(front.x - bounds.width() / 2.0, front.y + 4.0),
                    vec2(bounds.width(), 16.0),
                );
                (
                    below,
                    RichText::new(name).monospace().size(11.0).color(quiet),
                )
            }
        };
        // The sense goes on the response, not the widget, so the entry stays a
        // label in the accessibility tree.
        let response = ui
            .put(
                at,
                egui::Label::new(text)
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .interact(if self.arranging && matches!(naming, Naming::District(_)) {
                Sense::click_and_drag()
            } else {
                Sense::click()
            });
        if response.dragged() || response.drag_stopped() {
            let mut by = response.drag_delta();
            // The drag starts once the pointer has moved a little: the move
            // counts from where the button went down.
            if response.drag_started()
                && let (Some(down), Some(now)) = (
                    ui.input(|i| i.pointer.press_origin()),
                    response.interact_pointer_pos(),
                )
            {
                by = now - down;
            }
            self.name_dragged = Some((path.to_path_buf(), by, response.drag_stopped()));
        }
        if self.selected == Some(path) {
            let stroke = ui.visuals().selection.stroke;
            let outline = match naming {
                Naming::District(_) | Naming::FolderOutline => self.round(area, path, lift),
                Naming::Block(_) => self.plot(area, path, lift),
                _ => self.view.corners(area, lift),
            };
            ui.painter().add(Shape::closed_line(outline, stroke));
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_selected(true));
        }
        if response.clicked() {
            self.clicked = Some(path.to_path_buf());
        }
    }
}

/// The square around the circle around the ground `area`: a district or
/// folder drawn round is drawn in it.
fn round(area: Rect) -> Rect {
    Rect::from_center_size(area.center(), Vec2::splat(area.size().length()))
}

/// How wide `name` is on screen at `size`, with a little room either side.
fn name_width(name: &str, size: f32) -> f32 {
    name.chars().count() as f32 * size * 0.62 + 16.0
}

/// A cable drawn: from the base of the building whose mast it leaves, to
/// the base of the building it calls.
pub type Cable = (PathBuf, PathBuf);

/// What a piece drawn between the buildings belongs to: a skybridge, a
/// cable or a billboard, by its index among those drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Overlay {
    Bridge(usize),
    Cable(usize),
    Billboard(usize),
}

/// A tower queued to be drawn, with the building it stands in: the base of
/// that building, whether the tower is the base and whether its top, what
/// the building shows of itself, and the depths it is drawn in order of.
struct Standing<'a> {
    item: &'a Item,
    colour: Color32,
    base: &'a Item,
    is_base: bool,
    top: bool,
    trim: Option<&'a Trim>,
    order: (f32, f32),
}

impl Standing<'_> {
    fn form(&self) -> Form {
        self.trim.map_or(Form::Tower, |trim| trim.form)
    }
}

/// A piece of something drawn between the buildings, with its ends on the
/// ground and their heights, how far back it lies, as a tower's
/// [`depth`], and where its middle is on screen.
struct Piece {
    overlay: Overlay,
    ends: [(Pos2, f32); 2],
    depth: f32,
    middle: Pos2,
}

impl Piece {
    fn of(view: &View, overlay: Overlay, ends: [(Pos2, f32); 2]) -> Piece {
        let [(a, za), (b, zb)] = ends;
        let ground = a + (b - a) / 2.0;
        Piece {
            overlay,
            ends,
            depth: view.to_screen(ground, 0.0).y,
            middle: view.to_screen(ground, (za + zb) / 2.0),
        }
    }

    /// Draws the piece, a skybridge `above` as bright as the city above the
    /// ground; a cable is shown only where its masts are, so at full light.
    fn draw(&self, view: &View, shapes: &mut Vec<Shape>, above: f32) {
        let [(a, z), (b, _)] = self.ends;
        match self.overlay {
            Overlay::Bridge(_) => buildings::bridge(view, shapes, (a, b), z, above),
            Overlay::Cable(_) => buildings::cable(view, shapes, self.ends),
            Overlay::Billboard(_) => {
                let ground = Rect::from_two_pos(a, self.ends[1].0);
                buildings::billboard(view, shapes, ground, z, above);
            }
        }
    }
}

/// How far back `item` stands as `view` sees it: lower on screen is nearer,
/// and towers further back are drawn first.
fn depth(view: &View, item: &Item) -> f32 {
    view.to_screen(item.area.center(), 0.0).y
}

/// The minimap's size, and its distance from the panel's edge.
const MINIMAP: Vec2 = vec2(210.0, 140.0);
const MINIMAP_MARGIN: f32 = 12.0;

/// Draws the minimap of `map` in the bottom right corner of `panel`: the city
/// seen as the main view sees it, one point per tower in its district's
/// colour, and a frame around the part of the ground `view` shows. Returns
/// the path of the district clicked on it this frame. Each district is named
/// "<district> on the minimap" in the accessibility tree, and the frame "View
/// on the minimap".
pub fn minimap(ui: &mut egui::Ui, map: &ProjectMap, view: &View, panel: Rect) -> Option<PathBuf> {
    let seen = map.districts.iter().fold(Rect::NOTHING, |seen, district| {
        seen.union(iso_bounds(district.area))
    });
    if map.is_empty() || !seen.is_positive() {
        return None;
    }
    let at = Rect::from_min_size(panel.max - MINIMAP - Vec2::splat(MINIMAP_MARGIN), MINIMAP);
    let inner = at.shrink(6.0);
    let k = (inner.width() / seen.width()).min(inner.height() / seen.height());
    let offset = inner.center().to_vec2() - seen.center().to_vec2() * k;
    let to_minimap = |ground: Pos2| (iso(ground, 0.0).to_vec2() * k + offset).to_pos2();

    let painter = ui.painter_at(at);
    painter.rect(
        at,
        0.0,
        PANEL.gamma_multiply(0.92),
        Stroke::new(1.0, DIM.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    let mut clicked = None;
    for (index, district) in map.districts.iter().enumerate() {
        let colour = district_colour(index);
        let mut home = home_of(&map.homes);
        for item in crate::graph::items(district) {
            let fill = match item.kind {
                ItemKind::Test => TEST_COLOUR,
                _ => home(item).map_or(colour, district_colour),
            };
            let point = to_minimap(item.area.center());
            painter.rect_filled(
                Rect::from_center_size(point, vec2(1.6, 1.6)),
                0.0,
                fill.gamma_multiply(0.8),
            );
        }
        let area = district.area;
        let corners = [
            area.left_top(),
            area.right_top(),
            area.right_bottom(),
            area.left_bottom(),
        ];
        let ring = Rect::from_points(&corners.map(to_minimap));
        let id = ui.id().with(("minimap", &district.path));
        let response = ui.interact(ring.intersect(at), id, Sense::click());
        let label = format!("{} on the minimap", district.name);
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Button);
            node.set_label(label);
        });
        if response.clicked() {
            clicked = Some(district.path.clone());
        }
    }
    let corners: Vec<Pos2> = [
        panel.left_top(),
        panel.right_top(),
        panel.right_bottom(),
        panel.left_bottom(),
    ]
    .into_iter()
    .map(|corner| to_minimap(view.to_ground(corner)))
    .collect();
    let frame = Rect::from_points(&corners).intersect(at);
    painter.add(Shape::closed_line(
        corners,
        Stroke::new(1.0, Color32::WHITE.gamma_multiply(0.8)),
    ));
    painter.text(
        at.min + vec2(6.0, 4.0),
        egui::Align2::LEFT_TOP,
        "minimap",
        egui::FontId::monospace(10.0),
        DIM.gamma_multiply(0.9),
    );
    let response = ui.interact(frame, ui.id().with("minimap view"), Sense::hover());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::Image);
        node.set_label("View on the minimap");
        // A widget that only senses hovering gets no bounds of its own.
        let bounds = egui::accesskit::Rect {
            x0: frame.min.x.into(),
            y0: frame.min.y.into(),
            x1: frame.max.x.into(),
            y1: frame.max.y.into(),
        };
        node.set_bounds(bounds);
    });
    clicked
}

/// Rows inside a tower: their height, and where the line number ends, the
/// line starts, the pull request that last wrote it is marked and the calls
/// start, from the panel's left edge.
const ROW: f32 = 17.0;
const NUMBER_END: f32 = 70.0;
const TEXT_START: f32 = 86.0;
const MARK_START: f32 = 620.0;
const CALLS_START: f32 = 760.0;

/// Draws the tower at `item` from inside, face on, over the city dimmed
/// behind it: one row per floor in source order, each with its line number,
/// its line, a tick under each token in the tower's colour, and the items it
/// calls on the right; floor `floor`, from 0, selected. Returns the floor
/// whose row was clicked this frame. Each row is named by its text in the
/// accessibility tree and marked when selected.
pub fn inside(
    ui: &mut egui::Ui,
    map: &ProjectMap,
    item: &Path,
    floor: usize,
    panel: Rect,
) -> Option<usize> {
    let item = map.item(item)?;
    // Over the city: the city takes no click while a tower is open.
    ui.interact(panel, ui.id().with("inside"), Sense::click());
    let painter = ui.painter_at(panel);
    painter.rect_filled(panel, 0.0, NIGHT.gamma_multiply(0.86));
    let colour = match item.kind {
        ItemKind::Test => TEST_COLOUR,
        _ => district_colour_of(map, item),
    };
    let left = panel.min.x + 40.0;
    let file = map.find(item.path.parent()?).map_or("", |found| found.name);
    painter.text(
        pos2(left, panel.min.y + 24.0),
        egui::Align2::LEFT_TOP,
        format!("Inside {}", item.name),
        egui::FontId::proportional(18.0),
        INK,
    );
    let selected_line = item.floors.get(floor).map_or(0, |f| f.number);
    painter.text(
        pos2(left, panel.min.y + 50.0),
        egui::Align2::LEFT_TOP,
        format!(
            "{file} · floor {} of {} · line {selected_line}   j k move · Esc leave",
            floor + 1,
            item.floors.len()
        ),
        egui::FontId::monospace(12.0),
        DIM,
    );

    let top = panel.min.y + 84.0;
    let rows = ((panel.max.y - top) / ROW).floor().max(1.0) as usize;
    let first = floor
        .saturating_sub(rows / 2)
        .min(item.floors.len().saturating_sub(rows));
    let mut clicked = None;
    for (index, line) in item.floors.iter().enumerate().skip(first).take(rows) {
        let y = top + (index - first) as f32 * ROW;
        let row = Rect::from_min_size(pos2(left - 6.0, y), vec2(panel.max.x - left - 20.0, ROW));
        let selected = index == floor;
        if selected {
            painter.rect_filled(row, 0.0, ATTENTION_SOFT.gamma_multiply(0.9));
            let bar = Rect::from_min_size(row.min, vec2(2.0, ROW));
            painter.rect_filled(bar, 0.0, ATTENTION);
        }
        let middle = y + ROW / 2.0;
        painter.text(
            pos2(left + NUMBER_END, middle),
            egui::Align2::RIGHT_CENTER,
            line.number.to_string(),
            egui::FontId::monospace(11.0),
            DIM.gamma_multiply(0.9),
        );
        let text = painter.layout_no_wrap(
            line.text.replace('\t', "    "),
            egui::FontId::monospace(12.0),
            INK.gamma_multiply(0.92),
        );
        let column = text.size().x / line.text.chars().count().max(1) as f32;
        let text_at = pos2(left + TEXT_START, middle - text.size().y / 2.0);
        for token in &line.tokens {
            let tick = Rect::from_min_size(
                pos2(text_at.x + token.start as f32 * column, middle + 6.0),
                vec2(3.0, 1.5),
            );
            painter.rect_filled(tick, 0.0, colour.gamma_multiply(0.35));
        }
        painter.galley(text_at, text, INK);
        // What built this floor: the pull request that last wrote it and the
        // stories its commit names. Nothing for an uncommitted floor or a
        // commit without a pull request.
        let mark = crate::why::floor_commit(map, item, index).and_then(why::floor_mark);
        if let Some(mark) = &mark {
            painter.text(
                pos2(left + MARK_START, middle),
                egui::Align2::LEFT_CENTER,
                mark,
                egui::FontId::monospace(11.0),
                DIM,
            );
        }
        let calls: Vec<&str> = line
            .calls
            .iter()
            .filter_map(|call| map.find(call).map(|found| found.name))
            .collect();
        if !calls.is_empty() {
            painter.text(
                pos2(left + CALLS_START, middle),
                egui::Align2::LEFT_CENTER,
                format!("→ {}", calls.join(", ")),
                egui::FontId::monospace(12.0),
                FLOW,
            );
        }

        let response = ui.interact(row, ui.id().with(("floor", index)), Sense::click());
        let mut name = format!("{:>5}  {}", line.number, line.text);
        if let Some(mark) = &mark {
            name.push_str(&format!("    {mark}"));
        }
        if !calls.is_empty() {
            name.push_str(&format!("    calls {}", calls.join(", ")));
        }
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::ListBoxOption);
            node.set_label(name);
            node.set_toggled(if selected {
                egui::accesskit::Toggled::True
            } else {
                egui::accesskit::Toggled::False
            });
        });
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

/// The district each item keeps the colour of, by `homes`: that of the file
/// it stands with. Items come file by file, so the file last looked up is
/// kept.
fn home_of(
    homes: &std::collections::BTreeMap<PathBuf, usize>,
) -> impl FnMut(&Item) -> Option<usize> {
    let mut last: Option<(PathBuf, Option<usize>)> = None;
    move |item| {
        if homes.is_empty() {
            return None;
        }
        let file = item.standing_file()?;
        match &last {
            // Compared as bytes: as paths, component by component, it
            // costs as much as the lookup it saves.
            Some((seen, home)) if seen.as_os_str() == file.as_os_str() => *home,
            _ => {
                let home = homes.get(file).copied();
                last = Some((file.to_path_buf(), home));
                home
            }
        }
    }
}

/// The colour of the district `item` keeps: its home's, else that of the
/// innermost district whose path holds it.
fn district_colour_of(map: &ProjectMap, item: &Item) -> Color32 {
    if let Some(home) = item.standing_file().and_then(|file| map.homes.get(file)) {
        return district_colour(*home);
    }
    let path = &item.path;
    map.districts
        .iter()
        .enumerate()
        .filter(|(_, district)| path.starts_with(&district.path))
        .max_by_key(|(_, district)| district.path.as_os_str().len())
        .map_or(DISTRICT_COLOURS[0], |(index, _)| district_colour(index))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::{Directories, default_prefix_rule};

    #[test]
    fn a_folder_view_still_draws_the_city_around_it_faded() {
        use std::collections::BTreeMap;

        use crate::graph::{Floor, Item, ItemKind};

        let tower = |file: &str| Item {
            name: "f".to_owned(),
            path: Path::new("/p/shop").join(file).join("f"),
            kind: ItemKind::Function,
            tests: false,
            floors: vec![Floor {
                number: 1,
                text: "fn f() {}".to_owned(),
                indent: 0,
                width: 9,
                tokens: Vec::new(),
                calls: Vec::new(),
                imports: Vec::new(),
            }],
            area: Rect::NOTHING,
            base: 0.0,
            declaration: Default::default(),
        };
        let files = ["catalog/a.rs", "orders/b.rs"].map(PathBuf::from);
        let items = BTreeMap::from(files.clone().map(|file| {
            let item = tower(file.to_str().unwrap());
            (file, vec![item])
        }));
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of_code(
            "shop",
            Path::new("/p/shop"),
            &files,
            &strategy,
            items,
            &Default::default(),
        );
        let framing = Framing {
            area: map.extent,
            fill: 1.0,
            closest: 1.0,
            headroom: 0.0,
        };
        let catalog = Path::new("/p/shop/catalog");

        let ctx = egui::Context::default();
        let mut towers = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            towers = project(
                ui,
                &map,
                ViewOptions {
                    framing,
                    selected: None,
                    folder_view: Some(catalog),
                    pivot: None,
                    underground: false,
                    lanes: Lanes::default(),
                    picked: None,
                    network: None,
                    cables: false,
                    tint: None,
                },
            )
            .towers;
        });
        output.textures_delta.clear();

        assert_eq!(
            towers.len(),
            2,
            "the tower outside the folder view is drawn"
        );
        let colour = |file: &str| {
            let path = Path::new("/p/shop").join(file).join("f");
            towers.iter().find(|t| t.path == path).map(|t| t.colour)
        };
        let (inside, outside) = (
            colour("catalog/a.rs").unwrap(),
            colour("orders/b.rs").unwrap(),
        );
        assert!(
            outside.a() < inside.a(),
            "{outside:?} is not fainter than {inside:?}"
        );
    }

    #[test]
    fn far_out_a_tower_is_one_box_picked_by_its_middle() {
        use std::collections::BTreeMap;

        use crate::graph::{Floor, Item, ItemKind};

        let floor = |number: usize| Floor {
            number,
            text: "let a = 1;".to_owned(),
            indent: 0,
            width: 10,
            tokens: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
        };
        let file = PathBuf::from("catalog/a.rs");
        let path = Path::new("/p/shop/catalog/a.rs/f").to_path_buf();
        let item = Item {
            name: "f".to_owned(),
            path: path.clone(),
            kind: ItemKind::Function,
            tests: false,
            floors: (1..=3).map(floor).collect(),
            area: Rect::NOTHING,
            base: 0.0,
            declaration: Default::default(),
        };
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let items = BTreeMap::from([(file.clone(), vec![item])]);
        let map = ProjectMap::of_code(
            "shop",
            Path::new("/p/shop"),
            &[file],
            &strategy,
            items,
            &Default::default(),
        );
        // Framed as a part of a far larger city: far out.
        let framing = Framing {
            area: map.extent.expand(map.extent.width() * 200.0),
            fill: 1.0,
            closest: 1.0,
            headroom: 0.0,
        };

        let ctx = egui::Context::default();
        let mut towers = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            towers = project(
                ui,
                &map,
                ViewOptions {
                    framing,
                    selected: None,
                    folder_view: None,
                    pivot: None,
                    underground: false,
                    lanes: Lanes::default(),
                    picked: None,
                    network: None,
                    cables: false,
                    tint: None,
                },
            )
            .towers;
        });
        output.textures_delta.clear();

        let tower = towers.iter().find(|t| t.path == path).expect("drawn");
        assert_eq!(tower.floors.len(), 1, "one span, not one per floor");
        let middle = tower.floors[0].start + (tower.floors[0].end - tower.floors[0].start) / 2.0;
        assert_eq!(pick(&towers, middle, false), Some(path));
    }

    #[test]
    fn far_out_a_building_leaves_its_doors_and_masts_out() {
        use std::collections::BTreeMap;

        use crate::graph::{Declaration, Floor, Item, ItemKind};

        let root = Path::new("/p/shop");
        let item = |file: &str, name: &str, public: bool, part_of: Option<&str>| Item {
            name: name.to_owned(),
            path: root.join(file).join(name),
            kind: ItemKind::Function,
            tests: false,
            floors: vec![Floor {
                number: 1,
                text: "fn f() {}".to_owned(),
                indent: 0,
                width: 9,
                tokens: Vec::new(),
                calls: Vec::new(),
                imports: Vec::new(),
            }],
            area: Rect::NOTHING,
            base: 0.0,
            declaration: Declaration {
                public,
                part_of: part_of.map(|base| root.join(file).join(base)),
                ..Default::default()
            },
        };
        // A type with a public and a private method, the private one calling
        // into another file: a door, a courtyard and a mast.
        let mut close = item("catalog/a.rs", "close", false, Some("Shop"));
        close.floors[0].calls = vec![root.join("orders/b.rs/f")];
        let files = ["catalog/a.rs", "orders/b.rs"].map(PathBuf::from);
        let items = BTreeMap::from([
            (
                files[0].clone(),
                vec![
                    item("catalog/a.rs", "Shop", true, None),
                    item("catalog/a.rs", "open", true, Some("Shop")),
                    close,
                ],
            ),
            (files[1].clone(), vec![item("orders/b.rs", "f", true, None)]),
        ]);
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of_code("shop", root, &files, &strategy, items, &Default::default());
        let network = crate::Network::of(&map);
        let shop = root.join("catalog/a.rs/Shop");
        let drawn = |grow: f32| {
            let framing = Framing {
                area: map.extent.expand(map.extent.width() * grow),
                fill: 1.0,
                closest: 1.0,
                headroom: 0.0,
            };
            let ctx = egui::Context::default();
            let mut buildings = Vec::new();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                buildings = project(
                    ui,
                    &map,
                    ViewOptions {
                        framing,
                        selected: None,
                        folder_view: None,
                        pivot: None,
                        underground: false,
                        lanes: Lanes::default(),
                        picked: None,
                        network: Some(&network),
                        cables: true,
                        tint: None,
                    },
                )
                .buildings;
            });
            output.textures_delta.clear();
            buildings
                .into_iter()
                .find(|building| building.path == shop)
                .expect("drawn")
        };

        let near = drawn(0.0);
        assert_eq!(near.form, Form::Skyscraper);
        assert_eq!(near.doors, 1, "{near:?}");
        assert!(near.courtyard, "{near:?}");
        assert_eq!(near.masts.len(), 1, "{near:?}");

        // Framed as a part of a far larger city: far out.
        let far = drawn(200.0);
        assert_eq!(far.form, Form::Skyscraper, "the form stays");
        assert_eq!(far.doors, 0, "{far:?}");
        assert!(!far.courtyard, "{far:?}");
        assert!(!far.forecourt, "{far:?}");
        assert!(far.masts.is_empty(), "{far:?}");
        assert!(far.crowns.is_empty(), "{far:?}");
    }
}
