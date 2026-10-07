mod ast;
mod backend;
mod jira;
mod otc;
mod data;
mod nvim;
mod settings;
mod shapes;
mod term;
mod world;

use backend::*;
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Instant, SystemTime};
use world::*;

const BG: Color32 = Color32::from_rgb(0x14, 0x12, 0x0f);
const AMBER: Color32 = Color32::from_rgb(0xf0, 0xb8, 0x4a);
const TEXT: Color32 = Color32::from_rgb(0xe8, 0xdc, 0xc0);
const DIM: Color32 = Color32::from_rgb(0x8a, 0x7a, 0x5a);
const TEAL: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xc0);
const RED: Color32 = Color32::from_rgb(0xd0, 0x4a, 0x3a);
const PALETTE: [Color32; 6] = [
    Color32::from_rgb(0xf0, 0xb8, 0x4a), Color32::from_rgb(0xe0, 0x7a, 0x5a), Color32::from_rgb(0xc8, 0x9a, 0xf0),
    Color32::from_rgb(0x7a, 0xc0, 0xf0), Color32::from_rgb(0xa0, 0xd0, 0x70), Color32::from_rgb(0xf0, 0x7a, 0xb0),
];
const ORANGE: Color32 = Color32::from_rgb(0xe0, 0x7a, 0x3a);
const PURPLE: Color32 = Color32::from_rgb(0xb0, 0x8a, 0xf0);

/// Cyclomatic complexity bands: simple, moderate, complex, untestable.
fn cx_color(cx: u32) -> Color32 {
    match cx {
        0..=5 => Color32::from_rgb(0x3a, 0xa8, 0x9a),
        6..=10 => Color32::from_rgb(0xe0, 0xa0, 0x30),
        11..=20 => ORANGE,
        _ => RED,
    }
}

const EDITOR_BAR: f32 = 44.0;
const MODES: [&str; 4] = ["auto", "acceptEdits", "manual", "plan"];

fn tag_color(t: Tag) -> Color32 {
    match t {
        Tag::Source => Color32::from_rgb(0x4a, 0x8a, 0xc8),
        Tag::Ours => Color32::from_rgb(0xe0, 0xa0, 0x30),
        Tag::Open => RED,
        Tag::Code => Color32::from_rgb(0x3a, 0xa8, 0x9a),
        Tag::None => Color32::from_rgb(0x7a, 0x8a, 0x6a),
    }
}

/// A project as a region of the universe. `center` is in universe coordinates; the current
/// project's world is drawn in its own local coordinates, other worlds are offset by `center - current.center`.
struct Continent {
    path: String,
    name: String,
    /// universe position of the project's local origin; its disc is centred on the content, see `App::disc`
    center: Pos2,
    /// scanned and settled world; taken out while the project is current
    world: Option<World>,
}

/// Display name of a project: its path below ~/Projects, or its folder name elsewhere.
/// Scans a project; a Jira project syncs its tickets first when asked to (or when nothing is cached yet).
fn load_world(p: &str, ignore: &[String], sync: bool) -> World {
    if let Some((site, key)) = jira::parse(p) {
        if sync || jira::load(site, key).is_none() {
            jira::set_status(format!("syncing {key}…"));
            let r = jira::sync(site, key, &|n| jira::set_status(format!("syncing {key}: {n} tickets…")));
            jira::set_status(match r {
                Ok(_) => String::new(),
                Err(e) => format!("Jira sync failed: {e}"),
            });
        }
    }
    scan(p, ignore)
}

fn project_name(path: &str) -> String {
    if let Some((_, key)) = jira::parse(path) {
        return format!("Jira {key}");
    }
    if let Some(root) = otc::parse(path) {
        return format!("Infra {}", root.rsplit('/').next().unwrap_or(root));
    }
    if let Some(root) = data::parse(path) {
        return format!("Data {}", root.rsplit('/').next().unwrap_or(root));
    }
    let root = format!("{}/", projects_root().display());
    path.strip_prefix(&root).map(String::from).unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string())
}

/// The disc that fits a world's factories: its centre relative to the local origin, and its radius.
fn disc_of(w: &World) -> (Vec2, f32) {
    let bb = w.facs.iter().fold(Rect::NOTHING, |a, f| a.union(f.rect));
    if !bb.is_positive() {
        return (Vec2::ZERO, 1500.0);
    }
    (bb.center().to_vec2(), bb.size().length() * 0.5 + (bb.size().max_elem() * 0.06).max(250.0))
}

/// A question a session is waiting on (read off its terminal screen).
struct Ask {
    tool: String,
}

struct Target {
    pos: Pos2,
    sec: Option<(usize, usize)>,
    label: String,
    wait: f32,
    /// tool name, which decides the work animation at the target
    tool: String,
}

struct Servo {
    parent: Option<String>,
    cwd: String,
    name: String,
    pos: Pos2,
    q: VecDeque<Target>,
    task: String,
    last: Instant,
    color: Color32,
    state: String,
    pending: Vec<Ask>,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Inspect,
    Servitors,
    Chats,
    Files,
    Git,
}

#[derive(PartialEq, Clone, Debug)]
enum Sel {
    None,
    Fac(usize),
    Sec(usize, usize),
    Item(usize, usize, usize),
    Servo(String),
    /// a project's circle in the universe
    Continent(usize),
    /// a top-level district, or a sub-district inside one
    District(usize),
    Sub(usize),
    /// a folder box inside a folder view (its path)
    Dir(String),
}

/// Where the user was: project, camera, folder view and selection.
#[derive(Clone)]
struct ViewState {
    project: Option<String>,
    goal: Pos2,
    zoom: f32,
    scope: Option<Scope>,
    sel: Sel,
}

