//! The city: the geometry that turns items into towers standing on their
//! node's block, and the isometric projection the city is seen through.
//!
//! A tower is a building on a narrow strip of ground: one floor per line of
//! the item, the first line on top as in an editor, and every token of a line
//! a point along its front face, starting at the line's indentation, so the
//! tower's silhouette is the shape of its code. Functions have taller floors
//! than types and `use` blocks. The towers of a block stand in a skyline:
//! the tallest at the back, the shortest at the front.
//!
//! The ground is the layout's own plane. It is seen isometrically: ground
//! `x` runs down to the right, ground `y` down to the left, and height
//! straight up the screen.
//!
//! Pure geometry: `egui::{Rect, Vec2, Pos2}` only, no widget.

use egui::{Pos2, Rect, Vec2, pos2, vec2};

use crate::graph::{Item, ItemKind};

/// Layout points per column of a line.
pub const COLUMN: f32 = 0.3;

/// Layout points per floor of a function, and of a type or `use` block.
pub const FLOOR: f32 = 1.3;
pub const LOW_FLOOR: f32 = 0.6;

/// The widest a tower gets; longer lines are cut off at it.
pub const WIDEST: f32 = 46.0;

/// The narrowest a tower gets, so a one-character line still stands.
const NARROWEST: f32 = 4.0;

/// The depth of the ground a tower stands on.
pub const WALL: f32 = 3.0;

/// Space between towers in a row, between rows, and around them on a block.
const TOWER_GAP: f32 = 4.5;
const ROW: f32 = 12.0;
const MARGIN: f32 = 4.0;

/// The isometric axes: how far one layout point along ground `x` moves on
/// screen across and down.
const ACROSS: f32 = 0.866;
const DOWN: f32 = 0.5;

/// Where the ground point `ground` at height `z` is seen, at the layout's
/// scale, before the view moves and scales it.
pub fn iso(ground: Pos2, z: f32) -> Pos2 {
    pos2(
        (ground.x - ground.y) * ACROSS,
        (ground.x + ground.y) * DOWN - z,
    )
}

/// The ground point seen at `seen`, the inverse of [`iso`] at height 0.
pub fn ground_of(seen: Pos2) -> Pos2 {
    let a = seen.x / ACROSS;
    let b = seen.y / DOWN;
    pos2((a + b) / 2.0, (b - a) / 2.0)
}

/// The rectangle the corners of the ground rectangle `rect` are seen in.
pub fn iso_bounds(rect: Rect) -> Rect {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    Rect::from_points(&corners.map(|corner| iso(corner, 0.0)))
}

/// The height of one floor of `item`.
pub fn floor_height(item: &Item) -> f32 {
    match item.kind {
        ItemKind::Function | ItemKind::Test => FLOOR,
        ItemKind::Type | ItemKind::Uses => LOW_FLOOR,
    }
}

/// How high the top of `item` stands: above its base, or at its base for a
/// tower hanging in the cellar.
pub fn height(item: &Item) -> f32 {
    if in_cellar(item) {
        return item.base;
    }
    item.base + reach(item)
}

/// How far `item` reaches from the ground: up for a tower, down for a tower
/// in the cellar.
pub fn reach(item: &Item) -> f32 {
    item.floors.len() as f32 * floor_height(item)
}

/// Whether `item` lives in the cellar: tests hang below the ground under
/// their block.
pub fn in_cellar(item: &Item) -> bool {
    item.tests
}

/// The length of `item`'s wall: its longest line, within bounds.
pub fn width(item: &Item) -> f32 {
    let widest = item.floors.iter().map(|f| f.width).max().unwrap_or(0);
    (widest as f32 * COLUMN).clamp(NARROWEST, WIDEST)
}

/// Whether `item`'s front face runs along ground x. Its ground is longer
/// along the face than it is deep, and a turned map turns both.
pub fn along_x(item: &Item) -> bool {
    item.area.width() >= item.area.height()
}

/// The ground point `along` layout points from the start of `item`'s front
/// face: the edge of its ground nearest the viewer from the corner.
pub fn front(item: &Item, along: f32) -> Pos2 {
    let ground = item.area;
    if along_x(item) {
        pos2(ground.min.x + along.min(ground.width()), ground.max.y)
    } else {
        pos2(ground.max.x, ground.min.y + along.min(ground.height()))
    }
}

