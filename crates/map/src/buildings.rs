//! Buildings: the items of a block stood together as the building their
//! declarations make.
//!
//! A loader says what each item declares ([`crate::Declaration`]); the map
//! chooses the building. An item with parts (a type with its methods) is a
//! skyscraper: the item is its base and each part a tier stepping back above
//! it, the largest lowest, with a crown on top for each interface it
//! implements. An enum without parts is a terrace of row houses, a constant a
//! monument, anything else a tower; an entry point is a tower with a spire.
//! Tests hang in the cellar, each a tower of its own.
//!
//! A file that is not code stands as the building of its kind
//! ([`NodeKind`]): a document a library hall with a bay on its roof for each
//! section, a configuration a cabinet per top-level key with its nested
//! levels stacked on it, a feature file a courthouse with a column per
//! scenario, Terraform a plant with a chimney per further block, a build
//! file a depot, a contract a customs house, secrets a sealed vault (in
//! glass for an example), a generated file a ghost, a schema a slab sunk
//! below the ground and an image a billboard.
//!
//! A part written in another file than its base stands on its base's
//! building all the same: [`gather`] moves it onto its base's block before
//! the blocks are laid out, so every block holds its buildings.
//!
//! A building also shows how its code is used ([`Buildings::of`]): a door on
//! its front for each public part and a courtyard behind when it has private
//! ones, a forecourt in front when called from many places, a mast on its
//! roof for each other file it calls, and a skybridge to each type it holds.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::arrange;
use crate::city::{self, WALL};
use crate::graph::{self, Declares, Group, Item, Node, NodeKind, ProjectMap};

/// The building an item stands as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Tower,
    Skyscraper,
    Terrace,
    Monument,
    /// A document: a low hall the map draws, with a colonnade along its
    /// front and a bay on its roof for each section.
    Library,
    /// A configuration key: a square cabinet with the levels nested under
    /// it stacked on top, each smaller.
    Switchboard,
    /// A feature file: its header a hall on a plinth, with a column in front
    /// for each scenario.
    Courthouse,
    /// Infrastructure: its largest block the body, a chimney on its roof for
    /// each further block.
    Plant,
    /// A build file: a wide shed with a sawtooth roof and loading docks.
    Depot,
    /// A contract between services: a square house with an upper storey on
    /// a border.
    CustomsHouse,
    /// Secrets: a sealed box whose lines are never shown; in glass for an
    /// example of them.
    Vault {
        glass: bool,
    },
    /// A generated file: a dashed outline.
    Ghost,
    /// A schema: a slab sunk below the ground.
    Slab,
    /// An image: a billboard on a post, on a block holding nothing.
    Billboard,
}

/// One building of a block: its form, the index of its base item and of its
/// parts, bottom to top once it stands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Building {
    pub form: Form,
    pub base: usize,
    pub parts: Vec<usize>,
}

/// What each building of a map shows of itself, worked out once per map:
/// the trims of the buildings, for each block by the index of each item on
/// it the building it stands in, so a frame looks up a block rather than
/// each of its items, and the skybridges between buildings; and the outline
/// of every block, folder and district, by its path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Buildings {
    pub trims: Vec<Trim>,
    pub stands: HashMap<PathBuf, Vec<Option<Stand>>>,
    pub bridges: Vec<Bridge>,
    pub outlines: HashMap<PathBuf, Vec<Pos2>>,
}

/// Where an item stands: the index of its building's base on its block, its
/// building's trim by index, and whether it is the building's top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand {
    pub base: usize,
    pub trim: usize,
    pub top: bool,
}

/// What one building shows: the path of its base, its form, its tiers
/// bottom to top, the crowns on its top, a door for each public part, a
/// courtyard when any part is private, how many call sites outside the tests
/// call into it, a mast for each other file it calls, and the ground a hall
/// or a plinth covers under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Trim {
    pub base: PathBuf,
    pub form: Form,
    pub tiers: Vec<PathBuf>,
    pub crowns: Vec<String>,
    pub doors: usize,
    pub courtyard: bool,
    pub callers: usize,
    pub masts: Vec<Mast>,
    pub foundation: Option<Rect>,
}

/// A mast on a building's roof: the building it calls in another file, and
/// the middle of that building's roof and its height.
#[derive(Clone, Debug, PartialEq)]
pub struct Mast {
    pub target: PathBuf,
    pub roof: (Pos2, f32),
}

/// A skybridge from a type to a type one of its fields holds: the bases of
/// both buildings, where it leaves the one's wall and meets the other's on
/// the ground, how high it runs, and what it stands for: the holding type's
/// name, the held type's name, and the field as written with its line.
#[derive(Clone, Debug, PartialEq)]
pub struct Bridge {
    pub from: PathBuf,
    pub to: PathBuf,
    pub start: Pos2,
    pub end: Pos2,
    pub z: f32,
    pub holder: String,
    pub held: String,
    pub field: String,
    pub line: usize,
}

/// The most masts a roof carries.
pub(crate) const MASTS: usize = 8;

/// How few call sites make a forecourt.
const FORECOURT_FROM: usize = 3;

/// How long on the ground a piece of a skybridge is, and the most pieces one
/// is cut into, so a building standing in front of part of it hides that
/// part.
pub(crate) const PIECE: f32 = 4.0;
const PIECES_AT_MOST: usize = 64;

/// How far below the roof of the building it reaches a skybridge runs at
/// the highest.
const BELOW_ROOF: f32 = 0.5;