struct App {
    rx: Receiver<Event>,
    projects: Vec<Project>,
    add_path: String,
    add_error: String,
    continents: Vec<Continent>,
    settings: settings::Settings,
    settings_open: bool,
    view_open: bool,
    /// last saved copy, to persist any change to the settings
    settings_saved: settings::Settings,
    /// index of the current project in `continents`
    cur: usize,
    universe_rx: Receiver<World>,
    /// last time the user zoomed or panned; only then does flying over a continent enter it
    last_nav: Instant,
    /// select whatever fills the view once a zoom gesture settles
    zoom_select: bool,
    agents_open: bool,
    /// ask the OS for attention on the next frame (a servitor needs approval)
    attention: bool,
    /// screen position of the selected servitor, for its chat card
    card_anchor: Option<Pos2>,
    palette_open: bool,
    palette_q: String,
    palette_sel: usize,
    palette_focus: bool,
    filter: String,
    current: Option<String>,
    world: Option<World>,
    world_rx: Option<Receiver<World>>,
    cam: Pos2,
    zoom: f32,
    goal: Pos2,
    goal_zoom: f32,
    back: Vec<(Pos2, f32)>,
    view: Vec2,
    servos: HashMap<String, Servo>,
    /// attached Claude sessions (all running hosts, so blocked ones are noticed), see term.rs
    terms: HashMap<String, term::Term>,
    /// sessions whose terminal window is open
    term_open: Vec<String>,
    term_focus: Option<String>,
    term_rects: Vec<Rect>,
    live_at: Instant,
    /// folder view: only this folder and the files elsewhere it connects to
    scope: Option<Scope>,
    scope_back: Vec<Scope>,
    /// camera framing the whole folder view, ring included
    scope_frame: Option<(Pos2, f32)>,
    /// folders laid out and pinned by the current folder view
    scope_pinned: Vec<usize>,
    /// views visited before (back) and after (forward) the current one, like a browser
    hist_back: Vec<ViewState>,
    hist_fwd: Vec<ViewState>,
    restoring: bool,
    /// an editor may only open once the camera has arrived after a jump
    editor_hold: bool,
    /// lanes and lines drawn this frame, for cmd+click: (screen from, screen to, what each end is)
    conn_hits: Vec<(Pos2, Pos2, Sel, Sel)>,
    /// folder boxes in the current folder view: (path, region)
    scope_regions: Vec<(String, Rect)>,
    /// files shown around the folder in folder view, with their ring positions (world)
    satellites: Vec<((usize, usize), Rect)>,
    rescan_rx: Option<Receiver<World>>,
    /// a servitor changed files: rescan the project at this moment (debounced)
    rescan_due: Option<Instant>,
    rebuild_rx: Option<Receiver<(bool, String)>>,
    rebuild_status: String,
    sel: Sel,
    trace: Option<(usize, usize, usize)>,
    follow: Option<String>,
    tab: Tab,
    new_prompt: String,
    mode: usize,
    sessions: Vec<SessionInfo>,
    files_filter: String,
    git: GitState,
    git_rx: Option<Receiver<(String, GitState)>>,
    git_at: Instant,
    filter_q: String,
    /// what the filter matches: projects, districts, folders, files, cells
    filter_kinds: [bool; 5],
    filter_focus: bool,
    git_sel: HashSet<String>,
    commit_msg: String,
    /// name typed into the branch menu, for a new branch or worktree
    branch_new: String,
    /// (project, folder or file) shown once that project's map is loaded: a folder opens its folder view,
    /// a file is zoomed into until its editor opens
    pending_dir: Option<(String, String)>,
    /// what git said when the last branch-menu action failed
    branch_err: String,
    git_out: String,
    viewer: Option<(String, String, bool)>,
    viewer_line: Option<u32>,
    viewer_jump: bool,
    fit_pending: bool,
    /// camera keeps the selection centred while the layout moves
    lock: bool,
    dragging: Option<usize>,
    /// a district (false) or sub-district (true) being dragged
    dragging_group: Option<(bool, usize)>,
    dragging_dir: Option<String>,
    nvim: Option<nvim::Nvim>,
    /// screen area the embedded editor was painted in last frame
    nvim_rect: Option<Rect>,
    nvim_focus: bool,
    /// editors popped out into their own windows; the focused one gets the keyboard
    popped: Vec<nvim::Nvim>,
    /// locked into the current project (double-clicked): other projects are hidden until Esc
    project_locked: bool,
    pop_focus: Option<usize>,
    pop_rects: Vec<Rect>,
    nvim_scroll: f32,
    /// file whose editor is showing; the camera stays centred on it
    editor_sec: Option<(usize, usize)>,
    /// editor covers the whole canvas instead of sitting in its machine
    editor_full: bool,
    left_open: bool,
    right_open: bool,
    log: VecDeque<String>,
    t: f32,
    seed: u64,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let (tx, rx) = channel();
        start_tailer(tx.clone(), cc.egui_ctx.clone());
        let mut v = egui::Visuals::dark();
        v.panel_fill = Color32::from_rgb(0x1e, 0x1b, 0x16);
        v.window_fill = Color32::from_rgb(0x1e, 0x1b, 0x16);
        v.extreme_bg_color = Color32::from_rgb(0x12, 0x10, 0x0d);
        v.selection.bg_fill = Color32::from_rgb(0x6a, 0x50, 0x20);
        v.hyperlink_color = TEAL;
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.set_visuals_of(egui::Theme::Dark, v);
        let settings = settings::Settings::load();
        let mut projects = projects();
        for p in settings.extra_projects.iter().filter(|p| std::path::Path::new(p).is_dir() || jira::parse(p).is_some() || otc::parse(p).is_some() || data::parse(p).is_some()) {
            if !projects.iter().any(|x| &x.path == p) {
                projects.push(Project { path: p.clone(), name: project_name(p), last: SystemTime::UNIX_EPOCH });
            }
        }
        let start = std::env::args().nth(1).filter(|a| !a.starts_with("--")).or_else(|| projects.first().map(|p| p.path.clone()));
        if let Some(p) = start.as_ref().filter(|p| !projects.iter().any(|x| &x.path == *p)) {
            projects.insert(0, Project { path: p.clone(), name: p.rsplit('/').next().unwrap_or(p).into(), last: SystemTime::now() });
        }
        let continents = projects.iter().map(|p| Continent { path: p.path.clone(), name: p.name.clone(), center: Pos2::ZERO, world: None }).collect();
        // scan every other project in the background so the universe has terrain
        let (utx, universe_rx) = channel();
        let others: Vec<String> = if settings.background_scan { projects.iter().map(|p| p.path.clone()).filter(|p| Some(p) != start.as_ref()).collect() } else { vec![] };
        let ctx = cc.egui_ctx.clone();
        let ignore = settings.ignored_folders.clone();
        std::thread::spawn(move || {
            for p in others {
                let mut w = scan(&p, &ignore);
                for _ in 0..900 {
                    if !w.step(1.0 / 60.0) {
                        break;
                    }
                }
                if utx.send(w).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
        let mut app = App {
            continents, cur: usize::MAX, universe_rx, settings_open: false, view_open: true, settings_saved: settings.clone(),
            left_open: settings.left_panel_open, right_open: settings.right_panel_open, mode: settings.default_servitor_mode.min(MODES.len() - 1), settings, last_nav: Instant::now() - std::time::Duration::from_secs(60), zoom_select: false, agents_open: true, attention: false, card_anchor: None, palette_open: false, palette_q: String::new(), palette_sel: 0, palette_focus: false,
            rx, projects, add_path: String::new(), add_error: String::new(), filter: String::new(), current: None, world: None, world_rx: None,
            cam: Pos2::ZERO, zoom: 0.2, goal: Pos2::ZERO, goal_zoom: 0.2, back: vec![], view: vec2(1200.0, 800.0),
            servos: HashMap::new(), terms: HashMap::new(), term_open: vec![], term_focus: None, term_rects: vec![], live_at: Instant::now() - std::time::Duration::from_secs(10), scope: None, scope_back: vec![], scope_frame: None, scope_pinned: vec![], hist_back: vec![], hist_fwd: vec![], restoring: false, editor_hold: false, conn_hits: vec![], scope_regions: vec![], satellites: vec![], rescan_rx: None, rescan_due: None, rebuild_rx: None, rebuild_status: String::new(), sel: Sel::None, trace: None, follow: None,
            tab: Tab::Servitors, new_prompt: String::new(),
            sessions: vec![], files_filter: String::new(), git: GitState::default(), git_rx: None, git_at: Instant::now(), filter_q: String::new(), filter_kinds: [true; 5], filter_focus: false, git_sel: HashSet::new(), commit_msg: String::new(), branch_new: String::new(), pending_dir: None, branch_err: String::new(),
            git_out: String::new(), viewer: None, viewer_line: None, viewer_jump: false, fit_pending: false, lock: false, dragging: None, dragging_group: None, dragging_dir: None, nvim: None, nvim_rect: None, nvim_focus: false, popped: vec![], project_locked: false, pop_focus: None, pop_rects: vec![], nvim_scroll: 0.0, editor_sec: None, editor_full: false, log: VecDeque::new(), t: 0.0, seed: 0x9e3779b97f4a7c15,
        };
        app.layout_universe();
        if let Some(p) = start {
            app.open_project(&p, &cc.egui_ctx);
        }
        app
    }

    /// Adds any folder to the project list (and the universe), remembers it, and flies there.
    fn add_project(&mut self, raw: &str, ctx: &egui::Context) {
        // infrastructure ("otc:<folder>") or data structures ("data:<repo>") of a folder
        if let Some((kind, root)) = raw.trim().split_once(':').filter(|(k, _)| matches!(*k, "otc" | "data")) {
            let root = if let Some(rest) = root.strip_prefix('~') { format!("{}{rest}", std::env::var("HOME").unwrap_or_default()) } else { root.to_string() };
            let path = format!("{kind}:{}", root.trim_end_matches('/'));
            self.add_error.clear();
            self.add_path.clear();
            if !self.projects.iter().any(|p| p.path == path) {
                self.projects.insert(0, Project { path: path.clone(), name: project_name(&path), last: SystemTime::now() });
                self.continents.push(Continent { path: path.clone(), name: project_name(&path), center: Pos2::ZERO, world: None });
                self.layout_universe();
            }
            if !self.settings.extra_projects.contains(&path) {
                self.settings.extra_projects.push(path.clone());
            }
            self.open_project(&path, ctx);
            return;
        }
        // a Jira project: "jira:<site>/<KEY>" or any link into it, e.g. https://x.atlassian.net/browse/KEY-12
        let re = regex::Regex::new(r"^(?:jira:|https?://)([a-z0-9-]+\.atlassian\.net)/(?:.*?(?:browse|projects)/)?([A-Z][A-Z0-9]+)").unwrap();
        if let Some(c) = re.captures(raw.trim()) {
            let path = format!("jira:{}/{}", &c[1], &c[2]);
            self.add_error.clear();
            self.add_path.clear();
            if !self.projects.iter().any(|p| p.path == path) {
                self.projects.insert(0, Project { path: path.clone(), name: format!("Jira {}", &c[2]), last: SystemTime::now() });
                self.continents.push(Continent { path: path.clone(), name: format!("Jira {}", &c[2]), center: Pos2::ZERO, world: None });
                self.layout_universe();
            }
            if !self.settings.extra_projects.contains(&path) {
                self.settings.extra_projects.push(path.clone());
            }
            self.open_project(&path, ctx);
            return;
        }
        let expanded = if let Some(rest) = raw.trim().strip_prefix('~') { format!("{}{rest}", std::env::var("HOME").unwrap_or_default()) } else { raw.trim().to_string() };
        let Ok(path) = std::fs::canonicalize(&expanded) else {
            self.add_error = format!("{expanded} does not exist");
            return;
        };
        if !path.is_dir() {
            self.add_error = format!("{} is not a folder", path.display());
            return;
        }
        let path = path.to_string_lossy().trim_end_matches('/').to_string();
        self.add_error.clear();
        self.add_path.clear();
        if !self.projects.iter().any(|p| p.path == path) {
            self.projects.insert(0, Project { path: path.clone(), name: project_name(&path), last: SystemTime::now() });
            self.continents.push(Continent { path: path.clone(), name: project_name(&path), center: Pos2::ZERO, world: None });
            self.layout_universe();
        }
        if !self.settings.extra_projects.contains(&path) {
            self.settings.extra_projects.push(path.clone());
        }
        self.open_project(&path, ctx);
    }

    fn remove_project(&mut self, path: &str) {
        self.settings.extra_projects.retain(|p| p != path);
        if self.current.as_deref() == Some(path) {
            return; // keep the one in view; it disappears from the list at the next start
        }
        self.projects.retain(|p| p.path != path);
        if let Some(i) = self.continents.iter().position(|c| c.path == path) {
            self.continents.remove(i);
            if i < self.cur {
                self.cur -= 1;
            }
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed % 10_000) as f32 / 10_000.0
    }

    fn log(&mut self, s: String) {
        self.log.push_front(s);
        self.log.truncate(10);
    }

    /// Flies to a project from the list.
    fn open_project(&mut self, path: &str, ctx: &egui::Context) {
        self.record();
        self.enter(path, ctx);
        if self.world.is_some() {
            self.overview();
        } else {
            self.fit_pending = true;
        }
    }

    /// Makes `path` the current project without moving the camera in universe space.
    fn enter(&mut self, path: &str, ctx: &egui::Context) {
        let Some(q) = self.continents.iter().position(|c| c.path == path) else { return };
        let old_center = self.continents.get(self.cur).map_or(Pos2::ZERO, |c| c.center);
        self.drop_scope();
        self.scope_back.clear();
        if let (Some(w), Some(c)) = (self.world.take(), self.continents.get_mut(self.cur)) {
            c.world = Some(w);
        }
        self.cur = q;
        let d = old_center - self.continents[q].center;
        self.cam += d;
        self.goal += d;
        self.current = Some(path.to_string());
        self.world = self.continents[q].world.take();
        self.sel = Sel::None;
        self.trace = None;
        self.follow = None;
        self.lock = false;
        self.editor_sec = None;
        self.back.clear();
        if self.world.is_none() {
            let (tx, rx) = channel();
            let (p, ctx2, ignore) = (path.to_string(), ctx.clone(), self.settings.ignored_folders.clone());
            std::thread::spawn(move || {
                let _ = tx.send(load_world(&p, &ignore, false));
                ctx2.request_repaint();
            });
            self.world_rx = Some(rx);
        } else {
            self.world_rx = None;
        }
        let gate = self.gate().center();
        for s in self.servos.values_mut() {
            s.q.clear();
            if s.cwd == path || s.cwd.starts_with(&format!("{path}/")) {
                s.pos = gate;
            }
        }
        self.sessions = sessions(path);
        self.git = git_state(path);
        self.git_sel.clear();
        for s in self.servos.values_mut() {
            s.q.clear();
        }
    }

    /// Places continents on a golden-angle spiral, most recent project in the middle, without overlaps.
    fn layout_universe(&mut self) {
        let old = self.continents.get(self.cur).map(|c| c.center);
        let discs: Vec<(Vec2, f32)> = (0..self.continents.len())
            .map(|i| if i == self.cur { self.world.as_ref() } else { self.continents[i].world.as_ref() }.map_or((Vec2::ZERO, 2000.0), disc_of))
            .collect();
        let mut placed: Vec<(Pos2, f32)> = vec![];
        for (c, (off, radius)) in self.continents.iter_mut().zip(discs) {
            let mut k = 0;
            let pos = loop {
                let a = k as f32 * 2.399_963;
                let r = 700.0 * (k as f32).sqrt();
                let pos = pos2(a.cos() * r, a.sin() * r);
                if placed.iter().all(|(p, pr)| p.distance(pos) >= pr + radius + 900.0) {
                    break pos;
                }
                k += 1;
            };
            c.center = pos - off;
            placed.push((pos, radius));
        }
        // the current project stays where it is; the others settle around it
        if let (Some(old), Some(c)) = (old, self.continents.get(self.cur)) {
            let d = old - c.center;
            for c in &mut self.continents {
                c.center += d;
            }
        }
    }

    /// Disc of continent `i` in universe coordinates; the current project's follows its live layout.
    fn disc(&self, i: usize) -> (Pos2, f32) {
        let c = &self.continents[i];
        let w = if i == self.cur { self.world.as_ref() } else { c.world.as_ref() };
        let (off, r) = w.map_or((Vec2::ZERO, 2000.0), disc_of);
        (c.center + off, r)
    }

    fn in_project(&self, s: &Servo) -> bool {
        self.current.as_ref().is_some_and(|p| s.cwd == *p || s.cwd.starts_with(&format!("{p}/")))
    }

    /// Short name of what a selection points at, for tooltips.
    fn sel_label(&self, s: &Sel) -> String {
        let Some(w) = &self.world else { return String::new() };
        match s {
            Sel::Fac(f) => format!("{}/", w.facs[*f].title),
            Sel::Sec(f, s) | Sel::Item(f, s, _) => w.facs[*f].sections[*s].rel.clone().unwrap_or_else(|| format!("{} › {}", w.facs[*f].id, w.facs[*f].sections[*s].title)),
            Sel::District(d) => w.districts[*d].clone(),
            Sel::Sub(k) => w.subdistricts[*k].1.clone(),
            Sel::Dir(p) => format!("{p}/"),
            _ => String::new(),
        }
    }

    /// Flies to the project holding `dir` (a folder or a file) and shows it once its map is there.
    fn show_code(&mut self, dir: &str, ctx: &egui::Context) {
        // macOS paths match regardless of case (a project may be added as ~/projects/...), so compare without it
        let lower = dir.to_lowercase();
        let project = self.continents.iter().map(|c| c.path.clone()).filter(|p| lower.starts_with(&format!("{}/", p.to_lowercase()))).max_by_key(|p| p.len());
        let Some(project) = project.or_else(|| {
            // nothing holds it yet: add the repository it lives in
            let repo = std::path::Path::new(dir).ancestors().find(|a| a.join(".git").exists()).map(|a| a.to_string_lossy().to_string());
            repo.inspect(|r| self.add_project(r, ctx))
        }) else { return };
        let rel = dir[project.len() + 1..].to_string();
        if self.current.as_deref() != Some(project.as_str()) {
            self.open_project(&project, ctx);
        }
        self.pending_dir = Some((project, rel));
    }

    /// Goes to what a connection leads to: inside the current folder view it is selected and framed;
    /// outside it, its folder's view opens first.
    fn navigate_to(&mut self, target: Sel) {
        let Some(f) = (match &target {
            Sel::Fac(f) | Sel::Sec(f, _) | Sel::Item(f, _, _) => Some(*f),
            _ => None,
        }) else {
            if let Sel::District(d) = target {
                if let Some(r) = self.world.as_ref().map(|w| w.region(false, d).0) {
                    self.sel = Sel::District(d);
                    self.fit_to(r);
                }
            }
            return;
        };
        self.reveal(f);
        match target {
            Sel::Sec(f, s) | Sel::Item(f, s, _) => self.goto_section(f, s),
            _ => {
                if let Some(r) = self.world.as_ref().map(|w| w.facs[f].rect) {
                    self.sel = Sel::Fac(f);
                    self.tab = Tab::Inspect;
                    self.fit_to(r);
                }
            }
        }
    }

    /// Whether the other projects are shown around the current one.
    fn universe(&self) -> bool {
        self.settings.show_universe && !self.project_locked
    }

    fn view_state(&self) -> ViewState {
        ViewState { project: self.current.clone(), goal: self.goal, zoom: self.goal_zoom, scope: self.scope.clone(), sel: self.sel.clone() }
    }

    /// Remembers the current view before a jump (so Back returns to it) and closes any editor until the camera arrives.
    fn record(&mut self) {
        self.editor_sec = None;
        self.editor_hold = true;
        self.nvim_focus = false;
        if self.restoring {
            return;
        }
        let v = self.view_state();
        let same = self.hist_back.last().is_some_and(|l| l.project == v.project && l.scope == v.scope && l.sel == v.sel && l.goal.distance(v.goal) * v.zoom < 40.0 && (l.zoom / v.zoom - 1.0).abs() < 0.1);
        if !same {
            self.hist_back.push(v);
            if self.hist_back.len() > 80 {
                self.hist_back.remove(0);
            }
        }
        self.hist_fwd.clear();
    }

    fn restore(&mut self, v: ViewState, ctx: &egui::Context) {
        self.restoring = true;
        self.editor_sec = None;
        self.editor_hold = true;
        if let Some(p) = v.project.as_ref().filter(|p| self.current.as_ref() != Some(*p)) {
            self.enter(p, ctx);
        }
        if v.scope != self.scope {
            // leave whatever view is open (without re-entering older ones), then open the remembered one
            self.scope_back.clear();
            if self.scope.is_some() {
                self.leave_scope();
            }
            if let Some(sc) = v.scope.clone() {
                self.enter_scope(sc);
            }
            self.scope_back.clear();
        }
        self.sel = v.sel;
        self.goal = v.goal;
        self.goal_zoom = v.zoom;
        self.lock = false;
        self.follow = None;
        self.restoring = false;
    }

    fn go_back(&mut self, ctx: &egui::Context) {
        if let Some(v) = self.hist_back.pop() {
            self.hist_fwd.push(self.view_state());
            self.restore(v, ctx);
        }
    }

    fn go_forward(&mut self, ctx: &egui::Context) {
        if let Some(v) = self.hist_fwd.pop() {
            self.hist_back.push(self.view_state());
            self.restore(v, ctx);
        }
    }

    fn fit_to(&mut self, r: Rect) {
        self.record();
        self.back.push((self.goal, self.goal_zoom));
        self.goal = r.center();
        self.goal_zoom = (self.view.x / r.width()).min(self.view.y / r.height()) * 0.85;
        self.follow = None;
    }

    fn bounds(&self) -> Rect {
        self.world.as_ref().map(|w| w.facs.iter().fold(Rect::NOTHING, |a, f| a.union(f.rect))).filter(|r| r.is_positive()).unwrap_or(Rect::from_center_size(Pos2::ZERO, Vec2::splat(1000.0)))
    }

    fn overview(&mut self) {
        let bb = self.bounds().union(self.gate());
        self.goal = bb.center();
        self.goal_zoom = (self.view.x / bb.width()).min(self.view.y / bb.height()) * 0.9 * self.settings.default_zoom;
        self.back.clear();
    }

    fn gate(&self) -> Rect {
        let bb = self.bounds();
        Rect::from_min_size(pos2(bb.center().x - 500.0, bb.min.y - 700.0), vec2(1000.0, 400.0))
    }

    fn servo_for(&mut self, agent: &str, parent: Option<String>, cwd: Option<String>) {
        if self.servos.contains_key(agent) {
            return;
        }
        let pinfo = parent.as_ref().and_then(|p| self.servos.get(p)).map(|p| (p.cwd.clone(), p.pos));
        let cwd = cwd.or(pinfo.as_ref().map(|p| p.0.clone())).or(self.terms.get(agent).map(|t| t.cwd.clone())).unwrap_or_default();
        let base = cwd.rsplit('/').next().unwrap_or("").to_string();
        let name = if parent.is_some() { format!("sub-{}", agent.chars().skip(6).take(5).collect::<String>()) } else { format!("{base}·{}", agent.chars().take(4).collect::<String>()) };
        let color = if parent.is_some() { TEAL } else { PALETTE[self.servos.len() % PALETTE.len()] };
        let pos = pinfo.map(|p| p.1).unwrap_or(self.gate().center());
        self.log(format!("{name} {}", if parent.is_some() { "spawned" } else { "came online" }));
        self.servos.insert(agent.to_string(), Servo {
            parent, cwd, name, pos, q: VecDeque::new(), task: "arriving".into(), last: Instant::now(), color, state: "working".into(), pending: vec![],
        });
    }

    fn resolve(&mut self, tool: &str, input: &Value) -> Target {
        let label = format!("{tool} {}", short_input(input));
        let p = ["file_path", "notebook_path", "path"].iter().find_map(|k| input[*k].as_str()).unwrap_or("").to_string();
        let cmd = input["command"].as_str().unwrap_or("");
        let snippet = input["old_string"].as_str().or(input["new_string"].as_str()).unwrap_or("").chars().take(60).collect::<String>().trim().to_string();
        let r1 = self.rand();
        let hit = self.world.as_ref().and_then(|w| {
            let mut hit = w.by_path.get(&p).copied();
            if hit.is_none() && !cmd.is_empty() {
                hit = w.by_path.iter().filter(|(k, _)| cmd.contains(k.as_str())).max_by_key(|(k, _)| k.len()).map(|(_, v)| *v);
            }
            if hit.is_none() && !p.is_empty() {
                hit = w.by_path.iter().filter(|(k, _)| p.starts_with(&format!("{k}/"))).max_by_key(|(k, _)| k.len()).map(|(_, v)| *v);
            }
            let (f, s) = hit?;
            let fac = &w.facs[f];
            let s = s
                .or_else(|| (!snippet.is_empty()).then(|| fac.sections.iter().position(|x| x.body.contains(&snippet))).flatten())
                .or_else(|| (!fac.sections.is_empty()).then(|| (r1 * fac.sections.len() as f32) as usize))?;
            Some((f, s, fac.sections[s].rect.center()))
        });
        match hit {
            Some((f, s, pos)) => Target { pos, sec: Some((f, s)), label, wait: 0.8, tool: tool.to_string() },
            None => {
                let g = self.gate();
                let (a, b) = (self.rand(), self.rand());
                Target { pos: pos2(g.min.x + 60.0 + a * (g.width() - 120.0), g.min.y + 90.0 + b * (g.height() - 140.0)), sec: None, label, wait: 0.8, tool: tool.to_string() }
            }
        }
    }

    fn handle(&mut self, e: Event) {
        match e {
            Event::Tool { agent, parent, cwd, tool, input } => {
                self.servo_for(&agent, parent, cwd);
                let here = self.in_project(&self.servos[&agent]);
                // new files become construction sites; edits type their new lines into the file
                let mut build_secs = None;
                if let (true, Some(w)) = (here, self.world.as_mut()) {
                    let path = input["file_path"].as_str().unwrap_or("");
                    if tool == "Write" {
                        let content = input["content"].as_str().unwrap_or("");
                        if w.add_build(path, content).is_some() {
                            build_secs = Some(World::build_secs(content.lines().count()));
                            self.log.push_front(format!("construction: {}", path.rsplit('/').next().unwrap_or(path)));
                        }
                    }
                    let new_text = input["new_string"].as_str().or(input["edits"][0]["new_string"].as_str()).or(input["content"].as_str().filter(|_| build_secs.is_none()));
                    if let (Some(text), Some(&(f, Some(si)))) = (new_text, w.by_path.get(path)) {
                        if w.facs[f].sections[si].build.is_none() {
                            w.facs[f].sections[si].edit = Some(world::Build { lines: text.lines().map(String::from).collect(), progress: 0.0, done_at: None });
                        }
                    }
                }
                if here && matches!(tool.as_str(), "Write" | "Edit" | "MultiEdit" | "NotebookEdit") {
                    self.rescan_due = Some(Instant::now() + std::time::Duration::from_secs(4));
                }
                let t = if here { Some(self.resolve(&tool, &input)).map(|mut t| {
                    if let Some(secs) = build_secs {
                        t.wait = secs;
                    }
                    t
                }) } else { None };
                let s = self.servos.get_mut(&agent).unwrap();
                s.last = Instant::now();
                if s.pending.is_empty() {
                    s.state = "working".into();
                }
                match t {
                    Some(t) => {
                        s.q.push_back(t);
                        while s.q.len() > 8 {
                            s.q.pop_front();
                        }
                    }
                    None => s.task = format!("{tool} {}", short_input(&input)),
                }
            }
        }
    }

    /// Scans the current project again in the background (AST metrics, imports, links); the layout is kept.
    fn rescan(&mut self, ctx: &egui::Context) {
        let Some(p) = self.current.clone() else { return };
        if self.rescan_rx.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let (ctx2, ignore) = (ctx.clone(), self.settings.ignored_folders.clone());
        std::thread::spawn(move || {
            let _ = tx.send(load_world(&p, &ignore, true));
            ctx2.request_repaint();
        });
        self.rescan_rx = Some(rx);
        self.rescan_due = None;
    }

    fn take_rescan(&mut self) {
        let Some(mut w) = self.rescan_rx.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
        self.rescan_rx = None;
        if self.current.as_deref() != Some(&w.repo) {
            return;
        }
        if let Some(old) = &self.world {
            if old.organize != 0 {
                let changed: HashSet<String> = self.git.status.iter().map(|(_, f)| f.clone()).collect();
                w.organize(old.organize, &changed);
            }
            w.adopt_layout(old);
        }
        // indices may have shifted: keep only selections that cannot point at the wrong thing
        if !matches!(self.sel, Sel::Servo(_) | Sel::Continent(_) | Sel::None) {
            self.sel = Sel::None;
            self.trace = None;
        }
        self.world = Some(w);
        self.log("rescanned".into());
    }

    /// Rebuilds Cellworks from its own source and, when that works, restarts into the new binary.
    /// Claude sessions live in their own hosts and are attached again after the restart.
    fn rebuild_and_restart(&mut self, ctx: &egui::Context) {
        if self.rebuild_rx.is_some() {
            return;
        }
        self.rebuild_status = "building…".into();
        let (tx, rx) = channel();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let out = std::process::Command::new("cargo").args(["build", "--release"]).current_dir(env!("CARGO_MANIFEST_DIR")).output();
            let r = match out {
                Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stderr).lines().rev().take(30).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")),
                Err(e) => (false, e.to_string()),
            };
            let _ = tx.send(r);
            ctx2.request_repaint();
        });
        self.rebuild_rx = Some(rx);
    }

    fn take_rebuild(&mut self) {
        let Some((ok, out)) = self.rebuild_rx.as_ref().and_then(|rx| rx.try_recv().ok()) else { return };
        self.rebuild_rx = None;
        if !ok {
            self.rebuild_status = format!("build failed:\n{out}");
            self.settings_open = true;
            return;
        }
        use std::os::unix::process::CommandExt;
        let exe = std::env::current_exe().unwrap_or_default();
        let err = std::process::Command::new(exe).args(self.current.iter()).exec();
        self.rebuild_status = format!("restart failed: {err}");
    }

    /// The folder view a selection opens: a folder with everything below it, a district or sub-district.
    fn scope_for(&self, sel: &Sel) -> Option<Scope> {
        let w = self.world.as_ref()?;
        Some(match sel {
            Sel::Fac(f) | Sel::Sec(f, _) if w.facs[*f].kind == Kind::Cell => Scope::Cells,
            Sel::Fac(f) | Sel::Sec(f, _) => Scope::Path(w.facs[*f].id.clone()),
            Sel::Dir(p) => Scope::Path(p.clone()),
            Sel::District(d) if w.n_cells > 0 && *d == 0 && w.organize == 0 => Scope::Cells,
            Sel::District(d) if w.organize == 0 => Scope::Path(w.districts[*d].clone()),
            Sel::District(d) => Scope::District(*d),
            Sel::Sub(k) => Scope::Path(format!("{}/{}", w.districts[w.subdistricts[*k].0], w.subdistricts[*k].1)),
            _ => return None,
        })
    }

    /// Folder view: only these folders, laid out tidily in boxes by their next subfolder, with what they
    /// connect to outside on a ring around them. Double-click a box to go deeper; Esc goes back up.
    fn enter_scope(&mut self, sc: Scope) {
        self.record();
        let Some(w) = &mut self.world else { return };
        let members = scope_members(w, &sc);
        if members.is_empty() {
            return;
        }
        for &i in &self.scope_pinned {
            if let Some(f) = w.facs.get_mut(i) {
                f.pinned = false;
            }
        }
        if let Some(old) = self.scope.clone().filter(|o| *o != sc) {
            self.scope_back.push(old);
        }
        layout_scope(w, &sc, &members);
        w.set_focus(world::Focus::None);
        let st = &self.settings;
        let (ring, _) = scope_ring(w, &members, st.conn_level == 0, [st.show_import_belts, st.conn_calls, st.show_context_pipes], st.conn_min_weight);
        let inner = members.iter().fold(Rect::NOTHING, |a, &f| a.union(w.facs[f].rect));
        let bounds = ring.iter().fold(inner, |a, (_, r, _)| a.union(*r)).expand(250.0);
        self.scope_pinned = members;
        self.scope = Some(sc);
        self.trace = None;
        self.sel = Sel::None;
        self.lock = false;
        self.follow = None;
        self.goal = bounds.center();
        self.goal_zoom = (self.view.x / bounds.width()).min(self.view.y / bounds.height());
        self.scope_frame = Some((self.goal, self.goal_zoom));
    }

    /// Whether factory `fi` is hidden by the folder view.
    fn outside_scope(&self, fi: usize) -> bool {
        match (&self.scope, &self.world) {
            (Some(sc), Some(w)) => !scope_has(w, sc, fi),
            _ => false,
        }
    }

    /// Closes the folder view without moving the camera: the folders return to the physics.
    fn drop_scope(&mut self) {
        self.scope = None;
        self.scope_frame = None;
        self.satellites.clear();
        self.scope_regions.clear();
        if let Some(w) = &mut self.world {
            for &i in &self.scope_pinned {
                if let Some(f) = w.facs.get_mut(i) {
                    f.pinned = false;
                    f.alpha = 1.0;
                }
            }
            w.alpha = 1.0;
        }
        self.scope_pinned.clear();
    }

    /// Opens the folder view that shows factory `f` when the current one hides it.
    fn reveal(&mut self, f: usize) {
        if self.outside_scope(f) {
            if let Some(sc) = self.scope_for(&Sel::Fac(f)) {
                self.enter_scope(sc);
            }
        }
    }

    /// Back to the previous folder view, or out of folder view (the folders return to the physics).
    fn leave_scope(&mut self) {
        self.record();
        self.drop_scope();
        match self.scope_back.pop() {
            Some(sc) => self.enter_scope(sc),
            None => {
                self.sel = Sel::None;
                self.lock = false;
                self.overview();
            }
        }
    }

    // ---------------- Claude sessions: detachable hosts with the real CLI (see term.rs) ----------------

    /// Starts Claude Code in its own host (or resumes `resume` there) and opens its terminal.
    fn start_session(&mut self, cwd: &str, prompt: Option<String>, resume: Option<String>, ctx: &egui::Context) {
        let id = resume.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut cmd = vec!["claude".to_string(), "--permission-mode".into(), MODES[self.mode].into()];
        match &resume {
            Some(r) => cmd.extend(["--resume".to_string(), r.clone()]),
            None => cmd.extend(["--session-id".to_string(), id.clone()]),
        }
        if let Some(p) = prompt.filter(|p| !p.trim().is_empty()) {
            cmd.push(p);
        }
        match term::spawn(&id, cwd, &cmd) {
            Ok(()) => {
                self.servo_for(&id, None, Some(cwd.to_string()));
                if self.attach(&id, cwd, ctx) {
                    self.show_term(&id);
                }
            }
            Err(e) => self.log(format!("could not start claude: {e}")),
        }
    }

    fn attach(&mut self, id: &str, cwd: &str, ctx: &egui::Context) -> bool {
        if self.terms.contains_key(id) {
            return true;
        }
        match term::Term::attach(id, cwd, ctx.clone()) {
            Ok(t) => {
                self.terms.insert(id.to_string(), t);
                true
            }
            Err(_) => false,
        }
    }

    fn show_term(&mut self, id: &str) {
        if !self.term_open.iter().any(|x| x == id) {
            self.term_open.push(id.to_string());
        }
        self.term_focus = Some(id.to_string());
        self.nvim_focus = false;
    }

    /// Opens a session's terminal: attaches when its host runs, otherwise resumes the session in a new host.
    fn open_term(&mut self, id: &str, ctx: &egui::Context) {
        if let Some(m) = term::list().into_iter().find(|m| m.id == id) {
            if self.attach(id, &m.cwd, ctx) {
                self.show_term(id);
            }
            return;
        }
        let cwd = self.servos.get(id).map(|s| s.cwd.clone()).filter(|c| !c.is_empty()).or(self.current.clone()).unwrap_or_default();
        self.start_session(&cwd, None, Some(id.to_string()), ctx);
    }

    /// Every 2 s: attach to hosts started elsewhere (or before a restart) and drop ended ones.
    fn sync_sessions(&mut self, ctx: &egui::Context) {
        if self.live_at.elapsed().as_secs() < 2 {
            return;
        }
        self.live_at = Instant::now();
        for m in term::list() {
            if !self.terms.contains_key(&m.id) && self.attach(&m.id, &m.cwd, ctx) {
                self.servo_for(&m.id, None, Some(m.cwd.clone()));
            }
        }
        self.terms.retain(|_, t| t.alive);
        let terms = &self.terms;
        self.term_open.retain(|id| terms.contains_key(id));
    }

    /// Feeds terminal output to the screens and turns permission questions into blocked servitors.
    fn pump_terms(&mut self) {
        let mut blocked = vec![];
        for (id, t) in &mut self.terms {
            t.pump();
            let q = t.waiting_question();
            if let Some(s) = self.servos.get_mut(id) {
                match q {
                    Some(question) if s.pending.is_empty() => {
                        s.pending.push(Ask { tool: question });
                        s.state = "waiting".into();
                        blocked.push(s.name.clone());
                    }
                    None if !s.pending.is_empty() => {
                        s.pending.clear();
                        s.state = "working".into();
                    }
                    _ => {}
                }
            }
        }
        for name in blocked {
            self.attention = true;
            self.log(format!("{name} needs approval"));
        }
    }

    /// Popped-out editors, one window per file, so several files can be edited while the map stays free.
    fn editor_windows(&mut self, ctx: &egui::Context) {
        self.pop_rects.clear();
        let font = FontId::monospace(self.settings.editor_font_size);
        let (cw, lh) = (ctx.fonts_mut(|f| f.glyph_width(&font, 'M')), (self.settings.editor_font_size * 1.25).round());
        let root = self.current.clone().unwrap_or_default();
        let mut close = None;
        for i in 0..self.popped.len() {
            let focused = self.pop_focus == Some(i);
            let title = self.popped[i].path.strip_prefix(&format!("{root}/")).unwrap_or(&self.popped[i].path).to_string();
            let mut open = true;
            let mut show = false;
            let resp = egui::Window::new(RichText::new(&title).color(if focused { AMBER } else { TEXT })).id(egui::Id::new(("editor", &self.popped[i].path)))
                .default_size([720.0, 520.0]).min_size([320.0, 160.0]).open(&mut open).show(ctx, |ui| {
                    show = ui.small_button("Show on map").clicked();
                    let (r, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
                    let n = &mut self.popped[i];
                    n.pump();
                    n.resize(((r.rect.width() / cw) as usize).max(10), ((r.rect.height() / lh) as usize).max(3));
                    paint_editor(&painter, n, r.rect, &font, cw, lh, focused);
                    let cell = |p: Pos2| (((p.y - r.rect.min.y) / lh) as usize, ((p.x - r.rect.min.x) / cw) as usize);
                    if r.hovered() {
                        let dy = ui.input(|i| i.smooth_scroll_delta.y);
                        if let (Some(p), true) = (r.hover_pos(), dy.abs() > 0.5) {
                            let (row, col) = cell(p);
                            n.mouse("wheel", if dy > 0.0 { "up" } else { "down" }, row, col);
                        }
                    }
                    if let Some(p) = r.interact_pointer_pos().filter(|_| r.clicked()) {
                        let (row, col) = cell(p);
                        n.mouse("left", "press", row, col);
                        n.mouse("left", "release", row, col);
                        self.pop_focus = Some(i);
                        self.nvim_focus = false;
                        self.term_focus = None;
                    }
                });
            if let Some(resp) = resp {
                self.pop_rects.push(resp.response.rect);
            }
            if show {
                let path = self.popped[i].path.clone();
                let hit = self.world.as_ref().and_then(|w| w.facs.iter().enumerate().find_map(|(f, fac)| fac.sections.iter().position(|s| s.path.as_deref() == Some(path.as_str())).map(|s| (f, s))));
                if let Some((f, s)) = hit {
                    self.navigate_to(Sel::Sec(f, s));
                }
            }
            if !open {
                close = Some(i);
            }
        }
        if let Some(i) = close {
            let mut n = self.popped.remove(i);
            n.input("<Esc>");
            n.input(":silent! wall<CR>");
            std::thread::sleep(std::time::Duration::from_millis(50)); // let it write before it is killed
            self.pop_focus = None;
        }
    }

    /// One window per open session, drawing the real CLI; closing a window only detaches from it.
    fn term_windows(&mut self, ctx: &egui::Context) {
        self.term_rects.clear();
        let font = FontId::monospace(13.0);
        let cw = ctx.fonts_mut(|f| f.glyph_width(&font, 'M'));
        let lh = 17.0;
        for id in self.term_open.clone() {
            let name = self.servos.get(&id).map_or(id.chars().take(8).collect(), |s| s.name.clone());
            let cwd = self.terms.get(&id).map(|t| t.cwd.clone()).unwrap_or_default();
            let mut open = true;
            let (mut follow, mut end) = (false, false);
            let focused = self.term_focus.as_deref() == Some(id.as_str());
            let resp = egui::Window::new(RichText::new(format!("claude · {name}")).color(if focused { AMBER } else { TEXT })).id(egui::Id::new(("term", &id)))
                .default_size([840.0, 540.0]).min_size([360.0, 200.0]).open(&mut open).show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&cwd).size(11.0).color(DIM));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            end = ui.small_button("End session").on_hover_text("stops Claude in this session").clicked();
                            follow = ui.small_button("Show on map").clicked();
                        });
                    });
                    let (r, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
                    let Some(t) = self.terms.get_mut(&id) else { return };
                    let cols = ((r.rect.width() - 8.0) / cw).max(10.0) as u16;
                    let rows = ((r.rect.height() - 8.0) / lh).max(4.0) as u16;
                    t.resize(cols, rows);
                    if r.hovered() {
                        let dy = ui.input(|i| i.smooth_scroll_delta.y);
                        if dy != 0.0 {
                            t.scroll = (t.scroll as f32 + dy / lh).clamp(0.0, 5000.0) as usize;
                        }
                    }
                    term::paint(&painter, t, r.rect, &font, cw, lh, focused);
                    if r.clicked() {
                        self.term_focus = Some(id.clone());
                        self.nvim_focus = false;
                        self.pop_focus = None;
                    }
                });
            if let Some(resp) = resp {
                self.term_rects.push(resp.response.rect);
            }
            if follow {
                self.sel = Sel::Servo(id.clone());
                self.follow = Some(id.clone());
            }
            if end {
                term::kill(&id);
            }
            if !open {
                self.term_open.retain(|x| *x != id);
                if self.term_focus.as_deref() == Some(id.as_str()) {
                    self.term_focus = None;
                }
            }
        }
    }

    fn step_servos(&mut self, dt: f32) {
        let mut heats = vec![];
        let mut logs = vec![];
        let world = &self.world;
        for s in self.servos.values_mut() {
            let qlen = s.q.len() as f32;
            let Some(t) = s.q.front_mut() else { continue };
            if s.task != t.label {
                s.task = t.label.clone();
                logs.push(format!("{}: {}", s.name, t.label));
            }
            if let Some((f, sec)) = t.sec {
                if let Some(w) = world {
                    t.pos = w.facs[f].sections[sec].rect.center();
                }
            }
            let d = t.pos - s.pos;
            let v = (300.0 + d.length()) * (1.0 + qlen) * dt;
            if d.length() > v {
                s.pos += d.normalized() * v;
                continue;
            }
            s.pos = t.pos;
            if let Some(sec) = t.sec {
                heats.push(sec);
            }
            t.wait -= dt;
            if t.wait <= 0.0 {
                s.q.pop_front();
            }
        }
        for l in logs {
            self.log(l);
        }
        if let Some(w) = &mut self.world {
            for f in &mut w.facs {
                for s in &mut f.sections {
                    s.heat = (s.heat - dt * 0.3).max(0.0);
                }
            }
            for (f, s) in heats {
                w.facs[f].sections[s].heat = 1.0;
            }
        }
    }

    fn pick(&self, p: Pos2) -> Sel {
        for (id, s) in &self.servos {
            if self.in_project(s) && s.pos.distance(p) < (12.0 / self.zoom).max(14.0) {
                return Sel::Servo(id.clone());
            }
        }
        let Some(w) = &self.world else { return Sel::None };
        if self.scope.is_some() {
            if let Some((sec, _)) = self.satellites.iter().find(|(_, r)| r.contains(p)) {
                return if sec.1 == usize::MAX { Sel::Fac(sec.0) } else { Sel::Sec(sec.0, sec.1) };
            }
        }
        for (fi, f) in w.facs.iter().enumerate() {
            if !f.rect.contains(p) || self.scope.as_ref().is_some_and(|sc| !scope_has(w, sc, fi)) {
                continue;
            }
            if self.zoom > 0.45 {
                for (si, s) in f.sections.iter().enumerate() {
                    if !s.rect.contains(p) {
                        continue;
                    }
                    if let Some(ii) = s.items.iter().position(|it| it.rect.is_positive() && it.rect.contains(p)) {
                        return Sel::Item(fi, si, ii);
                    }
                    return Sel::Sec(fi, si);
                }
            }
            return Sel::Fac(fi);
        }
        if self.scope.is_some() {
            // folder boxes: the smallest one under the pointer
            return self.scope_regions.iter().filter(|(_, r)| r.contains(p)).min_by(|a, b| a.1.area().total_cmp(&b.1.area())).map_or(Sel::None, |(path, _)| Sel::Dir(path.clone()));
        }
        if w.districts.len() > 1 {
            for k in 0..w.subdistricts.len() {
                let (r, n) = w.region(true, k);
                if n > 1 && r.contains(p) {
                    return Sel::Sub(k);
                }
            }
            for d in 0..w.districts.len() {
                let (r, n) = w.region(false, d);
                if n > 1 && r.contains(p) {
                    return Sel::District(d);
                }
            }
        }
        self.continent_at(p).map_or(Sel::None, Sel::Continent)
    }

    /// Continent under a point given in the current project's local coordinates.
    fn continent_at(&self, p: Pos2) -> Option<usize> {
        let here = self.continents.get(self.cur)?.center;
        self.continents.iter().enumerate().filter(|(i, _)| self.universe() || *i == self.cur)
            .map(|(i, _)| (i, self.disc(i))).find(|(_, (dc, r))| (*dc - here).to_pos2().distance(p) < *r).map(|(i, _)| i)
    }

    /// After a zoom gesture: the file, else factory, else project circle under `ptr` that fills enough of the view.
    fn select_by_zoom(&mut self, ptr: Pos2, rect: Rect) {
        let m = rect.width().min(rect.height());
        let p = self.cam + (ptr - rect.center()) / self.zoom;
        let z = self.zoom;
        let mut found = None;
        if let Some(w) = &self.world {
            if let Some((fi, f)) = w.facs.iter().enumerate().find(|(fi, f)| f.rect.contains(p) && self.scope.as_ref().is_none_or(|sc| scope_has(w, sc, *fi))) {
                if let Some(si) = f.sections.iter().position(|s| s.rect.contains(p) && s.rect.width() * z >= 0.35 * m) {
                    found = Some(Sel::Sec(fi, si));
                } else if f.rect.width() * z >= 0.45 * m {
                    found = Some(Sel::Fac(fi));
                }
            }
        }
        if found.is_none() && self.scope.is_none() {
            found = self.continent_at(p).filter(|&q| self.disc(q).1 * 2.0 * z >= 0.5 * m).map(Sel::Continent);
        }
        let Some(new) = found else { return };
        let same = match (&self.sel, &new) {
            (Sel::Item(f, s, _), Sel::Sec(nf, ns)) => (f, s) == (nf, ns),
            (a, b) => a == b,
        };
        if !same {
            self.sel = new;
            self.tab = Tab::Inspect;
        }
    }

    fn trace_item(&mut self, f: usize, s: usize, i: usize) {
        let Some(w) = &self.world else { return };
        let it = &w.facs[f].sections[s].items[i];
        let r = it.uses.iter().fold(w.facs[f].sections[s].rect, |a, &(uf, us)| a.union(w.facs[uf].sections[us].rect)).expand(100.0);
        self.trace = Some((f, s, i));
        self.fit_to(r);
    }

    fn open_file(&mut self, path: &str, line: Option<u32>) {
        self.viewer_line = line;
        self.viewer_jump = line.is_some();
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| e.to_string());
        let title = path.strip_prefix(&format!("{}/", projects_root().display())).unwrap_or(path).to_string();
        self.viewer = Some((title, text, false));
    }

    fn goto_section(&mut self, f: usize, s: usize) {
        self.reveal(f);
        if let Some(r) = self.world.as_ref().map(|w| w.facs[f].sections[s].rect) {
            self.sel = Sel::Sec(f, s);
            self.tab = Tab::Inspect;
            self.fit_to(r);
            self.refocus();
        }
    }

    /// Re-centres the graph on the current selection and keeps the camera on it.
    fn refocus(&mut self) {
        let focus = match self.sel {
            Sel::Fac(f) => world::Focus::Fac(f),
            Sel::Sec(f, s) | Sel::Item(f, s, _) => world::Focus::Sec((f, s)),
            _ => world::Focus::None,
        };
        self.lock = !matches!(focus, world::Focus::None);
        if let Some(w) = &mut self.world {
            w.set_focus(focus);
        }
    }

    fn sel_rect(&self) -> Option<Rect> {
        let w = self.world.as_ref()?;
        Some(match self.sel {
            Sel::Fac(f) => w.facs[f].rect,
            Sel::Sec(f, s) => w.facs[f].sections[s].rect,
            Sel::Item(f, s, i) => w.facs[f].sections[s].items[i].rect,
            Sel::District(d) => w.region(false, d).0,
            Sel::Sub(k) => w.region(true, k).0,
            _ => return None,
        })
    }

    // ---------------- world canvas ----------------
    fn draw_world(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.view = rect.size();
        if std::mem::take(&mut self.fit_pending) && self.world.is_some() {
            self.overview();
            self.cam = self.goal;
            self.zoom = self.goal_zoom;
            if std::env::var("CELLWORKS_SELECT").is_err() {
                if let Some(zf) = std::env::var("CELLWORKS_ZOOM").ok().and_then(|z| z.parse::<f32>().ok()) {
                    self.goal_zoom *= zf;
                    self.zoom = self.goal_zoom;
                }
            }
            // dev aid: CELLWORKS_SELECT=<relative file> preselects a file once the world is loaded
            if let Ok(rel) = std::env::var("CELLWORKS_SELECT") {
                let hit = self.world.as_ref().and_then(|w| w.facs.iter().enumerate().find_map(|(fi, f)| f.sections.iter().position(|s| s.rel.as_deref() == Some(&rel) || s.title.starts_with(&rel)).map(|si| (fi, si))));
                if let Some((f, s)) = hit {
                    self.goto_section(f, s);
                    self.goal_zoom *= std::env::var("CELLWORKS_ZOOM").ok().and_then(|z| z.parse().ok()).unwrap_or(1.0);
                    self.cam = self.goal;
                    self.zoom = self.goal_zoom;
                }
            }
        }
        painter.rect_filled(rect, 0.0, BG);
        let s2w = |p: Pos2, cam: Pos2, z: f32| cam + (p - rect.center()) / z;

        // drag a factory while zoomed out, otherwise pan
        if resp.drag_started() && self.zoom < 0.6 {
            if let Some(ptr) = resp.interact_pointer_pos() {
                let hit = self.pick(s2w(ptr, self.cam, self.zoom));
                self.dragging = match hit {
                    Sel::Fac(f) | Sel::Sec(f, _) | Sel::Item(f, _, _) => Some(f),
                    _ => None,
                };
                self.dragging_group = match hit {
                    Sel::District(d) => Some((false, d)),
                    Sel::Sub(k) => Some((true, k)),
                    _ => None,
                };
                self.dragging_dir = match &hit {
                    Sel::Dir(p) => Some(p.clone()),
                    _ => None,
                };
                if self.dragging.is_some_and(|f| self.outside_scope(f)) {
                    self.dragging = None; // files on the ring are not draggable
                }
            }
        }
        if resp.dragged() {
            let d = resp.drag_delta() / self.zoom;
            match (self.dragging, self.dragging_group, self.world.as_mut()) {
                (Some(f), _, Some(w)) => w.translate_fac(f, d),
                (None, Some((sub, k)), Some(w)) => w.translate_group(sub, k, d),
                (None, None, Some(w)) if self.dragging_dir.is_some() => {
                    let p = Scope::Path(self.dragging_dir.clone().unwrap());
                    for i in scope_members(w, &p) {
                        w.facs[i].rect = w.facs[i].rect.translate(d);
                    }
                }
                _ => {
                    self.cam -= d;
                    self.goal -= d;
                    self.follow = None;
                    self.lock = false;
                    self.last_nav = Instant::now();
                }
            }
        }
        if resp.drag_stopped() {
            self.dragging_group = None;
            self.dragging_dir = None;
            let keep = self.scope.is_some();
            if let (Some(f), Some(w)) = (self.dragging.take(), self.world.as_mut()) {
                w.facs[f].pinned = keep; // inside a folder view, folders stay where you put them
            }
        }
        let font = FontId::monospace(self.settings.editor_font_size);
        let (char_w, line_h) = (painter.layout_no_wrap("M".into(), font.clone(), TEXT).size().x, (self.settings.editor_font_size * 1.25).round());
        let in_editor = |p: Pos2| self.nvim_rect.filter(|r| r.contains(p)).map(|r| (((p.y - r.min.y) / line_h) as usize, ((p.x - r.min.x) / char_w) as usize));
        let mut editor_scroll = false;
        if self.nvim_focus {
            let (scroll, ptr) = ui.input(|i| (i.smooth_scroll_delta.y, i.pointer.hover_pos()));
            if let (Some((row, col)), Some(n)) = (ptr.and_then(in_editor), self.nvim.as_mut()) {
                editor_scroll = true;
                self.nvim_scroll += scroll;
                while self.nvim_scroll.abs() >= 18.0 {
                    n.mouse("wheel", if self.nvim_scroll > 0.0 { "up" } else { "down" }, row, col);
                    self.nvim_scroll -= 18.0 * self.nvim_scroll.signum();
                }
            }
        }
        if resp.clicked() {
            let hit = resp.interact_pointer_pos().and_then(in_editor);
            self.nvim_focus = hit.is_some();
            if let (Some((row, col)), Some(n)) = (hit, self.nvim.as_mut()) {
                n.mouse("left", "press", row, col);
                n.mouse("left", "release", row, col);
            }
        }
        if resp.hovered() && !editor_scroll {
            let (scroll, pinch, ptr) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta(), i.pointer.hover_pos()));
            if let Some(ptr) = ptr.filter(|_| scroll != 0.0 || pinch != 1.0) {
                let p = s2w(ptr, self.goal, self.goal_zoom);
                let z = (self.goal_zoom * (scroll * 0.003 * self.settings.scroll_zoom_speed).exp() * pinch).clamp(0.002, 14.0);
                self.last_nav = Instant::now();
                self.zoom_select = true;
                if self.editor_sec.is_none() {
                    self.goal = p - (ptr - rect.center()) / z;
                }
                self.goal_zoom = z;
            }
        }
        if let Some(ptr) = resp.hover_pos().filter(|_| !self.nvim_focus && self.nvim_rect.is_none_or(|r| !r.contains(resp.hover_pos().unwrap()))) {
            let hovered = self.pick(s2w(ptr, self.cam, self.zoom));
            if let Some(w) = &self.world {
                match hovered {
                    Sel::Item(f, s, i) => {
                        let it = &w.facs[f].sections[s].items[i];
                        resp.clone().on_hover_ui_at_pointer(|ui| {
                            ui.set_max_width(420.0);
                            ui.label(RichText::new(&it.term).strong().color(AMBER));
                            ui.label(&it.text);
                        });
                    }
                    Sel::Sec(f, s) => {
                        let sec = &w.facs[f].sections[s];
                        resp.clone().on_hover_ui_at_pointer(|ui| {
                            ui.set_max_width(420.0);
                            ui.label(RichText::new(sec.rel.as_deref().unwrap_or(&sec.title)).strong().color(AMBER));
                            ui.label(RichText::new(machine_summary(sec)).color(DIM));
                        });
                    }
                    _ => {}
                }
            }
        }
        let mut lane_highlight = None;
        if ui.input(|i| i.modifiers.command) && !self.nvim_focus {
            if let Some(ptr) = resp.hover_pos() {
                let near = self.conn_hits.iter().map(|h| (seg_dist(ptr, h.0, h.1), h)).filter(|(d, _)| *d < 8.0).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, h)| h.clone());
                if let Some((a, b, sa, sb)) = near {
                    // the far end from the pointer is where the connection leads
                    let to = if ptr.distance(a) > ptr.distance(b) { sa } else { sb };
                    lane_highlight = Some((a, b));
                    let label = self.sel_label(&to);
                    resp.clone().on_hover_ui_at_pointer(|ui| {
                        ui.label(RichText::new(format!("go to {label}")).color(AMBER));
                        ui.label(RichText::new("cmd+click").size(11.0).color(DIM));
                    });
                    if resp.clicked() {
                        self.navigate_to(to);
                        return;
                    }
                }
            }
        }
        if let Some(ptr) = resp.interact_pointer_pos().filter(|_| !self.nvim_focus) {
            let p = s2w(ptr, self.cam, self.zoom);
            if resp.double_clicked() {
                let sel = self.pick(p);
                match sel.clone() {
                    Sel::Fac(_) | Sel::Dir(_) | Sel::District(_) | Sel::Sub(_) => {
                        if let Some(sc) = self.scope_for(&sel) {
                            self.enter_scope(sc);
                        }
                        return;
                    }
                    Sel::Sec(f, _) if self.outside_scope(f) => {
                        if let Some(sc) = self.scope_for(&Sel::Fac(f)) {
                            self.enter_scope(sc);
                        }
                        return;
                    }
                    Sel::Sec(f, s) => { let r = self.world.as_ref().unwrap().facs[f].sections[s].rect; self.fit_to(r) }
                    Sel::Item(f, s, i) => self.trace_item(f, s, i),
                    Sel::Servo(id) => self.follow = Some(id),
                    Sel::Continent(q) => {
                        if q != self.cur {
                            let path = self.continents[q].path.clone();
                            self.open_project(&path, ui.ctx());
                        } else {
                            self.overview();
                        }
                        self.project_locked = true;
                        self.sel = Sel::None;
                        return;
                    }
                    Sel::None => {}
                }
                self.sel = sel;
                self.refocus();
            } else if resp.clicked() {
                self.sel = self.pick(p);
                match self.sel.clone() {
                    Sel::Servo(id) => {
                        if self.terms.contains_key(&id) {
                            self.show_term(&id);
                        }
                    }
                    Sel::None => {}
                    _ => self.tab = Tab::Inspect,
                }
                let on_ring = matches!(self.sel, Sel::Sec(f, _) | Sel::Item(f, _, _) | Sel::Fac(f) if self.outside_scope(f)) || matches!(self.sel, Sel::Dir(_));
                if on_ring {
                    self.lock = false; // a file on the ring: inspect it without flying to its real place
                } else if !matches!(self.sel, Sel::Servo(_)) {
                    self.refocus();
                }
            }
        }

        let dt = ui.input(|i| i.stable_dt).min(0.05);
        self.t += dt;
        if let Some(pos) = self.follow.as_ref().and_then(|id| self.servos.get(id)).map(|s| s.pos) {
            self.goal = pos;
            self.goal_zoom = self.goal_zoom.max(0.8);
        }
        if let Some(w) = &mut self.world {
            w.tick_builds(dt, self.t);
            if self.settings.physics {
                w.step(dt * self.settings.physics_speed);
            } else {
                w.sync_rects();
            }
        }
        if self.editor_hold && (self.goal - self.cam).length() * self.zoom < 6.0 && (self.goal_zoom / self.zoom - 1.0).abs() < 0.03 {
            self.editor_hold = false; // arrived
        }
        let editor_rect = self.editor_sec.and_then(|(f, s)| self.world.as_ref().map(|w| w.facs[f].sections[s].rect));
        if let Some(r) = editor_rect {
            self.goal = r.center();
            self.cam = self.cam + (self.goal - self.cam) * 0.5; // snap harder than the normal glide
        } else if self.lock {
            if let Some(r) = self.sel_rect() {
                self.goal = r.center();
            }
        }
        if self.scope.is_none() && self.universe() && self.zoom < 0.6 && self.dragging.is_none() && self.last_nav.elapsed().as_secs_f32() < 1.5 {
            let here = self.continents.get(self.cur).map_or(Pos2::ZERO, |c| c.center);
            let u = self.cam + here.to_vec2();
            let next = (0..self.continents.len()).filter(|&i| i != self.cur).find(|&i| {
                let (dc, r) = self.disc(i);
                dc.distance(u) < r * 0.8
            }).map(|i| self.continents[i].path.clone());
            if let Some(p) = next {
                self.enter(&p, ui.ctx());
                self.log(format!("entered {}", p.rsplit('/').next().unwrap_or("")));
            }
        }
        if self.zoom_select && (self.goal_zoom / self.zoom - 1.0).abs() < 0.03 && !self.nvim_focus {
            self.zoom_select = false;
            let ptr = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p)).unwrap_or(rect.center());
            self.select_by_zoom(ptr, rect);
        }
        let k = 1.0 - 0.001f32.powf(dt);
        self.cam += (self.goal - self.cam) * k;
        self.zoom *= (self.goal_zoom / self.zoom).powf(k);
        self.step_servos(dt);

        let (cam, z, t) = (self.cam, self.zoom, self.t);
        if let Some(w) = &mut self.world {
            let to_s = |p: Pos2| rect.center() + (p - cam) * z;
            let to_w = |p: Pos2| cam + (p - rect.center()) / z;
            let floor_visible = z > 0.3;
            for f in &mut w.facs {
                for s in &mut f.sections {
                    let sr = Rect::from_min_max(to_s(s.rect.min), to_s(s.rect.max));
                    let slots = if floor_visible && sr.intersects(rect) && machine_lod(sr) == 2 { row_slots(sr, s.items.len()).0 } else { vec![] };
                    for (k, it) in s.items.iter_mut().enumerate() {
                        it.rect = slots.get(k).map_or(Rect::NOTHING, |r| Rect::from_min_max(to_w(r.min), to_w(r.max)));
                    }
                }
            }
        }
        let w2s = |p: Pos2| rect.center() + (p - cam) * z;
        let wr = |r: Rect| Rect::from_min_max(w2s(r.min), w2s(r.max));
        let visible = Rect::from_min_max(s2w(rect.min, cam, z), s2w(rect.max, cam, z));
        let txt = |p: &Painter, pos: Pos2, s: &str, px: f32, c: Color32, max_w: f32| {
            let px_s = (px * z).min(22.0);
            if px_s < 5.0 {
                return;
            }
            let n = ((max_w * z) / (px_s * 0.62)).max(1.0) as usize;
            let s: String = if s.chars().count() > n { s.chars().take(n.saturating_sub(1)).chain(['…']).collect() } else { s.into() };
            p.text(w2s(pos), Align2::LEFT_TOP, s, FontId::monospace(px_s), c);
        };

        if z > 0.08 {
            let g = 100.0;
            let stroke = Stroke::new(1.0, Color32::from_rgb(0x1d, 0x1a, 0x15));
            let mut x = (visible.min.x / g).floor() * g;
            while x < visible.max.x {
                painter.line_segment([w2s(pos2(x, visible.min.y)), w2s(pos2(x, visible.max.y))], stroke);
                x += g;
            }
            let mut y = (visible.min.y / g).floor() * g;
            while y < visible.max.y {
                painter.line_segment([w2s(pos2(visible.min.x, y)), w2s(pos2(visible.max.x, y))], stroke);
                y += g;
            }
        }

        let hits = self.filter_hits();
        let here = self.continents.get(self.cur).map_or(Pos2::ZERO, |c| c.center);
        for (qi, c) in self.continents.iter().enumerate().filter(|_| self.scope.is_none()) {
            let off = c.center - here;
            let (dc, radius) = self.disc(qi);
            let dl = (dc - here).to_pos2();
            let disc = Rect::from_center_size(dl, Vec2::splat(radius * 2.0));
            if !disc.intersects(visible) || (!self.universe() && qi != self.cur) {
                continue;
            }
            let cs = w2s(dl);
            let current = qi == self.cur;
            if self.sel == Sel::Continent(qi) {
                painter.circle_stroke(cs, radius * z + 3.0, Stroke::new(3.0, Color32::WHITE));
            }
            painter.circle(cs, radius * z, Color32::from_rgb(0x1a, 0x18, 0x14).gamma_multiply(if current { 0.5 } else { 1.0 }), Stroke::new(2.0, if current { Color32::from_rgb(0x5a, 0x4a, 0x2a) } else { Color32::from_rgb(0x3a, 0x34, 0x28) }));
            if hits.as_ref().is_some_and(|h| !h.conts.contains(&qi)) && !current {
                continue;
            }
            if !current {
                if let Some(w) = &c.world {
                    for f in &w.facs {
                        let r = wr(f.rect.translate(off));
                        if r.width() < 1.5 || !r.intersects(rect) {
                            continue;
                        }
                        painter.rect_filled(r, 0.0, if f.kind == Kind::Code { Color32::from_rgb(0x24, 0x30, 0x2c) } else { Color32::from_rgb(0x3a, 0x32, 0x26) });
                        if r.width() >= 40.0 {
                            let font = FontId::proportional((r.width() * 0.09).clamp(10.0, 14.0));
                            if let Some(t) = fit_label(&painter, &f.title, &font, r.width() - 10.0) {
                                painter.text(r.left_center() + vec2(5.0, 0.0), Align2::LEFT_CENTER, t, font, DIM);
                            }
                        }
                    }
                }
            }
            // name above the disc, and the project's servitors around its rim
            let rs = radius * z;
            if rs > 25.0 && (!current || z < 0.08) {
                let px = (rs * 0.12).clamp(11.0, 34.0);
                painter.text(cs - vec2(0.0, rs + 8.0), Align2::CENTER_BOTTOM, &c.name, FontId::monospace(px), if current { AMBER } else { TEXT });
            }
            let mine: Vec<&Servo> = self.servos.values().filter(|s| (s.cwd == c.path || s.cwd.starts_with(&format!("{}/", c.path))) && s.last.elapsed().as_secs() < 600).collect();
            if !current || z < 0.08 {
                for (k, s) in mine.iter().enumerate() {
                    let a = k as f32 * 0.5 + t * 0.3;
                    let p = cs + vec2(a.cos(), a.sin()) * (rs + 10.0);
                    painter.circle_filled(p, 5.0, s.color);
                    if !s.pending.is_empty() {
                        painter.circle_stroke(p, 8.0 + (t * 4.0).sin().abs() * 3.0, Stroke::new(2.0, RED));
                    }
                }
            }
        }
        let gate = self.gate();
        // the gate only shows while a servitor is working outside the map
        if self.scope.is_none() && self.servos.values().any(|s| self.in_project(s) && gate.expand(50.0).contains(s.pos)) {
            let g = wr(gate);
            painter.rect_stroke(g, 10.0, Stroke::new(1.5, Color32::from_rgb(0x5a, 0x4a, 0x2a)), StrokeKind::Inside);
            if let Some(t) = fit_text(&painter, "outside this map", &FontId::proportional(12.0), g.width() - 16.0) {
                painter.text(g.min + vec2(8.0, 6.0), Align2::LEFT_TOP, t, FontId::proportional(12.0), DIM);
            }
        }

        let changed: HashMap<&str, &str> = self.git.status.iter().map(|(xy, f)| (f.as_str(), xy.as_str())).collect();
        let roof = 1.0 - ((z - 0.3) / 0.3).clamp(0.0, 1.0);
        let fac_lit = |w: &World, fi: usize| hits.as_ref().is_none_or(|h| h.fac_lit(w, fi));
        let sec_lit = |w: &World, fi: usize, si: usize| hits.as_ref().is_none_or(|h| h.sec_lit(w, fi, si));
        let trace_secs: Option<Vec<(usize, usize)>> = self.trace.and_then(|(f, s, i)| {
            let w = self.world.as_ref()?;
            Some(std::iter::once((f, s)).chain(w.facs[f].sections[s].items[i].uses.iter().copied()).collect())
        });
        let mut editor_target: Option<(String, Option<u32>, Rect, (usize, usize))> = None;
        let mut satellites_out: Vec<((usize, usize), Rect)> = vec![];
        let lane_hits: std::cell::RefCell<Vec<(Pos2, Pos2, Sel, Sel)>> = std::cell::RefCell::new(vec![]);
        let hit = |a: Pos2, b: Pos2, sa: Sel, sb: Sel| lane_hits.borrow_mut().push((a, b, sa, sb));
        let mut regions_out: Vec<(String, Rect)> = vec![];
        if let Some(w) = &self.world {
            if z > 1.5 && self.settings.editor_enabled && !self.editor_hold {
                for (fi, f) in w.facs.iter().enumerate().filter(|(_, f)| f.rect.intersects(visible)) {
                    for (si, s) in f.sections.iter().enumerate() {
                        let sr = wr(s.rect);
                        // editor once the file takes up most of the canvas
                        if sr.width() > (rect.width() * 0.55).min(420.0) && sr.height() > (rect.height() * 0.35).min(260.0) && sr.contains(rect.center()) {
                            let area = if self.editor_full {
                                Rect::from_min_max(rect.min + vec2(0.0, EDITOR_BAR), rect.max)
                            } else {
                                // inside the machine, under its header and metrics line
                                let head_h = (16.0 * z).min(24.0);
                                Rect::from_min_max(sr.min + vec2(6.0, head_h + 18.0), sr.max - vec2(6.0, 6.0)).intersect(rect)
                            };
                            let (path, line) = match &s.path {
                                Some(p) => (p.clone(), (s.line > 0).then_some(s.line)),
                                None => (f.path.clone(), std::fs::read_to_string(&f.path).ok().and_then(|t| t.lines().position(|l| l == format!("## {}", s.title)).map(|i| i as u32 + 1))),
                            };
                            if !self.popped.iter().any(|n| n.path == path) && !path.starts_with("jira:") && !path.starts_with("otc:") && !path.starts_with("data:") {
                                editor_target = Some((path, line, area, (fi, si)));
                            }
                        }
                    }
                }
            }
            if z < 0.5 && self.scope.is_none() {
                let cells = w.facs[..w.n_cells].iter().fold(Rect::NOTHING, |a, f| a.union(f.rect));
                let code = w.facs[w.n_cells..].iter().fold(Rect::NOTHING, |a, f| a.union(f.rect));
                for (r, l) in [(cells, "KNOWLEDGE · context cells".to_string()), (code, format!("CODE · {}", w.repo.rsplit('/').next().unwrap_or("")))] {
                    if r.is_positive() {
                        txt(&painter, r.min - vec2(0.0, 120.0), &l, 60.0, Color32::from_rgb(0x5a, 0x4a, 0x2a), 3000.0);
                    }
                }
            }
            // districts: one tinted region per top-level folder, under everything else
            let mut district_rects: Vec<(usize, Rect)> = vec![];
            if w.districts.len() > 1 && self.scope.is_none() {
                for d in 0..w.districts.len() {
                    let members = w.facs.iter().filter(|f| f.district == d).count();
                    let bb = w.facs.iter().filter(|f| f.district == d).fold(Rect::NOTHING, |a, f| a.union(f.rect));
                    if members < 2 || !bb.is_positive() || !bb.intersects(visible) {
                        continue;
                    }
                    let r = wr(bb.expand(140.0));
                    let tint = district_color(d);
                    painter.rect_filled(r, (40.0 * z).clamp(8.0, 40.0), tint.gamma_multiply(0.06));
                    painter.rect_stroke(r, (40.0 * z).clamp(8.0, 40.0), Stroke::new(1.0, tint.gamma_multiply(0.25)), StrokeKind::Inside);
                    if self.sel == Sel::District(d) {
                        painter.rect_stroke(r, (40.0 * z).clamp(8.0, 40.0), Stroke::new(2.5, Color32::WHITE), StrokeKind::Outside);
                    }
                    district_rects.push((d, r));
                }
            }
            // sub-districts inside districts that are big enough on screen to be read in detail
            let mut sub_rects: Vec<(usize, Rect)> = vec![];
            for (k, (d, _)) in w.subdistricts.iter().enumerate().filter(|_| self.scope.is_none()) {
                if self.sel != Sel::Sub(k) && !district_rects.iter().any(|(dd, r)| dd == d && r.width() >= 700.0) {
                    continue;
                }
                let members = w.facs.iter().filter(|f| f.sub == k).count();
                let bb = w.facs.iter().filter(|f| f.sub == k).fold(Rect::NOTHING, |a, f| a.union(f.rect));
                if members < 2 || !bb.is_positive() || !bb.intersects(visible) {
                    continue;
                }
                let r = wr(bb.expand(70.0));
                let tint = district_color(*d);
                painter.rect_filled(r, (24.0 * z).clamp(6.0, 24.0), tint.gamma_multiply(0.05));
                painter.rect_stroke(r, (24.0 * z).clamp(6.0, 24.0), Stroke::new(1.0, tint.gamma_multiply(0.18)), StrokeKind::Inside);
                if self.sel == Sel::Sub(k) {
                    painter.rect_stroke(r, (24.0 * z).clamp(6.0, 24.0), Stroke::new(2.5, Color32::WHITE), StrokeKind::Outside);
                }
                sub_rects.push((k, r));
            }
            // connections: what, at which level, how strong, and whether only around the selection
            let st = &self.settings;
            let dot = |p: Pos2, c: Color32| painter.circle_filled(w2s(p), (4.0 * z).clamp(1.5, 5.0), c);
            let focus: Option<usize> = match &self.sel {
                Sel::Fac(f) | Sel::Sec(f, _) | Sel::Item(f, _, _) => Some(*f),
                _ => None,
            };
            let around: HashSet<usize> = match self.sel {
                Sel::District(d) => (0..w.facs.len()).filter(|&i| w.facs[i].district == d).collect(),
                Sel::Sub(k) => (0..w.facs.len()).filter(|&i| w.facs[i].sub == k).collect(),
                _ => focus.into_iter().collect(),
            };
            let scoped = self.scope.is_some();
            let shown = |a: usize, b: usize, weight: u32| !scoped && weight >= st.conn_min_weight && (!st.conn_selected_only || around.is_empty() || around.contains(&a) || around.contains(&b));
            let belt = |pa: Pos2, pb: Pos2, weight: u32, lit: bool, base: Color32, flow: Color32| {
                let width = ((4.0 + (weight as f32 + 1.0).log2() * 4.0) * z).clamp(1.0, 14.0);
                painter.line_segment([w2s(pa), w2s(pb)], Stroke::new(width, if lit { base.gamma_multiply(1.8) } else { base }));
                let len = pa.distance(pb);
                for i in 0..(len / 160.0).min(200.0) as usize {
                    let d = (i as f32 * 160.0 + t * 50.0) % len;
                    dot(pa + (pb - pa).normalized() * d, flow.gamma_multiply(if lit { 0.9 } else { 0.3 }));
                }
            };
            if st.conn_cell_refs {
                for &(a, b) in &w.belts {
                    let (pa, pb) = (w.facs[a].rect.center(), w.facs[b].rect.center());
                    if shown(a, b, u32::MAX) {
                        belt(pa, pb, 2, false, Color32::from_rgb(0x2a, 0x26, 0x1f), DIM);
                        hit(w2s(pa), w2s(pb), Sel::Fac(a), Sel::Fac(b));
                    }
                }
            }
            // imports and calls, aggregated to the chosen level
            let calls_between = |level: u8| -> HashMap<(usize, usize), u32> {
                let mut m = HashMap::new();
                for (fi, f) in w.facs.iter().enumerate() {
                    for s in &f.sections {
                        for &((tf, _), c) in &s.calls_out {
                            let key = if level == 2 { (f.district, w.facs[tf].district) } else { (fi, tf) };
                            if key.0 != key.1 {
                                *m.entry(key).or_default() += c;
                            }
                        }
                    }
                }
                m
            };
            match st.conn_level {
                _ if scoped => {}
                2 => {
                    let mut imports: HashMap<(usize, usize), u32> = HashMap::new();
                    for &(a, b, c) in &w.code_belts {
                        let (da, db) = (w.facs[a].district, w.facs[b].district);
                        if da != db {
                            *imports.entry((da.min(db), da.max(db))).or_default() += c;
                        }
                    }
                    let centre = |d: usize| w.region(false, d).0.center();
                    let lit_d = match self.sel { Sel::District(d) => Some(d), _ => focus.map(|f| w.facs[f].district) };
                    for ((da, db), c) in imports.into_iter().filter(|(_, c)| *c >= st.conn_min_weight) {
                        let lit = lit_d == Some(da) || lit_d == Some(db);
                        if !st.conn_selected_only || lit_d.is_none() || lit {
                            belt(centre(da), centre(db), c, lit, Color32::from_rgb(0x1f, 0x30, 0x2c), TEAL);
                            hit(w2s(centre(da)), w2s(centre(db)), Sel::District(da), Sel::District(db));
                            painter.text(w2s(centre(da) + (centre(db) - centre(da)) * 0.5), Align2::CENTER_CENTER, format!("{c} imports"), FontId::proportional(12.0), TEAL.gamma_multiply(if lit { 1.0 } else { 0.6 }));
                        }
                    }
                    if st.conn_calls {
                        for ((da, db), c) in calls_between(2).into_iter().filter(|(_, c)| *c >= st.conn_min_weight) {
                            painter.line_segment([w2s(centre(da)), w2s(centre(db))], Stroke::new(2.0, PURPLE.gamma_multiply(0.5)));
                            let _ = c;
                        }
                    }
                }
                1 => {
                    if st.show_import_belts {
                        for &(a, b, c) in w.code_belts.iter().filter(|e| shown(e.0, e.1, e.2)) {
                            let lit = focus == Some(a) || focus == Some(b) || (around.contains(&a) != around.contains(&b));
                            // `a` imports `b`: dots flow from upstream (b) to downstream (a)
                            belt(w.facs[b].rect.center(), w.facs[a].rect.center(), c, lit, Color32::from_rgb(0x1f, 0x30, 0x2c), TEAL);
                            hit(w2s(w.facs[b].rect.center()), w2s(w.facs[a].rect.center()), Sel::Fac(b), Sel::Fac(a));
                        }
                    }
                    if st.conn_calls {
                        for ((a, b), c) in calls_between(1).into_iter().filter(|((a, b), c)| shown(*a, *b, *c)) {
                            let (pa, pb) = (w.facs[a].rect.center(), w.facs[b].rect.center());
                            for sh in egui::Shape::dashed_line(&[w2s(pa), w2s(pb)], Stroke::new(((c as f32).sqrt() * z * 2.0).clamp(1.0, 5.0), PURPLE.gamma_multiply(0.55)), 10.0, 6.0) {
                                painter.add(sh);
                            }
                            hit(w2s(pa), w2s(pb), Sel::Fac(a), Sel::Fac(b));
                        }
                    }
                }
                _ => {
                    // file level: every import (and call) between files of different folders that are on screen
                    let mut drawn = 0;
                    for (fi, f) in w.facs.iter().enumerate() {
                        for s in f.sections.iter().filter(|s| s.rect.intersects(visible)) {
                            for &(df, ds) in s.deps.iter().filter(|d| d.0 != fi) {
                                if drawn > 4000 || !shown(fi, df, 1) {
                                    continue;
                                }
                                drawn += 1;
                                painter.line_segment([w2s(s.rect.center()), w2s(w.facs[df].sections[ds].rect.center())], Stroke::new(1.0, TEAL.gamma_multiply(0.3)));
                                if let Some(si) = f.sections.iter().position(|x| std::ptr::eq(x, s)) {
                                    hit(w2s(s.rect.center()), w2s(w.facs[df].sections[ds].rect.center()), Sel::Sec(fi, si), Sel::Sec(df, ds));
                                }
                            }
                            if st.conn_calls {
                                for &((df, ds), c) in s.calls_out.iter().filter(|d| d.0 .0 != fi && d.1 >= st.conn_min_weight) {
                                    painter.line_segment([w2s(s.rect.center()), w2s(w.facs[df].sections[ds].rect.center())], Stroke::new(1.0, PURPLE.gamma_multiply(0.4)));
                                    let _ = c;
                                }
                            }
                        }
                    }
                }
            }
            for &(cf, kf, c) in w.pipes.iter().filter(|e| st.show_context_pipes && shown(e.0, e.1, e.2)) {
                let lit = focus == Some(cf) || focus == Some(kf);
                let (pa, pb) = (w.facs[cf].rect.center(), w.facs[kf].rect.center());
                painter.line_segment([w2s(pa), w2s(pb)], Stroke::new(((2.0 + c as f32) * z).clamp(1.0, 6.0), AMBER.gamma_multiply(if lit { 0.55 } else { 0.06 })));
                hit(w2s(pa), w2s(pb), Sel::Fac(cf), Sel::Fac(kf));
                if lit {
                    painter.text(w2s(pa + (pb - pa) * 0.5), Align2::CENTER_CENTER, format!("{c} links"), FontId::proportional(12.0), AMBER);
                }
            }
            // folder-view boxes and lanes go here, underneath the machines
            let scope_slot = painter.add(egui::Shape::Noop);
            let mut deferred: Vec<(usize, usize, f32)> = vec![];
            let mut roof_labels: Vec<(f32, usize, Rect, String, f32)> = vec![];
            for (fi, f) in w.facs.iter().enumerate() {
                if !f.rect.intersects(visible) || self.scope.as_ref().is_some_and(|sc| !scope_has(w, sc, fi)) {
                    continue;
                }
                let code = f.kind == Kind::Code;
                let outside = match self.sel {
                    Sel::District(d) => f.district != d,
                    Sel::Sub(k) => f.sub != k,
                    _ => false,
                };
                let (adds, dels, dirty_files) = f.sections.iter().filter_map(|s| s.rel.as_deref()).filter(|r| changed.contains_key(r)).fold((0, 0, 0), |(a, d, n), r| {
                    let (x, y) = self.git.numstat.get(r).copied().unwrap_or((0, 0));
                    (a + x, d + y, n + 1)
                });
                let painter = {
                    let mut p = painter.clone();
                    p.set_opacity(if outside { 0.3 } else { 1.0 } * if self.settings.git_overlay && code && dirty_files == 0 { 0.4 } else { 1.0 } * if fac_lit(w, fi) { 1.0 } else { 0.18 });
                    p
                };
                let r = wr(f.rect);
                let rad = (18.0 * z).clamp(3.0, 18.0);
                let edge = if code { Color32::from_rgb(0x2f, 0x6e, 0x64) } else { Color32::from_rgb(0x7a, 0x66, 0x40) };
                painter.rect_filled(r.translate(vec2(0.0, 4.0)), rad, Color32::from_black_alpha(90));
                painter.rect_filled(r, rad, if code { Color32::from_rgb(0x1b, 0x23, 0x21) } else { Color32::from_rgb(0x23, 0x20, 0x1a) });
                if f.building {
                    dashed_outline(&painter, &[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()], AMBER, 2.0);
                } else {
                    painter.rect_stroke(r, rad, Stroke::new(1.5, edge.gamma_multiply(0.8)), StrokeKind::Inside);
                }
                if roof < 1.0 {
                    let a = 1.0 - roof;
                    let label = if code { format!("{}/", f.title) } else { format!("{}  ·  {}", f.title, f.id) };
                    let lfont = FontId::proportional(14.0);
                    let lx = if code { rad.max(10.0) + 6.0 } else { rad.max(8.0) + 150.0 };
                    if let Some(t) = fit_text(&painter, &label, &lfont, r.width() - lx - 20.0) {
                        painter.text(r.min + vec2(lx, rad.max(8.0) + 6.0), Align2::LEFT_CENTER, t, lfont, edge.gamma_multiply(a));
                    }
                    if code {
                        for s in &f.sections {
                            for d in s.deps.iter().filter(|d| d.0 == fi) {
                                painter.line_segment([w2s(s.rect.center()), w2s(f.sections[d.1].rect.center())], Stroke::new(1.5, TEAL.gamma_multiply(0.3 * a)));
                            }
                        }
                    } else {
                        let line: Vec<Pos2> = f.sections.iter().map(|s| w2s(s.rect.center())).collect();
                        painter.add(egui::Shape::line(line, Stroke::new(2.0, Color32::from_rgb(0x4a, 0x42, 0x32).gamma_multiply(a))));
                    }
                    for (si, s) in f.sections.iter().enumerate() {
                        if !s.rect.intersects(visible) {
                            continue;
                        }
                        let a = a * if trace_secs.as_ref().is_some_and(|ts| !ts.contains(&(fi, si))) { 0.25 } else { 1.0 } * if outside { 0.3 } else { 1.0 };
                        deferred.push((fi, si, a)); // drawn after the relation lines, so lines pass underneath
                    }
                }
                if roof > 0.0 {
                    let inner = wr(f.rect.shrink(6.0));
                    painter.rect_filled(inner, (rad - 2.0).max(2.0), if code { Color32::from_rgb(0x22, 0x2d, 0x29) } else { Color32::from_rgb(0x30, 0x2a, 0x21) }.gamma_multiply(roof));
                    let sw = (16.0 * z).clamp(3.0, 14.0);
                    for (j, s) in f.sections.iter().take(((inner.width() - 20.0) / (sw + 4.0)).max(0.0) as usize).enumerate() {
                        let c = s.metrics.as_ref().map_or(tag_color(s.tag), |m| cx_color(m.cx_max));
                        painter.rect_filled(Rect::from_min_size(pos2(inner.min.x + 10.0 + j as f32 * (sw + 4.0), inner.max.y - sw - 10.0), Vec2::splat(sw)), sw * 0.3, c.gamma_multiply(roof));
                    }
                    let hot = f.sections.iter().map(|s| s.heat).fold(0.0, f32::max);
                    if hot > 0.0 {
                        painter.rect_stroke(r.expand(4.0), rad + 4.0, Stroke::new(4.0, AMBER.gamma_multiply(hot)), StrokeKind::Outside);
                    }
                    if self.settings.git_overlay && dirty_files > 0 {
                        painter.rect_stroke(r, rad, Stroke::new(2.5, AMBER.gamma_multiply(roof)), StrokeKind::Inside);
                        let badge = format!("{dirty_files} changed  +{adds} −{dels}");
                        let g = painter.layout_no_wrap(badge, FontId::proportional(11.5), TEXT.gamma_multiply(roof));
                        let br = Rect::from_min_size(inner.right_top() + vec2(-g.size().x - 14.0, 6.0), g.size() + vec2(8.0, 4.0));
                        if br.width() < inner.width() {
                            painter.rect_filled(br, 4.0, Color32::from_rgb(0x4a, 0x3a, 0x1a).gamma_multiply(roof));
                            painter.galley(br.min + vec2(4.0, 2.0), g, TEXT.gamma_multiply(roof));
                        }
                    }
                    let dirty = f.sections.iter().filter(|s| s.rel.as_deref().is_some_and(|r| changed.contains_key(r))).count();
                    let sub = if code {
                        let loc: u32 = f.sections.iter().filter_map(|s| s.metrics.as_ref()).map(|m| m.loc).sum();
                        let hot = f.sections.iter().flat_map(|s| &s.items).filter(|i| i.cx > 10).count();
                        let mut sub = format!("{} files · {loc} loc", f.sections.len());
                        if hot > 0 {
                            sub += &format!(" · {hot} hotspots");
                        }
                        if dirty > 0 {
                            sub += &format!(" · {dirty} changed");
                        }
                        sub
                    } else {
                        f.id.clone()
                    };
                    roof_labels.push((inner.area(), fi, inner, sub, roof));
                }
                if !code {
                    let stale = f.age > self.settings.stale_after_days;
                    let c = if !stale { Color32::from_rgb(0x5a, 0xc0, 0x5a) } else if (t * 4.0).sin() > 0.0 { RED } else { Color32::from_rgb(0x60, 0x24, 0x1c) };
                    let dot_r = (8.0 * z).clamp(2.5, 6.0);
                    let at = r.min + vec2(rad.max(8.0) + dot_r, rad.max(8.0) + dot_r);
                    painter.circle_filled(at, dot_r, c);
                    let label = format!("{}{}", f.status.as_deref().unwrap_or("no status"), if stale { " · stale" } else { "" });
                    if dot_r >= 4.0 {
                        painter.text(at + vec2(dot_r + 6.0, 0.0), Align2::LEFT_CENTER, label, FontId::proportional(11.0), if stale { RED } else { DIM });
                    }
                }
            }
            if let Some(sc) = self.scope.clone() {
                let members = scope_members(w, &sc);
                // folder boxes, one per next subfolder
                let mut regions = vec![];
                let under: std::cell::RefCell<Vec<egui::Shape>> = std::cell::RefCell::new(vec![]);
                for (path, label, group) in scope_groups(w, &sc, &members) {
                    let Some(path) = path else { continue };
                    let bb = group.iter().fold(Rect::NOTHING, |a, &f| a.union(w.facs[f].rect)).expand(70.0);
                    let r = wr(bb);
                    let selected = self.sel == Sel::Dir(path.clone());
                    under.borrow_mut().push(egui::Shape::rect_filled(r, 14.0, TEAL.gamma_multiply(0.05)));
                    under.borrow_mut().push(egui::Shape::rect_stroke(r, 14.0, Stroke::new(if selected { 2.5 } else { 1.0 }, if selected { Color32::WHITE } else { TEAL.gamma_multiply(0.3) }), StrokeKind::Inside));
                    let text = format!("{label}/  ·  {} folder{}", group.len(), if group.len() == 1 { "" } else { "s" });
                    let font = FontId::proportional((r.width() * 0.05).clamp(12.0, 17.0));
                    if let Some(t) = fit_label(&painter, &text, &font, r.width() - 24.0) {
                        painter.text(r.min + vec2(14.0, 10.0), Align2::LEFT_TOP, t, font, TEAL.gamma_multiply(0.9));
                    }
                    regions.push((path, bb));
                }
                regions_out = regions;
                let st = &self.settings;
                // lanes: a faint band with dots flowing from upstream (what is used) to downstream (who uses it)
                let lane = |from: Pos2, to: Pos2, weight: u32, c: Color32| {
                    let width = ((3.0 + (weight as f32 + 1.0).log2() * 3.0) * z).clamp(2.0, 12.0);
                    under.borrow_mut().push(egui::Shape::line_segment([w2s(from), w2s(to)], Stroke::new(width, c.gamma_multiply(0.13))));
                    let len = from.distance(to);
                    if len < 1.0 {
                        return;
                    }
                    let dir = (to - from) / len;
                    for i in 0..(len / 140.0).min(120.0) as usize {
                        let d = (i as f32 * 140.0 + t * 55.0) % len;
                        under.borrow_mut().push(egui::Shape::circle_filled(w2s(from + dir * d), (3.5 * z).clamp(1.5, 4.0), c.gamma_multiply(0.65)));
                    }
                };
                // between folders of the view
                if st.show_import_belts {
                    for &(a, b, c) in w.code_belts.iter().filter(|e| e.2 >= st.conn_min_weight && members.contains(&e.0) && members.contains(&e.1)) {
                        let (ra, rb) = (w.facs[a].rect, w.facs[b].rect);
                        lane(edge_point(rb, ra.center()), edge_point(ra, rb.center()), c, TEAL);
                        hit(w2s(edge_point(rb, ra.center())), w2s(edge_point(ra, rb.center())), Sel::Fac(b), Sel::Fac(a));
                    }
                }
                // the outside world on a ring: folders (with link counts) or single files, per the View panel
                let by_folder = st.conn_level != 0;
                let (ring, links) = scope_ring(w, &members, !by_folder, [st.show_import_belts, st.conn_calls, st.show_context_pipes], st.conn_min_weight);
                // inflowing: what the view imports, and context cells; outflowing: who imports or calls it
                let inflow = |c: Color32| c == TEAL || c == AMBER;
                let mut agg: HashMap<((usize, usize), (usize, usize), [u8; 4]), u32> = HashMap::new();
                for (ext, (fi, si), c) in &links {
                    let inner_key = if by_folder { (*fi, usize::MAX) } else { (*fi, *si) };
                    *agg.entry((*ext, inner_key, c.to_array())).or_default() += 1;
                }
                for ((ext, (fi, si), c), n) in agg {
                    let Some((_, sr, _)) = ring.iter().find(|x| x.0 == ext) else { continue };
                    let ir = if si == usize::MAX { w.facs[fi].rect } else { w.facs[fi].sections[si].rect };
                    let col = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);
                    let (outer, inner) = (edge_point(*sr, ir.center()), edge_point(ir, sr.center()));
                    let ext_sel = if ext.1 == usize::MAX { Sel::Fac(ext.0) } else { Sel::Sec(ext.0, ext.1) };
                    let inner_sel = if si == usize::MAX { Sel::Fac(fi) } else { Sel::Sec(fi, si) };
                    hit(w2s(outer), w2s(inner), ext_sel, inner_sel);
                    if inflow(col) {
                        lane(outer, inner, n, col);
                    } else {
                        lane(inner, outer, n, col);
                    }
                }
                for (ext, r, n) in &ring {
                    let selected = if ext.1 == usize::MAX { self.sel == Sel::Fac(ext.0) } else { self.sel == Sel::Sec(ext.0, ext.1) };
                    if ext.1 == usize::MAX {
                        draw_ring_folder(&painter, &w.facs[ext.0], *n, wr(*r), selected);
                    } else {
                        draw_satellite(&painter, &w.facs[ext.0], &w.facs[ext.0].sections[ext.1], wr(*r), selected);
                    }
                }
                painter.set(scope_slot, egui::Shape::Vec(under.take()));
                satellites_out = ring.into_iter().map(|(e, r, _)| (e, r)).collect();
            }
            let mut placed: Vec<Rect> = vec![];
            let mut aggregated: HashSet<usize> = HashSet::new();
            district_rects.sort_by(|a, b| b.1.area().total_cmp(&a.1.area()));
            for &(d, r) in &district_rects {
                let tint = district_color(d);
                let facs: Vec<&world::Fac> = w.facs.iter().filter(|f| f.district == d).collect();
                let files: usize = facs.iter().map(|f| f.sections.len()).sum();
                let loc: u32 = facs.iter().flat_map(|f| &f.sections).filter_map(|s| s.metrics.as_ref()).map(|m| m.loc).sum();
                let stats = if w.n_cells > 0 && d == 0 { format!("{} cells", facs.len()) } else { format!("{} folders · {files} files · {loc} loc", facs.len()) };
                if r.width() < 700.0 {
                    // far away: the district speaks for its factories
                    aggregated.insert(d);
                    let px = (r.width() * 0.08).clamp(13.0, 30.0);
                    let font = FontId::proportional(px);
                    if let Some(t) = fit_label(&painter, &w.districts[d], &font, r.width() - 20.0) {
                        let g = painter.layout_no_wrap(t, font, tint.gamma_multiply(1.4));
                        let at = r.center() - vec2(g.size().x * 0.5, g.size().y);
                        let lr = Rect::from_min_size(at, g.size());
                        if placed.iter().any(|p| p.intersects(lr.expand(6.0))) {
                            continue;
                        }
                        painter.rect_filled(lr.expand2(vec2(8.0, 3.0)), 6.0, BG.gamma_multiply(0.7));
                        painter.galley(at, g, tint.gamma_multiply(1.4));
                        placed.push(lr);
                        let sfont = FontId::proportional((px * 0.5).max(11.0));
                        if let Some(t) = fit_text(&painter, &stats, &sfont, r.width() - 20.0) {
                            let g = painter.layout_no_wrap(t, sfont, DIM);
                            let at = r.center() + vec2(-g.size().x * 0.5, 4.0);
                            placed.push(Rect::from_min_size(at, g.size()));
                            painter.galley(at, g, DIM);
                        }
                    }
                } else {
                    // close up: a section header in the corner
                    let font = FontId::proportional(15.0);
                    let label = format!("{}  ·  {stats}", w.districts[d]);
                    if let Some(t) = fit_text(&painter, &label, &font, r.width() - 40.0) {
                        let at = r.min + vec2(20.0, 14.0);
                        let g = painter.layout_no_wrap(t, font, tint.gamma_multiply(1.3));
                        placed.push(Rect::from_min_size(at, g.size()));
                        painter.galley(at, g, tint.gamma_multiply(1.3));
                    }
                }
            }
            let mut aggregated_subs: HashSet<usize> = HashSet::new();
            sub_rects.sort_by(|a, b| b.1.area().total_cmp(&a.1.area()));
            for &(k, r) in &sub_rects {
                let tint = district_color(w.subdistricts[k].0).gamma_multiply(1.2);
                let name = &w.subdistricts[k].1;
                if r.width() < 480.0 {
                    aggregated_subs.insert(k);
                    let font = FontId::proportional((r.width() * 0.08).clamp(12.0, 20.0));
                    let Some(t) = fit_label(&painter, name, &font, r.width() - 16.0) else { continue };
                    let g = painter.layout_no_wrap(t, font, tint);
                    let at = r.center() - g.size() * 0.5;
                    let lr = Rect::from_min_size(at, g.size());
                    if placed.iter().any(|p| p.intersects(lr.expand(4.0))) {
                        continue;
                    }
                    painter.rect_filled(lr.expand2(vec2(6.0, 2.0)), 5.0, BG.gamma_multiply(0.7));
                    painter.galley(at, g, tint);
                    placed.push(lr);
                } else {
                    let font = FontId::proportional(13.0);
                    if let Some(t) = fit_label(&painter, name, &font, r.width() - 30.0) {
                        let at = r.min + vec2(14.0, 10.0);
                        let g = painter.layout_no_wrap(t, font, tint);
                        let lr = Rect::from_min_size(at, g.size());
                        if !placed.iter().any(|p| p.intersects(lr)) {
                            placed.push(lr);
                            painter.galley(at, g, tint);
                        }
                    }
                }
            }
            let rel_slot = painter.add(egui::Shape::Noop);
            for &(fi, si, a) in &deferred {
                let s = &w.facs[fi].sections[si];
                let a = if sec_lit(w, fi, si) { a } else { a * 0.2 };
                let traced = self.trace.filter(|t| (t.0, t.1) == (fi, si)).map(|t| t.2);
                let diff = s.rel.as_deref().and_then(|r| self.git.numstat.get(r)).copied();
                draw_machine(&painter, s, wr(s.rect), a, changed.get(s.rel.as_deref().unwrap_or("")).copied(), diff, self.settings.git_overlay, traced, &wr);
            }
            // folder titles go on top of the machines, so they stay readable while zooming in
            roof_labels.sort_by(|a, b| b.0.total_cmp(&a.0));
            let obstacles: Vec<Rect> = roof_labels.iter().map(|l| l.2.expand(6.0)).collect();
            let mut beside: Vec<(usize, Rect, f32)> = vec![];
            for (_, fi, inner, sub, roof) in roof_labels {
                let f = &w.facs[fi];
                if aggregated.contains(&f.district) || aggregated_subs.contains(&f.sub) || !fac_lit(w, fi) {
                    continue;
                }
                if inner.width() < 60.0 {
                    beside.push((fi, inner, roof));
                    continue;
                }
                let tpx = (inner.width() * 0.09).clamp(11.0, 18.0);
                let font = FontId::proportional(tpx);
                let name = if w.districts.len() > 1 { &f.short } else { &f.title };
                let Some(t) = fit_label(&painter, name, &font, inner.width() - 16.0) else { continue };
                let two_lines = inner.height() > tpx * 3.2;
                // full strength until the inside is half faded in, then gone; on a plate while the inside shows
                let la = ((roof - 0.1) / 0.4).clamp(0.0, 1.0);
                if la <= 0.0 {
                    continue;
                }
                let g = painter.layout_no_wrap(t, font, TEXT.gamma_multiply(la));
                let at = inner.left_center() + vec2(8.0, if two_lines { -tpx * 0.6 } else { 0.0 } - g.size().y * 0.5);
                let lr = Rect::from_min_size(at, g.size());
                if placed.iter().any(|p| p.intersects(lr)) {
                    continue;
                }
                placed.push(lr);
                let sfont = FontId::proportional((tpx * 0.75).max(10.0));
                let sub_g = two_lines.then(|| fit_text(&painter, &sub, &sfont, inner.width() - 16.0)).flatten().map(|t| painter.layout_no_wrap(t, sfont, DIM.gamma_multiply(la)));
                if roof < 1.0 {
                    let plate = sub_g.as_ref().map_or(lr, |sg| lr.union(Rect::from_min_size(pos2(at.x, inner.center().y + tpx * 0.6 - sg.size().y * 0.5), sg.size())));
                    painter.rect_filled(plate.expand2(vec2(6.0, 3.0)), 5.0, BG.gamma_multiply(0.85 * la));
                }
                painter.galley(at, g, TEXT.gamma_multiply(la));
                if let Some(sg) = sub_g {
                    let y = inner.center().y + tpx * 0.6 - sg.size().y * 0.5;
                    painter.galley(pos2(at.x, y), sg, DIM.gamma_multiply(la));
                }
            }
            // map-style point labels for factories too small to hold their name: right of the box if free, else left
            for (fi, inner, roof) in beside {
                let f = &w.facs[fi];
                if inner.width() < 8.0 {
                    continue;
                }
                let name = if w.districts.len() > 1 { &f.short } else { &f.title };
                let font = FontId::proportional(11.5);
                let Some(t) = fit_label(&painter, name, &font, 170.0) else { continue };
                let g = painter.layout_no_wrap(t, font, TEXT.gamma_multiply(0.85 * roof));
                let size = g.size();
                let candidates = [inner.right_center() + vec2(8.0, -size.y * 0.5), inner.left_center() - vec2(8.0 + size.x, size.y * 0.5), inner.center_bottom() + vec2(-size.x * 0.5, 4.0)];
                let free = candidates.iter().map(|at| Rect::from_min_size(*at, size)).find(|lr| {
                    !placed.iter().any(|p| p.intersects(*lr)) && !obstacles.iter().any(|o| o.intersects(*lr) && !o.contains_rect(inner))
                });
                if let Some(lr) = free {
                    painter.rect_filled(lr.expand2(vec2(4.0, 1.0)), 3.0, BG.gamma_multiply(0.75));
                    painter.galley(lr.min, g, TEXT.gamma_multiply(0.85 * roof));
                    placed.push(lr);
                }
            }

            let rel_shapes = std::cell::RefCell::new(Vec::new());
            let target = |(tf, ts): (usize, Option<usize>)| ts.map_or(w.facs[tf].rect, |ts| w.facs[tf].sections[ts].rect);
            let relate = |from: Rect, to: Rect, c: Color32, width: f32, label: Option<String>, to_sel: Sel| {
                // edge to edge and underneath the machines; outlines and labels on top
                let (a, b) = (edge_point(from, to.center()), edge_point(to, from.center()));
                hit(w2s(a), w2s(b), self.sel.clone(), to_sel);
                rel_shapes.borrow_mut().push(egui::Shape::line_segment([w2s(a), w2s(b)], Stroke::new(width, c)));
                painter.rect_stroke(wr(to), 8.0, Stroke::new(2.0, c), StrokeKind::Outside);
                if let Some(l) = label {
                    let at = w2s(b);
                    let g = painter.layout_no_wrap(l, FontId::proportional(11.0), c);
                    let bg = Rect::from_center_size(at, g.size() + vec2(10.0, 4.0));
                    painter.rect_filled(bg, 4.0, BG.gamma_multiply(0.9));
                    painter.galley(bg.min + vec2(5.0, 2.0), g, c);
                }
            };
            match if self.scope.is_some() { Sel::None } else { self.sel.clone() } {
                Sel::Sec(f, s) => {
                    let sec = &w.facs[f].sections[s];
                    for &d in &sec.deps {
                        relate(sec.rect, w.facs[d.0].sections[d.1].rect, TEAL, 2.0, None, Sel::Sec(d.0, d.1));
                    }
                    for &d in &sec.dependents {
                        relate(sec.rect, w.facs[d.0].sections[d.1].rect, ORANGE, 2.0, None, Sel::Sec(d.0, d.1));
                    }
                    for &(d, c) in &sec.calls_out {
                        relate(sec.rect, w.facs[d.0].sections[d.1].rect, PURPLE, 1.0 + (c as f32).sqrt(), Some(format!("{c} calls")), Sel::Sec(d.0, d.1));
                    }
                    for &(d, c) in &sec.calls_in {
                        relate(sec.rect, w.facs[d.0].sections[d.1].rect, PURPLE.gamma_multiply(0.6), 1.0 + (c as f32).sqrt(), Some(format!("{c} calls in")), Sel::Sec(d.0, d.1));
                    }
                    for l in &sec.links {
                        relate(sec.rect, target(l.to), AMBER, 1.5, None, l.to.1.map_or(Sel::Fac(l.to.0), |x| Sel::Sec(l.to.0, x)));
                    }
                    painter.rect_stroke(wr(sec.rect), 8.0, Stroke::new(2.5, Color32::WHITE), StrokeKind::Outside);

                }
                Sel::Fac(f) => {
                    for l in &w.facs[f].links {
                        relate(w.facs[f].rect, target(l.to), AMBER, 1.5, None, l.to.1.map_or(Sel::Fac(l.to.0), |x| Sel::Sec(l.to.0, x)));
                    }
                }
                _ => {}
            }
            painter.set(rel_slot, egui::Shape::Vec(rel_shapes.take()));
            if let Some((f, s, i)) = self.trace {
                let it = &w.facs[f].sections[s].items[i];
                let it_rect = if it.rect.is_positive() { it.rect } else { w.facs[f].sections[s].rect };
                for &(uf, us) in &it.uses {
                    let m = w.facs[uf].sections[us].rect;
                    painter.line_segment([w2s(it_rect.center()), w2s(m.center())], Stroke::new(2.0, AMBER));
                    painter.rect_stroke(wr(m), 0.0, Stroke::new(2.0, AMBER), StrokeKind::Outside);
                }
            }
        } else {
            painter.text(rect.center(), Align2::CENTER_CENTER, if self.current.is_some() { "scanning…" } else { "pick a project" }, FontId::monospace(18.0), DIM);
        }
        let mut card_at = None;
        for (id, s) in &self.servos {
            if !self.in_project(s) {
                continue;
            }
            let a = if s.last.elapsed().as_secs() > 300 && s.pending.is_empty() { 0.35 } else { 1.0 };
            if let Some(p) = s.parent.as_ref().and_then(|p| self.servos.get(p)) {
                painter.line_segment([w2s(s.pos), w2s(p.pos)], Stroke::new(1.5, TEAL.gamma_multiply(0.3)));
            }
            let r = if s.parent.is_some() { (7.0 * z).max(5.0) } else { (11.0 * z).max(8.0) };
            let c = w2s(s.pos);
            let mut ang = t * if s.q.is_empty() { 2.0 } else { 8.0 } + s.pos.x * 0.01;
            // what it is doing right now, shown at its target
            if let Some(tg) = s.q.front() {
                let at_target = s.pos.distance(tg.pos) < 2.0;
                let target = tg.sec.and_then(|(f, si)| self.world.as_ref().and_then(|w| w.facs.get(f)?.sections.get(si)).map(|x| wr(x.rect))).unwrap_or(Rect::from_center_size(c, Vec2::splat(20.0)));
                if !at_target {
                    painter.line_segment([c, w2s(tg.pos)], Stroke::new(1.0, s.color.gamma_multiply(0.15)));
                } else {
                    match tool_kind(&tg.tool) {
                        ToolKind::Read => {
                            let pulse = 0.35 + 0.25 * (t * 6.0).sin().abs();
                            painter.add(egui::Shape::convex_polygon(vec![c, target.left_top(), target.right_top()], TEAL.gamma_multiply(0.08 * pulse * 3.0), Stroke::NONE));
                            painter.rect_stroke(target, 6.0, Stroke::new(1.5, TEAL.gamma_multiply(pulse)), StrokeKind::Outside);
                        }
                        ToolKind::Build => {
                            ang = -1.2 + (t * 14.0).sin() * 0.9; // hammering
                            for k in 0..7 {
                                let ph = t * 9.0 + k as f32 * 2.3;
                                let d = r * (1.6 + (ph * 1.7).sin().abs() * 1.4);
                                painter.circle_filled(c + vec2(ph.cos(), ph.sin()) * d, 1.8, AMBER.gamma_multiply(0.5 + 0.5 * (ph * 3.0).sin().abs()));
                            }
                        }
                        ToolKind::Search => {
                            for k in 0..2 {
                                let f = (t * 0.9 + k as f32 * 0.5).fract();
                                painter.circle_stroke(c, r + f * 60.0, Stroke::new(1.5, TEAL.gamma_multiply(1.0 - f)));
                            }
                        }
                        ToolKind::Shell => {
                            let box_r = Rect::from_min_size(c + vec2(r + 4.0, -r - 20.0), vec2(30.0, 16.0));
                            painter.rect_filled(box_r, 3.0, Color32::from_rgb(0x10, 0x10, 0x0e));
                            painter.rect_stroke(box_r, 3.0, Stroke::new(1.0, TEAL), StrokeKind::Inside);
                            let cursor = if (t * 3.0).sin() > 0.0 { "$_" } else { "$ " };
                            painter.text(box_r.center(), Align2::CENTER_CENTER, cursor, FontId::monospace(11.0), TEAL);
                        }
                        ToolKind::Web => {
                            let f = (t * 0.7).fract();
                            painter.circle_stroke(c, r + f * 40.0, Stroke::new(2.0, PURPLE.gamma_multiply(1.0 - f)));
                        }
                        ToolKind::Other => {}
                    }
                }
            }
            painter.circle_filled(c, r, s.color.gamma_multiply(a));
            if self.sel == Sel::Servo(id.clone()) {
                painter.circle_stroke(c, r + 2.0, Stroke::new(2.0, Color32::WHITE));
                card_at = Some(c);
            }
            painter.line_segment([c, c + vec2(ang.cos(), ang.sin()) * r * 1.8], Stroke::new(r * 0.35, s.color.gamma_multiply(a)));
            painter.text(c + vec2(r * 1.6, -r), Align2::LEFT_TOP, &s.name, FontId::monospace(11.0), s.color.gamma_multiply(a));
            painter.text(c + vec2(r * 1.6, -r + 13.0), Align2::LEFT_TOP, &s.task, FontId::monospace(11.0), Color32::from_rgb(0xc8, 0xb8, 0x90).gamma_multiply(a));
            if !s.pending.is_empty() {
                // blocked: a pulsing exclamation bubble, and an arrow on the screen edge when it is off screen
                let pulse = 1.0 + 0.15 * (t * 5.0).sin();
                painter.circle_stroke(c, r + 4.0 + (t * 4.0).sin().abs() * 5.0, Stroke::new(2.5, RED));
                let bubble = c + vec2(0.0, -r - 24.0);
                painter.circle_filled(bubble, 12.0 * pulse, RED);
                painter.circle_stroke(bubble, 12.0 * pulse, Stroke::new(2.0, Color32::WHITE));
                painter.text(bubble, Align2::CENTER_CENTER, "!", FontId::proportional(17.0), Color32::WHITE);
                painter.text(bubble + vec2(18.0, 0.0), Align2::LEFT_CENTER, "needs approval", FontId::proportional(12.0), RED);
                let inner = rect.shrink2(vec2(24.0, 40.0));
                if !inner.contains(c) {
                    let e = pos2(c.x.clamp(inner.min.x, inner.max.x), c.y.clamp(inner.min.y, inner.max.y));
                    let dir = (c - e).normalized();
                    let side = vec2(-dir.y, dir.x) * 9.0;
                    painter.add(egui::Shape::convex_polygon(vec![e + dir * 14.0, e + side, e - side], RED, Stroke::new(1.5, Color32::WHITE)));
                    painter.text(e - dir * 14.0, Align2::CENTER_CENTER, &s.name, FontId::proportional(11.0), RED);
                }
            }
        }
        self.card_anchor = card_at;

        if let Some((a, b)) = lane_highlight {
            painter.line_segment([a, b], Stroke::new(3.0, Color32::WHITE.gamma_multiply(0.8)));
            painter.circle_filled(a, 4.0, Color32::WHITE);
            painter.circle_filled(b, 4.0, Color32::WHITE);
        }
        if !self.log.is_empty() && self.settings.show_log {
            let h = self.log.len() as f32 * 15.0 + 10.0;
            painter.rect_filled(Rect::from_min_size(rect.left_bottom() - vec2(0.0, h), vec2(rect.width().min(560.0), h)), 0.0, BG.gamma_multiply(0.85));
        }
        for (i, l) in self.log.iter().enumerate().filter(|_| self.settings.show_log) {
            let first = l.lines().next().unwrap_or("");
            screen_text(&painter, rect.left_bottom() + vec2(10.0, -20.0 - i as f32 * 15.0), first, 11.0, TEXT.gamma_multiply(1.0 - i as f32 * 0.08), rect.width().min(560.0) - 20.0);
        }
        if let Some(w) = self.world.as_mut().filter(|w| w.organize != self.settings.organize) {
            let changed: HashSet<String> = self.git.status.iter().map(|(_, f)| f.clone()).collect();
            w.organize(self.settings.organize, &changed);
            self.sel = Sel::None;
        }
        let mode = if z < 0.08 { "universe" } else if z < 0.45 { "site" } else if z < 1.2 { "factory floor" } else { "machine detail" };
        let legend = matches!(self.sel, Sel::Sec(f, s) if self.world.as_ref().is_some_and(|w| w.facs[f].sections[s].rel.is_some()));
        painter.rect_filled(Rect::from_min_size(rect.left_top(), vec2(rect.width(), if legend { 44.0 } else { 28.0 })), 0.0, BG.gamma_multiply(0.85));
        if legend {
            let mut x = rect.left_top().x + 34.0;
            for (label, c) in [("imports", TEAL), ("imported by", ORANGE), ("calls", PURPLE), ("context", AMBER)] {
                painter.line_segment([pos2(x, rect.top() + 33.0), pos2(x + 14.0, rect.top() + 33.0)], Stroke::new(2.0, c));
                let g = painter.layout_no_wrap(label.into(), FontId::proportional(11.0), c);
                let w = g.size().x;
                painter.galley(pos2(x + 18.0, rect.top() + 26.0), g, c);
                x += w + 40.0;
            }
        }
        let scope_text = self.scope.as_ref().map(|sc| match sc {
            Scope::Path(p) => format!("{p}/ · double-click a box to go deeper, esc to go up · "),
            Scope::Cells => "context cells · esc to go up · ".into(),
            Scope::District(_) => "district · esc to go up · ".into(),
        });
        let jira_status = jira::STATUS.lock().unwrap().clone();
        let jira_busy = if jira_status.is_empty() { String::new() } else { format!("{jira_status} · ") };
        let busy = if let Some(t) = &scope_text { t.as_str() } else if !jira_busy.is_empty() { jira_busy.as_str() } else if self.rebuild_rx.is_some() { "rebuilding Cellworks… · " } else if self.rescan_rx.is_some() { "rescanning… · " } else { "" };
        painter.text(rect.right_top() + vec2(-340.0, 8.0), Align2::RIGHT_TOP, format!("{busy}⌘K search · esc back · {mode}"), FontId::proportional(11.0), DIM);
        let (can_back, can_fwd) = (!self.hist_back.is_empty(), !self.hist_fwd.is_empty());
        if ui.put(Rect::from_min_size(rect.left_top() + vec2(30.0, 3.0), vec2(22.0, 20.0)), egui::Button::new("‹").small()).on_hover_text("back (cmd+left)").clicked() && can_back {
            self.go_back(ui.ctx());
        }
        if ui.put(Rect::from_min_size(rect.left_top() + vec2(54.0, 3.0), vec2(22.0, 20.0)), egui::Button::new("›").small()).on_hover_text("forward (cmd+right)").clicked() && can_fwd {
            self.go_forward(ui.ctx());
        }
        self.breadcrumb(ui, Rect::from_min_size(rect.left_top() + vec2(82.0, 3.0), vec2((rect.width() * 0.5 - 220.0).max(120.0), 22.0)), rect);
        self.filter_bar(ui, rect, hits.as_ref());
        self.satellites = satellites_out;
        self.conn_hits = lane_hits.take();
        self.scope_regions = regions_out;
        self.nvim_rect = None;
        let settled = (self.goal_zoom / self.zoom - 1.0).abs() < 0.15;
        let shown = editor_target.as_ref().map(|t| t.3);
        if shown.is_some() && shown != self.editor_sec {
            // a new file reached editor size: centre it and fit it to the view
            if let Some((f, s)) = shown {
                let r = self.world.as_ref().unwrap().facs[f].sections[s].rect;
                self.goal_zoom = (rect.width() / r.width()).min(rect.height() / r.height()) * 0.82;
            }
        }
        if shown.is_some() && self.editor_sec.is_none() {
            self.editor_full = self.settings.editor_full_screen_by_default;
        }
        self.editor_sec = shown;
        if shown.is_none() {
            self.editor_full = false;
        }
        if let Some((path, line, area, _)) = editor_target {
            let (cols, rows) = ((area.width() / char_w) as usize, (area.height() / line_h) as usize);
            if self.nvim.as_mut().is_some_and(|n| !n.alive()) {
                self.nvim = None;
            }
            match &mut self.nvim {
                Some(n) if n.path != path && settled => {
                    n.open(&path, line);
                    self.nvim_focus = false;
                }
                Some(_) => {}
                None if settled => match nvim::Nvim::spawn(&path, line, cols.max(20), rows.max(5), ui.ctx().clone()) {
                    Ok(n) => self.nvim = Some(n),
                    Err(e) => self.log(format!("nvim: {e}")),
                },
                None => {}
            }
            if let Some(n) = self.nvim.as_mut().filter(|n| n.path == path) {
                n.pump();
                n.resize(cols, rows);
                paint_editor(&painter, n, area, &font, char_w, line_h, self.nvim_focus);
                self.nvim_rect = Some(area);
            }
        }
        if self.nvim_rect.is_none() {
            self.nvim_focus = false;
        }

        if let (Some((f, s)), true) = (self.editor_sec, self.nvim_rect.is_some()) {
            let w = self.world.as_ref().unwrap();
            let sec = &w.facs[f].sections[s];
            let fac_rect = w.facs[f].rect;
            let mut zoom_out = false;
            let pop;
            let toggle = if self.editor_full {
                let bar = Rect::from_min_size(rect.min, vec2(rect.width(), EDITOR_BAR));
                painter.rect_filled(bar, 0.0, Color32::from_rgb(0x1e, 0x1b, 0x16));
                let mut info = sec.rel.clone().unwrap_or_else(|| format!("{} › {}", w.facs[f].id, sec.title));
                if let Some(m) = &sec.metrics {
                    info += &format!("   {} · {} loc · {} fn · max cx {} · ↗{} ↙{}", m.lang, m.loc, m.funcs, m.cx_max, sec.deps.len(), sec.dependents.len());
                }
                screen_text(&painter, bar.min + vec2(34.0, 15.0), &info, 13.0, AMBER, bar.width() - 330.0);
                zoom_out = ui.put(Rect::from_min_size(bar.right_top() + vec2(-130.0, 10.0), vec2(96.0, 24.0)), egui::Button::new("zoom out")).clicked();
                pop = ui.put(Rect::from_min_size(bar.right_top() + vec2(-350.0, 10.0), vec2(80.0, 24.0)), egui::Button::new("pop out")).clicked();
                ui.put(Rect::from_min_size(bar.right_top() + vec2(-262.0, 10.0), vec2(124.0, 24.0)), egui::Button::new("exit full screen")).on_hover_text("⌘F").clicked()
            } else {
                let sr = wr(sec.rect);
                let head_h = (16.0 * z).min(24.0);
                pop = ui.put(Rect::from_min_size(pos2(sr.max.x - 180.0, sr.min.y + 2.0), vec2(74.0, head_h - 4.0)), egui::Button::new("pop out").small()).on_hover_text("edit in its own window and keep navigating").clicked();
                ui.put(Rect::from_min_size(pos2(sr.max.x - 100.0, sr.min.y + 2.0), vec2(94.0, head_h - 4.0)), egui::Button::new("full screen").small()).on_hover_text("⌘F").clicked()
            };
            if toggle || ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                self.editor_full = !self.editor_full;
            }
            if pop {
                if let Some(n) = self.nvim.take() {
                    self.popped.push(n);
                    self.pop_focus = Some(self.popped.len() - 1);
                }
                zoom_out = true;
            }
            if zoom_out {
                self.goal_zoom = (rect.width() / fac_rect.width()).min(rect.height() / fac_rect.height()) * 0.85;
                self.nvim_focus = false;
                self.editor_full = false;
            }
        }
        if ui.put(Rect::from_min_size(rect.right_top() + vec2(-100.0, 4.0), vec2(70.0, 20.0)), egui::Button::new("Settings").small()).clicked() {
            self.settings_open = !self.settings_open;
        }
        if self.current.is_some() {
            let r = Rect::from_min_size(rect.right_top() + vec2(-330.0, 4.0), vec2(164.0, 20.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| self.branch_menu(ui));
        }
        if ui.put(Rect::from_min_size(rect.right_top() + vec2(-160.0, 4.0), vec2(54.0, 20.0)), egui::Button::new("View").small().selected(self.view_open)).clicked() {
            self.view_open = !self.view_open;
        }
        for (open, pos, l, r) in [(&mut self.left_open, rect.left_top() + vec2(4.0, 4.0), "◀", "▶"), (&mut self.right_open, rect.right_top() + vec2(-26.0, 4.0), "▶", "◀")] {
            if ui.put(Rect::from_min_size(pos, vec2(22.0, 20.0)), egui::Button::new(if *open { l } else { r }).small()).clicked() {
                *open = !*open;
            }
        }
        if let Some(w) = &mut self.world {
            w.frozen = self.editor_sec;
        }
        self.servo_card(&ui.ctx().clone(), rect);
        if self.settings.show_agent_list && !self.editor_full {
            self.agent_list(ui.ctx(), rect);
        }
        self.palette(&ui.ctx().clone(), rect);
        ui.ctx().request_repaint();
    }

    // ---------------- side panels ----------------
    fn projects_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Projects").color(AMBER));
        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("filter"));
        // any folder can be added, git repository or not
        let mut add = false;
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut self.add_path).hint_text("add project: ~/path/to/folder").desired_width(ui.available_width() - 44.0));
            add = (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Add").clicked();
        });
        if !self.add_error.is_empty() {
            ui.label(RichText::new(&self.add_error).size(11.0).color(RED));
        }
        if add && !self.add_path.trim().is_empty() {
            let p = self.add_path.clone();
            self.add_project(&p, ui.ctx());
        }
        ui.separator();
        let mut open = None;
        let mut remove = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for p in &self.projects {
                if !self.filter.is_empty() && !p.name.to_lowercase().contains(&self.filter.to_lowercase()) {
                    continue;
                }
                let mine: Vec<&Servo> = self.servos.values().filter(|s| s.cwd == p.path || s.cwd.starts_with(&format!("{}/", p.path))).collect();
                let active = mine.iter().filter(|s| s.last.elapsed().as_secs() < 600).count();
                let waiting = mine.iter().filter(|s| !s.pending.is_empty()).count();
                let mut label = p.name.clone();
                if active > 0 {
                    label += &format!("  ⚙{active}");
                }
                if waiting > 0 {
                    label += &format!("  !{waiting}");
                }
                let color = if waiting > 0 { RED } else if active > 0 { AMBER } else { TEXT };
                let row = ui.selectable_label(self.current.as_deref() == Some(&p.path), RichText::new(label).monospace().color(color)).on_hover_text(&p.path);
                if row.clicked() {
                    open = Some(p.path.clone());
                }
                if self.settings.extra_projects.contains(&p.path) {
                    row.context_menu(|ui| {
                        if ui.button("Remove from list").clicked() {
                            remove = Some(p.path.clone());
                            ui.close();
                        }
                    });
                }
            }
        });
        if let Some(p) = open {
            self.open_project(&p, ui.ctx());
        }
        if let Some(p) = remove {
            self.remove_project(&p);
        }
    }

    fn side_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (t, l) in [(Tab::Inspect, "Inspect"), (Tab::Servitors, "Servitors"), (Tab::Chats, "Chats"), (Tab::Files, "Files"), (Tab::Git, "Git")] {
                ui.selectable_value(&mut self.tab, t, l);
            }
        });
        ui.separator();
        match self.tab {
            Tab::Inspect => self.inspect_tab(ui),
            Tab::Servitors => self.servitors_tab(ui),
            Tab::Chats => self.chats_tab(ui),
            Tab::Files => self.files_tab(ui),
            Tab::Git => self.git_tab(ui),
        }
    }

    fn inspect_tab(&mut self, ui: &mut egui::Ui) {
        if let Sel::Continent(q) = self.sel {
            let c = &self.continents[q];
            let w = if q == self.cur { self.world.as_ref() } else { c.world.as_ref() };
            let mine: Vec<&Servo> = self.servos.values().filter(|s| s.cwd == c.path || s.cwd.starts_with(&format!("{}/", c.path))).collect();
            let mut fly = false;
            continent_view(ui, c, w, &mine, q == self.cur, &mut fly, self.settings.stale_after_days);
            if fly {
                let p = c.path.clone();
                if q == self.cur {
                    self.overview();
                } else {
                    self.open_project(&p, ui.ctx());
                }
            }
            return;
        }
        let Some(w) = &self.world else { return };
        let mut act = None;
        if let Sel::District(_) | Sel::Sub(_) | Sel::Dir(_) = self.sel {
            let (name, kind, members, district) = match &self.sel {
                Sel::Sub(k) => (format!("{} › {}", w.districts[w.subdistricts[*k].0], w.subdistricts[*k].1), "sub-district", (0..w.facs.len()).filter(|&i| w.facs[i].sub == *k).collect::<Vec<_>>(), None),
                Sel::District(d) => (w.districts[*d].clone(), "district", (0..w.facs.len()).filter(|&i| w.facs[i].district == *d).collect(), Some(*d)),
                Sel::Dir(p) => (format!("{p}/"), "folder and everything below it", scope_members(w, &Scope::Path(p.clone())), None),
                _ => unreachable!(),
            };
            let changed: HashSet<&str> = self.git.status.iter().map(|(_, f)| f.as_str()).collect();
            egui::ScrollArea::vertical().show(ui, |ui| group_view(ui, w, &name, kind, &members, district, &changed, &mut act));
            match act {
                Some(Act::GoFac(f)) => {
                    self.navigate_to(Sel::Fac(f));
                    self.refocus();
                }
                Some(Act::File(p, l)) => self.open_file(&p, l),
                Some(Act::Go((f, s))) => self.goto_section(f, s),
                _ => {}
            }
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| match &self.sel {
            Sel::Fac(fi) if w.facs[*fi].kind == Kind::Code => code_fac_view(ui, w, *fi, &mut act),
            Sel::Fac(fi) => {
                let f = &w.facs[*fi];
                ui.label(RichText::new(&f.title).color(AMBER).strong());
                if let Some(st) = &f.status {
                    ui.label(RichText::new(format!("status {st} ({} days old){}", f.age, if f.age > self.settings.stale_after_days { " · STALE" } else { "" })).color(DIM));
                }
                if ui.link("open file").clicked() {
                    act = Some(Act::File(f.path.clone(), None));
                }
                for (si, s) in f.sections.iter().enumerate() {
                    if ui.link(&s.title).clicked() {
                        act = Some(Act::Go((*fi, si)));
                    }
                }
                links_view(ui, w, "Mentioned by code", &f.links, &mut act);
            }
            Sel::Sec(fi, si) if w.facs[*fi].sections[*si].rel.is_some() => code_file_view(ui, w, (*fi, *si), &mut act),
            Sel::Sec(fi, si) => {
                let sec = &w.facs[*fi].sections[*si];
                ui.label(RichText::new(&sec.title).color(AMBER).strong());
                if let Some(dir) = &sec.code {
                    if ui.button(format!("show code: {}", dir.rsplit('/').take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"))).clicked() {
                        act = Some(Act::Code(dir.clone()));
                    }
                    // a running workload: the manifest it is deployed from, for this environment
                    if otc::parse(&w.repo).is_some() {
                        let env = &w.districts[w.facs[*fi].district];
                        if let Some(m) = otc::manifest(dir, &sec.title, env) {
                            if ui.button(format!("open manifest: {}", m.rsplit('/').take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("/"))).on_hover_text(&m).clicked() {
                                act = Some(Act::Code(m));
                            }
                        }
                    }
                }
                if let Some((site, _)) = jira::parse(&w.repo) {
                    let key = sec.title.split_whitespace().next().unwrap_or_default();
                    if ui.button("open in Jira").clicked() {
                        let _ = std::process::Command::new("open").arg(format!("https://{site}/browse/{key}")).spawn();
                    }
                    for &d in &sec.deps {
                        if ui.link(format!("→ {}", w.facs[d.0].sections[d.1].title)).clicked() {
                            act = Some(Act::Go(d));
                        }
                    }
                } else if ui.link("open file").clicked() {
                    act = Some(Act::File(w.facs[*fi].path.clone(), None));
                }
                links_view(ui, w, "Implemented in", &sec.links, &mut act);
                ui.horizontal_wrapped(|ui| {
                    for (i, it) in sec.items.iter().enumerate() {
                        if ui.link(&it.term).clicked() {
                            act = Some(Act::Trace(*fi, *si, i));
                        }
                    }
                });
                ui.separator();
                ui.label(RichText::new(&sec.body).monospace().size(11.0));
            }
            Sel::Item(fi, si, i) => {
                let sec = &w.facs[*fi].sections[*si];
                let it = &sec.items[*i];
                ui.label(RichText::new(&it.term).color(AMBER).strong());
                ui.label(RichText::new(format!("{} › {}{}", w.facs[*fi].id, sec.title, if it.concept { " · glossary concept" } else { "" })).color(DIM));
                ui.label(&it.text);
                if let (Some(p), true) = (&sec.path, it.line > 0) {
                    if ui.link(format!("open at line {}", it.line)).clicked() {
                        act = Some(Act::File(p.clone(), Some(it.line)));
                    }
                }
                ui.separator();
                ui.label(format!("{} {}:", if sec.rel.is_some() { "called from" } else { "used in" }, it.uses.len()));
                for &u in &it.uses {
                    if ui.link(format!("→ {}", sec_label(w, u))).clicked() {
                        act = Some(Act::Go(u));
                    }
                }
                if ui.button("trace on map").clicked() {
                    act = Some(Act::Trace(*fi, *si, *i));
                }
            }
            _ => {
                ui.label(RichText::new("Click something on the map.").color(DIM));
            }
        });
        match act {
            Some(Act::Go((f, s))) => self.goto_section(f, s),
            Some(Act::GoFac(f)) => self.navigate_to(Sel::Fac(f)),
            Some(Act::Trace(f, s, i)) => {
                self.sel = Sel::Item(f, s, i);
                self.trace_item(f, s, i);
            }
            Some(Act::File(p, line)) => self.open_file(&p, line),
            Some(Act::Code(dir)) => self.show_code(&dir, ui.ctx()),
            None => {}
        }
    }

    fn servitors_tab(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("New servitor").color(AMBER).strong());
        ui.label(RichText::new("Starts Claude Code in a detachable session in this project.").size(11.0).color(DIM));
        ui.add(egui::TextEdit::multiline(&mut self.new_prompt).desired_rows(3).hint_text("First prompt (optional)").desired_width(f32::INFINITY));
        let mut start = false;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("mode").selected_text(MODES[self.mode]).show_ui(ui, |ui| {
                for (i, m) in MODES.iter().enumerate() {
                    ui.selectable_value(&mut self.mode, i, *m);
                }
            });
            start = ui.add_enabled(self.current.is_some(), egui::Button::new("Start")).clicked();
        });
        if start {
            let (cwd, prompt) = (self.current.clone().unwrap_or_default(), std::mem::take(&mut self.new_prompt));
            self.start_session(&cwd, Some(prompt), None, ui.ctx());
        }
        ui.separator();
        let mut list: Vec<(&String, &Servo)> = self.servos.iter().filter(|(_, s)| self.in_project(s)).collect();
        list.sort_by_key(|(_, s)| std::cmp::Reverse(s.last));
        let (mut open, mut follow) = (None, None);
        egui::ScrollArea::vertical().show(ui, |ui| {
            if list.is_empty() {
                ui.label(RichText::new("No servitors active in this project.").color(DIM));
            }
            for (id, s) in list {
                ui.horizontal(|ui| {
                    color_dot(ui, s.color);
                    ui.label(RichText::new(&s.name).monospace());
                    let st = if !s.pending.is_empty() { "waiting".to_string() } else if s.last.elapsed().as_secs() > 300 { "idle".into() } else { s.state.clone() };
                    ui.label(RichText::new(st).color(if s.pending.is_empty() { DIM } else { RED }));
                    let hosted = self.terms.contains_key(id.as_str());
                    if ui.small_button(if hosted { "terminal" } else { "take over" }).on_hover_text(if hosted { "open its terminal" } else { "running elsewhere: resume it in a Cellworks session" }).clicked() {
                        open = Some((*id).clone());
                    }
                    if ui.small_button("follow").clicked() {
                        follow = Some((*id).clone());
                    }
                });
                ui.label(RichText::new(&s.task).monospace().size(11.0).color(DIM));
            }
        });
        if let Some(id) = open {
            self.open_term(&id, ui.ctx());
        }
        if follow.is_some() {
            self.follow = follow;
        }
    }

    fn chats_tab(&mut self, ui: &mut egui::Ui) {
        if ui.button("Refresh").clicked() {
            if let Some(p) = &self.current {
                self.sessions = sessions(p);
            }
        }
        let mut open = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for s in &self.sessions {
                let ago = SystemTime::now().duration_since(s.mtime).map(|d| d.as_secs()).unwrap_or(0);
                let ago = if ago < 3600 { format!("{}m", ago / 60) } else if ago < 86400 { format!("{}h", ago / 3600) } else { format!("{}d", ago / 86400) };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{ago:>4}")).monospace().color(DIM));
                    if ui.link(&s.title).clicked() {
                        open = Some(s.id.clone());
                    }
                });
            }
        });
        if let Some(id) = open {
            self.open_term(&id, ui.ctx());
        }
    }

    fn files_tab(&mut self, ui: &mut egui::Ui) {
        let Some(w) = &self.world else { return };
        ui.add(egui::TextEdit::singleline(&mut self.files_filter).hint_text("filter files"));
        let changed: HashMap<&str, &str> = self.git.status.iter().map(|(xy, f)| (f.as_str(), xy.as_str())).collect();
        let q = self.files_filter.to_lowercase();
        let mut open = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (fi, f) in w.facs.iter().enumerate() {
                if f.kind == Kind::Cell {
                    if (q.is_empty() || f.id.contains(&q)) && ui.link(RichText::new(format!("▤ {}", f.id)).monospace()).clicked() {
                        open = Some((f.path.clone(), fi, None));
                    }
                    continue;
                }
                let rows: Vec<(usize, &Section)> = f.sections.iter().enumerate().filter(|(_, s)| q.is_empty() || s.rel.as_deref().unwrap_or("").to_lowercase().contains(&q)).collect();
                if rows.is_empty() {
                    continue;
                }
                egui::CollapsingHeader::new(RichText::new(&f.title).monospace().color(TEAL)).default_open(!q.is_empty()).id_salt(fi).show(ui, |ui| {
                    for (si, s) in rows {
                        let mark = s.rel.as_deref().and_then(|r| changed.get(r)).map(|x| format!("  {}", x.trim())).unwrap_or_default();
                        if ui.link(RichText::new(format!("{}{mark}", s.title)).monospace()).clicked() {
                            open = Some((s.path.clone().unwrap_or_default(), fi, Some(si)));
                        }
                    }
                });
            }
        });
        if let Some((p, f, s)) = open {
            self.open_file(&p, None);
            match s {
                Some(s) => self.goto_section(f, s),
                None => {
                    let r = self.world.as_ref().unwrap().facs[f].rect;
                    self.fit_to(r)
                }
            }
        }
    }

    /// Top-bar branch switcher: switch or create branches, open worktrees as their own projects.
    fn branch_menu(&mut self, ui: &mut egui::Ui) {
        let Some(repo) = self.current.clone() else { return };
        if self.git.branch.is_empty() && self.git.worktrees.is_empty() {
            return; // not a git repository
        }
        let label = if self.git.branch.is_empty() { "⎇ detached".to_string() } else { format!("⎇ {}", self.git.branch) };
        let dirty = self.git.status.len();
        let mut act: Option<(&str, String)> = None;
        ui.menu_button(RichText::new(label).size(12.0).color(AMBER), |ui| {
            ui.set_min_width(280.0);
            if !self.branch_err.is_empty() {
                ui.label(RichText::new(&self.branch_err).monospace().size(11.0).color(RED));
            }
            if dirty > 0 {
                ui.label(RichText::new(format!("{dirty} uncommitted change{} come along when switching", if dirty == 1 { "" } else { "s" })).size(11.0).color(DIM));
            }
            ui.label(RichText::new("Branches").color(AMBER).strong());
            egui::ScrollArea::vertical().id_salt("branches").max_height(260.0).show(ui, |ui| {
                for b in &self.git.branches {
                    let here = *b == self.git.branch;
                    let wt = self.git.worktrees.iter().find(|w| w.1 == *b && w.0 != repo).map(|w| w.0.clone());
                    ui.horizontal(|ui| {
                        if ui.selectable_label(here, RichText::new(b).monospace()).clicked() && !here {
                            act = Some(match &wt {
                                Some(p) => ("open", p.clone()), // checked out elsewhere: go to that worktree
                                None => ("switch", b.clone()),
                            });
                        }
                        if wt.is_some() {
                            ui.label(RichText::new("worktree").size(10.0).color(TEAL));
                        }
                    });
                }
            });
            ui.separator();
            ui.label(RichText::new("Worktrees").color(AMBER).strong());
            for (p, b) in &self.git.worktrees {
                let here = *p == repo;
                if ui.selectable_label(here, RichText::new(format!("{b}  ·  {}", p.rsplit('/').next().unwrap_or(p))).monospace()).on_hover_text(p).clicked() && !here {
                    act = Some(("open", p.clone()));
                }
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut self.branch_new).hint_text("new branch name").desired_width(f32::INFINITY));
            let name = self.branch_new.trim().to_string();
            ui.horizontal(|ui| {
                if ui.add_enabled(!name.is_empty(), egui::Button::new("create branch")).on_hover_text("from the current commit, and switch to it").clicked() {
                    act = Some(("create", name.clone()));
                }
                if ui.add_enabled(!name.is_empty(), egui::Button::new("create worktree")).on_hover_text("new branch in a folder next to this one, opened as its own project").clicked() {
                    act = Some(("worktree", name.clone()));
                }
            });
        });
        let Some((op, arg)) = act else { return };
        let ctx = ui.ctx().clone();
        self.branch_err.clear();
        let out = match op {
            "switch" => git(&repo, &["switch", &arg]),
            "create" => git(&repo, &["switch", "-c", &arg]),
            "worktree" => {
                let dir = format!("{repo}-{}", arg.replace('/', "-"));
                let out = git(&repo, &["worktree", "add", &dir, "-b", &arg]);
                if std::path::Path::new(&dir).is_dir() {
                    self.branch_new.clear();
                    self.add_project(&dir, &ctx);
                } else {
                    self.branch_err = out.trim().to_string();
                }
                return;
            }
            _ => {
                self.add_project(&arg, &ctx);
                return;
            }
        };
        self.git = git_state(&repo);
        if self.git.branch == arg {
            self.branch_new.clear();
            self.rescan(&ctx);
        } else {
            self.branch_err = out.trim().to_string(); // the menu stays open and shows why
        }
    }

    fn git_tab(&mut self, ui: &mut egui::Ui) {
        let Some(repo) = self.current.clone() else { return };
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("⎇ {}", self.git.branch)).color(AMBER).strong());
            if let Some((behind, ahead)) = self.git.ahead_behind.split_once('\t') {
                ui.label(RichText::new(format!("↑{ahead} ↓{behind}")).color(DIM));
            }
        });
        ui.horizontal(|ui| {
            for op in ["fetch", "pull", "push"] {
                if ui.button(op).clicked() {
                    self.git_out = git(&repo, &[op]); // ponytail: blocks the UI while git talks to the remote; move to a thread if it hurts
                    self.git = git_state(&repo);
                }
            }
            if ui.button("refresh").clicked() {
                self.git = git_state(&repo);
            }
            if ui.button("terminal").on_hover_text("new Herdr tab in this project").clicked() {
                let out = herdr_tab(&repo, repo.rsplit('/').next().unwrap_or("shell"), None);
                self.log(format!("herdr: {}", out.lines().next().unwrap_or("")));
            }
        });
        ui.separator();
        ui.label(format!("{} changed", self.git.status.len()));
        let mut diff = None;
        egui::ScrollArea::vertical().id_salt("status").max_height(220.0).show(ui, |ui| {
            for (xy, f) in &self.git.status {
                ui.horizontal(|ui| {
                    let mut on = self.git_sel.contains(f);
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            self.git_sel.insert(f.clone());
                        } else {
                            self.git_sel.remove(f);
                        }
                    }
                    ui.label(RichText::new(xy).monospace().color(AMBER));
                    if ui.link(RichText::new(f).monospace()).clicked() {
                        diff = Some(f.clone());
                    }
                });
            }
        });
        if let Some(f) = diff {
            self.viewer = Some((format!("diff {f}"), git_diff(&repo, &f), true));
        }
        ui.horizontal(|ui| {
            let files: Vec<&str> = self.git_sel.iter().map(String::as_str).collect();
            if ui.add_enabled(!files.is_empty(), egui::Button::new("stage")).clicked() {
                self.git_out = git(&repo, &[&["add", "--"], files.as_slice()].concat());
                self.git = git_state(&repo);
            }
            if ui.add_enabled(!files.is_empty(), egui::Button::new("unstage")).clicked() {
                self.git_out = git(&repo, &[&["restore", "--staged", "--"], files.as_slice()].concat());
                self.git = git_state(&repo);
            }
        });
        ui.add(egui::TextEdit::multiline(&mut self.commit_msg).desired_rows(2).hint_text("commit message").desired_width(f32::INFINITY));
        if ui.add_enabled(!self.commit_msg.trim().is_empty(), egui::Button::new("commit staged")).clicked() {
            self.git_out = git(&repo, &["commit", "-m", &self.commit_msg]);
            self.commit_msg.clear();
            self.git_sel.clear();
            self.git = git_state(&repo);
        }
        if !self.git_out.is_empty() {
            ui.label(RichText::new(&self.git_out).monospace().size(11.0).color(DIM));
        }
        ui.separator();
        egui::CollapsingHeader::new("worktrees").show(ui, |ui| {
            for (w, b) in &self.git.worktrees {
                ui.label(RichText::new(format!("{b}  {w}")).monospace().size(11.0));
            }
        });
        ui.label("log");
        egui::ScrollArea::vertical().id_salt("log").show(ui, |ui| {
            for l in &self.git.log {
                ui.label(RichText::new(l).monospace().size(11.0));
            }
        });
    }

    /// Where the camera is: Universe › project › district › sub-district › folder › file. Every part is clickable.
    fn breadcrumb(&mut self, ui: &mut egui::Ui, strip: Rect, canvas: Rect) {
        #[derive(Clone, Copy)]
        enum Crumb {
            Universe,
            Project,
            Group(bool, usize),
            Fac(usize),
            Sec(usize, usize),
        }
        let mut crumbs: Vec<(String, Crumb)> = vec![("Universe".into(), Crumb::Universe)];
        if let Some(c) = self.continents.get(self.cur) {
            crumbs.push((c.name.clone(), Crumb::Project));
        }
        let focus = match self.sel {
            Sel::Sec(f, s) | Sel::Item(f, s, _) => Some((f, Some(s))),
            Sel::Fac(f) => Some((f, None)),
            _ if self.zoom > 0.08 => match self.pick(self.cam) {
                Sel::Sec(f, s) | Sel::Item(f, s, _) => Some((f, Some(s))),
                Sel::Fac(f) => Some((f, None)),
                _ => None,
            },
            _ => None,
        };
        if let (Some((f, s)), Some(w)) = (focus, self.world.as_ref()) {
            let fac = &w.facs[f];
            if w.districts.len() > 1 && w.facs.iter().filter(|x| x.district == fac.district).count() > 1 {
                crumbs.push((w.districts[fac.district].clone(), Crumb::Group(false, fac.district)));
            }
            if let Some((_, name)) = w.subdistricts.get(fac.sub) {
                crumbs.push((name.clone(), Crumb::Group(true, fac.sub)));
            }
            crumbs.push((if w.districts.len() > 1 { fac.short.clone() } else { fac.title.clone() }, Crumb::Fac(f)));
            if let Some(s) = s {
                crumbs.push((fac.sections[s].title.clone(), Crumb::Sec(f, s)));
            }
        }
        let mut go = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(strip), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let n = crumbs.len();
                for (k, (label, c)) in crumbs.iter().enumerate() {
                    let text = RichText::new(label).size(12.5).color(if k + 1 == n { AMBER } else { TEXT });
                    if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        go = Some(*c);
                    }
                    if k + 1 < n {
                        ui.label(RichText::new("›").size(12.5).color(DIM));
                    }
                }
            });
        });
        match go {
            Some(Crumb::Universe) => {
                let here = self.continents.get(self.cur).map_or(Pos2::ZERO, |c| c.center);
                let all = (0..self.continents.len()).map(|i| self.disc(i)).fold(Rect::NOTHING, |a, (c, r)| a.union(Rect::from_center_size((c - here).to_pos2(), Vec2::splat(r * 2.0))));
                if all.is_positive() {
                    self.sel = Sel::None;
                    self.refocus();
                    self.goal = all.center();
                    self.goal_zoom = (canvas.width() / all.width()).min(canvas.height() / all.height()) * 0.9;
                }
            }
            Some(Crumb::Project) => {
                self.sel = Sel::None;
                self.refocus();
                self.overview();
            }
            Some(Crumb::Group(sub, k)) => {
                let r = self.world.as_ref().unwrap().region(sub, k).0;
                self.sel = if sub { Sel::Sub(k) } else { Sel::District(k) };
                self.tab = Tab::Inspect;
                self.fit_to(r);
                self.refocus();
            }
            Some(Crumb::Fac(f)) => {
                let r = self.world.as_ref().unwrap().facs[f].rect;
                self.sel = Sel::Fac(f);
                self.fit_to(r);
                self.refocus();
            }
            Some(Crumb::Sec(f, s)) => self.goto_section(f, s),
            _ => {}
        }
    }

    /// What the filter text matches in the current project and among projects; None while the filter is empty.
    fn filter_hits(&self) -> Option<FilterHits> {
        let words: Vec<String> = self.filter_q.to_lowercase().split_whitespace().map(String::from).collect();
        if words.is_empty() {
            return None;
        }
        let m = |s: &str| {
            let s = s.to_lowercase();
            words.iter().all(|w| s.contains(w.as_str()))
        };
        let k = self.filter_kinds;
        let mut h = FilterHits::default();
        for (i, c) in self.continents.iter().enumerate() {
            if k[0] && m(&c.name) {
                h.conts.insert(i);
            }
        }
        h.conts.insert(self.cur);
        if let Some(w) = &self.world {
            for (d, name) in w.districts.iter().enumerate() {
                if k[1] && m(name) {
                    h.districts.insert(d);
                }
            }
            for (sk, (_, name)) in w.subdistricts.iter().enumerate() {
                if k[1] && m(name) {
                    h.subs.insert(sk);
                }
            }
            for (fi, f) in w.facs.iter().enumerate() {
                let cell = f.kind == Kind::Cell;
                if (if cell { k[4] } else { k[2] }) && (m(&f.title) || m(&f.id)) {
                    h.facs.insert(fi);
                }
                for (si, s) in f.sections.iter().enumerate() {
                    let ok = if cell { k[4] && m(&s.title) } else { k[3] && s.rel.as_deref().is_some_and(&m) };
                    if ok {
                        h.secs.insert((fi, si));
                    }
                }
            }
        }
        Some(h)
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui, canvas: Rect, hits: Option<&FilterHits>) {
        let r = Rect::from_center_size(pos2(canvas.center().x, canvas.top() + 14.0), vec2(280.0, 22.0));
        let edit = ui.put(r, egui::TextEdit::singleline(&mut self.filter_q).hint_text("Filter   /").font(FontId::proportional(13.0)));
        if std::mem::take(&mut self.filter_focus) {
            edit.request_focus();
        }
        let Some(h) = hits else { return };
        let count = h.facs.len() + h.secs.len() + h.districts.len() + h.subs.len();
        let mut fit = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let row = Rect::from_min_size(r.left_bottom() + vec2(-80.0, 6.0), vec2(440.0, 22.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(row), |ui| {
            ui.horizontal(|ui| {
                for (k, label) in ["projects", "districts", "folders", "files", "cells"].iter().enumerate() {
                    ui.toggle_value(&mut self.filter_kinds[k], RichText::new(*label).size(11.0));
                }
                ui.label(RichText::new(format!("{count} matches")).size(11.0).color(DIM));
                fit |= ui.small_button("fit").on_hover_text("Enter: zoom to all matches").clicked();
                if ui.small_button("clear").clicked() {
                    self.filter_q.clear();
                }
            });
        });
        if fit {
            if let Some(w) = &self.world {
                let mut bb = Rect::NOTHING;
                for &fi in &h.facs {
                    bb = bb.union(w.facs[fi].rect);
                }
                for &(fi, si) in &h.secs {
                    bb = bb.union(w.facs[fi].sections[si].rect);
                }
                for &d in &h.districts {
                    bb = bb.union(w.region(false, d).0);
                }
                for &sk in &h.subs {
                    bb = bb.union(w.region(true, sk).0);
                }
                if bb.is_positive() {
                    self.fit_to(bb.expand(200.0));
                }
            }
        }
    }

    /// Every known servitor, most urgent first, in the bottom-left corner of the map.
    fn agent_list(&mut self, ctx: &egui::Context, canvas: Rect) {
        let rank = |s: &Servo| if !s.pending.is_empty() { 0 } else if s.state == "exited" { 3 } else if s.last.elapsed().as_secs() > 300 { 2 } else { 1 };
        let mut list: Vec<(&String, &Servo)> = self.servos.iter().filter(|(_, s)| s.last.elapsed().as_secs() < 1800).collect();
        list.sort_by_key(|(_, s)| (rank(s), std::cmp::Reverse(s.last)));
        let waiting = list.iter().filter(|(_, s)| rank(s) == 0).count();
        let mut clicked = None;
        let mut answers: Vec<(String, usize, bool)> = vec![];
        egui::Area::new(egui::Id::new("agent-list")).order(egui::Order::Foreground).fixed_pos(canvas.left_bottom() + vec2(8.0, -8.0)).pivot(Align2::LEFT_BOTTOM).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgba_unmultiplied(0x1e, 0x1b, 0x16, 235)).corner_radius(8.0).inner_margin(8.0).stroke(Stroke::new(1.0, Color32::from_rgb(0x3a, 0x34, 0x28))).show(ui, |ui| {
                ui.set_max_width(360.0);
                let head = format!("{} Servitors ({}){}", if self.agents_open { "–" } else { "+" }, list.len(), if waiting > 0 { format!(" · {waiting} waiting") } else { String::new() });
                if ui.add(egui::Label::new(RichText::new(head).strong().color(if waiting > 0 { RED } else { AMBER })).sense(Sense::click())).clicked() {
                    self.agents_open = !self.agents_open;
                }
                if !self.agents_open {
                    return;
                }
                if list.is_empty() {
                    ui.label(RichText::new("No Claude sessions in the last 30 minutes.").size(11.5).color(DIM));
                }
                egui::ScrollArea::vertical().max_height((canvas.height() * 0.4).max(120.0)).show(ui, |ui| {
                    for (id, s) in &list {
                        let (state, color) = match rank(s) {
                            0 => ("waiting", RED),
                            1 => (if s.state == "idle" { "idle" } else { "working" }, if s.state == "idle" { DIM } else { TEAL }),
                            2 => ("idle", DIM),
                            _ => ("exited", DIM),
                        };
                        let project = s.cwd.rsplit('/').next().unwrap_or("");
                        let row = ui.horizontal(|ui| {
                            color_dot(ui, s.color);
                            ui.label(RichText::new(&s.name).size(12.0).strong());
                            ui.label(RichText::new(project).size(11.0).color(DIM));
                            ui.label(RichText::new(state).size(11.0).color(color));
                        });
                        let task = ui.add(egui::Label::new(RichText::new(&s.task).size(11.0).color(DIM)).truncate());
                        for (i, ask) in s.pending.iter().enumerate() {
                            egui::Frame::new().fill(Color32::from_rgb(0x4a, 0x18, 0x14)).corner_radius(5.0).inner_margin(5.0).show(ui, |ui| {
                                ui.label(RichText::new(format!("! {}", ask.tool)).size(11.5).color(Color32::WHITE));
                                ui.horizontal(|ui| {
                                    if ui.button(RichText::new("Allow").color(Color32::from_rgb(0x8a, 0xe0, 0x8a))).clicked() {
                                        answers.push(((*id).clone(), i, true));
                                    }
                                    if ui.button("Deny").clicked() {
                                        answers.push(((*id).clone(), i, false));
                                    }
                                });
                            });
                        }
                        let r = row.response.rect.union(task.rect);
                        let resp = ui.interact(r, ui.id().with(id.as_str()), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r.expand(2.0), 4.0, Color32::from_white_alpha(10));
                        }
                        if resp.clicked() {
                            clicked = Some(((*id).clone(), s.cwd.clone()));
                        }
                        ui.add_space(4.0);
                    }
                });
            });
        });
        if let Some((id, _)) = clicked {
            self.go_to_servo(&id, ctx);
        }
        for (id, i, allow) in answers.into_iter().rev() {
            self.answer_ask(&id, i, allow);
        }
    }

    /// Answers the permission question on a session's screen: Enter takes the highlighted "Yes", Esc declines.
    fn answer_ask(&mut self, id: &str, i: usize, allow: bool) {
        let Some(s) = self.servos.get_mut(id) else { return };
        if i < s.pending.len() {
            s.pending.remove(i);
        }
        if let Some(t) = self.terms.get_mut(id) {
            t.input(if allow { b"\r" } else { b"\x1b" });
        }
    }

    /// A card next to the selected servitor: what it does, approvals, and a way into its terminal.
    fn servo_card(&mut self, ctx: &egui::Context, canvas: Rect) {
        let Sel::Servo(id) = self.sel.clone() else { return };
        let Some(at) = self.card_anchor else { return };
        let Some(s) = self.servos.get(&id) else { return };
        let (name, color, task, state) = (s.name.clone(), s.color, s.task.clone(), if s.pending.is_empty() { s.state.clone() } else { "waiting for you".into() });
        let pending: Vec<String> = s.pending.iter().map(|a| a.tool.clone()).collect();
        let hosted = self.terms.contains_key(&id);
        let w = 320.0;
        let pos = pos2((at.x + 28.0).min(canvas.max.x - w - 12.0).max(canvas.min.x + 8.0), (at.y - 30.0).clamp(canvas.min.y + 40.0, canvas.max.y - 220.0));
        let (mut close, mut open, mut herdr) = (false, false, false);
        let mut answer = None;
        egui::Area::new(egui::Id::new("servo-card")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgb(0x1e, 0x1b, 0x16)).corner_radius(10.0).inner_margin(10.0).stroke(Stroke::new(1.5, if pending.is_empty() { color.gamma_multiply(0.7) } else { RED })).show(ui, |ui| {
                ui.set_width(w);
                ui.horizontal(|ui| {
                    color_dot(ui, color);
                    ui.label(RichText::new(&name).strong().color(color));
                    ui.label(RichText::new(&state).size(11.0).color(if pending.is_empty() { TEAL } else { RED }));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = ui.small_button("x").clicked();
                    });
                });
                ui.add(egui::Label::new(RichText::new(&task).size(11.0).color(DIM)).truncate());
                for (i, q) in pending.iter().enumerate() {
                    egui::Frame::new().fill(Color32::from_rgb(0x4a, 0x18, 0x14)).corner_radius(6.0).inner_margin(6.0).show(ui, |ui| {
                        ui.label(RichText::new(format!("! {q}")).strong().color(Color32::WHITE));
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("Yes").color(Color32::from_rgb(0x8a, 0xe0, 0x8a))).clicked() {
                                answer = Some((i, true));
                            }
                            if ui.button("No").clicked() {
                                answer = Some((i, false));
                            }
                            ui.label(RichText::new("or answer in its terminal").size(10.5).color(DIM));
                        });
                    });
                }
                ui.horizontal(|ui| {
                    open = ui.button(if hosted { "Open terminal" } else { "Take over in Cellworks" }).clicked();
                    herdr = ui.button("Open in Herdr").clicked();
                });
            });
        });
        if let Some((i, allow)) = answer {
            self.answer_ask(&id, i, allow);
        }
        if open {
            self.open_term(&id, ctx);
        }
        if herdr {
            let out = herdr_tab(&self.current.clone().unwrap_or_default(), &name, Some(&format!("claude --resume {id}")));
            self.log(format!("herdr: {}", out.lines().next().unwrap_or("")));
        }
        if close {
            self.sel = Sel::None;
            self.follow = None;
        }
    }

    /// Flies to a servitor's project if needed, then follows it and opens its chat.
    fn go_to_servo(&mut self, id: &str, ctx: &egui::Context) {
        let Some(cwd) = self.servos.get(id).map(|s| s.cwd.clone()) else { return };
        let project = self.continents.iter().filter(|c| cwd == c.path || cwd.starts_with(&format!("{}/", c.path))).max_by_key(|c| c.path.len()).map(|c| c.path.clone());
        if let Some(p) = project.filter(|p| self.current.as_deref() != Some(p)) {
            self.open_project(&p, ctx);
        }
        self.sel = Sel::Servo(id.to_string());
        self.follow = Some(id.to_string());
        if self.terms.contains_key(id) {
            self.show_term(id);
        }
    }

    /// Everything the search palette can jump to, best matches first.
    fn palette_hits(&self, q: &str) -> Vec<(i32, &'static str, String, String, Jump)> {
        let q = q.to_lowercase();
        let mut hits = vec![];
        let mut add = |kind: &'static str, label: String, detail: String, jump: Jump, bonus: i32| {
            let score = if q.is_empty() { Some(0) } else { fuzzy(&q, &label.to_lowercase()) };
            if let Some(sc) = score {
                hits.push((sc + bonus, kind, label, detail, jump));
            }
        };
        for (id, s) in &self.servos {
            add("servitor", s.name.clone(), s.task.clone(), Jump::Servo(id.clone()), 40);
        }
        for (i, c) in self.continents.iter().enumerate() {
            add("project", c.name.clone(), c.path.clone(), Jump::Project(c.path.clone()), 20);
            if q.is_empty() {
                continue;
            }
            let w = if i == self.cur { self.world.as_ref() } else { c.world.as_ref() };
            let here = if i == self.cur { 30 } else { 0 };
            for (fi, f) in w.into_iter().flat_map(|w| w.facs.iter().enumerate()) {
                let kind = if f.kind == Kind::Cell { "cell" } else { "folder" };
                add(kind, f.title.clone(), c.name.clone(), Jump::Fac(c.path.clone(), fi), here);
                for (si, s) in f.sections.iter().enumerate() {
                    match &s.rel {
                        Some(rel) => add("file", rel.clone(), c.name.clone(), Jump::Sec(c.path.clone(), fi, si), here),
                        None => add("section", s.title.clone(), format!("{} · {}", f.id, c.name), Jump::Sec(c.path.clone(), fi, si), here),
                    }
                }
            }
        }
        for (k, (label, detail)) in [("Rebuild & restart Cellworks", "cargo build --release, sessions keep running"), ("Rescan project", "refresh AST metrics, imports and links (cmd+R)"), ("New servitor here", "start Claude Code in this project")].iter().enumerate() {
            add("command", label.to_string(), detail.to_string(), Jump::Command(k as u8), 10);
        }
        hits.sort_by(|a, b| b.0.cmp(&a.0));
        hits.truncate(14);
        let typed = q.trim();
        if typed.starts_with('/') || typed.starts_with('~') {
            let expanded = if let Some(rest) = typed.strip_prefix('~') { format!("{}{rest}", std::env::var("HOME").unwrap_or_default()) } else { typed.to_string() };
            if std::path::Path::new(&expanded).is_dir() {
                hits.insert(0, (i32::MAX, "add", format!("Add project: {expanded}"), "adds this folder to the universe".into(), Jump::AddProject(expanded)));
            }
        } else if typed.starts_with("data:") {
            hits.insert(0, (i32::MAX, "add", format!("Add data structures: {typed}"), "tables, collections, contracts and DTOs as a map".into(), Jump::AddProject(typed.to_string())));
        } else if typed.starts_with("otc:") {
            hits.insert(0, (i32::MAX, "add", format!("Add infrastructure: {typed}"), "Terraform and live Kubernetes as a map".into(), Jump::AddProject(typed.to_string())));
        } else if typed.starts_with("jira:") || typed.contains(".atlassian.net/") {
            hits.insert(0, (i32::MAX, "add", format!("Add Jira project: {typed}"), "tickets as a map, synced from Jira".into(), Jump::AddProject(typed.to_string())));
        }
        hits
    }

    fn jump(&mut self, j: Jump, ctx: &egui::Context) {
        if let Jump::Command(k) = j {
            match k {
                0 => self.rebuild_and_restart(ctx),
                1 => self.rescan(ctx),
                _ => {
                    let cwd = self.current.clone().unwrap_or_default();
                    self.start_session(&cwd, None, None, ctx);
                }
            }
            return;
        }
        if let Jump::AddProject(p) = &j {
            let p = p.clone();
            self.add_project(&p, ctx);
            return;
        }
        let project = match &j {
            Jump::Project(p) | Jump::Fac(p, _) | Jump::Sec(p, _, _) => Some(p.clone()),
            Jump::Servo(_) | Jump::AddProject(_) | Jump::Command(_) => None,
        };
        if let Some(p) = project.filter(|p| self.current.as_deref() != Some(p)) {
            self.open_project(&p, ctx);
        }
        match j {
            Jump::Project(_) => self.overview(),
            Jump::Fac(_, f) => {
                if self.world.as_ref().is_some_and(|w| f < w.facs.len()) {
                    self.navigate_to(Sel::Fac(f));
                    self.refocus();
                }
            }
            Jump::Sec(_, f, s) => {
                if self.world.as_ref().is_some_and(|w| w.facs.get(f).is_some_and(|x| s < x.sections.len())) {
                    self.goto_section(f, s);
                }
            }
            Jump::Servo(id) => self.go_to_servo(&id, ctx),
            Jump::AddProject(_) | Jump::Command(_) => {}
        }
    }

    fn palette(&mut self, ctx: &egui::Context, canvas: Rect) {
        if !self.palette_open {
            return;
        }
        let hits = self.palette_hits(&self.palette_q);
        self.palette_sel = self.palette_sel.min(hits.len().saturating_sub(1));
        let (down, up, enter) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::Enter)));
        if down {
            self.palette_sel = (self.palette_sel + 1).min(hits.len().saturating_sub(1));
        }
        if up {
            self.palette_sel = self.palette_sel.saturating_sub(1);
        }
        let mut go = enter.then(|| hits.get(self.palette_sel).map(|h| h.4.clone())).flatten();
        let width = canvas.width().min(640.0) - 20.0;
        egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).fixed_pos(canvas.center_top() + vec2(0.0, 60.0)).pivot(Align2::CENTER_TOP).show(ctx, |ui| {
            egui::Frame::new().fill(Color32::from_rgb(0x1e, 0x1b, 0x16)).corner_radius(10.0).inner_margin(10.0).stroke(Stroke::new(1.0, AMBER.gamma_multiply(0.5))).show(ui, |ui| {
                ui.set_width(width);
                let edit = ui.add(egui::TextEdit::singleline(&mut self.palette_q).hint_text("Jump to a file, folder, cell, project or servitor").desired_width(f32::INFINITY).font(FontId::proportional(16.0)));
                if std::mem::take(&mut self.palette_focus) {
                    edit.request_focus();
                }
                if edit.changed() {
                    self.palette_sel = 0;
                }
                ui.add_space(4.0);
                for (k, (_, kind, label, detail, j)) in hits.iter().enumerate() {
                    let selected = k == self.palette_sel;
                    let row = ui.horizontal(|ui| {
                        ui.add_sized([62.0, 18.0], egui::Label::new(RichText::new(*kind).size(11.0).color(match *kind {
                            "servitor" => PURPLE, "project" | "command" => AMBER, "file" => TEAL, "cell" | "section" => Color32::from_rgb(0xc8, 0xa0, 0x60), _ => TEXT,
                        })));
                        ui.add(egui::Label::new(RichText::new(label).size(13.5).color(if selected { Color32::WHITE } else { TEXT })).truncate());
                        ui.add(egui::Label::new(RichText::new(detail).size(11.0).color(DIM)).truncate());
                    });
                    let r = row.response.rect.expand(3.0);
                    if selected {
                        ui.painter().rect_filled(r, 4.0, Color32::from_white_alpha(14));
                    }
                    let resp = ui.interact(r, ui.id().with(("hit", k)), Sense::click());
                    if resp.clicked() {
                        go = Some(j.clone());
                    }
                    if resp.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                        self.palette_sel = k;
                    }
                }
                if hits.is_empty() {
                    ui.label(RichText::new("Nothing matches.").color(DIM));
                }
            });
        });
        if let Some(j) = go {
            self.palette_open = false;
            self.jump(j, ctx);
        }
    }

    /// Bottom-centre bar of view chips: what is connected, how strongly, how the map is organized.
    fn view_window(&mut self, ctx: &egui::Context) {
        if !self.view_open {
            return;
        }
        let chip = |ui: &mut egui::Ui, on: &mut bool, label: &str, c: Color32, tip: &str| {
            let text = RichText::new(label).size(12.0).color(if *on { c } else { DIM });
            let r = ui.add(egui::Button::new(text).small().corner_radius(10.0).stroke(Stroke::new(1.0, if *on { c.gamma_multiply(0.7) } else { DIM.gamma_multiply(0.3) })).fill(if *on { c.gamma_multiply(0.12) } else { Color32::TRANSPARENT }));
            if r.on_hover_text(tip).clicked() {
                *on = !*on;
            }
        };
        let pick = |ui: &mut egui::Ui, v: &mut u8, opts: &[&str], tip: &str| {
            for (i, o) in opts.iter().enumerate() {
                if ui.selectable_label(*v == i as u8, RichText::new(*o).size(12.0)).on_hover_text(tip).clicked() {
                    *v = i as u8;
                }
            }
        };
        egui::Area::new("view_bar".into()).anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -10.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).corner_radius(14.0).inner_margin(egui::Margin::symmetric(10, 5)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let st = &mut self.settings;
                    chip(ui, &mut st.show_import_belts, "imports", TEAL, "import connections");
                    chip(ui, &mut st.conn_calls, "calls", PURPLE, "function calls between files");
                    chip(ui, &mut st.show_context_pipes, "context", AMBER, "links from context cells to code");
                    chip(ui, &mut st.conn_cell_refs, "cell refs", AMBER, "references between context cells");
                    ui.separator();
                    pick(ui, &mut st.conn_level, &["files", "folders", "districts"], "connections drawn between");
                    ui.separator();
                    if ui.small_button("−").on_hover_text("weaker connections too").clicked() {
                        st.conn_min_weight = st.conn_min_weight.saturating_sub(1).max(1);
                    }
                    ui.label(RichText::new(format!("≥{}", st.conn_min_weight)).size(12.0)).on_hover_text("minimum imports or calls a connection carries");
                    if ui.small_button("+").on_hover_text("only stronger connections").clicked() {
                        st.conn_min_weight = (st.conn_min_weight + 1).min(30);
                    }
                    chip(ui, &mut st.conn_selected_only, "around selection", Color32::WHITE, "only connections touching the selection");
                    ui.separator();
                    pick(ui, &mut st.organize, &["by folder", "by graph", "by changes"], "how the map is organized");
                    ui.separator();
                    chip(ui, &mut st.git_overlay, "git", git_color(" M"), "colour changed files by git status; unchanged files fade");
                });
            });
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let before = self.settings.clone();
        let mut open = self.settings_open;
        let (mut rebuild, mut rescan) = (false, false);
        egui::Window::new("Settings").open(&mut open).resizable(false).default_width(360.0).show(ctx, |ui| {
            let st = &mut self.settings;
            egui::Grid::new("settings").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                let section = |ui: &mut egui::Ui, title: &str| {
                    ui.label(RichText::new(title).color(AMBER).strong());
                    ui.end_row();
                };
                section(ui, "Camera");
                ui.label("default zoom").on_hover_text("after opening a project, relative to fitting the whole project");
                ui.add(egui::Slider::new(&mut st.default_zoom, 0.3..=4.0).logarithmic(true));
                ui.end_row();
                ui.label("scroll zoom speed");
                ui.add(egui::Slider::new(&mut st.scroll_zoom_speed, 0.2..=3.0));
                ui.end_row();
                section(ui, "Layout");
                ui.label("physics");
                ui.checkbox(&mut st.physics, "nodes arrange themselves");
                ui.end_row();
                ui.label("physics speed");
                ui.add(egui::Slider::new(&mut st.physics_speed, 0.25..=3.0));
                ui.end_row();
                ui.label("universe");
                ui.checkbox(&mut st.show_universe, "show and fly to other projects");
                ui.end_row();
                ui.label("background scan");
                ui.checkbox(&mut st.background_scan, "scan all projects at start");
                ui.end_row();
                section(ui, "Map");
                ui.label("import belts");
                ui.checkbox(&mut st.show_import_belts, "");
                ui.end_row();
                ui.label("context pipes");
                ui.checkbox(&mut st.show_context_pipes, "");
                ui.end_row();
                ui.label("activity log");
                ui.checkbox(&mut st.show_log, "");
                ui.end_row();
                ui.label("servitor list");
                ui.checkbox(&mut st.show_agent_list, "in the corner of the map");
                ui.end_row();
                ui.label("ignored folders").on_hover_text("skipped when scanning; comma separated; applies to projects opened after the change");
                let mut text = st.ignored_folders.join(", ");
                if ui.add(egui::TextEdit::singleline(&mut text).desired_width(220.0)).changed() {
                    st.ignored_folders = text.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                }
                ui.end_row();
                ui.label("cell stale after");
                ui.add(egui::Slider::new(&mut st.stale_after_days, 1..=90).suffix(" days"));
                ui.end_row();
                section(ui, "Editor");
                ui.label("embedded nvim");
                ui.checkbox(&mut st.editor_enabled, "open when zoomed into a file");
                ui.end_row();
                ui.label("font size");
                ui.add(egui::Slider::new(&mut st.editor_font_size, 9.0..=24.0));
                ui.end_row();
                ui.label("full screen");
                ui.checkbox(&mut st.editor_full_screen_by_default, "by default");
                ui.end_row();
                section(ui, "Panels and servitors");
                ui.label("panels at start");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut st.left_panel_open, "projects");
                    ui.checkbox(&mut st.right_panel_open, "side panel");
                });
                ui.end_row();
                ui.label("servitor mode");
                egui::ComboBox::from_id_salt("default-mode").selected_text(MODES[st.default_servitor_mode.min(MODES.len() - 1)]).show_ui(ui, |ui| {
                    for (i, m) in MODES.iter().enumerate() {
                        ui.selectable_value(&mut st.default_servitor_mode, i, *m);
                    }
                });
                ui.end_row();
            });
            ui.separator();
            ui.label(RichText::new("File shapes").color(AMBER).strong());
            for (shape, label) in shapes::ALL {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(36.0, 22.0), Sense::hover());
                    ui.painter().add(egui::Shape::convex_polygon(shapes::outline(shape, r), Color32::from_rgb(0x32, 0x2c, 0x22), Stroke::new(1.0, TEAL)));
                    ui.label(label);
                });
            }
            ui.separator();
            ui.label(RichText::new("Develop Cellworks").color(AMBER).strong());
            ui.label(RichText::new(format!("source: {}", env!("CARGO_MANIFEST_DIR"))).size(11.0).color(DIM));
            ui.horizontal(|ui| {
                rebuild = ui.add_enabled(self.rebuild_rx.is_none(), egui::Button::new("Rebuild & restart")).on_hover_text("cargo build --release, then restart into the new build; Claude sessions keep running").clicked();
                rescan = ui.button("Rescan project (cmd+R)").clicked();
            });
            if !self.rebuild_status.is_empty() {
                ui.label(RichText::new(&self.rebuild_status).monospace().size(10.5).color(if self.rebuild_status.starts_with("build failed") { RED } else { DIM }));
            }
            ui.separator();
            let st = &mut self.settings;
            if ui.button("Reset to defaults").clicked() {
                *st = settings::Settings::default();
            }
            ui.label(RichText::new("Saved to ~/.config/cellworks/settings.json. Background scan applies on next start.").size(10.5).color(DIM));
        });
        self.settings_open = open;
        if rebuild {
            self.rebuild_and_restart(ctx);
        }
        if rescan {
            self.rescan(ctx);
        }
        if self.settings != before {
            if let Err(e) = self.settings.save() {
                self.log(format!("settings not saved: {e}"));
            }
        }
    }

    fn viewer_window(&mut self, ctx: &egui::Context) {
        let (line, jump) = (self.viewer_line, std::mem::take(&mut self.viewer_jump));
        let Some((title, text, is_diff)) = &self.viewer else { return };
        let mut open = true;
        egui::Window::new(title.as_str()).open(&mut open).default_size([760.0, 640.0]).show(ctx, |ui| {
            let lines: Vec<&str> = text.lines().collect();
            let mut area = egui::ScrollArea::both().auto_shrink([false, false]);
            if let (true, Some(l)) = (jump, line) {
                area = area.vertical_scroll_offset((l as f32 - 6.0).max(0.0) * (15.0 + ui.spacing().item_spacing.y));
            }
            area.show_rows(ui, 15.0, lines.len(), |ui, range| {
                for i in range {
                    let l = lines[i];
                    let c = if line == Some(i as u32 + 1) { AMBER } else if !is_diff { TEXT } else if l.starts_with('+') { Color32::from_rgb(0x8a, 0xc8, 0x7a) } else if l.starts_with('-') { Color32::from_rgb(0xd8, 0x7a, 0x6a) } else if l.starts_with("@@") { TEAL } else { DIM };
                    let line = if *is_diff { l.to_string() } else { format!("{:>5}  {l}", i + 1) };
                    ui.label(RichText::new(line).monospace().size(12.0).color(c));
                }
            });
        });
        if !open {
            self.viewer = None;
        }
    }
}