/// The ground point `along` layout points from the start of `item`'s back
/// face, opposite [`front`].
pub fn back(item: &Item, along: f32) -> Pos2 {
    let ground = item.area;
    if along_x(item) {
        pos2(ground.min.x + along.min(ground.width()), ground.min.y)
    } else {
        pos2(ground.min.x, ground.min.y + along.min(ground.height()))
    }
}

/// The height of floor `index`, from 0: the first line is the top floor, or
/// in the cellar the floor nearest the ground.
pub fn floor_z(item: &Item, index: usize) -> f32 {
    if in_cellar(item) {
        // Hanging down, the first line just under the ground.
        return item.base - ((index + 1) as f32) * floor_height(item);
    }
    item.base + (item.floors.len() - index) as f32 * floor_height(item)
}

/// Where floor `index` of `item` starts and ends on the ground: from its
/// indentation to its last character, cut off at the wall's end.
pub fn floor_span(item: &Item, index: usize) -> (Pos2, Pos2) {
    let floor = &item.floors[index];
    let start = floor.indent as f32 * COLUMN;
    let end = (floor.width as f32 * COLUMN).max(start);
    (front(item, start), front(item, end))
}

/// The ground points of the tokens of floor `index`, each where its token
/// starts, cut off at the wall's end.
pub fn token_points(item: &Item, index: usize) -> impl Iterator<Item = Pos2> + '_ {
    let length = item.area.width().max(item.area.height());
    item.floors[index]
        .tokens
        .iter()
        .map(|token| token.start as f32 * COLUMN)
        .take_while(move |start| *start <= length)
        .map(move |start| front(item, start))
}

/// Where each token of floor `index` starts and ends on the ground, cut off
/// at the wall's end.
pub fn token_spans(item: &Item, index: usize) -> impl Iterator<Item = (Pos2, Pos2)> + '_ {
    let length = item.area.width().max(item.area.height());
    item.floors[index]
        .tokens
        .iter()
        .map(move |token| {
            let start = token.start as f32 * COLUMN;
            (start, (start + token.len as f32 * COLUMN).min(length))
        })
        .take_while(move |(start, _)| *start <= length)
        .map(move |(start, end)| (front(item, start), front(item, end)))
}

/// A step from one facade to the next, as a camera facing it sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Left,
    Right,
    Behind,
    Front,
}

/// The tower next to `from` going `toward` among `block`, the items of
/// `from`'s block, on the same level as `from` (above the ground or in the
/// cellar). Left and right walk the rows as they stand, on to the next row
/// past a row's end, and stop at the block's first and last tower; behind
/// and in front step to the nearest row, landing on the tower nearest
/// across.
pub fn neighbour<'a>(block: &[&'a Item], from: &Item, toward: Toward) -> Option<&'a Item> {
    let level = in_cellar(from);
    // Along the facade, and how far forward: rows lie at one depth each.
    let place = |item: &Item| {
        let at = item.area.center();
        if along_x(from) {
            (at.x, at.y)
        } else {
            (at.y, -at.x)
        }
    };
    let mut rows: Vec<Vec<&'a Item>> = Vec::new();
    let mut items: Vec<&'a Item> = block
        .iter()
        .copied()
        .filter(|item| in_cellar(item) == level)
        .collect();
    items.sort_by(|a, b| {
        let (a, b) = (place(a), place(b));
        a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0))
    });
    for item in items {
        match rows.last_mut() {
            Some(row) if (place(row[0]).1 - place(item).1).abs() < 0.5 => row.push(item),
            _ => rows.push(vec![item]),
        }
    }
    let row = rows
        .iter()
        .position(|row| row.iter().any(|item| item.path == from.path))?;
    let column = rows[row].iter().position(|item| item.path == from.path)?;
    match toward {
        Toward::Right => rows[row]
            .get(column + 1)
            .or_else(|| rows.get(row + 1).and_then(|next| next.first()))
            .copied(),
        Toward::Left => match column.checked_sub(1) {
            Some(before) => Some(rows[row][before]),
            None => rows
                .get(row.checked_sub(1)?)
                .and_then(|row| row.last())
                .copied(),
        },
        Toward::Behind | Toward::Front => {
            let other = if toward == Toward::Behind {
                rows.get(row.checked_sub(1)?)?
            } else {
                rows.get(row + 1)?
            };
            let across = place(from).0;
            other.iter().copied().min_by(|a, b| {
                (place(a).0 - across)
                    .abs()
                    .total_cmp(&(place(b).0 - across).abs())
            })
        }
    }
}