impl Buildings {
    /// The buildings of `map`, above the ground and in the cellar, and the
    /// skybridges between them.
    pub fn of(map: &ProjectMap) -> Buildings {
        let all: Vec<&Item> = map.districts.iter().flat_map(graph::items).collect();
        let items: HashMap<&Path, &Item> = all
            .iter()
            .map(|item| (item.path.as_path(), *item))
            .collect();
        let mut trims = Vec::new();
        // The trim of each building by its base, and the base of the
        // building each item stands in.
        let mut trim_of: HashMap<&Path, usize> = HashMap::new();
        let mut base_of: HashMap<&Path, &Path> = HashMap::new();
        let mut stands = HashMap::new();
        for node in map.districts.iter().flat_map(graph::nodes) {
            let mut standing = vec![None; node.items.len()];
            for building in buildings(&node.items, node.kind) {
                let base = &node.items[building.base];
                let parts: Vec<&Item> = building.parts.iter().map(|p| &node.items[*p]).collect();
                base_of.insert(&base.path, &base.path);
                for part in &parts {
                    base_of.insert(&part.path, &base.path);
                }
                let top = building.parts.last().copied().unwrap_or(building.base);
                for index in std::iter::once(building.base).chain(building.parts.iter().copied()) {
                    standing[index] = Some(Stand {
                        base: building.base,
                        trim: trims.len(),
                        top: index == top,
                    });
                }
                let grounds = std::iter::once(base).chain(parts.iter().copied());
                let foundation = match building.form {
                    Form::Library => Some(hall(grounds.map(|item| item.area))),
                    Form::Courthouse => Some(
                        grounds
                            .map(|item| item.area)
                            .fold(Rect::NOTHING, Rect::union),
                    ),
                    _ => None,
                };
                let crowned = matches!(building.form, Form::Tower | Form::Skyscraper);
                // Only code has parts others may use or not.
                let code = crowned || matches!(building.form, Form::Terrace | Form::Monument);
                let trim = Trim {
                    base: base.path.clone(),
                    form: building.form,
                    tiers: parts.iter().map(|part| part.path.clone()).collect(),
                    crowns: if crowned {
                        crowns(base).to_vec()
                    } else {
                        Vec::new()
                    },
                    doors: parts.iter().filter(|p| p.declaration.public).count(),
                    courtyard: code && parts.iter().any(|p| !p.declaration.public),
                    callers: 0,
                    masts: Vec::new(),
                    foundation,
                };
                trim_of.insert(&base.path, trims.len());
                trims.push(trim);
            }
            stands.insert(node.path.clone(), standing);
        }
        // The top of the building whose base is `base`: the middle of its
        // roof and its height.
        let roof = |base: &Path| {
            let top = trim_of
                .get(base)
                .and_then(|trim| trims[*trim].tiers.last())
                .map_or(base, PathBuf::as_path);
            let top = items[top];
            (top.area.center(), city::height(top))
        };
        let mut callers: HashMap<&Path, usize> = HashMap::new();
        // For each building, the files it calls and a mast for each.
        let mut masts: HashMap<&Path, Vec<(&Path, Mast)>> = HashMap::new();
        for item in all {
            let Some(&from) = base_of.get(item.path.as_path()) else {
                continue;
            };
            for target in item.floors.iter().flat_map(|floor| &floor.calls) {
                let Some(&to) = base_of.get(target.as_path()) else {
                    continue;
                };
                if to == from {
                    continue;
                }
                if !item.tests {
                    *callers.entry(to).or_default() += 1;
                }
                let Some(file) = target.parent().filter(|file| Some(*file) != from.parent()) else {
                    continue;
                };
                let held = masts.entry(from).or_default();
                if !held.iter().any(|(called, _)| *called == file) {
                    let mast = Mast {
                        target: to.to_path_buf(),
                        roof: roof(to),
                    };
                    held.push((file, mast));
                }
            }
        }
        // The block each item stands on: a part from another file stands on
        // its base's block.
        let block_of: HashMap<&Path, usize> = map
            .districts
            .iter()
            .flat_map(graph::nodes)
            .enumerate()
            .flat_map(|(i, node)| node.items.iter().map(move |item| (item.path.as_path(), i)))
            .collect();
        let bridges = bridges(&items, &base_of, &block_of, &roof);
        for (base, n) in callers {
            if let Some(&trim) = trim_of.get(base) {
                trims[trim].callers = n;
            }
        }
        for (base, held) in masts {
            if let Some(&trim) = trim_of.get(base) {
                trims[trim].masts = held.into_iter().map(|(_, mast)| mast).take(MASTS).collect();
            }
        }
        Buildings {
            trims,
            stands,
            bridges,
            outlines: outlines(map),
        }
    }

    /// The trim of the building whose base is `base`.
    pub fn trim(&self, base: &Path) -> Option<&Trim> {
        self.trims.iter().find(|trim| trim.base == base)
    }
}

/// How far an outline reaches past what it holds: a block's past its
/// buildings, a folder's past its blocks and folders, a district's past
/// what it holds.
const BLOCK_OUTLINE: f32 = 2.0;
const FOLDER_OUTLINE: f32 = 3.0;
const DISTRICT_OUTLINE: f32 = 6.0;

/// The outline of every block, folder and district of `map`, round what
/// each holds; a block holding nothing keeps its rectangle.
fn outlines(map: &ProjectMap) -> HashMap<PathBuf, Vec<Pos2>> {
    fn corners(r: Rect) -> [Pos2; 4] {
        [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
        ]
    }
    fn blocks(nodes: &[Node], into: &mut HashMap<PathBuf, Vec<Pos2>>) -> Vec<Pos2> {
        let mut held = Vec::new();
        for node in nodes {
            let outline = if node.items.is_empty() {
                corners(node.area).to_vec()
            } else {
                let grounds = node.items.iter().flat_map(|item| corners(item.area));
                arrange::outline(grounds, BLOCK_OUTLINE)
            };
            held.extend(&outline);
            into.insert(node.path.clone(), outline);
        }
        held
    }
    fn folders(groups: &[Group], into: &mut HashMap<PathBuf, Vec<Pos2>>) -> Vec<Pos2> {
        let mut held = Vec::new();
        for group in groups {
            let mut inside = folders(&group.groups, into);
            inside.extend(blocks(&group.nodes, into));
            let outline = arrange::outline(inside.into_iter(), FOLDER_OUTLINE);
            held.extend(&outline);
            into.insert(group.path.clone(), outline);
        }
        held
    }
    let mut outlines = HashMap::new();
    for district in &map.districts {
        let mut held = blocks(&district.nodes, &mut outlines);
        for sub in &district.sub_districts {
            let mut inside = folders(&sub.groups, &mut outlines);
            inside.extend(blocks(&sub.nodes, &mut outlines));
            let outline = arrange::outline(inside.into_iter(), FOLDER_OUTLINE);
            held.extend(&outline);
            outlines.insert(sub.path.clone(), outline);
        }
        let outline = arrange::outline(held.into_iter(), DISTRICT_OUTLINE);
        outlines.insert(district.path.clone(), outline);
    }
    outlines
}

