//! A session as a unit on the map: it walks along the streets to whatever
//! its next tool call touches, works there for a readable moment, and
//! rests with a marker when the session thinks, waits for the engineer or
//! has finished.
//!
//! Pure model, no painting: `ui::SessionsView::paint` drives it once per
//! frame with the time, and draws what it reports in the city's or the flat
//! view's projection alike, since both lay the map out on the same ground.

use std::collections::{BinaryHeap, VecDeque};
use std::path::{Path, PathBuf};

use cellworks_map::ProjectMap;
use egui::{Pos2, Rect, pos2, vec2};

use crate::{ActionKind, FileEvent, Session, SessionState, city_node, tower_of};

/// How long a unit shows one action before it moves on to the next queued
/// one, in seconds; halved while a backlog builds up.
pub const WORK_SECONDS: f64 = 1.2;

/// Queued actions beyond which only the newest are kept, so a unit never
/// lags far behind its session.
pub const MAX_BACKLOG: usize = 4;

/// How many of the newest actions survive a compressed backlog.
const KEEP_ON_SKIP: usize = 2;

/// The shortest and longest walk, in seconds, whatever the distance.
const WALK_MIN: f64 = 0.5;
const WALK_MAX: f64 = 2.5;

/// How much of the map a unit crosses per second.
const MAP_PER_SECOND: f32 = 0.35;

/// Grid cells across the longer side of a route's search area.
const ROUTE_CELLS: f32 = 90.0;

/// Where an action happens on the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    /// The tower, building or folder; the map's root for the gate.
    pub tower: PathBuf,
    /// The ground of what is worked on.
    pub area: Rect,
    /// The ground of the file or folder holding it: `area` itself unless
    /// it is a function's tower. The flat view draws a file's functions in
    /// slots of its own, so it shows work on the whole file.
    pub block: Rect,
    /// Where the unit stands while it works: at the front corner of a
    /// tower or building, in the middle of a folder or the gate.
    pub stand: Pos2,
    /// The height of the tower, 0 for a building or folder.
    pub z: f32,
    pub floor: Option<(f32, f32)>,
    pub item: Option<String>,
    /// False at the gate: the call left the map, or touched a path this
    /// city does not hold.
    pub on_map: bool,
}

/// Where `event` happens in `map`'s city: its tower or building; else the
/// nearest folder above it that the city holds (a file just created, a
/// command run in a folder); else the gate.
pub fn target(map: &ProjectMap, common_dir: Option<&Path>, event: &FileEvent) -> Target {
    let Some(node) = city_node(event, map, common_dir) else {
        return gate(map);
    };
    if let Some(placed) = tower_of(map, &node, event) {
        let folder = map
            .find(&placed.tower)
            .is_some_and(|found| found.kind == cellworks_map::Element::Folder);
        let stand = if folder {
            placed.area.center()
        } else {
            placed.area.max + vec2(1.5, 1.5)
        };
        let block = map.find(&node).map_or(placed.area, |found| found.area);
        return Target {
            tower: placed.tower,
            area: placed.area,
            block,
            stand,
            z: placed.z,
            floor: placed.floor,
            item: placed.item,
            on_map: true,
        };
    }
    node.ancestors()
        .skip(1)
        .take_while(|folder| folder.starts_with(&map.root) && *folder != map.root)
        .find_map(|folder| {
            let found = map.find(folder)?;
            Some(Target {
                tower: folder.to_owned(),
                area: found.area,
                block: found.area,
                stand: found.area.center(),
                z: 0.0,
                floor: None,
                item: None,
                on_map: true,
            })
        })
        .unwrap_or_else(|| Target {
            on_map: true,
            ..gate(map)
        })
}

/// Just off the map's front corner, the one nearest the viewer in the
/// city and the bottom right in the flat view: where a unit works on the
/// project as a whole, or on anything off the map.
pub fn gate(map: &ProjectMap) -> Target {
    let extent = map.extent;
    let margin = (extent.size().max_elem() * 0.03).max(4.0);
    let stand = extent.max + vec2(margin, margin);
    let area = Rect::from_center_size(stand, vec2(margin, margin));
    Target {
        tower: map.root.clone(),
        area,
        block: area,
        stand,
        z: 0.0,
        floor: None,
        item: None,
        on_map: false,
    }
}