/// Places `items` in rows on a block whose plot starts at the origin, and
/// returns the size the plot takes. Rows aim for a roughly square plot no
/// narrower than `narrowest`. They form a skyline: the tallest towers fill
/// the back rows and the shortest the front, so whatever stands behind rises
/// above what stands in front of it.
pub(crate) fn place(items: &mut [Item], narrowest: f32) -> Vec2 {
    // Above the ground and in the cellar share the plot, in rows of their
    // own. The cellar's rows run the other way: the deepest at the front, so
    // a wall hanging behind does not reach over one in front.
    let (mut cellar, mut above): (Vec<&mut Item>, Vec<&mut Item>) =
        items.iter_mut().partition(|item| in_cellar(item));
    above.sort_by(|a, b| reach(b).total_cmp(&reach(a)));
    cellar.sort_by(|a, b| reach(a).total_cmp(&reach(b)));
    place_rows(above, narrowest).max(place_rows(cellar, narrowest))
}

fn place_rows(mut items: Vec<&mut Item>, narrowest: f32) -> Vec2 {
    if items.is_empty() {
        return Vec2::ZERO;
    }
    let widths: Vec<f32> = items.iter().map(|item| width(item)).collect();
    let total: f32 = widths.iter().map(|w| w + TOWER_GAP).sum();
    let limit = (total.sqrt() * 8.0)
        .max(narrowest - 2.0 * MARGIN)
        .max(widths.iter().copied().fold(0.0, f32::max));

    let (mut x, mut y, mut widest) = (0.0_f32, 0.0_f32, 0.0_f32);
    for (item, width) in items.iter_mut().zip(widths) {
        if x > 0.0 && x + width > limit {
            x = 0.0;
            y += ROW;
        }
        item.area = Rect::from_min_size(pos2(MARGIN + x, MARGIN + y), vec2(width, WALL));
        widest = widest.max(x + width);
        x += width + TOWER_GAP;
    }
    vec2(widest + 2.0 * MARGIN, y + WALL + 2.0 * MARGIN)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::graph::{Floor, ItemKind, Token};

    fn item(lines: &[(usize, usize)]) -> Item {
        Item {
            name: "f".to_owned(),
            path: PathBuf::from("/p/a.rs/f"),
            kind: ItemKind::Function,
            tests: false,
            floors: lines
                .iter()
                .enumerate()
                .map(|(n, &(indent, width))| Floor {
                    number: n + 1,
                    text: String::new(),
                    indent,
                    width,
                    tokens: vec![Token {
                        start: indent,
                        len: width - indent,
                    }],
                    calls: Vec::new(),
                    imports: Vec::new(),
                })
                .collect(),
            area: Rect::from_min_size(pos2(10.0, 100.0), vec2(30.0, WALL)),
            base: 0.0,
        }
    }

    #[test]
    fn the_projection_and_its_inverse_meet() {
        let ground = pos2(12.0, -7.5);

        let back = ground_of(iso(ground, 0.0));

        assert!((back - ground).length() < 1e-4, "{back:?}");
    }

    #[test]
    fn the_first_line_is_the_top_floor_and_floors_start_at_their_indentation() {
        let tower = item(&[(0, 40), (4, 30), (8, 20)]);

        let (second, _) = floor_span(&tower, 1);
        let (third, end) = floor_span(&tower, 2);

        assert!(floor_z(&tower, 0) > floor_z(&tower, 1));
        assert_eq!(floor_z(&tower, 2), FLOOR);
        assert!((third.x - second.x - 4.0 * COLUMN).abs() < 1e-4);
        assert!((end.x - (10.0 + 20.0 * COLUMN)).abs() < 1e-4);
        assert_eq!(height(&tower), 3.0 * FLOOR);
    }

    #[test]
    fn the_tallest_towers_of_a_block_stand_at_the_back() {
        let mut items: Vec<Item> = [3, 40, 12, 25, 8]
            .iter()
            .map(|&lines| item(&vec![(0, 200); lines]))
            .collect();

        place(&mut items, 48.0);

        for one in &items {
            for other in &items {
                if height(one) > height(other) {
                    assert!(
                        one.area.min.y <= other.area.min.y,
                        "{} floors stand in front of {} floors",
                        one.floors.len(),
                        other.floors.len()
                    );
                }
            }
        }
    }

    #[test]
    fn walking_a_block_stays_on_it_and_on_its_level() {
        let mut items: Vec<Item> = [40, 30, 20, 12, 8, 3]
            .iter()
            .map(|&lines| item(&vec![(0, 120); lines]))
            .collect();
        for (n, item) in items.iter_mut().enumerate() {
            item.path = PathBuf::from(format!("/p/a.rs/f{n}"));
        }
        items[5].tests = true;
        place(&mut items, 48.0);
        let block: Vec<&Item> = items.iter().collect();
        let name = |item: Option<&Item>| item.map(|item| item.path.file_name().unwrap().to_owned());
        let at = |n: usize| &items[n];

        // The tallest stands first, at the back; walking right goes through
        // every tower above the ground and stops at the last.
        let mut seen = vec![name(Some(at(0)))];
        let mut here = at(0);
        while let Some(next) = neighbour(&block, here, Toward::Right) {
            seen.push(name(Some(next)));
            here = next;
        }
        assert_eq!(seen.len(), 5, "{seen:?}");
        assert!(
            !seen.contains(&name(Some(at(5)))),
            "the test is in the cellar"
        );
        assert_eq!(neighbour(&block, here, Toward::Right), None);
        assert_eq!(neighbour(&block, at(0), Toward::Left), None);
        assert_eq!(
            neighbour(&block, at(5), Toward::Right),
            None,
            "alone in the cellar"
        );
    }

    #[test]
    fn behind_and_in_front_step_to_the_nearest_row() {
        let mut items: Vec<Item> = [40, 30, 20, 12]
            .iter()
            .map(|&lines| item(&vec![(0, 150); lines]))
            .collect();
        for (n, item) in items.iter_mut().enumerate() {
            item.path = PathBuf::from(format!("/p/a.rs/f{n}"));
        }
        place(&mut items, 48.0);
        let block: Vec<&Item> = items.iter().collect();

        let back = &items[0];
        let front = neighbour(&block, back, Toward::Front).expect("a row in front");

        assert!(front.area.min.y > back.area.max.y);
        assert_eq!(
            neighbour(&block, front, Toward::Behind).map(|i| &i.path),
            Some(&back.path)
        );
        assert_eq!(neighbour(&block, back, Toward::Behind), None);
    }

    #[test]
    fn tokens_past_the_end_of_the_wall_are_cut_off() {
        let mut tower = item(&[(0, 400)]);
        tower.floors[0].tokens.push(Token { start: 300, len: 4 });

        assert_eq!(token_points(&tower, 0).count(), 1);
    }

    #[test]
    fn a_turned_wall_runs_along_the_ground_y() {
        let mut tower = item(&[(0, 40), (4, 30)]);
        tower.area = Rect::from_min_size(pos2(0.0, 0.0), vec2(WALL, 30.0));

        let (start, end) = floor_span(&tower, 1);

        assert_eq!(start.x, end.x);
        assert!(end.y > start.y);
    }

    #[test]
    fn towers_on_a_block_never_share_ground() {
        let mut items: Vec<Item> = (1..30).map(|n| item(&[(0, n * 7)])).collect();

        let size = place(&mut items, 120.0);

        for (i, one) in items.iter().enumerate() {
            assert!(Rect::from_min_size(pos2(0.0, 0.0), size).contains_rect(one.area));
            for other in &items[i + 1..] {
                assert!(!one.area.intersects(other.area), "{one:?} {other:?}");
            }
        }
    }
}