/// The skybridges of the buildings `base_of` knows: from each base above the
/// ground to the building of each type a field of it holds on the same
/// block, the first such field of each, at that field's height but below the
/// other's roof, from wall to wall. No bridge spans two blocks.
fn bridges(
    items: &HashMap<&Path, &Item>,
    base_of: &HashMap<&Path, &Path>,
    block_of: &HashMap<&Path, usize>,
    roof: &dyn Fn(&Path) -> (Pos2, f32),
) -> Vec<Bridge> {
    let mut bridges = Vec::new();
    let mut holders: Vec<&Item> = items
        .values()
        .copied()
        .filter(|item| {
            !item.tests && base_of.get(item.path.as_path()) == Some(&item.path.as_path())
        })
        .collect();
    holders.sort_by(|a, b| a.path.cmp(&b.path));
    for from in holders {
        let mut joined: Vec<&Path> = Vec::new();
        for (floor, held) in &from.declaration.holds {
            let Some(&to) = base_of.get(held.as_path()) else {
                continue;
            };
            let Some(other) = items.get(to).filter(|other| !other.tests) else {
                continue;
            };
            let apart = block_of.get(from.path.as_path()) != block_of.get(to);
            if apart || to == from.path || joined.contains(&to) || *floor >= from.floors.len() {
                continue;
            }
            joined.push(to);
            let z = city::floor_z(from, *floor).min(roof(to).1 - BELOW_ROOF);
            let (a, b) = (from.area.center(), other.area.center());
            let field = &from.floors[*floor];
            bridges.push(Bridge {
                holder: from.name.clone(),
                held: held
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                field: field.text.trim().trim_end_matches(',').to_owned(),
                line: field.number,
                from: from.path.clone(),
                to: to.to_path_buf(),
                start: leave(from.area, a, b),
                end: leave(other.area, b, a),
                z,
            });
        }
    }
    bridges
}

/// Where the line from `from`, inside `area`, towards `to` leaves `area`.
fn leave(area: Rect, from: Pos2, to: Pos2) -> Pos2 {
    let d = to - from;
    let exit = |lo: f32, hi: f32, p: f32, d: f32| {
        if d > 0.0 {
            (hi - p) / d
        } else if d < 0.0 {
            (lo - p) / d
        } else {
            f32::INFINITY
        }
    };
    let t =
        exit(area.min.x, area.max.x, from.x, d.x).min(exit(area.min.y, area.max.y, from.y, d.y));
    from + d * t.clamp(0.0, 1.0)
}

/// `from` to `to`, each a ground point and its height, cut into pieces about
/// [`PIECE`] long on the ground, at most [`PIECES_AT_MOST`], so each piece
/// can be drawn at its own depth among the buildings.
pub(crate) fn pieces(from: (Pos2, f32), to: (Pos2, f32)) -> Vec<[(Pos2, f32); 2]> {
    let n = ((to.0 - from.0).length() / PIECE).ceil() as usize;
    let n = n.clamp(1, PIECES_AT_MOST);
    let at = |t: f32| (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    (0..n)
        .map(|k| [at(k as f32 / n as f32), at((k + 1) as f32 / n as f32)])
        .collect()
}

/// Where mast `i` of `n` stands on the roof `roof` at height `z`, and how
/// high its tip reaches: masts stand along the roof's diagonal, of three
/// heights in turn; hanging below a tower in the cellar, `z` its bottom,
/// they point down.
pub(crate) fn mast(roof: Rect, z: f32, i: usize, n: usize, hangs: bool) -> (Pos2, f32) {
    let t = (i as f32 + 0.5) / n.max(1) as f32;
    let at = pos2(
        roof.min.x + roof.width() * t,
        roof.min.y + roof.height() * (1.0 - t),
    );
    let tall = MAST + (i % 3) as f32 * MAST_STEP;
    (at, if hangs { z - tall } else { z + tall })
}

/// How tall the shortest mast is, and how much taller each of the next two.
const MAST: f32 = 5.0;
const MAST_STEP: f32 = 2.5;

/// How deep the forecourt of a building called from `callers` places is:
/// none below [`FORECOURT_FROM`], then deeper with each caller, up to
/// [`FORECOURT_DEEPEST`].
pub(crate) fn forecourt(callers: usize) -> Option<f32> {
    (callers >= FORECOURT_FROM).then(|| (1.5 + callers as f32 * 0.4).min(FORECOURT_DEEPEST))
}

/// The deepest a forecourt gets.
const FORECOURT_DEEPEST: f32 = 7.0;

/// How far a skyscraper's tiers step back on each side, as a share of the
/// base's shorter side, and the most all of them together may.
const STEP: f32 = 0.1;
const STEPS_AT_MOST: f32 = 0.42;

/// How much wider than its widest line a skyscraper's base is, and the
/// least and most depth it takes.
const SKYSCRAPER_ROOM: f32 = 4.0;
const SKYSCRAPER_DEPTH: (f32, f32) = (8.0, 30.0);

/// How tall a crown stands.
pub const CROWN: f32 = 1.6;

/// How wide each house of a terrace is, and how deep the terrace.
pub const HOUSE: f32 = 3.4;
const TERRACE_DEPTH: f32 = 8.0;

/// The side of a monument's square ground.
const MONUMENT: f32 = 6.0;

/// How high a library hall stands, the widest a bay on its roof gets, the
/// margin of the hall around its bays and the gap between them, how deep
/// the bays are and how much ground the colonnade takes in front.
pub const HALL: f32 = 3.0;
const BAY_WIDEST: f32 = 12.0;
pub const HALL_MARGIN: f32 = 0.6;
const BAY_GAP: f32 = 0.3;
const BAY_DEPTH: f32 = 6.8;
const COLONNADE: f32 = 1.0;

/// The side of a configuration cabinet, and how much smaller each level
/// stacked on it is.
const CABINET: f32 = 7.0;
const CABINET_STEP: f32 = 0.75;

/// How wide a courthouse is for each column, how deep its hall, the side of
/// a column, how far in front of the hall the columns stand and how high
/// the plinth is that all of it stands on.
const COLUMN: f32 = 3.5;
const COURT_DEPTH: f32 = 7.0;
const COLUMN_SIDE: f32 = 1.4;
const COLUMNS_AT: f32 = 8.6;
pub const PLINTH: f32 = 0.7;

/// How much width a plant takes for each chimney, the side of a chimney and
/// the least width of a plant.
const CHIMNEY_ROOM: f32 = 3.5;
const CHIMNEY: f32 = 2.2;
const PLANT_WIDEST: f32 = 10.0;

/// The buildings `items`, one block's of `kind`, stand as, in the order of
/// their bases. An item is a part when it is part of an item of the same
/// block that is itself no part; tests are never parts and have none. A
/// document is one library, its first section the base and every further
/// one a part; infrastructure one plant, its largest block the base.
pub(crate) fn buildings(items: &[Item], kind: NodeKind) -> Vec<Building> {
    let all = || 0..items.len();
    match kind {
        NodeKind::Document if !items.is_empty() => {
            return vec![Building {
                form: Form::Library,
                base: 0,
                parts: all().skip(1).collect(),
            }];
        }
        NodeKind::Infrastructure if !items.is_empty() => {
            // The largest, the first written among equals.
            let base = all()
                .max_by(|a, b| {
                    items[*a]
                        .floors
                        .len()
                        .cmp(&items[*b].floors.len())
                        .then(b.cmp(a))
                })
                .unwrap_or(0);
            return vec![Building {
                form: Form::Plant,
                base,
                parts: all().filter(|i| *i != base).collect(),
            }];
        }
        _ => {}
    }
    let index: HashMap<&Path, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (item.path.as_path(), i))
        .collect();
    let base_of = |item: &Item| {
        let base = *index.get(item.declaration.part_of.as_deref()?)?;
        let standing = !item.tests && !items[base].tests;
        (standing && items[base].declaration.part_of.is_none()).then_some(base)
    };
    let mut parts: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut bases = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match base_of(item) {
            Some(base) => parts.entry(base).or_default().push(i),
            None => bases.push(i),
        }
    }
    bases
        .into_iter()
        .map(|base| {
            let mut parts = parts.remove(&base).unwrap_or_default();
            let form = match kind {
                NodeKind::Configuration => Form::Switchboard,
                NodeKind::Specification => Form::Courthouse,
                NodeKind::Build => Form::Depot,
                NodeKind::Contract => Form::CustomsHouse,
                NodeKind::Secrets => Form::Vault { glass: false },
                NodeKind::SecretsExample => Form::Vault { glass: true },
                NodeKind::Generated => Form::Ghost,
                NodeKind::Schema => Form::Slab,
                _ => {
                    // Largest lowest; the first written first among equals.
                    parts.sort_by(|a, b| {
                        city::reach(&items[*b]).total_cmp(&city::reach(&items[*a]))
                    });
                    match (&items[base].declaration.declares, parts.is_empty()) {
                        (_, false) => Form::Skyscraper,
                        (Declares::Enum { .. }, true) => Form::Terrace,
                        (Declares::Constant, true) => Form::Monument,
                        _ => Form::Tower,
                    }
                }
            };
            Building { form, base, parts }
        })
        .collect()
}