const MACHINE_BODY: Color32 = Color32::from_rgb(0x34, 0x2f, 0x27);
const ROW_H: f32 = 24.0;

/// Level of detail of a machine from its size on screen: 0 block, 1 summary, 2 readable list.
fn machine_lod(sr: Rect) -> u8 {
    if sr.width() < 56.0 || sr.height() < 40.0 {
        0
    } else if sr.width() < 190.0 || sr.height() < 120.0 {
        1
    } else {
        2
    }
}

fn head_h(sr: Rect) -> f32 {
    (sr.height() * 0.16).clamp(16.0, 28.0)
}

/// Screen slots for item rows at LOD 2: below header and meta line, one column, two when wide; the last
/// slot is returned separately for a "+N more" when the items do not fit.
fn row_slots(sr: Rect, n: usize) -> (Vec<Rect>, Option<Rect>) {
    let top = sr.min.y + head_h(sr) + 24.0;
    let inner_w = sr.width() - 20.0;
    let cols = if inner_w > 520.0 { 2 } else { 1 };
    let col_w = (inner_w - 8.0 * (cols as f32 - 1.0)) / cols as f32;
    let rows = ((sr.max.y - 10.0 - top) / ROW_H).floor().max(0.0) as usize;
    let cap = rows * cols;
    if cap == 0 || n == 0 {
        return (vec![], None);
    }
    let slot = |k: usize| Rect::from_min_size(pos2(sr.min.x + 10.0 + (k / rows) as f32 * (col_w + 8.0), top + (k % rows) as f32 * ROW_H), vec2(col_w, ROW_H - 4.0));
    let shown = if n > cap { cap - 1 } else { n };
    ((0..shown).map(slot).collect(), (n > cap).then(|| slot(cap - 1)))
}