/// What a unit is doing, as its marker shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Moving,
    Working(ActionKind),
    /// Busy, with every tool call answered: the model is thinking.
    Thinking,
    /// The session waits for the engineer: an approval, a question.
    Waiting,
    Idle,
    Done,
}

impl Pose {
    pub fn name(self) -> &'static str {
        match self {
            Pose::Moving => "moving",
            Pose::Working(ActionKind::Read) => "reading",
            Pose::Working(ActionKind::Write) => "editing",
            Pose::Working(ActionKind::Run) => "running",
            Pose::Working(ActionKind::Search) => "searching",
            Pose::Working(ActionKind::Delegate) => "delegating",
            Pose::Working(ActionKind::Fetch) => "fetching",
            Pose::Thinking => "thinking",
            Pose::Waiting => "needs you",
            Pose::Idle => "idle",
            Pose::Done => "done",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Walking `path` (ground points, from where it stood to the target's
    /// stand), started at `started`, for `duration` seconds.
    Moving {
        path: Vec<Pos2>,
        started: f64,
        duration: f64,
    },
    /// Working at the target since `started`.
    Working { started: f64 },
}

/// One session's unit, kept across frames.
#[derive(Clone, Debug)]
pub struct Unit {
    /// The action walked to or worked on.
    pub action: FileEvent,
    /// Where `action` happens, refreshed every frame so a relayout or a
    /// lens switch moves the work with its tower.
    pub target: Target,
    pub phase: Phase,
    /// Where it stands when not walking.
    pub ground: Pos2,
    queue: VecDeque<FileEvent>,
    /// The newest event taken from the session so far (whole, since a
    /// hand-made event has no id).
    last: FileEvent,
    /// How many events of the session were skipped in a compressed
    /// backlog, in all.
    pub skipped: usize,
}

impl Unit {
    /// A unit for a session seen for the first time: it already stands at
    /// its newest action, so opening the map does not replay history.
    pub fn new(
        session: &Session,
        map: &ProjectMap,
        common_dir: Option<&Path>,
        now: f64,
    ) -> Option<Unit> {
        let action = session.events.last()?.clone();
        let target = target(map, common_dir, &action);
        Some(Unit {
            ground: target.stand,
            last: action.clone(),
            action,
            target,
            phase: Phase::Working { started: now },
            queue: VecDeque::new(),
            skipped: 0,
        })
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Takes the session's events that are new since the last call into
    /// the queue (compressed to the newest few when it grows past
    /// [`MAX_BACKLOG`]), and copies each known call's outcome over.
    pub fn sync(&mut self, session: &Session) {
        let events = &session.events;
        let new = match events.iter().rposition(|e| {
            e.id == self.last.id && e.path == self.last.path && e.kind == self.last.kind
        }) {
            Some(at) => &events[at + 1..],
            // The last one taken was trimmed away: a burst; take the end.
            None => &events[events.len().saturating_sub(KEEP_ON_SKIP + 1)..],
        };
        self.queue.extend(new.iter().cloned());
        if let Some(newest) = events.last() {
            self.last = newest.clone();
        }
        if self.queue.len() > MAX_BACKLOG {
            let cut = self.queue.len() - KEEP_ON_SKIP;
            self.skipped += cut;
            self.queue.drain(..cut);
        }

        let outcome = |id: &str| {
            events
                .iter()
                .rev()
                .find(|e| !id.is_empty() && e.id == id)
                .and_then(|e| e.outcome)
        };
        if let Some(found) = outcome(&self.action.id) {
            self.action.outcome = Some(found);
        }
        for queued in &mut self.queue {
            if let Some(found) = outcome(&queued.id) {
                queued.outcome = Some(found);
            }
        }
    }

    /// Moves the unit on to `now`: a finished walk turns into work; work
    /// shown for long enough gives way to the next queued action, walked
    /// to along the streets (or stepped to, on the same tower, or with
    /// `motion` off).
    pub fn advance(&mut self, now: f64, map: &ProjectMap, common_dir: Option<&Path>, motion: bool) {
        self.target = target(map, common_dir, &self.action);
        loop {
            match &self.phase {
                Phase::Moving {
                    started, duration, ..
                } => {
                    if now < started + duration && motion {
                        return;
                    }
                    self.ground = self.target.stand;
                    self.phase = Phase::Working { started: now };
                }
                Phase::Working { started } => {
                    let hurry = if self.queue.len() > 1 { 0.5 } else { 1.0 };
                    if now - started < WORK_SECONDS * hurry {
                        if !matches!(self.phase, Phase::Moving { .. }) {
                            self.ground = self.target.stand;
                        }
                        return;
                    }
                    let Some(next) = self.queue.pop_front() else {
                        self.ground = self.target.stand;
                        return;
                    };
                    let from = self.ground;
                    self.action = next;
                    self.target = target(map, common_dir, &self.action);
                    let to = self.target.stand;
                    if !motion || (to - from).length() < 0.5 {
                        self.ground = to;
                        self.phase = Phase::Working { started: now };
                        return;
                    }
                    let path = route(&obstacles(map), from, to);
                    let speed = map.extent.size().max_elem().max(1.0) * MAP_PER_SECOND;
                    let duration = (length(&path) / speed) as f64 * hurry.max(0.6);
                    self.phase = Phase::Moving {
                        path,
                        started: now,
                        duration: duration.clamp(WALK_MIN * hurry, WALK_MAX * hurry),
                    };
                    return;
                }
            }
        }
    }

    /// Where the unit is on the ground at `now`: along its path while it
    /// walks, eased in and out.
    pub fn position(&self, now: f64) -> Pos2 {
        match &self.phase {
            Phase::Moving {
                path,
                started,
                duration,
            } => {
                let t = ((now - started) / duration.max(1e-3)).clamp(0.0, 1.0) as f32;
                along(path, t * t * (3.0 - 2.0 * t))
            }
            Phase::Working { .. } => self.ground,
        }
    }

    /// The path still ahead at `now`, from the unit; empty when it is not
    /// walking.
    pub fn ahead(&self, now: f64) -> Vec<Pos2> {
        let Phase::Moving { path, .. } = &self.phase else {
            return Vec::new();
        };
        let at = self.position(now);
        // The corner the unit is heading to: the first one past the
        // segment holding it.
        let mut rest = vec![at];
        let mut best = (f32::MAX, path.len() - 1);
        for (i, pair) in path.windows(2).enumerate() {
            let d = distance_to_segment(at, pair[0], pair[1]);
            if d < best.0 {
                best = (d, i + 1);
            }
        }
        rest.extend_from_slice(&path[best.1..]);
        rest
    }

    /// Seconds the current phase has lasted at `now`.
    pub fn since(&self, now: f64) -> f64 {
        match self.phase {
            Phase::Moving { started, .. } | Phase::Working { started } => now - started,
        }
    }

    /// What the unit is doing at `now`, for `session` as listed.
    pub fn pose(&self, now: f64, session: &Session) -> Pose {
        if matches!(self.phase, Phase::Moving { .. }) {
            return Pose::Moving;
        }
        let backlog = !self.queue.is_empty();
        match session.session_state() {
            SessionState::Waiting => Pose::Waiting,
            SessionState::Idle if !backlog => {
                if session.state == "done" {
                    Pose::Done
                } else {
                    Pose::Idle
                }
            }
            _ if !backlog
                && self.action.outcome.is_some()
                && self.since(now) >= WORK_SECONDS * 2.0 =>
            {
                Pose::Thinking
            }
            _ => Pose::Working(self.action.kind),
        }
    }

    /// "editing pricing.rs · discount", "testing · cargo test -p map".
    pub fn label(&self) -> String {
        action_label(&self.action, &self.target)
    }
}

/// What `action` does where `target` is, for a unit's floating label.
pub fn action_label(action: &FileEvent, target: &Target) -> String {
    let verb = action.verb();
    let short = |text: &str| {
        let text = text.trim();
        if text.chars().count() > 40 {
            format!("{}…", text.chars().take(39).collect::<String>())
        } else {
            text.to_owned()
        }
    };
    match action.kind {
        ActionKind::Read | ActionKind::Write => {
            let file = action
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            match &target.item {
                Some(item) => format!("{verb} {file} · {item}"),
                None => format!("{verb} {file}"),
            }
        }
        _ if action.detail.is_empty() => verb.to_owned(),
        _ => format!("{verb} · {}", short(&action.detail)),
    }
}

/// Every tower, and every building without towers, of `map`: what a unit
/// walks around.
pub fn obstacles(map: &ProjectMap) -> Vec<Rect> {
    fn nodes(nodes: &[cellworks_map::Node], out: &mut Vec<Rect>) {
        for node in nodes {
            if node.items.is_empty() {
                out.push(node.area);
            } else {
                out.extend(node.items.iter().map(|item| item.area));
            }
        }
    }
    fn groups(list: &[cellworks_map::Group], out: &mut Vec<Rect>) {
        for group in list {
            nodes(&group.nodes, out);
            groups(&group.groups, out);
        }
    }
    let mut out = Vec::new();
    for district in &map.districts {
        nodes(&district.nodes, &mut out);
        for sub in &district.sub_districts {
            nodes(&sub.nodes, &mut out);
            groups(&sub.groups, &mut out);
        }
    }
    out
}

/// A walk from `from` to `to` around `obstacles`, as the corners of an
/// axis-aligned street path found on a grid over the two points' surrounds
/// (fewest turns among the shortest); an L-shaped walk, x then y, when the
/// grid finds none. Obstacles holding either end do not block.
pub fn route(obstacles: &[Rect], from: Pos2, to: Pos2) -> Vec<Pos2> {
    let fallback = vec![from, pos2(to.x, from.y), to];
    let span = Rect::from_two_pos(from, to);
    let pad = span.size().max_elem().max(20.0) * 0.75;
    let area = span.expand(pad);
    let cell = (area.size().max_elem() / ROUTE_CELLS).max(0.5);
    let (w, h) = (
        (area.width() / cell).ceil() as usize + 1,
        (area.height() / cell).ceil() as usize + 1,
    );
    let index = |p: Pos2| {
        (
            (((p.x - area.min.x) / cell).round() as usize).min(w - 1),
            (((p.y - area.min.y) / cell).round() as usize).min(h - 1),
        )
    };
    let centre =
        |x: usize, y: usize| pos2(area.min.x + x as f32 * cell, area.min.y + y as f32 * cell);

    let mut blocked = vec![false; w * h];
    for rect in obstacles {
        if rect.contains(from) || rect.contains(to) || !rect.intersects(area) {
            continue;
        }
        let (x0, y0) = index(rect.min);
        let (x1, y1) = index(rect.max);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if rect.contains(centre(x, y)) {
                    blocked[y * w + x] = true;
                }
            }
        }
    }
    let (start, goal) = (index(from), index(to));
    blocked[start.1 * w + start.0] = false;
    blocked[goal.1 * w + goal.0] = false;

    // A* over (cell, heading), a turn costing a few steps, so streets are
    // walked straight rather than in a staircase.
    const DIRS: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    const TURN: u32 = 3;
    let state = |x: usize, y: usize, d: usize| (y * w + x) * 4 + d;
    let guess = |x: usize, y: usize| (x.abs_diff(goal.0) + y.abs_diff(goal.1)) as u32;
    let mut cost = vec![u32::MAX; w * h * 4];
    let mut came = vec![usize::MAX; w * h * 4];
    let mut open = BinaryHeap::new();
    for d in 0..4 {
        let s = state(start.0, start.1, d);
        cost[s] = 0;
        open.push(std::cmp::Reverse((guess(start.0, start.1), s)));
    }
    let mut end = None;
    while let Some(std::cmp::Reverse((_, s))) = open.pop() {
        let (cell_index, d) = (s / 4, s % 4);
        let (x, y) = (cell_index % w, cell_index / w);
        if (x, y) == goal {
            end = Some(s);
            break;
        }
        for (nd, (dx, dy)) in DIRS.iter().enumerate() {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            if blocked[ny * w + nx] {
                continue;
            }
            let next = state(nx, ny, nd);
            let step = cost[s] + 1 + if nd == d { 0 } else { TURN };
            if step < cost[next] {
                cost[next] = step;
                came[next] = s;
                open.push(std::cmp::Reverse((step + guess(nx, ny), next)));
            }
        }
    }
    let Some(mut s) = end else {
        return fallback;
    };

    let mut cells = vec![s];
    while came[s] != usize::MAX {
        s = came[s];
        cells.push(s);
    }
    cells.reverse();
    let mut corners = vec![from];
    for pair in cells.windows(2) {
        if pair[0] % 4 != pair[1] % 4 {
            let c = pair[0] / 4;
            corners.push(centre(c % w, c / w));
        }
    }
    // Snap the first and last legs onto the ends, axis-aligned.
    if let Some(first) = corners.get(1).copied() {
        corners[1] = if (first.x - centre(start.0, start.1).x).abs() < cell * 0.5 {
            pos2(from.x, first.y)
        } else {
            pos2(first.x, from.y)
        };
    }
    if corners.len() >= 2 {
        let last = corners.len() - 1;
        let before = corners[last];
        corners[last] = if (before.x - centre(goal.0, goal.1).x).abs() < cell * 0.5 {
            pos2(to.x, before.y)
        } else {
            pos2(before.x, to.y)
        };
    }
    corners.push(to);
    corners.dedup_by(|a, b| (*a - *b).length() < 1e-3);
    corners
}

/// The path's length.
pub fn length(path: &[Pos2]) -> f32 {
    path.windows(2).map(|p| (p[1] - p[0]).length()).sum()
}

/// The point `t` of the way along `path` by length.
pub fn along(path: &[Pos2], t: f32) -> Pos2 {
    let total = length(path);
    let mut left = total * t.clamp(0.0, 1.0);
    for pair in path.windows(2) {
        let leg = (pair[1] - pair[0]).length();
        if left <= leg && leg > 0.0 {
            return pair[0] + (pair[1] - pair[0]) * (left / leg);
        }
        left -= leg;
    }
    path.last().copied().unwrap_or(Pos2::ZERO)
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_walks_around_a_building_in_the_way() {
        let wall = Rect::from_min_max(pos2(40.0, -50.0), pos2(60.0, 50.0));
        let path = route(&[wall], pos2(0.0, 0.0), pos2(100.0, 0.0));
        assert_eq!(path.first(), Some(&pos2(0.0, 0.0)));
        assert_eq!(path.last(), Some(&pos2(100.0, 0.0)));
        for t in 0..=200 {
            let p = along(&path, t as f32 / 200.0);
            assert!(
                !wall.shrink(1.0).contains(p),
                "{p:?} is inside the wall: {path:?}"
            );
        }
        for pair in path.windows(2) {
            let d = pair[1] - pair[0];
            assert!(
                d.x.abs() < 1e-3 || d.y.abs() < 1e-3,
                "streets run straight: {path:?}"
            );
        }
    }

    #[test]
    fn an_open_route_is_one_turn_at_most() {
        let path = route(&[], pos2(0.0, 0.0), pos2(30.0, 40.0));
        assert!(path.len() <= 3, "{path:?}");
        assert!((length(&path) - 70.0).abs() < 1.0, "{path:?}");
    }

    #[test]
    fn along_walks_a_path_by_length() {
        let path = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(10.0, 10.0)];
        assert_eq!(along(&path, 0.0), pos2(0.0, 0.0));
        assert_eq!(along(&path, 0.5), pos2(10.0, 0.0));
        assert_eq!(along(&path, 0.75), pos2(10.0, 5.0));
        assert_eq!(along(&path, 1.0), pos2(10.0, 10.0));
    }
}