/// Stands `building` on ground whose corner is the origin: sets the ground
/// and base height of each of its items, and returns the size of the ground
/// it takes.
pub(crate) fn stand(items: &mut [Item], building: &Building) -> Vec2 {
    let base = building.base;
    let width = city::width(&items[base]);
    let size = match building.form {
        Form::Tower | Form::Billboard => vec2(width, WALL),
        Form::Monument => Vec2::splat(MONUMENT),
        Form::Terrace => {
            let houses = houses(&items[base]).len().max(1) as f32;
            vec2((houses * HOUSE + 0.6).max(TERRACE_DEPTH), TERRACE_DEPTH)
        }
        Form::Skyscraper => {
            let widest = building
                .parts
                .iter()
                .map(|part| city::width(&items[*part]))
                .fold(width, f32::max);
            let width = widest + SKYSCRAPER_ROOM;
            vec2(width, width.clamp(SKYSCRAPER_DEPTH.0, SKYSCRAPER_DEPTH.1))
        }
        Form::Library => return library(items, building),
        Form::Switchboard => return switchboard(items, building),
        Form::Courthouse => return courthouse(items, building),
        Form::Plant => return plant(items, building),
        Form::Depot => vec2(width.max(12.0), (width * 0.45).clamp(6.0, 18.0)),
        Form::CustomsHouse | Form::Vault { .. } => Vec2::splat(width.clamp(6.0, 14.0)),
        Form::Slab => vec2(width.max(8.0), (width * 0.7).clamp(6.0, 20.0)),
        Form::Ghost => vec2(width, (width * 0.5).clamp(5.0, 16.0)),
    };
    items[base].area = Rect::from_min_size(Pos2::ZERO, size);
    // A slab lies sunk below the ground, its top at ground level.
    items[base].base = match building.form {
        Form::Slab => -city::reach(&items[base]),
        _ => 0.0,
    };
    let mut z = city::height(&items[base]);
    for (k, part) in building.parts.iter().enumerate() {
        let inset = step(size, building.parts.len()) * (k + 1) as f32;
        let item = &mut items[*part];
        item.area = Rect::from_min_max(pos2(inset, inset), (size - Vec2::splat(inset)).to_pos2());
        item.base = z;
        z += city::reach(item);
    }
    size
}

/// A library: its sections side by side as bays, each at most
/// [`BAY_WIDEST`] wide, on the roof of a hall [`HALL`] high, with the
/// colonnade's ground in front.
fn library(items: &mut [Item], building: &Building) -> Vec2 {
    let mut x = HALL_MARGIN;
    for bay in std::iter::once(&building.base).chain(&building.parts) {
        let item = &mut items[*bay];
        let width = city::width(item).min(BAY_WIDEST);
        item.area = Rect::from_min_size(pos2(x, HALL_MARGIN), vec2(width, BAY_DEPTH));
        item.base = HALL;
        x += width + BAY_GAP;
    }
    let width = x - BAY_GAP + HALL_MARGIN;
    vec2(width, BAY_DEPTH + 2.0 * HALL_MARGIN + COLONNADE)
}