/// `s` cut to `max_w` pixels with an ellipsis, or None when fewer than six characters would remain.
fn fit_text(p: &Painter, s: &str, font: &FontId, max_w: f32) -> Option<String> {
    let full = p.layout_no_wrap(s.into(), font.clone(), Color32::WHITE).size().x;
    if full <= max_w {
        return Some(s.into());
    }
    let n = s.chars().count();
    let keep = ((max_w / (full / n as f32)) as usize).saturating_sub(1);
    (keep >= 6 && max_w > 0.0).then(|| s.chars().take(keep).chain(['…']).collect())
}

/// Like `fit_text`, but falls back to the last path segment so directories stay identifiable.
fn fit_label(p: &Painter, s: &str, font: &FontId, max_w: f32) -> Option<String> {
    if p.layout_no_wrap(s.into(), font.clone(), Color32::WHITE).size().x <= max_w {
        return Some(s.into());
    }
    let last = s.trim_end_matches('/').rsplit('/').next().unwrap_or(s);
    fit_text(p, last, font, max_w).or_else(|| fit_text(p, s, font, max_w))
}

/// A small filled circle inline with text (the font has no reliable bullet glyph).
fn color_dot(ui: &mut egui::Ui, c: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 4.0, c);
}

/// A stable, muted colour per district.
fn district_color(d: usize) -> Color32 {
    const C: [Color32; 7] = [
        Color32::from_rgb(0xc8, 0xa0, 0x60), Color32::from_rgb(0x60, 0xb0, 0xa0), Color32::from_rgb(0x90, 0x90, 0xd0), Color32::from_rgb(0xc0, 0x80, 0x80),
        Color32::from_rgb(0x90, 0xb8, 0x70), Color32::from_rgb(0xb0, 0x88, 0xc0), Color32::from_rgb(0x70, 0xa8, 0xd0),
    ];
    C[d % C.len()]
}