/// The ground of the hall a library's bays stand on: around them by
/// [`HALL_MARGIN`].
pub fn hall(bays: impl Iterator<Item = Rect>) -> Rect {
    bays.fold(Rect::NOTHING, Rect::union).expand(HALL_MARGIN)
}

/// A configuration key: a cabinet [`CABINET`] square, each level nested
/// under it stacked on it, each [`CABINET_STEP`] the side of the one below.
fn switchboard(items: &mut [Item], building: &Building) -> Vec2 {
    let mut side = CABINET;
    let mut z = 0.0;
    for level in std::iter::once(&building.base).chain(&building.parts) {
        let inset = (CABINET - side) / 2.0;
        let item = &mut items[*level];
        item.area = Rect::from_min_size(pos2(inset, inset), Vec2::splat(side));
        item.base = z;
        z += city::reach(item);
        side *= CABINET_STEP;
    }
    Vec2::splat(CABINET)
}

/// A feature file: its header a hall at the back, a column for each
/// scenario in front of it, all standing on a plinth [`PLINTH`] high.
fn courthouse(items: &mut [Item], building: &Building) -> Vec2 {
    let n = building.parts.len().max(1) as f32;
    let width = (n * COLUMN + 2.0).max(city::width(&items[building.base]));
    let hall = &mut items[building.base];
    hall.area = Rect::from_min_size(Pos2::ZERO, vec2(width, COURT_DEPTH));
    hall.base = PLINTH;
    let spacing = (width - 1.0) / n;
    for (i, column) in building.parts.iter().enumerate() {
        let x = 0.5 + spacing * (i as f32 + 0.5) - COLUMN_SIDE / 2.0;
        let item = &mut items[*column];
        item.area = Rect::from_min_size(pos2(x, COLUMNS_AT), Vec2::splat(COLUMN_SIDE));
        item.base = PLINTH;
    }
    vec2(width, COLUMNS_AT + COLUMN_SIDE + 0.4)
}

/// Infrastructure: its largest block the body, a chimney [`CHIMNEY`] square
/// on its roof for each further block, in a row along its back.
fn plant(items: &mut [Item], building: &Building) -> Vec2 {
    let chimneys = building.parts.len() as f32;
    let width = (chimneys * CHIMNEY_ROOM + 2.0)
        .max(city::width(&items[building.base]))
        .max(PLANT_WIDEST);
    let size = vec2(width, (width * 0.7).clamp(8.0, 25.0));
    let body = &mut items[building.base];
    body.area = Rect::from_min_size(Pos2::ZERO, size);
    body.base = 0.0;
    let roof = city::reach(body);
    for (i, chimney) in building.parts.iter().enumerate() {
        let item = &mut items[*chimney];
        let x = 1.0 + i as f32 * CHIMNEY_ROOM;
        item.area = Rect::from_min_size(pos2(x, 1.0), Vec2::splat(CHIMNEY));
        item.base = roof;
    }
    size
}

/// How far each tier of a skyscraper on ground of `size` with `tiers` tiers
/// steps back on every side: a tenth of the shorter side, less when the
/// tiers would otherwise run out of room.
fn step(size: Vec2, tiers: usize) -> f32 {
    let side = size.x.min(size.y);
    (side * STEP).min(side * STEPS_AT_MOST / tiers.max(1) as f32)
}

/// How high `building` rises above its ground: its base, its tiers and its
/// crowns.
pub(crate) fn reach(items: &[Item], building: &Building) -> f32 {
    let base = city::reach(&items[building.base]);
    let parts = building.parts.iter().map(|part| city::reach(&items[*part]));
    match building.form {
        Form::Library => HALL + parts.fold(base, f32::max),
        Form::Courthouse => PLINTH + parts.fold(base, f32::max),
        Form::Plant => base + parts.fold(0.0, f32::max),
        Form::Slab => 0.0,
        _ => base + parts.sum::<f32>() + crowns(&items[building.base]).len() as f32 * CROWN,
    }
}

/// The crowns `base` wears: one for each interface it implements.
pub fn crowns(base: &Item) -> &[String] {
    &base.declaration.implements
}

/// The houses of a terrace standing for `base`: for each variant, whether it
/// carries data and so has an annex.
pub fn houses(base: &Item) -> &[bool] {
    match &base.declaration.declares {
        Declares::Enum { carries_data } => carries_data,
        _ => &[],
    }
}

/// Moves every part standing on a block other than its base's onto its
/// base's block, after that block's own items, so each block holds its
/// buildings whole. A part stays where it is when its base is itself a part,
/// or either lives in the cellar.
pub(crate) fn gather(nodes: &mut [&mut Node]) {
    let node_of: HashMap<PathBuf, (usize, bool)> = nodes
        .iter()
        .enumerate()
        .flat_map(|(n, node)| {
            node.items.iter().map(move |item| {
                let standing = !item.tests && item.declaration.part_of.is_none();
                (item.path.clone(), (n, standing))
            })
        })
        .collect();
    let mut moving: Vec<(usize, Item)> = Vec::new();
    for (n, node) in nodes.iter_mut().enumerate() {
        let target = |item: &Item| {
            let (to, standing) = *node_of.get(item.declaration.part_of.as_deref()?)?;
            (to != n && standing && !item.tests).then_some(to)
        };
        let (away, kept): (Vec<Item>, Vec<Item>) = std::mem::take(&mut node.items)
            .into_iter()
            .partition(|item| target(item).is_some());
        node.items = kept;
        moving.extend(
            away.into_iter()
                .filter_map(|item| Some((target(&item)?, item))),
        );
    }
    for (to, item) in moving {
        nodes[to].items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Declaration, Floor, ItemKind};

    fn item(path: &str, lines: usize, declaration: Declaration) -> Item {
        Item {
            name: path.rsplit('/').next().unwrap_or(path).to_owned(),
            path: PathBuf::from(path),
            kind: ItemKind::Function,
            tests: false,
            floors: (0..lines)
                .map(|n| Floor {
                    number: n + 1,
                    text: String::new(),
                    indent: 0,
                    width: 40,
                    tokens: Vec::new(),
                    calls: Vec::new(),
                    imports: Vec::new(),
                })
                .collect(),
            area: Rect::NOTHING,
            base: 0.0,
            declaration,
        }
    }

    fn part_of(base: &str) -> Declaration {
        Declaration {
            part_of: Some(PathBuf::from(base)),
            ..Declaration::default()
        }
    }

    fn declares(declares: Declares) -> Declaration {
        Declaration {
            declares,
            ..Declaration::default()
        }
    }

    #[test]
    fn each_kind_of_declaration_stands_as_its_building() {
        let items = vec![
            item("/a.rs/Cart", 3, Declaration::default()),
            item("/a.rs/Cart::add", 4, part_of("/a.rs/Cart")),
            item("/a.rs/Cart::total", 9, part_of("/a.rs/Cart")),
            item(
                "/a.rs/Lens",
                5,
                declares(Declares::Enum {
                    carries_data: vec![false, true],
                }),
            ),
            item("/a.rs/GAP", 1, declares(Declares::Constant)),
            item("/a.rs/main", 3, declares(Declares::EntryPoint)),
            item("/a.rs/Price::new", 2, part_of("/b.rs/Price")),
        ];

        let forms: Vec<(usize, Form, Vec<usize>)> = buildings(&items, NodeKind::Code)
            .into_iter()
            .map(|b| (b.base, b.form, b.parts))
            .collect();

        assert_eq!(
            forms,
            [
                (0, Form::Skyscraper, vec![2, 1]),
                (3, Form::Terrace, vec![]),
                (4, Form::Monument, vec![]),
                (5, Form::Tower, vec![]),
                (6, Form::Tower, vec![]),
            ]
        );
    }

    #[test]
    fn tiers_step_back_above_their_base_the_largest_lowest() {
        let mut items = vec![
            item("/a.rs/Cart", 3, Declaration::default()),
            item("/a.rs/Cart::add", 4, part_of("/a.rs/Cart")),
            item("/a.rs/Cart::total", 9, part_of("/a.rs/Cart")),
        ];
        let building = buildings(&items, NodeKind::Code).remove(0);

        let size = stand(&mut items, &building);

        let [base, add, total] = [&items[0], &items[1], &items[2]];
        assert_eq!(base.area, Rect::from_min_size(Pos2::ZERO, size));
        let step = (size.x.min(size.y) * STEP).min(size.x.min(size.y) * STEPS_AT_MOST / 2.0);
        assert!((total.area.min.x - step).abs() < 1e-4);
        assert!((add.area.min.x - 2.0 * step).abs() < 1e-4);
        assert!(base.area.contains_rect(total.area) && total.area.contains_rect(add.area));
        assert_eq!(total.base, city::reach(base));
        assert_eq!(add.base, total.base + city::reach(total));
    }

    #[test]
    fn many_tiers_never_step_back_past_the_middle() {
        let mut items = vec![item("/a.rs/T", 1, Declaration::default())];
        items.extend((0..20).map(|n| item(&format!("/a.rs/T::f{n}"), 2, part_of("/a.rs/T"))));
        let building = buildings(&items, NodeKind::Code).remove(0);

        let size = stand(&mut items, &building);

        let top = items[*building.parts.last().unwrap()].area;
        assert!(top.width() >= size.x - 2.0 * STEPS_AT_MOST * size.y - 1e-3);
        assert!(top.is_positive());
    }

    /// The building of `items` of `kind`, stood at the origin.
    fn stood(items: &mut [Item], kind: NodeKind) -> (Building, Vec2) {
        let mut standing = buildings(items, kind);
        assert_eq!(standing.len(), 1, "one building");
        let building = standing.remove(0);
        let size = stand(items, &building);
        (building, size)
    }

    #[test]
    fn a_document_is_a_library_with_its_sections_side_by_side_on_the_hall() {
        let mut items: Vec<Item> = [3, 9, 5]
            .iter()
            .enumerate()
            .map(|(n, lines)| item(&format!("/a.md/s{n}"), *lines, Declaration::default()))
            .collect();

        let (building, size) = stood(&mut items, NodeKind::Document);

        assert_eq!(
            (building.form, building.base, building.parts),
            (Form::Library, 0, vec![1, 2])
        );
        for pair in items.windows(2) {
            assert!(pair[0].area.max.x < pair[1].area.min.x, "side by side");
        }
        for bay in &items {
            assert_eq!(bay.base, HALL, "on the hall's roof");
            assert!(bay.area.width() <= BAY_WIDEST + 1e-4);
        }
        let hall = hall(items.iter().map(|bay| bay.area));
        assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(hall));
    }

    #[test]
    fn a_configuration_key_is_a_cabinet_with_its_levels_stacked_smaller() {
        let mut items = vec![
            item("/c.yaml/db", 1, Declaration::default()),
            item("/c.yaml/db.1", 2, part_of("/c.yaml/db")),
            item("/c.yaml/db.2", 1, part_of("/c.yaml/db")),
        ];

        let (building, size) = stood(&mut items, NodeKind::Configuration);

        assert_eq!(
            (building.form, building.parts),
            (Form::Switchboard, vec![1, 2])
        );
        assert_eq!(size, Vec2::splat(CABINET));
        for pair in items.windows(2) {
            let (below, above) = (&pair[0], &pair[1]);
            assert!((above.area.width() - below.area.width() * CABINET_STEP).abs() < 1e-4);
            assert!(below.area.contains_rect(above.area));
            assert_eq!(above.base, city::height(below), "stacked");
        }
    }

    #[test]
    fn a_feature_file_is_a_courthouse_with_its_columns_in_front_on_the_plinth() {
        let mut items = vec![
            item("/f.feature/Feature: Pay", 4, Declaration::default()),
            item(
                "/f.feature/Scenario: a",
                3,
                part_of("/f.feature/Feature: Pay"),
            ),
            item(
                "/f.feature/Scenario: b",
                6,
                part_of("/f.feature/Feature: Pay"),
            ),
        ];

        let (building, _) = stood(&mut items, NodeKind::Specification);

        assert_eq!(
            (building.form, building.parts),
            (Form::Courthouse, vec![1, 2])
        );
        let hall = &items[0];
        for column in &items[1..] {
            assert!(column.area.min.y > hall.area.max.y, "in front of the hall");
            assert!((column.area.size() - Vec2::splat(COLUMN_SIDE)).length() < 1e-4);
            assert_eq!(column.base, PLINTH);
        }
        assert_eq!(hall.base, PLINTH);
    }

    #[test]
    fn terraform_is_a_plant_its_largest_block_the_body_the_others_chimneys() {
        let mut items = vec![
            item("/m.tf/provider", 2, Declaration::default()),
            item("/m.tf/resource", 9, Declaration::default()),
            item("/m.tf/output", 3, Declaration::default()),
        ];

        let (building, _) = stood(&mut items, NodeKind::Infrastructure);

        assert_eq!(
            (building.form, building.base, building.parts),
            (Form::Plant, 1, vec![0, 2])
        );
        let body = &items[1];
        for chimney in [&items[0], &items[2]] {
            assert!(body.area.contains_rect(chimney.area), "on its roof");
            assert!((chimney.area.size() - Vec2::splat(CHIMNEY)).length() < 1e-4);
            assert_eq!(chimney.base, city::height(body));
        }
    }

    #[test]
    fn each_other_kind_of_file_stands_as_its_building_and_a_schema_sunk() {
        for (kind, form) in [
            (NodeKind::Build, Form::Depot),
            (NodeKind::Contract, Form::CustomsHouse),
            (NodeKind::Secrets, Form::Vault { glass: false }),
            (NodeKind::SecretsExample, Form::Vault { glass: true }),
            (NodeKind::Generated, Form::Ghost),
            (NodeKind::Schema, Form::Slab),
        ] {
            let mut items = vec![item("/f/f", 5, Declaration::default())];

            let (building, _) = stood(&mut items, kind);

            assert_eq!(building.form, form, "{kind:?}");
            let sunk = city::height(&items[0]) == 0.0 && items[0].base < 0.0;
            assert_eq!(sunk, form == Form::Slab, "{kind:?}");
        }
    }

    #[test]
    fn a_part_from_another_file_moves_onto_its_base_s_block() {
        let mut cart = Node {
            name: "cart.rs".to_owned(),
            path: PathBuf::from("/cart.rs"),
            items: vec![item("/cart.rs/Cart", 2, Declaration::default())],
            area: Rect::NOTHING,
            kind: Default::default(),
        };
        let mut show = Node {
            name: "show.rs".to_owned(),
            path: PathBuf::from("/show.rs"),
            items: vec![
                item("/show.rs/Cart::fmt", 3, part_of("/cart.rs/Cart")),
                item("/show.rs/helper", 3, Declaration::default()),
            ],
            area: Rect::NOTHING,
            kind: Default::default(),
        };

        gather(&mut [&mut cart, &mut show]);

        let paths = |node: &Node| {
            node.items
                .iter()
                .map(|i| i.path.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            paths(&cart),
            [
                PathBuf::from("/cart.rs/Cart"),
                PathBuf::from("/show.rs/Cart::fmt")
            ]
        );
        assert_eq!(paths(&show), [PathBuf::from("/show.rs/helper")]);
    }

    /// The map of the files `files` under `/p`, each with its items.
    fn map_of(files: Vec<(&str, Vec<Item>)>) -> ProjectMap {
        let paths: Vec<PathBuf> = files.iter().map(|(file, _)| PathBuf::from(file)).collect();
        let items = files
            .into_iter()
            .map(|(file, items)| (PathBuf::from(file), items))
            .collect();
        let strategy = crate::Directories {
            prefix: crate::default_prefix_rule(),
        };
        ProjectMap::of_code(
            "shop",
            Path::new("/p"),
            &paths,
            &strategy,
            items,
            &Default::default(),
        )
    }

    /// `item` with one line calling each of `callees`.
    fn calling(mut item: Item, callees: &[&str]) -> Item {
        for (floor, callee) in item.floors.iter_mut().zip(callees) {
            floor.calls.push(PathBuf::from(callee));
        }
        item
    }

    fn public(mut declaration: Declaration) -> Declaration {
        declaration.public = true;
        declaration
    }

    #[test]
    fn doors_courtyards_and_callers_count_public_parts_and_calls_outside_the_tests() {
        let mut test = calling(
            item("/p/src/b.rs/t", 6, Declaration::default()),
            &["/p/src/a.rs/Cart::total"; 6],
        );
        test.tests = true;
        let map = map_of(vec![
            (
                "src/a.rs",
                vec![
                    item("/p/src/a.rs/Cart", 2, Declaration::default()),
                    item(
                        "/p/src/a.rs/Cart::add",
                        2,
                        public(part_of("/p/src/a.rs/Cart")),
                    ),
                    item(
                        "/p/src/a.rs/Cart::total",
                        2,
                        public(part_of("/p/src/a.rs/Cart")),
                    ),
                    item("/p/src/a.rs/Cart::tidy", 2, part_of("/p/src/a.rs/Cart")),
                    calling(
                        item("/p/src/a.rs/helper", 2, Declaration::default()),
                        &["/p/src/a.rs/Cart::tidy"],
                    ),
                ],
            ),
            (
                "src/b.rs",
                vec![
                    calling(
                        item("/p/src/b.rs/f", 3, Declaration::default()),
                        &[
                            "/p/src/a.rs/Cart::total",
                            "/p/src/a.rs/Cart",
                            "/p/src/a.rs/helper",
                        ],
                    ),
                    test,
                ],
            ),
        ]);

        let buildings = Buildings::of(&map);

        let cart = buildings.trim(Path::new("/p/src/a.rs/Cart")).unwrap();
        assert_eq!((cart.doors, cart.courtyard), (2, true));
        assert_eq!(cart.callers, 3, "helper and f, but not the test");
        let helper = buildings.trim(Path::new("/p/src/a.rs/helper")).unwrap();
        assert_eq!(
            (helper.doors, helper.courtyard, helper.callers),
            (0, false, 1)
        );
        assert!(forecourt(cart.callers).is_some());
        assert_eq!(forecourt(2), None);
        assert!(forecourt(100).unwrap() > forecourt(3).unwrap());
        assert_eq!(forecourt(100), Some(FORECOURT_DEEPEST));
    }

    #[test]
    fn a_building_carries_a_mast_for_each_other_file_it_calls_at_most_eight() {
        let callees: Vec<String> = (0..10).map(|n| format!("/p/src/f{n}.rs/g")).collect();
        let mut files: Vec<(String, Vec<Item>)> = callees
            .iter()
            .enumerate()
            .map(|(n, callee)| {
                (
                    format!("src/f{n}.rs"),
                    vec![item(callee, 2, Declaration::default())],
                )
            })
            .collect();
        let callees: Vec<&str> = callees.iter().map(String::as_str).collect();
        files.push((
            "src/a.rs".to_owned(),
            vec![
                calling(
                    item("/p/src/a.rs/two", 4, Declaration::default()),
                    &[
                        "/p/src/f0.rs/g",
                        "/p/src/f1.rs/g",
                        "/p/src/f1.rs/g",
                        "/p/src/a.rs/many",
                    ],
                ),
                calling(
                    item("/p/src/a.rs/many", 10, Declaration::default()),
                    &callees,
                ),
            ],
        ));
        let map = map_of(files.iter().map(|(f, i)| (f.as_str(), i.clone())).collect());

        let buildings = Buildings::of(&map);

        let two = buildings.trim(Path::new("/p/src/a.rs/two")).unwrap();
        let targets: Vec<&Path> = two.masts.iter().map(|mast| mast.target.as_path()).collect();
        assert_eq!(
            targets,
            [Path::new("/p/src/f0.rs/g"), Path::new("/p/src/f1.rs/g")]
        );
        let f1 = map.item(Path::new("/p/src/f1.rs/g")).unwrap();
        assert_eq!(two.masts[1].roof, (f1.area.center(), city::height(f1)));
        assert_eq!(
            buildings
                .trim(Path::new("/p/src/a.rs/many"))
                .unwrap()
                .masts
                .len(),
            MASTS
        );
    }

    #[test]
    fn no_skybridge_spans_two_blocks() {
        let holds = |held: &str| Declaration {
            holds: vec![(1, PathBuf::from(held))],
            ..Declaration::default()
        };
        let map = map_of(vec![
            (
                "catalog/cart.rs",
                vec![
                    item(
                        "/p/catalog/cart.rs/Cart",
                        6,
                        holds("/p/orders/line.rs/Line"),
                    ),
                    item(
                        "/p/catalog/cart.rs/Price",
                        6,
                        holds("/p/catalog/cart.rs/Tag"),
                    ),
                    item("/p/catalog/cart.rs/Tag", 4, Declaration::default()),
                    item(
                        "/p/catalog/cart.rs/Offer",
                        6,
                        holds("/p/catalog/rate.rs/Rate"),
                    ),
                ],
            ),
            (
                "catalog/rate.rs",
                vec![item("/p/catalog/rate.rs/Rate", 4, Declaration::default())],
            ),
            (
                "orders/line.rs",
                vec![item("/p/orders/line.rs/Line", 4, Declaration::default())],
            ),
        ]);
        assert_ne!(
            map.find(Path::new("/p/catalog")).map(|f| f.parent),
            None,
            "catalog is a district"
        );

        let buildings = Buildings::of(&map);

        let joined: Vec<(&Path, &Path)> = buildings
            .bridges
            .iter()
            .map(|b| (b.from.as_path(), b.to.as_path()))
            .collect();
        assert_eq!(
            joined,
            [(
                Path::new("/p/catalog/cart.rs/Price"),
                Path::new("/p/catalog/cart.rs/Tag")
            )],
            "Cart holds Line of another district, Offer holds Rate of another \
             block: no bridge"
        );
    }

    #[test]
    fn a_skybridge_runs_wall_to_wall_at_its_field_s_height_below_the_other_roof() {
        let holds = |held: &[(usize, &str)]| Declaration {
            holds: held
                .iter()
                .map(|(floor, path)| (*floor, PathBuf::from(path)))
                .collect(),
            ..Declaration::default()
        };
        let map = map_of(vec![(
            "src/a.rs",
            vec![
                item(
                    "/p/src/a.rs/Cart",
                    6,
                    holds(&[(1, "/p/src/a.rs/Line"), (2, "/p/src/a.rs/Line")]),
                ),
                item("/p/src/a.rs/Line", 8, Declaration::default()),
                item("/p/src/a.rs/Tag", 1, Declaration::default()),
                item(
                    "/p/src/a.rs/Order",
                    6,
                    holds(&[(0, "/p/src/a.rs/Tag"), (1, "/p/src/a.rs/Order")]),
                ),
            ],
        )]);

        let buildings = Buildings::of(&map);

        let at = |path: &str| map.item(Path::new(path)).unwrap();
        let (cart, line) = (at("/p/src/a.rs/Cart"), at("/p/src/a.rs/Line"));
        let [to_line, to_tag] = &buildings.bridges[..] else {
            panic!(
                "one skybridge to each type held, none to itself: {:?}",
                buildings.bridges
            );
        };
        assert_eq!(to_line.to, line.path);
        assert_eq!(
            to_line.z,
            city::floor_z(cart, 1),
            "the first field holding it"
        );
        let on_wall =
            |area: Rect, p: Pos2| area.expand(1e-3).contains(p) && !area.shrink(1e-3).contains(p);
        assert!(on_wall(cart.area, to_line.start) && on_wall(line.area, to_line.end));
        let tag = at("/p/src/a.rs/Tag");
        assert_eq!(
            to_tag.z,
            city::height(tag) - BELOW_ROOF,
            "below the low roof"
        );
    }

    #[test]
    fn a_line_is_cut_into_pieces_about_a_piece_long_and_never_too_many() {
        let (from, to) = ((pos2(0.0, 0.0), 1.0), (pos2(10.0, 0.0), 3.0));

        let pieces = pieces(from, to);

        assert_eq!(pieces.len(), 3);
        assert_eq!((pieces[0][0], pieces[2][1]), (from, to));
        assert!(pieces.windows(2).all(|w| w[0][1] == w[1][0]));
        assert_eq!(
            super::pieces(from, (pos2(1000.0, 0.0), 1.0)).len(),
            PIECES_AT_MOST
        );
    }

    #[test]
    fn masts_stand_on_the_roof_or_hang_below_the_cellar() {
        let roof = Rect::from_min_size(Pos2::ZERO, vec2(4.0, 4.0));

        let (at, tip) = mast(roof, 10.0, 0, 2, false);
        let (_, hanging) = mast(roof, -10.0, 0, 2, true);

        assert!(roof.contains(at));
        assert_eq!((tip, hanging), (10.0 + MAST, -10.0 - MAST));
    }
}