/// What a folder view shows: a folder path with everything below it, the context cells, or a district
/// when districts are not folders (organized by connections or changes).
#[derive(Clone, PartialEq, Debug)]
enum Scope {
    Path(String),
    Cells,
    District(usize),
}

fn scope_has(w: &World, sc: &Scope, fi: usize) -> bool {
    let Some(f) = w.facs.get(fi) else { return false };
    match sc {
        Scope::Path(p) => f.kind == Kind::Code && (f.id == *p || f.id.starts_with(&format!("{p}/"))),
        Scope::Cells => f.kind == Kind::Cell,
        Scope::District(d) => f.district == *d,
    }
}

fn scope_members(w: &World, sc: &Scope) -> Vec<usize> {
    (0..w.facs.len()).filter(|&fi| scope_has(w, sc, fi)).collect()
}

/// The folders of a view grouped by their next subfolder: (path to drill into, label, folders).
/// The folder of the view itself has no path (it is already open).
fn scope_groups(w: &World, sc: &Scope, members: &[usize]) -> Vec<(Option<String>, String, Vec<usize>)> {
    let mut groups: Vec<(Option<String>, String, Vec<usize>)> = vec![];
    for &fi in members {
        let id = &w.facs[fi].id;
        let (path, label) = match sc {
            Scope::Path(p) => match id.strip_prefix(&format!("{p}/")) {
                Some(rest) => {
                    let first = rest.split('/').next().unwrap_or(rest);
                    (Some(format!("{p}/{first}")), first.to_string())
                }
                None => (None, "(this folder)".to_string()),
            },
            Scope::Cells => (None, String::new()),
            Scope::District(_) => {
                let first = id.split('/').next().unwrap_or(id);
                (Some(first.to_string()), first.to_string())
            }
        };
        match groups.iter_mut().find(|g| g.1 == label) {
            Some(g) => g.2.push(fi),
            None => groups.push((path, label, vec![fi])),
        }
    }
    groups.sort_by(|a, b| b.2.len().cmp(&a.2.len()).then(a.1.cmp(&b.1)));
    groups
}

/// Lays a view's folders out tidily around where they are now: a box per group (rows of folders, largest
/// first), boxes packed in rows. The folders are pinned while the view is open.
fn layout_scope(w: &mut World, sc: &Scope, members: &[usize]) {
    let centre = members.iter().fold(Rect::NOTHING, |a, &f| a.union(w.facs[f].rect)).center();
    let gap = 90.0;
    let mut blocks: Vec<(Vec2, Vec<(usize, Vec2)>)> = vec![]; // block size, folder offsets inside it
    for (_, _, mut group) in scope_groups(w, sc, members) {
        group.sort_by(|&a, &b| w.facs[b].rect.width().total_cmp(&w.facs[a].rect.width()));
        let area: f32 = group.iter().map(|&f| (w.facs[f].rect.width() + gap).powi(2)).sum();
        let max_w = group.iter().map(|&f| w.facs[f].rect.width()).fold(0.0, f32::max);
        let width = (area.sqrt() * 1.2).max(max_w);
        let (mut x, mut y, mut row_h, mut used_w) = (0.0f32, 70.0f32, 0.0f32, 0.0f32);
        let mut offs = vec![];
        for f in group {
            let s = w.facs[f].rect.size();
            if x > 0.0 && x + s.x > width {
                x = 0.0;
                y += row_h + gap;
                row_h = 0.0;
            }
            offs.push((f, vec2(x, y)));
            x += s.x + gap;
            used_w = used_w.max(x - gap);
            row_h = row_h.max(s.y);
        }
        blocks.push((vec2(used_w, y + row_h), offs));
    }
    let total: f32 = blocks.iter().map(|b| (b.0.x + 260.0) * (b.0.y + 260.0)).sum();
    let width = (total.sqrt() * 1.3).max(blocks.iter().map(|b| b.0.x).fold(0.0, f32::max));
    let (mut x, mut y, mut row_h) = (0.0f32, 0.0f32, 0.0f32);
    let mut placed: Vec<(usize, Pos2)> = vec![];
    for (size, offs) in blocks {
        if x > 0.0 && x + size.x > width {
            x = 0.0;
            y += row_h + 260.0;
            row_h = 0.0;
        }
        for (f, o) in offs {
            placed.push((f, pos2(x, y) + o));
        }
        x += size.x + 260.0;
        row_h = row_h.max(size.y);
    }
    let bb = placed.iter().fold(Rect::NOTHING, |a, (f, p)| a.union(Rect::from_min_size(*p, w.facs[*f].rect.size())));
    let shift = centre - bb.center();
    for (f, p) in placed {
        let fac = &mut w.facs[f];
        fac.rect = Rect::from_min_size(p + shift, fac.rect.size());
        fac.vel = Vec2::ZERO;
        fac.pinned = true;
    }
    w.sync_rects();
}

/// What the folders of a view connect to outside it, on a ring around them, grouped and sorted by folder:
/// single files (`by_file`) or whole folders, each with its number of links. Links come as (outside item,
/// inside file, colour): teal imports, orange imported by, purple calls, amber context; `kinds` switches
/// imports, calls and context on or off, and items with fewer than `min` links are left out.
#[allow(clippy::type_complexity)]
fn scope_ring(w: &World, members: &[usize], by_file: bool, kinds: [bool; 3], min: u32) -> (Vec<((usize, usize), Rect, u32)>, Vec<((usize, usize), (usize, usize), Color32)>) {
    let inside = |f: usize| members.contains(&f);
    let key = |d: (usize, usize)| if by_file { d } else { (d.0, usize::MAX) };
    let mut links: Vec<((usize, usize), (usize, usize), Color32)> = vec![];
    for &fi in members {
        for (si, s) in w.facs[fi].sections.iter().enumerate() {
            let me = (fi, si);
            if kinds[0] {
                links.extend(s.deps.iter().filter(|d| !inside(d.0)).map(|&d| (key(d), me, TEAL)));
                links.extend(s.dependents.iter().filter(|d| !inside(d.0)).map(|&d| (key(d), me, ORANGE)));
            }
            if kinds[1] {
                links.extend(s.calls_out.iter().chain(&s.calls_in).filter(|d| !inside(d.0 .0)).map(|&(d, _)| (key(d), me, PURPLE)));
            }
            if kinds[2] {
                links.extend(s.links.iter().filter_map(|l| match l.to {
                    (cf, Some(cs)) if !inside(cf) => Some((key((cf, cs)), me, AMBER)),
                    _ => None,
                }));
            }
        }
    }
    let mut count: HashMap<(usize, usize), u32> = HashMap::new();
    for l in &links {
        *count.entry(l.0).or_default() += 1;
    }
    let mut ext: Vec<(usize, usize)> = count.iter().filter(|(_, n)| **n >= min).map(|(k, _)| *k).collect();
    let title = |e: &(usize, usize)| (w.facs[e.0].title.clone(), if e.1 == usize::MAX { String::new() } else { w.facs[e.0].sections[e.1].title.clone() });
    ext.sort_by_key(title);
    ext.truncate(150);
    links.retain(|l| ext.contains(&l.0));
    let size = if by_file { vec2(240.0, 150.0) } else { vec2(260.0, 120.0) };
    let n = ext.len().max(1) as f32;
    let bb = members.iter().fold(Rect::NOTHING, |a, &f| a.union(w.facs[f].rect));
    // neighbours on the ring must clear each other in x or y at every angle: 330 between centres is enough
    let radius = (bb.size().length() * 0.5 + 300.0).max(n * 330.0 / std::f32::consts::TAU);
    let c = bb.center();
    let ring = ext.into_iter().enumerate().map(|(k, e)| {
        let a = k as f32 / n * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        (e, Rect::from_center_size(c + vec2(a.cos(), a.sin()) * radius, size), count[&e])
    }).collect();
    (ring, links)
}

/// An outside folder on the ring of a folder view: its path and how many links lead there.
fn draw_ring_folder(p: &Painter, f: &world::Fac, links: u32, r: Rect, selected: bool) {
    if r.width() < 30.0 {
        p.rect_filled(r, 3.0, TEAL.gamma_multiply(0.4));
        return;
    }
    p.rect_filled(r.translate(vec2(0.0, 3.0)), 10.0, Color32::from_black_alpha(90));
    p.rect_filled(r, 10.0, Color32::from_rgb(0x1b, 0x23, 0x21));
    p.rect_stroke(r, 10.0, Stroke::new(if selected { 2.5 } else { 1.2 }, if selected { Color32::WHITE } else { TEAL.gamma_multiply(0.5) }), StrokeKind::Outside);
    let px = (r.width() * 0.06).clamp(10.0, 14.0);
    let mut job = egui::text::LayoutJob::simple(format!("{}/", f.title), FontId::proportional(px), TEXT, r.width() - 16.0);
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = true;
    job.halign = egui::Align::Center;
    let g = p.layout_job(job);
    p.galley(pos2(r.center().x, r.center().y - g.size().y * 0.5 - 6.0), g, TEXT);
    p.text(pos2(r.center().x, r.max.y - 8.0), Align2::CENTER_BOTTOM, format!("{links} link{} · {} files", if links == 1 { "" } else { "s" }, f.sections.len()), FontId::proportional((px * 0.8).max(9.5)), DIM);
}

/// A file from another folder on the folder-view ring: name, its folder, complexity colour.
fn draw_satellite(p: &Painter, f: &world::Fac, s: &Section, r: Rect, selected: bool) {
    let accent = s.metrics.as_ref().map_or(tag_color(s.tag), |m| cx_color(m.cx_max));
    if r.width() < 30.0 {
        p.rect_filled(r, 3.0, accent.gamma_multiply(0.6));
        return;
    }
    p.rect_filled(r.translate(vec2(0.0, 3.0)), 8.0, Color32::from_black_alpha(90));
    p.rect_filled(r, 8.0, MACHINE_BODY);
    p.rect_filled(Rect::from_min_size(r.min, vec2(r.width(), 4.0)), 2.0, accent);
    p.rect_stroke(r, 8.0, Stroke::new(if selected { 2.5 } else { 1.0 }, if selected { Color32::WHITE } else { accent.gamma_multiply(0.5) }), StrokeKind::Outside);
    let px = (r.width() * 0.07).clamp(10.0, 14.0);
    let mut job = egui::text::LayoutJob::simple(s.title.clone(), FontId::proportional(px), TEXT, r.width() - 14.0);
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = true;
    job.halign = egui::Align::Center;
    let g = p.layout_job(job);
    p.galley(pos2(r.center().x, r.center().y - g.size().y * 0.5 - 4.0), g, TEXT);
    let folder = FontId::proportional((px * 0.8).max(9.5));
    if let Some(t) = fit_label(p, &format!("{}/", f.short), &folder, r.width() - 14.0) {
        p.text(pos2(r.center().x, r.max.y - 8.0), Align2::CENTER_BOTTOM, t, folder, DIM);
    }
}

fn machine_summary(s: &Section) -> String {
    match &s.metrics {
        Some(m) => format!("{} · {} loc · {} functions · max complexity {} · imports {} · imported by {}", m.lang, m.loc, m.funcs, m.cx_max, s.deps.len(), s.dependents.len()),
        None if !s.items.is_empty() => format!("{} terms · {} lines", s.items.len(), s.lines),
        None => format!("{} lines", s.lines),
    }
}

/// One file or cell section, drawn at the level of detail its screen size allows.
#[allow(clippy::too_many_arguments)]
/// Ticket status: to do, in progress, done.
fn ticket_color(state: u8) -> Color32 {
    match state {
        2 => Color32::from_rgb(0x5f, 0xa8, 0x6a),
        1 => AMBER,
        _ => Color32::from_rgb(0x6f, 0x86, 0xa8),
    }
}

fn draw_machine(p: &Painter, s: &Section, sr: Rect, a: f32, git: Option<&str>, diff: Option<(u32, u32)>, overlay: bool, traced: Option<usize>, wr: &dyn Fn(Rect) -> Rect) {
    let lod = machine_lod(sr);
    let shape = s.rel.as_deref().map_or(shapes::FileShape::Code, shapes::classify);
    let outline = shapes::outline(shape, sr);
    // git overlay: unchanged files fade, changed ones get their status colour and a +/- bar
    let a = if overlay && git.is_none() && s.rel.is_some() { a * 0.3 } else { a };
    let status = git.filter(|_| overlay).map(git_color);
    let mark = |p: &Painter, bar_y: f32| {
        if let Some(c) = status {
            p.add(egui::Shape::closed_line(outline.clone(), Stroke::new(2.5, c.gamma_multiply(a))));
            if let Some((add, del)) = diff {
                diff_bar(p, Rect::from_min_size(pos2(sr.min.x + 8.0, bar_y), vec2(sr.width() - 16.0, 4.0)), add, del, a);
            }
        }
    };
    let accent = match (&s.metrics, s.tag) {
        _ if s.state.is_some() => ticket_color(s.state.unwrap_or(0)),
        (Some(m), _) => cx_color(m.cx_max),
        (None, Tag::Code) => Color32::from_rgb(0x6a, 0x6a, 0x5e),
        (None, tag) => tag_color(tag),
    };
    if let Some(b) = &s.build {
        draw_build(p, s, sr, a, b, &outline, lod);
        return;
    }
    if lod == 0 {
        p.add(egui::Shape::convex_polygon(outline, status.unwrap_or(accent).gamma_multiply(0.55 * a), Stroke::NONE));
        return;
    }
    p.add(egui::Shape::convex_polygon(outline.iter().map(|q| *q + vec2(0.0, 3.0)).collect(), Color32::from_black_alpha((90.0 * a) as u8), Stroke::NONE));
    p.add(egui::Shape::convex_polygon(outline.clone(), MACHINE_BODY.gamma_multiply(a), Stroke::new(1.0, accent.gamma_multiply(0.4 * a))));
    if s.heat > 0.0 {
        p.add(egui::Shape::closed_line(outline.clone(), Stroke::new(3.0, AMBER.gamma_multiply(s.heat))));
    }
    if lod == 1 {
        // summary size: a thin colour strip and the full name, wrapped; numbers live in the tooltip
        p.add(egui::Shape::convex_polygon(shapes::clip_above(&outline, sr.min.y + 5.0), accent.gamma_multiply(a), Stroke::NONE));
        let px = (sr.width() * 0.085).clamp(11.0, 15.0);
        let mut job = egui::text::LayoutJob::simple(s.title.clone(), FontId::proportional(px), TEXT.gamma_multiply(a), sr.width() - 16.0);
        job.wrap.max_rows = ((sr.height() - 16.0) / (px * 1.25)).clamp(1.0, 3.0) as usize;
        job.wrap.break_anywhere = true;
        job.halign = egui::Align::Center;
        let g = p.layout_job(job);
        p.galley(pos2(sr.center().x, sr.center().y - g.size().y * 0.5 + 2.0), g, TEXT.gamma_multiply(a));
        if let Some(xy) = git {
            p.text(sr.right_top() + vec2(-6.0, 8.0), Align2::RIGHT_TOP, xy.trim(), FontId::monospace(10.0), status.unwrap_or(AMBER).gamma_multiply(a));
        }
        mark(p, sr.max.y - 9.0);
        if s.edit.is_some() {
            dashed_outline(p, &outline, AMBER.gamma_multiply(a), 2.0);
        }
        return;
    }
    let hh = head_h(sr);
    p.add(egui::Shape::convex_polygon(shapes::clip_above(&outline, sr.min.y + hh), accent.gamma_multiply(a), Stroke::NONE));
    let title_font = FontId::proportional((hh * 0.55).clamp(10.0, 15.0));
    let badge_w = if git.is_some() { 26.0 } else { 0.0 };
    if let Some(t) = fit_text(p, &s.title, &title_font, sr.width() - 16.0 - badge_w) {
        p.text(pos2(sr.min.x + 8.0, sr.min.y + hh * 0.5), Align2::LEFT_CENTER, t, title_font, Color32::from_rgb(0x16, 0x14, 0x10));
    }
    if let Some(xy) = git {
        p.text(pos2(sr.max.x - 8.0, sr.min.y + hh * 0.5), Align2::RIGHT_CENTER, xy.trim(), FontId::monospace(11.0), Color32::from_rgb(0x16, 0x14, 0x10));
    }
    if s.open {
        p.rect_filled(Rect::from_min_max(pos2(sr.min.x + 8.0, sr.max.y - 6.0), pos2(sr.max.x - 8.0, sr.max.y - 3.0)), 1.5, Color32::from_rgb(0xd0, 0xa0, 0x20).gamma_multiply(a));
    }
    mark(p, sr.min.y + hh + 19.0);
    if let Some(e) = &s.edit {
        edit_overlay(p, sr, &outline, e, a);
    }
    let meta_font = FontId::proportional(11.0);
    let meta = match &s.metrics {
        Some(m) => format!("{} loc · {} fn · max cx {} · ↗{} ↙{}", m.loc, m.funcs, m.cx_max, s.deps.len(), s.dependents.len()),
        None if !s.items.is_empty() => format!("{} terms", s.items.len()),
        None => format!("{} lines", s.lines),
    };
    if let Some(t) = fit_text(p, &meta, &meta_font, sr.width() - 16.0) {
        p.text(pos2(sr.min.x + 8.0, sr.min.y + hh + 4.0), Align2::LEFT_TOP, t, meta_font, DIM.gamma_multiply(a));
    }
    let row_font = FontId::proportional(12.5);
    let (_, more) = row_slots(sr, s.items.len());
    let mut shown = 0;
    for (k, it) in s.items.iter().enumerate() {
        if !it.rect.is_positive() {
            continue;
        }
        shown += 1;
        let r = wr(it.rect);
        let is_code = s.metrics.is_some();
        let tint = if is_code { cx_color(it.cx) } else if it.concept { Color32::from_rgb(0x6a, 0x8a, 0xb0) } else { tag_color(s.tag) };
        p.rect_filled(r, 5.0, Color32::from_rgb(0x40, 0x3a, 0x30).gamma_multiply(a));
        p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 1.5, tint.gamma_multiply(a));
        if traced == Some(k) {
            p.rect_stroke(r, 5.0, Stroke::new(1.5, AMBER), StrokeKind::Outside);
        }
        let right = if is_code { format!("{}", it.cx) } else { String::new() };
        let right_w = if right.is_empty() { 0.0 } else { 26.0 };
        if !right.is_empty() {
            p.text(pos2(r.max.x - 8.0, r.center().y), Align2::RIGHT_CENTER, &right, FontId::monospace(11.0), tint.gamma_multiply(a));
        }
        let avail = r.width() - 18.0 - right_w - if it.uses.is_empty() { 0.0 } else { 12.0 };
        let term_w = p.layout_no_wrap(it.term.clone(), row_font.clone(), TEXT).size().x;
        if let Some(t) = fit_text(p, &it.term, &row_font, avail) {
            p.text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, t, row_font.clone(), TEXT.gamma_multiply(a));
        }
        // definitions of terms in the remaining space
        if !is_code && term_w + 40.0 < avail {
            if let Some(d) = fit_text(p, &it.text, &row_font, avail - term_w - 14.0) {
                p.text(pos2(r.min.x + 10.0 + term_w + 12.0, r.center().y), Align2::LEFT_CENTER, d, row_font.clone(), DIM.gamma_multiply(a));
            }
        }
        if !it.uses.is_empty() {
            p.circle_filled(pos2(r.max.x - 8.0 - right_w, r.center().y), 3.0, TEAL.gamma_multiply(a));
        }
    }
    if let Some(m) = more {
        p.text(pos2(m.min.x + 10.0, m.center().y), Align2::LEFT_CENTER, format!("+{} more", s.items.len() - shown), row_font.clone(), DIM.gamma_multiply(a));
    }
    if s.items.is_empty() {
        // no functions or terms: show the start of the file instead
        let top = sr.min.y + hh + 26.0;
        let lines = ((sr.max.y - 10.0 - top) / 15.0).max(0.0) as usize;
        let font = FontId::monospace(11.0);
        for (k, l) in s.body.lines().filter(|l| !l.trim().is_empty()).take(lines).enumerate() {
            if let Some(t) = fit_text(p, l, &font, sr.width() - 20.0) {
                p.text(pos2(sr.min.x + 10.0, top + k as f32 * 15.0), Align2::LEFT_TOP, t, font.clone(), DIM.gamma_multiply(a * 0.9));
            }
        }
    }
}

fn dashed_outline(p: &Painter, outline: &[Pos2], c: Color32, width: f32) {
    let mut pts = outline.to_vec();
    pts.push(outline[0]);
    for sh in egui::Shape::dashed_line(&pts, Stroke::new(width, c), 8.0, 5.0) {
        p.add(sh);
    }
}

/// A file being written: dashed outline and scaffolding, the body rising with progress; at full size the
/// lines appear one by one with a blinking cursor.
fn draw_build(p: &Painter, s: &Section, sr: Rect, a: f32, b: &world::Build, outline: &[Pos2], lod: u8) {
    let now = p.ctx().input(|i| i.time) as f32;
    let n = b.lines.len().max(1);
    let frac = (b.progress / n as f32).min(1.0);
    p.add(egui::Shape::convex_polygon(outline.to_vec(), MACHINE_BODY.gamma_multiply(0.35 * a), Stroke::NONE));
    let built_top = sr.max.y - sr.height() * frac;
    let built = p.with_clip_rect(Rect::from_min_max(pos2(sr.min.x - 2.0, built_top), sr.max + vec2(2.0, 2.0)));
    built.add(egui::Shape::convex_polygon(outline.to_vec(), MACHINE_BODY.gamma_multiply(a), Stroke::NONE));
    // scaffolding over the part not built yet
    let scaffold = p.with_clip_rect(Rect::from_min_max(sr.min, pos2(sr.max.x, built_top)));
    let mut x = sr.min.x - sr.height();
    while x < sr.max.x {
        scaffold.line_segment([pos2(x, sr.max.y), pos2(x + sr.height(), sr.min.y)], Stroke::new(1.0, AMBER.gamma_multiply(0.25 * a)));
        x += 14.0;
    }
    if b.done_at.is_none() {
        p.line_segment([pos2(sr.min.x + 4.0, built_top), pos2(sr.max.x - 4.0, built_top)], Stroke::new(2.0, AMBER.gamma_multiply(a)));
    }
    dashed_outline(p, outline, AMBER.gamma_multiply(a), 1.8);
    if lod == 0 {
        return;
    }
    let shown = b.progress.floor() as usize;
    let title = format!("{} · {}/{} lines", if b.done_at.is_some() { "built" } else { "building" }, shown.min(n), n);
    let font = FontId::proportional(12.0);
    if let Some(t) = fit_text(p, &s.title, &font, sr.width() - 16.0) {
        p.text(sr.min + vec2(8.0, 6.0), Align2::LEFT_TOP, t, font.clone(), TEXT.gamma_multiply(a));
    }
    if let Some(t) = fit_text(p, &title, &FontId::proportional(11.0), sr.width() - 16.0) {
        p.text(sr.min + vec2(8.0, 22.0), Align2::LEFT_TOP, t, FontId::proportional(11.0), AMBER.gamma_multiply(a));
    }
    if lod == 1 {
        let bar = Rect::from_min_size(pos2(sr.min.x + 8.0, sr.max.y - 10.0), vec2(sr.width() - 16.0, 4.0));
        p.rect_filled(bar, 2.0, Color32::from_black_alpha(120));
        p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * frac, 4.0)), 2.0, AMBER.gamma_multiply(a));
        return;
    }
    // the most recent lines, the current one typing in
    let mono = FontId::monospace(11.0);
    let top = sr.min.y + 42.0;
    let rows = ((sr.max.y - 10.0 - top) / 14.0).max(0.0) as usize;
    let start = shown.saturating_sub(rows.saturating_sub(1));
    for (k, i) in (start..=shown.min(n - 1)).enumerate() {
        let line = &b.lines[i];
        let text: String = if i == shown && b.done_at.is_none() {
            let chars = line.chars().count();
            line.chars().take((b.progress.fract() * chars as f32) as usize).collect()
        } else {
            line.clone()
        };
        let y = top + k as f32 * 14.0;
        if let Some(t) = fit_text(p, &text, &mono, sr.width() - 24.0).or_else(|| (text.chars().count() < 6).then(|| text.clone())) {
            let g = p.layout_no_wrap(t, mono.clone(), TEXT.gamma_multiply(0.85 * a));
            let w = g.size().x;
            p.galley(pos2(sr.min.x + 10.0, y), g, TEXT.gamma_multiply(0.85 * a));
            if i == shown && b.done_at.is_none() && (now * 3.0).sin() > 0.0 {
                p.rect_filled(Rect::from_min_size(pos2(sr.min.x + 11.0 + w, y), vec2(6.0, 12.0)), 0.0, AMBER);
            }
        }
    }
}

/// Lines a servitor just wrote into an existing file, typing in over the lower part of the machine.
fn edit_overlay(p: &Painter, sr: Rect, outline: &[Pos2], e: &world::Build, a: f32) {
    dashed_outline(p, outline, AMBER.gamma_multiply(a), 2.0);
    let n = e.lines.len().max(1);
    let shown = (e.progress.floor() as usize).min(n);
    let h = (sr.height() * 0.45).max(40.0);
    let r = Rect::from_min_max(pos2(sr.min.x + 6.0, sr.max.y - h), sr.max - vec2(6.0, 6.0));
    p.rect_filled(r, 5.0, Color32::from_rgb(0x2a, 0x22, 0x12).gamma_multiply(0.95 * a));
    p.text(r.min + vec2(6.0, 4.0), Align2::LEFT_TOP, format!("editing · {shown}/{n} lines"), FontId::proportional(11.0), AMBER.gamma_multiply(a));
    let mono = FontId::monospace(11.0);
    let rows = ((r.height() - 22.0) / 14.0).max(0.0) as usize;
    let start = shown.saturating_sub(rows);
    for (k, line) in e.lines[start..shown].iter().enumerate() {
        if let Some(t) = fit_text(p, line, &mono, r.width() - 14.0).or_else(|| (line.chars().count() < 6).then(|| line.clone())) {
            p.text(pos2(r.min.x + 8.0, r.min.y + 20.0 + k as f32 * 14.0), Align2::LEFT_TOP, t, mono.clone(), Color32::from_rgb(0xe8, 0xc8, 0x80).gamma_multiply(a));
        }
    }
}

/// Status colour for a `git status --porcelain` code: new green, deleted red, staged blue, modified amber.
fn git_color(xy: &str) -> Color32 {
    if xy.contains('?') || xy.starts_with('A') {
        Color32::from_rgb(0x6a, 0xc0, 0x6a)
    } else if xy.contains('D') {
        RED
    } else if !xy.starts_with(' ') {
        Color32::from_rgb(0x6a, 0x9a, 0xd8)
    } else {
        AMBER
    }
}

/// Green/red bar for lines added/removed; its length grows with the size of the change (log scale).
fn diff_bar(p: &Painter, r: Rect, add: u32, del: u32, a: f32) {
    let total = (add + del).max(1) as f32;
    let len = r.width() * ((1.0 + total).ln() / (1.0 + 400.0f32).ln()).min(1.0);
    let wa = len * add as f32 / total;
    p.rect_filled(Rect::from_min_size(r.min, vec2(wa, r.height())), 1.0, Color32::from_rgb(0x6a, 0xc0, 0x6a).gamma_multiply(a));
    p.rect_filled(Rect::from_min_size(r.min + vec2(wa, 0.0), vec2(len - wa, r.height())), 1.0, RED.gamma_multiply(a));
}

/// Text at a screen position and pixel size, cut with an ellipsis to `max_w` pixels; skipped when too small to read.
fn screen_text(p: &Painter, pos: Pos2, s: &str, px: f32, c: Color32, max_w: f32) {
    if px < 5.0 {
        return;
    }
    let n = (max_w / (px * 0.62)).max(1.0) as usize;
    let s: String = if s.chars().count() > n { s.chars().take(n.saturating_sub(1)).chain(['…']).collect() } else { s.into() };
    p.text(pos, Align2::LEFT_TOP, s, FontId::monospace(px), c);
}

/// Distance from `p` to the segment a-b.
fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Where the line from the centre of `r` towards `p` leaves `r`.
fn edge_point(r: Rect, p: Pos2) -> Pos2 {
    let d = p - r.center();
    let t = (r.width() * 0.5 / d.x.abs().max(1e-3)).min(r.height() * 0.5 / d.y.abs().max(1e-3)).min(1.0);
    r.center() + d * t
}

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Paints the Neovim grid into `area`, one text run per stretch of equal highlight.
fn paint_editor(p: &Painter, n: &nvim::Nvim, area: Rect, font: &FontId, cw: f32, lh: f32, focused: bool) {
    p.rect_filled(area, 0.0, rgb(n.bg));
    let p = p.with_clip_rect(area);
    for (r, row) in n.cells.iter().enumerate() {
        let y = area.min.y + r as f32 * lh;
        let mut c = 0;
        while c < row.len() {
            let hl_id = row[c].1;
            let start = c;
            let mut text = String::new();
            while c < row.len() && row[c].1 == hl_id {
                text.push_str(&row[c].0);
                c += 1;
            }
            let hl = n.hl.get(&hl_id).copied().unwrap_or_default();
            let (mut fg, mut bg) = (hl.fg.unwrap_or(n.fg), hl.bg.unwrap_or(n.bg));
            if hl.reverse {
                std::mem::swap(&mut fg, &mut bg);
            }
            let x = area.min.x + start as f32 * cw;
            if bg != n.bg {
                p.rect_filled(Rect::from_min_size(pos2(x, y), vec2((c - start) as f32 * cw, lh)), 0.0, rgb(bg));
            }
            if !text.trim().is_empty() {
                p.text(pos2(x, y + 1.0), Align2::LEFT_TOP, text, font.clone(), rgb(fg));
            }
            if hl.underline {
                p.line_segment([pos2(x, y + lh - 1.0), pos2(x + (c - start) as f32 * cw, y + lh - 1.0)], Stroke::new(1.0, rgb(fg)));
            }
        }
    }
    let cur = Rect::from_min_size(area.min + vec2(n.cursor.1 as f32 * cw, n.cursor.0 as f32 * lh), vec2(cw, lh));
    if focused {
        p.rect_filled(cur, 0.0, rgb(n.fg).gamma_multiply(0.6));
        p.rect_stroke(area, 0.0, Stroke::new(2.0, AMBER), StrokeKind::Outside);
    } else {
        p.rect_stroke(cur, 0.0, Stroke::new(1.0, rgb(n.fg)), StrokeKind::Inside);
        p.text(area.right_top() + vec2(-6.0, 4.0), Align2::RIGHT_TOP, "click to edit", FontId::monospace(11.0), DIM);
    }
}

/// Summary of a district or sub-district: size, hotspots, changes, links to other districts, its folders.
fn group_view(ui: &mut egui::Ui, w: &World, name: &str, kind: &str, members: &[usize], district: Option<usize>, changed: &HashSet<&str>, act: &mut Option<Act>) {
    let (sub, k) = (district.is_none(), district.unwrap_or(0));
    let facs: Vec<(usize, &world::Fac)> = members.iter().map(|&i| (i, &w.facs[i])).collect();
    let files: Vec<&Section> = facs.iter().flat_map(|(_, f)| &f.sections).collect();
    let loc: u32 = files.iter().filter_map(|s| s.metrics.as_ref()).map(|m| m.loc).sum();
    let dirty = files.iter().filter(|s| s.rel.as_deref().is_some_and(|r| changed.contains(r))).count();
    ui.label(RichText::new(name).color(AMBER).strong());
    ui.label(RichText::new(kind).color(DIM));
    ui.label(RichText::new("double-click on the map to open it as a folder view").size(10.5).color(DIM));
    egui::Grid::new("group").num_columns(2).show(ui, |ui| {
        for (key, v) in [("folders", facs.len().to_string()), ("files", files.len().to_string()), ("lines of code", loc.to_string()), ("changed files", dirty.to_string())] {
            ui.label(RichText::new(key).color(DIM));
            ui.label(RichText::new(v).monospace());
            ui.end_row();
        }
    });
    let mut hot: Vec<(&Section, &world::Item)> = files.iter().flat_map(|s| s.items.iter().map(move |i| (*s, i))).filter(|(s, i)| s.metrics.is_some() && i.cx > 5).collect();
    hot.sort_by_key(|(_, i)| std::cmp::Reverse(i.cx));
    if !hot.is_empty() {
        ui.separator();
        ui.label(RichText::new("Hotspots").color(AMBER));
        for (s, it) in hot.iter().take(6) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{:>3}", it.cx)).monospace().color(cx_color(it.cx)));
                if ui.link(RichText::new(format!("{} · {}", s.title, it.term)).size(12.0)).clicked() {
                    *act = Some(Act::File(s.path.clone().unwrap_or_default(), Some(it.line)));
                }
            });
        }
    }
    if !sub {
        let mut links: HashMap<usize, u32> = HashMap::new();
        for &(a, b, c) in &w.code_belts {
            let (da, db) = (w.facs[a].district, w.facs[b].district);
            if da == k && db != k {
                *links.entry(db).or_default() += c;
            } else if db == k && da != k {
                *links.entry(da).or_default() += c;
            }
        }
        if !links.is_empty() {
            ui.separator();
            ui.label(RichText::new("Connected districts").color(TEAL));
            let mut v: Vec<_> = links.into_iter().collect();
            v.sort_by_key(|x| std::cmp::Reverse(x.1));
            for (d, c) in v {
                ui.label(RichText::new(format!("{c:>4}  {}", w.districts[d])).monospace().size(11.5));
            }
        }
    }
    ui.separator();
    ui.label(RichText::new("Folders").color(TEAL));
    for (fi, f) in facs {
        ui.horizontal(|ui| {
            let c = f.sections.iter().filter_map(|s| s.metrics.as_ref()).map(|m| m.cx_max).max().map_or(DIM, cx_color);
            color_dot(ui, c);
            if ui.link(RichText::new(&f.short).size(12.0)).clicked() {
                *act = Some(Act::GoFac(fi));
            }
            ui.label(RichText::new(format!("{} files", f.sections.len())).size(10.5).color(DIM));
        });
    }
}

fn continent_view(ui: &mut egui::Ui, c: &Continent, w: Option<&World>, servos: &[&Servo], current: bool, fly: &mut bool, stale_days: i64) {
    ui.label(RichText::new(&c.name).color(AMBER).strong());
    ui.label(RichText::new(&c.path).monospace().size(11.0).color(DIM));
    *fly = ui.button(if current { "Fit to view" } else { "Fly in" }).clicked();
    let Some(w) = w else {
        ui.label(RichText::new("Not scanned yet.").color(DIM));
        return;
    };
    let code = &w.facs[w.n_cells..];
    let files: Vec<&Section> = code.iter().flat_map(|f| &f.sections).collect();
    let loc: u32 = files.iter().filter_map(|s| s.metrics.as_ref()).map(|m| m.loc).sum();
    let hot = files.iter().flat_map(|s| &s.items).filter(|i| i.cx > 10).count();
    let stale = w.facs[..w.n_cells].iter().filter(|f| f.age > stale_days).count();
    egui::Grid::new("continent").num_columns(2).show(ui, |ui| {
        for (k, v) in [
            ("factories", code.len().to_string()), ("files", files.len().to_string()), ("lines of code", loc.to_string()),
            ("hotspots (cx > 10)", hot.to_string()), ("context cells", format!("{} ({stale} stale)", w.n_cells)), ("servitors", servos.len().to_string()),
        ] {
            ui.label(RichText::new(k).color(DIM));
            ui.label(RichText::new(v).monospace());
            ui.end_row();
        }
    });
    if !servos.is_empty() {
        ui.separator();
        for s in servos {
            ui.horizontal(|ui| {
                color_dot(ui, s.color);
                ui.label(RichText::new(&s.name).monospace());
                ui.label(RichText::new(if s.pending.is_empty() { s.state.as_str() } else { "waiting" }).color(if s.pending.is_empty() { DIM } else { RED }));
            });
        }
    }
}

enum ToolKind {
    Read,
    Build,
    Search,
    Shell,
    Web,
    Other,
}

fn tool_kind(t: &str) -> ToolKind {
    match t {
        "Read" | "NotebookRead" => ToolKind::Read,
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => ToolKind::Build,
        "Grep" | "Glob" | "LS" => ToolKind::Search,
        "Bash" => ToolKind::Shell,
        "WebFetch" | "WebSearch" => ToolKind::Web,
        _ => ToolKind::Other,
    }
}

#[derive(Default)]
struct FilterHits {
    conts: HashSet<usize>,
    districts: HashSet<usize>,
    subs: HashSet<usize>,
    facs: HashSet<usize>,
    secs: HashSet<(usize, usize)>,
}

impl FilterHits {
    /// A folder stays lit when it, one of its files, or its (sub-)district matches.
    fn fac_lit(&self, w: &World, fi: usize) -> bool {
        let f = &w.facs[fi];
        self.facs.contains(&fi) || self.districts.contains(&f.district) || self.subs.contains(&f.sub) || self.secs.iter().any(|s| s.0 == fi)
    }

    fn sec_lit(&self, w: &World, fi: usize, si: usize) -> bool {
        let f = &w.facs[fi];
        self.secs.contains(&(fi, si)) || self.facs.contains(&fi) || self.districts.contains(&f.district) || self.subs.contains(&f.sub)
    }
}

#[derive(Clone)]
enum Jump {
    AddProject(String),
    /// 0 rebuild and restart Cellworks, 1 rescan the project, 2 new servitor here
    Command(u8),
    Project(String),
    Fac(String, usize),
    Sec(String, usize, usize),
    Servo(String),
}

/// Subsequence match: consecutive characters and starts of path segments score higher.
fn fuzzy(q: &str, s: &str) -> Option<i32> {
    let mut qs = q.chars().filter(|c| !c.is_whitespace()).peekable();
    let (mut score, mut run, mut prev) = (0, 0, '/');
    for c in s.chars() {
        if qs.peek() == Some(&c) {
            qs.next();
            run += 1;
            score += run * 2 + if matches!(prev, '/' | '_' | '-' | '.' | ' ') { 8 } else { 0 };
        } else {
            run = 0;
        }
        prev = c;
    }
    if qs.peek().is_some() {
        return None;
    }
    let last = s.rsplit('/').next().unwrap_or(s);
    Some(score + if last.contains(q) { 40 } else { 0 } - s.len() as i32 / 4)
}

enum Act {
    Go(SecId),
    GoFac(usize),
    Trace(usize, usize, usize),
    File(String, Option<u32>),
    /// a code folder in whichever project holds it
    Code(String),
}

fn sec_label(w: &World, (f, s): SecId) -> String {
    let sec = &w.facs[f].sections[s];
    sec.rel.clone().unwrap_or_else(|| format!("{} › {}", w.facs[f].id, sec.title))
}

fn links_view(ui: &mut egui::Ui, w: &World, title: &str, links: &[Link], act: &mut Option<Act>) {
    if links.is_empty() {
        return;
    }
    ui.separator();
    ui.label(RichText::new(format!("{title} ({})", links.len())).color(AMBER));
    for l in links {
        ui.horizontal_wrapped(|ui| {
            let label = match l.to {
                (f, Some(s)) => sec_label(w, (f, s)),
                (f, None) => w.facs[f].id.clone(),
            };
            if ui.link(RichText::new(label).monospace().size(11.5)).clicked() {
                *act = Some(match l.to {
                    (f, Some(s)) => Act::Go((f, s)),
                    (f, None) => Act::GoFac(f),
                });
            }
            ui.label(RichText::new(&l.why).size(10.5).color(DIM));
        });
    }
}

/// Expandable import tree: what a file depends on (or, with `up`, what depends on it).
fn dep_tree(ui: &mut egui::Ui, w: &World, id: SecId, up: bool, path: &mut Vec<SecId>, act: &mut Option<Act>) {
    let s = &w.facs[id.0].sections[id.1];
    for &k in if up { &s.dependents } else { &s.deps } {
        let ks = &w.facs[k.0].sections[k.1];
        let label = RichText::new(ks.rel.clone().unwrap_or_default()).monospace().size(11.5);
        let kids = if up { ks.dependents.len() } else { ks.deps.len() };
        if kids == 0 || path.contains(&k) || path.len() > 8 {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                if ui.link(label).clicked() {
                    *act = Some(Act::Go(k));
                }
                if path.contains(&k) {
                    ui.label(RichText::new("↺ cycle").size(10.0).color(RED));
                }
            });
            continue;
        }
        let uid = ui.make_persistent_id((up, path.clone(), k));
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), uid, false)
            .show_header(ui, |ui| {
                if ui.link(label).clicked() {
                    *act = Some(Act::Go(k));
                }
                ui.label(RichText::new(format!("{kids}")).size(10.0).color(DIM));
            })
            .body(|ui| {
                path.push(k);
                dep_tree(ui, w, k, up, path, act);
                path.pop();
            });
    }
}

fn code_file_view(ui: &mut egui::Ui, w: &World, id: SecId, act: &mut Option<Act>) {
    let sec = &w.facs[id.0].sections[id.1];
    ui.label(RichText::new(&sec.title).color(AMBER).strong());
    ui.label(RichText::new(sec.rel.as_deref().unwrap_or("")).monospace().color(DIM));
    if ui.link("open file").clicked() {
        *act = Some(Act::File(sec.path.clone().unwrap_or_default(), None));
    }
    if let Some(m) = &sec.metrics {
        egui::Grid::new("metrics").num_columns(2).show(ui, |ui| {
            let avg = if m.funcs > 0 { m.cx_sum as f32 / m.funcs as f32 } else { 0.0 };
            for (k, v, c) in [
                ("language", m.lang.to_string(), TEXT), ("lines of code", m.loc.to_string(), TEXT), ("functions", m.funcs.to_string(), TEXT),
                ("max complexity", m.cx_max.to_string(), cx_color(m.cx_max)), ("avg complexity", format!("{avg:.1}"), cx_color(avg.round() as u32)),
                ("imports (fan-out)", sec.deps.len().to_string(), TEAL), ("imported by (fan-in)", sec.dependents.len().to_string(), ORANGE),
                ("calls out / in", format!("{} / {}", sec.calls_out.iter().map(|c| c.1).sum::<u32>(), sec.calls_in.iter().map(|c| c.1).sum::<u32>()), PURPLE),
            ] {
                ui.label(RichText::new(k).color(DIM));
                ui.label(RichText::new(v).color(c).monospace());
                ui.end_row();
            }
        });
    }
    links_view(ui, w, "Context", &sec.links, act);
    if !sec.items.is_empty() {
        ui.separator();
        egui::CollapsingHeader::new(RichText::new(format!("Functions by complexity ({})", sec.items.len())).color(AMBER)).default_open(true).show(ui, |ui| {
            for it in &sec.items {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:>3}", it.cx)).monospace().color(cx_color(it.cx)));
                    if ui.link(RichText::new(&it.term).monospace().size(11.5)).clicked() {
                        *act = Some(Act::File(sec.path.clone().unwrap_or_default(), Some(it.line)));
                    }
                    let used = if it.uses.is_empty() { String::new() } else { format!(" · called from {} files", it.uses.len()) };
                    ui.label(RichText::new(format!("L{} · {} lines{used}", it.line, it.loc)).size(10.0).color(DIM));
                });
            }
        });
    }
    for (title, up, n, c) in [("Depends on", false, sec.deps.len(), TEAL), ("Used by", true, sec.dependents.len(), ORANGE)] {
        ui.separator();
        ui.label(RichText::new(format!("{title} ({n})")).color(c));
        dep_tree(ui, w, id, up, &mut vec![id], act);
    }
    for (title, list) in [("Calls into", &sec.calls_out), ("Called from", &sec.calls_in)] {
        if list.is_empty() {
            continue;
        }
        ui.separator();
        ui.label(RichText::new(title).color(PURPLE));
        for &(t, c) in list {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{c:>3}×")).monospace().color(DIM));
                if ui.link(RichText::new(sec_label(w, t)).monospace().size(11.5)).clicked() {
                    *act = Some(Act::Go(t));
                }
            });
        }
    }
}

fn code_fac_view(ui: &mut egui::Ui, w: &World, fi: usize, act: &mut Option<Act>) {
    let f = &w.facs[fi];
    ui.label(RichText::new(format!("{}/", f.title)).color(AMBER).strong());
    let loc: u32 = f.sections.iter().filter_map(|s| s.metrics.as_ref()).map(|m| m.loc).sum();
    ui.label(RichText::new(format!("{} files · {loc} lines of code", f.sections.len())).color(DIM));
    let mut hot: Vec<(usize, &Item)> = f.sections.iter().enumerate().flat_map(|(si, s)| s.items.iter().map(move |i| (si, i))).filter(|(_, i)| i.cx > 0).collect();
    hot.sort_by_key(|(_, i)| std::cmp::Reverse(i.cx));
    if !hot.is_empty() {
        ui.separator();
        ui.label(RichText::new("Hotspots").color(AMBER));
        for (si, it) in hot.iter().take(6) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{:>3}", it.cx)).monospace().color(cx_color(it.cx)));
                if ui.link(RichText::new(format!("{} · {}", f.sections[*si].title, it.term)).monospace().size(11.5)).clicked() {
                    *act = Some(Act::File(f.sections[*si].path.clone().unwrap_or_default(), Some(it.line)));
                }
            });
        }
    }
    for (title, rows, c) in [
        ("Imports from", w.code_belts.iter().filter(|b| b.0 == fi).map(|b| (b.1, b.2)).collect::<Vec<_>>(), TEAL),
        ("Imported by", w.code_belts.iter().filter(|b| b.1 == fi).map(|b| (b.0, b.2)).collect(), ORANGE),
        ("Context cells", w.pipes.iter().filter(|p| p.1 == fi).map(|p| (p.0, p.2)).collect(), AMBER),
    ] {
        if rows.is_empty() {
            continue;
        }
        ui.separator();
        ui.label(RichText::new(title).color(c));
        for (other, n) in rows {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{n:>3}")).monospace().color(DIM));
                if ui.link(RichText::new(&w.facs[other].id).monospace().size(11.5)).clicked() {
                    *act = Some(Act::GoFac(other));
                }
            });
        }
    }
    ui.separator();
    for (si, s) in f.sections.iter().enumerate() {
        ui.horizontal(|ui| {
            let c = s.metrics.as_ref().map_or(DIM, |m| cx_color(m.cx_max));
            color_dot(ui, c);
            if ui.link(RichText::new(&s.title).monospace().size(11.5)).clicked() {
                *act = Some(Act::Go((fi, si)));
            }
        });
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(e) = self.rx.try_recv() {
            self.handle(e);
        }
        if let Some((path, g)) = self.git_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            if self.current.as_deref() == Some(&path) {
                self.git = g;
            }
            self.git_rx = None;
        }
        if self.git_rx.is_none() && self.git_at.elapsed().as_secs() >= 5 {
            if let Some(p) = self.current.clone() {
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let g = git_state(&p);
                    let _ = tx.send((p, g));
                });
                self.git_rx = Some(rx);
                self.git_at = Instant::now();
            }
        }
        let mut arrived = false;
        if let Some(w) = self.world_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.world_rx = None;
            if self.current.as_deref() == Some(&w.repo) {
                self.world = Some(w);
                self.fit_pending = true;
            } else if let Some(c) = self.continents.iter_mut().find(|c| c.path == w.repo) {
                c.world = Some(w);
            }
            arrived = true;
        }
        while let Ok(w) = self.universe_rx.try_recv() {
            if let Some(i) = self.continents.iter().position(|c| c.path == w.repo) {
                if i != self.cur || self.world.is_some() {
                    self.continents[i].world.get_or_insert(w);
                } else {
                    self.world = Some(w);
                    self.world_rx = None;
                    self.fit_pending = true;
                }
            }
            arrived = true;
        }
        if arrived {
            self.layout_universe();
        }
        if self.pop_focus.is_some() && ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !self.pop_rects.iter().any(|r| r.contains(p))) {
            self.pop_focus = None; // a click outside the editor windows gives the keyboard back
        }
        let editing = if self.nvim_focus { self.nvim.as_mut() } else { self.pop_focus.and_then(|i| self.popped.get_mut(i)) };
        if editing.as_ref().is_some_and(|n| n.mode == "normal") && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            // Esc with nothing left to cancel in the editor leaves it, like Esc anywhere else on the map
            self.nvim_focus = false;
            self.pop_focus = None;
        } else {
            if let Some(n) = editing {
                for ev in ctx.input(|i| i.events.clone()) {
                    match ev {
                        egui::Event::Text(t) => n.input(&t.replace('<', "<lt>")),
                        egui::Event::Key { key, pressed: true, modifiers, .. } => {
                            if let Some(k) = nvim::key_notation(key, modifiers) {
                                n.input(&k);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::K)) {
            self.palette_open = !self.palette_open;
            self.palette_q.clear();
            self.palette_sel = 0;
            self.palette_focus = true;
            self.nvim_focus = false;
            self.pop_focus = None;
        }
        if self.palette_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.palette_open = false;
            return;
        }
        if let Some((p, rel)) = self.pending_dir.clone().filter(|(p, _)| self.world.is_some() && !self.fit_pending && self.current.as_deref() == Some(p.as_str())) {
            // after the new project's first fit, which would otherwise move the camera away again
            self.pending_dir = None;
            let _ = p;
            let w = self.world.as_ref().unwrap();
            let file = w.facs.iter().enumerate().find_map(|(f, fac)| fac.sections.iter().position(|s| s.rel.as_deref().is_some_and(|r| r.eq_ignore_ascii_case(&rel))).map(|s| (f, s)));
            // the folder as the map spells it
            let rel = w.facs.iter().find(|f| f.id.len() >= rel.len() && f.id.is_char_boundary(rel.len()) && f.id[..rel.len()].eq_ignore_ascii_case(&rel)).map_or(rel.clone(), |f| f.id[..rel.len()].to_string());
            if let Some((f, s)) = file {
                self.goto_section(f, s); // close enough that the editor opens
            } else if !scope_members(w, &Scope::Path(rel.clone())).is_empty() {
                self.enter_scope(Scope::Path(rel));
            } else if let Some(f) = (0..w.facs.len()).filter(|&f| rel.to_lowercase().starts_with(&w.facs[f].id.to_lowercase())).max_by_key(|&f| w.facs[f].id.len()) {
                self.navigate_to(Sel::Fac(f)); // the folder is folded into a bigger one on this map
            }
        }
        let typing = ctx.egui_wants_keyboard_input() || self.nvim_focus || self.pop_focus.is_some() || self.palette_open || self.term_focus.is_some();
        self.sync_sessions(ctx);
        self.pump_terms();
        self.take_rescan();
        self.take_rebuild();
        // rescan once the servitors have been quiet for a few seconds (construction sites finish first)
        let building = self.world.as_ref().is_some_and(|w| w.facs.iter().any(|f| f.sections.iter().any(|s| s.build.is_some())));
        if self.rescan_due.is_some_and(|d| Instant::now() >= d) && !building {
            self.rescan(ctx);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::R)) {
            self.rescan(ctx);
        }
        if let Some(id) = self.term_focus.clone() {
            // a click outside every terminal window gives the keyboard back to the map
            if ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !self.term_rects.iter().any(|r| r.contains(p))) {
                self.term_focus = None;
            } else if let Some(t) = self.terms.get_mut(&id) {
                let app_cursor = t.parser.screen().application_cursor();
                let bracketed = t.parser.screen().bracketed_paste();
                for ev in ctx.input(|i| i.events.clone()) {
                    match ev {
                        egui::Event::Text(text) => t.input(text.as_bytes()),
                        egui::Event::Paste(text) => {
                            if bracketed {
                                t.input(format!("\x1b[200~{text}\x1b[201~").as_bytes());
                            } else {
                                t.input(text.as_bytes());
                            }
                        }
                        egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.command => {
                            if let Some(b) = term::key_bytes(key, modifiers, app_cursor) {
                                t.input(&b);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if std::mem::take(&mut self.attention) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
        }
        if !typing && ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
            self.filter_focus = true;
        }
        let (back, fwd) = ctx.input(|i| (
            i.pointer.button_pressed(egui::PointerButton::Extra1) || (!typing && i.modifiers.command && i.key_pressed(egui::Key::ArrowLeft)),
            i.pointer.button_pressed(egui::PointerButton::Extra2) || (!typing && i.modifiers.command && i.key_pressed(egui::Key::ArrowRight)),
        ));
        if back {
            self.go_back(ctx);
        }
        if fwd {
            self.go_forward(ctx);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::OpenBracket)) {
            self.left_open = !self.left_open;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::CloseBracket)) {
            self.right_open = !self.right_open;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !typing {
            if self.viewer.is_some() {
                self.viewer = None;
            } else if let Some((g, z)) = self.scope_frame.filter(|&(g, z)| self.goal_zoom > z * 1.25 || (self.goal - g).length() * z > 40.0) {
                // zoomed into the folder view: back out to the whole view with its ring first
                self.record();
                self.sel = Sel::None;
                self.lock = false;
                self.goal = g;
                self.goal_zoom = z;
            } else if self.scope.is_some() {
                self.leave_scope();
            } else if self.trace.is_none() && self.sel != Sel::None {
                self.sel = Sel::None;
                self.refocus();
            } else if self.trace.is_none() && self.project_locked {
                // out of the project: the other projects come back around it
                self.project_locked = false;
                self.overview();
                self.goal_zoom *= 0.5;
            } else if self.trace.take().is_none() {
                match self.back.pop() {
                    Some((g, z)) => {
                        self.goal = g;
                        self.goal_zoom = z;
                    }
                    None => self.overview(),
                }
            }
            self.follow = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F)) && !typing {
            if let Sel::Servo(id) = &self.sel {
                self.follow = Some(id.clone());
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (mut l, mut r) = (self.left_open, self.right_open);
        egui::Panel::left("projects").default_size(230.0).show_collapsible(ui, &mut l, |ui| self.projects_panel(ui));
        egui::Panel::right("side").default_size(400.0).show_collapsible(ui, &mut r, |ui| self.side_panel(ui));
        (self.left_open, self.right_open) = (l, r);
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.draw_world(ui));
        self.viewer_window(&ui.ctx().clone());
        self.term_windows(&ui.ctx().clone());
        self.editor_windows(&ui.ctx().clone());
        self.settings_window(&ui.ctx().clone());
        self.view_window(&ui.ctx().clone());
        if self.settings != self.settings_saved {
            if let Err(e) = self.settings.save() {
                self.log(format!("settings not saved: {e}"));
            }
            self.settings_saved = self.settings.clone();
        }
    }
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--claude-smoke") && args.len() > 2 {
        // check: the real CLI in a host, rendered by the terminal parser
        let id = uuid::Uuid::new_v4().to_string();
        term::spawn(&id, &args[2], &["claude".into(), "--session-id".into(), id.clone()]).expect("host");
        let mut t = term::Term::attach(&id, &args[2], egui::Context::default()).expect("attach");
        t.resize(100, 30);
        std::thread::sleep(std::time::Duration::from_secs(6));
        t.pump();
        for l in t.parser.screen().contents().lines().filter(|l| !l.trim().is_empty()) {
            println!("| {l}");
        }
        term::kill(&id);
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("--term-smoke") {
        // check: host + attach + question detection + input + detach/re-attach replay
        let id = format!("smoke-{}", std::process::id());
        let script = "printf 'Do you want to proceed?\\n 1. Yes\\n 2. No\\n'; read x; echo answered:$x; sleep 3".to_string();
        term::spawn(&id, "/tmp", &["sh".into(), "-c".into(), script]).expect("host");
        let mut t = term::Term::attach(&id, "/tmp", egui::Context::default()).expect("attach");
        std::thread::sleep(std::time::Duration::from_millis(500));
        t.pump();
        println!("question: {:?}", t.waiting_question());
        t.input(b"yes\r");
        std::thread::sleep(std::time::Duration::from_millis(500));
        t.pump();
        drop(t);
        let mut again = term::Term::attach(&id, "/tmp", egui::Context::default()).expect("re-attach");
        std::thread::sleep(std::time::Duration::from_millis(300));
        again.pump();
        let text = again.parser.screen().contents();
        println!("after re-attach: {:?}", text.lines().filter(|l| !l.trim().is_empty()).collect::<Vec<_>>());
        println!("listed: {}", term::list().iter().any(|m| m.id == id));
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("--host") && args.len() > 4 {
        // a detachable session host (see term.rs), started by the app itself
        if let Err(e) = term::run_host(&args[2], &args[3], &args[4..]) {
            eprintln!("host: {e}");
        }
        return Ok(());
    }
    if let (Some("--nvim-smoke"), Some(file)) = (std::env::args().nth(1).as_deref(), std::env::args().nth(2)) {
        let mut n = nvim::Nvim::spawn(&file, Some(3), 70, 8, egui::Context::default()).expect("nvim");
        std::thread::sleep(std::time::Duration::from_millis(1500));
        n.pump();
        n.input("ggdd");
        std::thread::sleep(std::time::Duration::from_millis(400));
        n.pump();
        for row in &n.cells {
            println!("|{}|", row.iter().map(|c| c.0.as_str()).collect::<String>());
        }
        println!("cursor {:?} · {} highlight groups · fg {:06x} bg {:06x}", n.cursor, n.hl.len(), n.fg, n.bg);
        return Ok(());
    }
    if let (Some("--jira-sync"), Some(path)) = (std::env::args().nth(1).as_deref(), std::env::args().nth(2)) {
        let (site, key) = jira::parse(&path).expect("jira:<site>/<KEY>");
        match jira::sync(site, key, &|n| eprintln!("{n} tickets")) {
            Ok(n) => {
                let w = scan(&path, &[]);
                println!("{n} tickets -> {} folders in {} districts: {}", w.facs.len(), w.districts.len(), w.districts.join(", "));
            }
            Err(e) => println!("sync failed: {e:#}"),
        }
        return Ok(());
    }
    if let (Some("--scan"), Some(dir)) = (std::env::args().nth(1).as_deref(), std::env::args().nth(2)) {
        let t = Instant::now();
        let w = scan(&dir, &settings::Settings::load().ignored_folders);
        let secs: Vec<&Section> = w.facs.iter().flat_map(|f| &f.sections).collect();
        let parsed = secs.iter().filter(|s| s.metrics.is_some()).count();
        let deps: usize = secs.iter().map(|s| s.deps.len()).sum();
        let calls: usize = secs.iter().map(|s| s.calls_out.len()).sum();
        let links: usize = secs.iter().filter(|s| s.rel.is_some()).map(|s| s.links.len()).sum();
        println!("{:?} · {} factories · {} files ({parsed} parsed) · {deps} imports · {calls} call edges · {links} code→cell links · {} code belts · {} pipes",
            t.elapsed(), w.facs.len(), secs.iter().filter(|s| s.rel.is_some()).count(), w.code_belts.len(), w.pipes.len());
        println!("  cells: {}", w.facs[..w.n_cells].iter().map(|f| format!("{} ({} sections, {} terms, {}d old)", f.title, f.sections.len(), f.sections.iter().map(|s| s.items.len()).sum::<usize>(), f.age)).take(6).collect::<Vec<_>>().join(" | "));
        println!("  districts: {}", w.districts.iter().enumerate().map(|(d, n)| format!("{n} ({})", w.facs.iter().filter(|f| f.district == d).count())).collect::<Vec<_>>().join(", "));
        println!("  subdistricts: {}", w.subdistricts.iter().enumerate().map(|(k, (d, n))| format!("{}/{n} ({})", w.districts[*d], w.facs.iter().filter(|f| f.sub == k).count())).collect::<Vec<_>>().join(", "));
        println!("  small: {}", w.facs.iter().filter(|f| f.sections.len() <= 2).take(12).map(|f| format!("{:?}", f.short)).collect::<Vec<_>>().join(" "));
        println!("  labels: {}", w.facs.iter().take(8).map(|f| f.short.as_str()).collect::<Vec<_>>().join(" | "));
        let mut hot: Vec<(&Section, &world::Item)> = secs.iter().flat_map(|s| s.items.iter().map(move |i| (*s, i))).filter(|(s, _)| s.rel.is_some()).collect();
        hot.sort_by_key(|(_, i)| std::cmp::Reverse(i.cx));
        for (s, i) in hot.iter().take(3) {
            println!("  hotspot {} {} cx {}", s.rel.as_deref().unwrap_or(""), i.term, i.cx);
        }
        for s in secs.iter().filter(|s| s.metrics.is_some() && !s.links.is_empty()).take(3) {
            println!("  {} → {}", s.rel.as_deref().unwrap(), s.links.iter().map(|l| l.why.as_str()).collect::<Vec<_>>().join(", "));
        }
        return Ok(());
    }
    let opts = eframe::NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([1600.0, 1000.0]).with_title("Cellworks"), ..Default::default() };
    eframe::run_native("Cellworks", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

#[cfg(test)]
mod tests {
    use super::{Scope, fuzzy, layout_scope, scope_members, scope_ring};

    #[test]
    #[ignore] // needs ~/Projects/Accounts/acme/shopfront-api
    fn a_manifest_is_on_the_map() {
        let repo = format!("{}/Projects/Accounts/acme/shopfront-api", std::env::var("HOME").unwrap());
        let mut w = crate::world::scan(&repo, &crate::settings::Settings::default().ignored_folders);
        let rel = "services/station/manifests/acme-production-v2/deployment.yaml";
        let hit = w.facs.iter().enumerate().find_map(|(f, fac)| fac.sections.iter().position(|s| s.rel.as_deref() == Some(rel)).map(|s| (f, s)));
        println!("manifest section: {hit:?}");
        let (f, s) = hit.expect("manifest on the map");
        let sc = Scope::Path(w.facs[f].id.clone());
        println!("its folder view: {sc:?}, rect before layout {:?}", w.facs[f].sections[s].rect);
        let members = scope_members(&w, &sc);
        layout_scope(&mut w, &sc, &members);
        println!("rect after layout {:?}, factory {:?}", w.facs[f].sections[s].rect, w.facs[f].rect);
    }

    #[test]
    #[ignore] // needs ~/Projects/Accounts/acme/shopfront-api
    fn a_service_folder_view_is_not_empty() {
        // "show code" opens Scope::Path(<service folder relative to the project>) on the project holding it
        let repo = format!("{}/Projects/Accounts/acme/shopfront-api", std::env::var("HOME").unwrap());
        let w = crate::world::scan(&repo, &[]);
        let members = scope_members(&w, &Scope::Path("services/api-gateway".into()));
        println!("services/api-gateway: {} folders, e.g. {:?}", members.len(), members.first().map(|&f| &w.facs[f].id));
        assert!(!members.is_empty());
    }

    #[test]
    #[ignore] // needs ~/Projects/Accounts/acme
    fn services_view_is_navigable() {
        let ignore: Vec<String> = crate::settings::DEFAULT_IGNORED.iter().map(|s| s.to_string()).collect();
        let mut w = crate::world::scan(&format!("{}/Projects/Accounts/acme", std::env::var("HOME").unwrap()), &ignore);
        for _ in 0..600 {
            w.step(1.0 / 60.0);
        }
        for path in ["shopfront-api", "shopfront-api/services"] {
            let sc = Scope::Path(path.into());
            let members = scope_members(&w, &sc);
            layout_scope(&mut w, &sc, &members);
            let overlaps = (0..members.len()).flat_map(|i| (i + 1..members.len()).map(move |j| (i, j))).filter(|&(i, j)| w.facs[members[i]].rect.intersects(w.facs[members[j]].rect)).count();
            let boxes = super::scope_groups(&w, &sc, &members).len();
            let (ring, _) = scope_ring(&w, &members, false, [true, false, true], 1);
            println!("{path}: {} folders in {boxes} boxes, {overlaps} overlaps, {} outside folders", members.len(), ring.len());
            assert_eq!(overlaps, 0);
        }
    }

    #[test]
    #[ignore] // needs ~/Projects/TheFactory/The-Factory
    fn folder_view_ring_holds_the_outside_connections() {
        let mut w = crate::world::scan(&format!("{}/Projects/TheFactory/The-Factory", std::env::var("HOME").unwrap()), &[]);
        for _ in 0..600 {
            w.step(1.0 / 60.0);
        }
        let sc = w.facs.iter().position(|f| f.id == "app/src/domain").unwrap();
        let (ring, links) = scope_ring(&w, &[sc], true, [true, true, true], 1);
        let folder = w.facs[sc].rect;
        println!("domain/: {} outside files, {} links, from {} folders", ring.len(), links.len(), ring.iter().map(|r| r.0 .0).collect::<std::collections::HashSet<_>>().len());
        assert!(!ring.is_empty());
        assert!(ring.iter().all(|(e, r, _)| e.0 != sc && !r.intersects(folder)));
        for i in 0..ring.len() {
            for j in i + 1..ring.len() {
                assert!(!ring[i].1.intersects(ring[j].1), "ring files overlap");
            }
        }
        // every link points at a file that is on the ring
        assert!(links.iter().all(|l| ring.iter().any(|r| r.0 == l.0)));
        // a folder view of app/: tidy boxes without overlaps, a ring of outside folders
        let sc = Scope::Path("app".into());
        let members = scope_members(&w, &sc);
        layout_scope(&mut w, &sc, &members);
        for i in 0..members.len() {
            for j in i + 1..members.len() {
                assert!(!w.facs[members[i]].rect.intersects(w.facs[members[j]].rect), "folders overlap in the view");
            }
        }
        let (ring, _) = scope_ring(&w, &members, false, [true, false, true], 1);
        println!("app/: {} folders inside, {} outside folders on the ring", members.len(), ring.len());
        assert!(ring.iter().all(|(e, _, n)| !members.contains(&e.0) && e.1 == usize::MAX && *n >= 1));
    }

    #[test]
    fn fuzzy_prefers_file_names_and_segment_starts() {
        assert!(fuzzy("jts", "app/src/domain/journey.ts").is_some());
        assert!(fuzzy("xyz", "app/src/domain/journey.ts").is_none());
        let a = fuzzy("journey", "app/src/domain/journey.ts").unwrap();
        let b = fuzzy("journey", "app/src/ui/journey-shell/a-very-long-path/thing.ts").unwrap();
        assert!(a > b, "{a} vs {b}");
        assert!(fuzzy("sca", "app/src/adapters/sca.ts").unwrap() > fuzzy("sca", "app/src/ui/sca-wait.component.ts").unwrap());
    }
}
