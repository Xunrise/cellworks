//! The AgentSessions bounded context: discovering sessions from transcripts,
//! placing them on the graph, and creating and steering them.
//!
//! Defined by docs/domain/contexts/AgentSessions/AgentSessions.md.

mod checkout;
mod claude;
mod transcript;
pub mod ui;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use cellworks_map::ProjectMap;
use egui::Pos2;

pub use claude::{Agent, AgentList, ClaudeCli, ClaudeUnavailable};

/// One file read or write a session made, placed relative to its own
/// checkout so the open project's own city can decide whether it is in it.
#[derive(Clone, Debug, PartialEq)]
pub struct FileEvent {
    pub write: bool,
    /// The checkout's git common dir (shared by every worktree of one
    /// repository); `None` when the checkout is not a git repository.
    pub checkout: Option<PathBuf>,
    /// Relative to its own checkout root when `checkout` is `Some`;
    /// otherwise the absolute path (a path prefix then decides whether it's
    /// in a given project's city).
    pub path: PathBuf,
    pub line: Option<usize>,
    pub added: usize,
}

/// A session's state, read off `claude agents --json`'s `status` and
/// `state`: `state` waiting or blocked is [`SessionState::Waiting`];
/// otherwise `status` busy or `state` working is [`SessionState::Working`];
/// else [`SessionState::Idle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionState {
    Waiting,
    Working,
    Idle,
}

/// One running Claude Code session, as the watch has assembled it from
/// `claude agents --json` and its transcript.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub cwd: PathBuf,
    pub status: String,
    pub state: String,
    pub repo: String,
    pub branch: String,
    pub summary: String,
    pub last_active: Option<u64>,
    /// The last 50 events across every checkout it has touched, oldest
    /// first.
    pub events: Vec<FileEvent>,
}

impl Session {
    pub fn session_state(&self) -> SessionState {
        if self.state == "waiting" || self.state == "blocked" {
            SessionState::Waiting
        } else if self.status == "busy" || self.state == "working" {
            SessionState::Working
        } else {
            SessionState::Idle
        }
    }
}

/// What a background watch reports to the UI thread, once per drain.
#[derive(Clone, Debug)]
pub enum SessionEvent {
    Listed(Vec<Session>),
    ListingFailed(String),
}

/// Where a watch reads sessions from, and how often it polls: passed in so
/// a test can replace both without reading `$HOME` or running `claude`.
pub struct SessionSources {
    pub transcripts: PathBuf,
    pub agents: Arc<dyn AgentList + Send + Sync>,
    pub poll: Duration,
}

impl SessionSources {
    /// `~/.claude/projects` and the real `claude` CLI, polled every 2s.
    /// `None` with no `$HOME`.
    pub fn for_this_user() -> Option<SessionSources> {
        let home = std::env::var("HOME").ok()?;
        Some(SessionSources {
            transcripts: PathBuf::from(home).join(".claude/projects"),
            agents: Arc::new(crate::ClaudeCli),
            poll: Duration::from_secs(2),
        })
    }
}

/// Per-session state the watch keeps alive across polls, so a transcript is
/// tailed incrementally rather than re-read from scratch every cycle.
struct SessionCache {
    /// One tail per transcript file this session has (the main
    /// `<id>.jsonl`, plus any `subagents/*.jsonl`).
    tails: HashMap<PathBuf, transcript::Tail>,
    /// The last 50 events, oldest first.
    events: Vec<FileEvent>,
    /// The last assistant text seen so far, folded forward over each poll's
    /// new lines - so no poll re-parses history it has already read.
    summary: String,
    /// The newest timestamp seen so far, folded the same way.
    last_active: Option<u64>,
}

impl SessionCache {
    fn new() -> SessionCache {
        SessionCache {
            tails: HashMap::new(),
            events: Vec::new(),
            summary: String::new(),
            last_active: None,
        }
    }
}

/// The open project's own git common dir at `root`; `None` when it is not a
/// git repository. For a caller outside this crate (the binary) that needs
/// to pass `common_dir` to `place`/`groups`/`SessionsView` methods.
pub fn common_dir(root: &Path) -> Option<PathBuf> {
    checkout::checkout_root(root).and_then(|checkout_root| checkout::common_dir(&checkout_root))
}

/// A `cwd`'s repository identity, read from git's own files once and reused
/// for every session sharing that `cwd`: its branch, git common dir (`None`
/// when it is not a git checkout) and display name.
type CheckoutInfo = (String, Option<PathBuf>, String);

/// Starts watching every running session on the machine on a background
/// thread; never blocks the caller.
pub fn start_watch(sources: SessionSources) -> Receiver<SessionEvent> {
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let mut sessions: HashMap<String, SessionCache> = HashMap::new();
        let mut checkouts: HashMap<PathBuf, CheckoutInfo> = HashMap::new();

        loop {
            let listed = match sources.agents.list() {
                Err(ClaudeUnavailable(message)) => tx.send(SessionEvent::ListingFailed(message)),
                Ok(agents) => {
                    let built = agents
                        .iter()
                        .map(|agent| build_session(agent, &sources, &mut sessions, &mut checkouts))
                        .collect();
                    // A session that has ended is no longer listed, so its
                    // cache (its tails above all) is dropped rather than
                    // kept for the lifetime of the app.
                    sessions.retain(|id, _| agents.iter().any(|agent| &agent.session_id == id));
                    tx.send(SessionEvent::Listed(built))
                }
            };
            if listed.is_err() {
                return;
            }
            std::thread::sleep(sources.poll);
        }
    });

    rx
}

/// Looks up (and caches by `cwd`) the repository identity for `agent`'s
/// `cwd`, then assembles the [`Session`] the sidebar and the city need:
/// tailing every transcript file this session has, resolving its new tool
/// calls to [`FileEvent`]s kept to the last 50, and folding `summary` and
/// `last_active` forward over the lines that are new this poll - no poll
/// holds or re-parses transcript text it has already read.
fn build_session(
    agent: &Agent,
    sources: &SessionSources,
    sessions: &mut HashMap<String, SessionCache>,
    checkouts: &mut HashMap<PathBuf, CheckoutInfo>,
) -> Session {
    let cwd = PathBuf::from(&agent.cwd);

    let (branch, _common, repo) = checkouts
        .entry(cwd.clone())
        .or_insert_with(|| {
            let checkout_root = checkout::checkout_root(&cwd);
            let common = checkout_root.as_deref().and_then(checkout::common_dir);
            let branch = checkout_root
                .as_deref()
                .map(checkout::branch)
                .unwrap_or_default();
            let repo =
                checkout::repo_name(checkout_root.as_deref().unwrap_or(&cwd), common.as_deref());
            (branch, common, repo)
        })
        .clone();

    let cache = sessions
        .entry(agent.session_id.clone())
        .or_insert_with(SessionCache::new);

    for file in transcript_files(&sources.transcripts, &agent.session_id) {
        // `Tail::new` (not `.or_default()`) keeps transcript.rs's own
        // constructor live in production code too, not only in its tests.
        #[allow(clippy::unwrap_or_default)]
        let tail = cache
            .tails
            .entry(file.clone())
            .or_insert_with(transcript::Tail::new);
        let new_lines = tail.read(&file);
        if new_lines.is_empty() {
            continue;
        }
        for tool_call in transcript::tool_events(&new_lines) {
            cache.events.push(resolve_event(&tool_call));
        }
        let (said, newest) = transcript::summary(&new_lines);
        if let Some(said) = said {
            cache.summary = said;
        }
        cache.last_active = cache.last_active.max(newest);
    }
    let cut = cache.events.len().saturating_sub(50);
    cache.events.drain(..cut);

    Session {
        id: agent.session_id.clone(),
        name: if agent.name.is_empty() {
            agent.session_id.clone()
        } else {
            agent.name.clone()
        },
        cwd,
        status: agent.status.clone(),
        state: agent.state.clone(),
        repo,
        branch,
        summary: cache.summary.clone(),
        last_active: cache.last_active,
        events: cache.events.clone(),
    }
}

/// Every transcript file `session_id` has under `transcripts_root`: the main
/// `<project-dir>/<session_id>.jsonl`, plus every
/// `<project-dir>/<session_id>/subagents/*.jsonl`, across every immediate
/// project subdirectory. Tolerant of any of it not existing.
fn transcript_files(transcripts_root: &Path, session_id: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for project_dir in std::fs::read_dir(transcripts_root)
        .into_iter()
        .flatten()
        .flatten()
    {
        let project_path = project_dir.path();
        if !project_path.is_dir() {
            continue;
        }

        let main = project_path.join(format!("{session_id}.jsonl"));
        if main.is_file() {
            files.push(main);
        }

        let subagents = project_path.join(session_id).join("subagents");
        for entry in std::fs::read_dir(&subagents)
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }
    files
}

/// Resolves one tool call to a [`FileEvent`], relative to its own checkout
/// (which may differ from the session's `cwd`'s checkout in degenerate
/// cases, so it is resolved from the touched path itself).
fn resolve_event(tool_call: &transcript::ToolCall) -> FileEvent {
    let event_checkout_root = checkout::checkout_root(&tool_call.path);
    let (checkout, path) = match event_checkout_root
        .as_deref()
        .and_then(checkout::common_dir)
    {
        Some(common) => {
            let root = event_checkout_root.as_deref().expect("just resolved above");
            let relative = tool_call
                .path
                .strip_prefix(root)
                .unwrap_or(&tool_call.path)
                .to_owned();
            (Some(common), relative)
        }
        None => (None, tool_call.path.clone()),
    };
    FileEvent {
        write: tool_call.write,
        checkout,
        path,
        line: tool_call.line,
        added: tool_call.added,
    }
}

/// Where a session's latest placeable event stands in a city.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub tower: PathBuf,
    pub centre: Pos2,
    pub z: f32,
    pub writing: bool,
    pub added: usize,
}

/// Where `session`'s latest event lands in the city at `map`, given the
/// open project's own git common dir (`None` when it is not a git
/// repository, in which case a path prefix against `map.root` decides which
/// events are in this city): the smallest item whose floors hold the
/// event's line; without a line, the file's first function; else the file's
/// node. `None` when it has touched no file this city holds.
pub fn place(map: &ProjectMap, common_dir: Option<&Path>, session: &Session) -> Option<Placement> {
    session.events.iter().rev().find_map(|event| {
        let node = city_node(event, map, common_dir)?;
        tower_of(map, &node, event)
    })
}

/// The node in `map`'s own tree that `event` touches, when `event` is in
/// this city: its checkout shares `common_dir` (any worktree), or, for a
/// project that is not a git repository, its path sits under `map.root`.
/// `None` when `event` is not in this city at all.
fn city_node(event: &FileEvent, map: &ProjectMap, common_dir: Option<&Path>) -> Option<PathBuf> {
    let relative: &Path = match (&event.checkout, common_dir) {
        (Some(checkout), Some(open)) if checkout == open => &event.path,
        (None, None) => event.path.strip_prefix(&map.root).ok()?,
        _ => return None,
    };
    Some(map.root.join(relative))
}

/// Whether `event` lands anywhere in `map`'s city at all, by the same rule
/// [`place`] uses to walk a session's events.
fn in_city_event(event: &FileEvent, map: &ProjectMap, common_dir: Option<&Path>) -> bool {
    city_node(event, map, common_dir).is_some()
}

/// `event`'s landing spot on `node`, the three-way chain of design.md's
/// requirement 5: with a line, the smallest item among `node`'s own items
/// whose floors hold it; without a line, the file's first function; else
/// `node` itself, as a plain node with no climbable height - a line no item
/// covers (a top-level `const`, an `impl` header, a blank line) says only
/// "somewhere in this file", so it must not claim the first function.
/// `None` when `map` holds neither an item nor a node at `node`.
fn tower_of(map: &ProjectMap, node: &Path, event: &FileEvent) -> Option<Placement> {
    if let Some(items) = map.items_of(node) {
        let item = match event.line {
            Some(line) => items
                .iter()
                .filter(|item| item.floors.iter().any(|floor| floor.number == line))
                .min_by_key(|item| item.floors.len()),
            None => items
                .iter()
                .find(|item| item.kind == cellworks_map::ItemKind::Function),
        };
        if let Some(item) = item {
            return Some(Placement {
                tower: item.path.clone(),
                centre: item.area.center(),
                z: cellworks_map::city::height(item),
                writing: event.write,
                added: event.added,
            });
        }
    }

    let found = map.find(node)?;
    Some(Placement {
        tower: node.to_owned(),
        centre: found.area.center(),
        z: 0.0,
        writing: event.write,
        added: event.added,
    })
}

/// The sidebar's groups for `query` in `map`'s city (`common_dir` as for
/// [`place`]), empty groups left out, each newest-`last_active`-first:
/// "Needs you", "Working", "Idle" (sessions with an event in this city),
/// then "Other projects" (every other session).
pub fn groups<'a>(
    sessions: &'a [Session],
    map: &ProjectMap,
    common_dir: Option<&Path>,
    query: &str,
) -> Vec<(&'static str, Vec<&'a Session>)> {
    let query = query.to_lowercase();
    let matches = |session: &Session| {
        query.is_empty()
            || session.name.to_lowercase().contains(&query)
            || session.repo.to_lowercase().contains(&query)
            || session.branch.to_lowercase().contains(&query)
    };

    let mut needs_you = Vec::new();
    let mut working = Vec::new();
    let mut idle = Vec::new();
    let mut other = Vec::new();

    for session in sessions {
        if !matches(session) {
            continue;
        }
        let in_city = session
            .events
            .iter()
            .any(|event| in_city_event(event, map, common_dir));
        if in_city {
            match session.session_state() {
                SessionState::Waiting => needs_you.push(session),
                SessionState::Working => working.push(session),
                SessionState::Idle => idle.push(session),
            }
        } else {
            other.push(session);
        }
    }

    let newest_first = |v: &mut Vec<&'a Session>| {
        v.sort_by_key(|session| std::cmp::Reverse(session.last_active));
    };
    newest_first(&mut needs_you);
    newest_first(&mut working);
    newest_first(&mut idle);
    newest_first(&mut other);

    [
        ("Needs you", needs_you),
        ("Working", working),
        ("Idle", idle),
        ("Other projects", other),
    ]
    .into_iter()
    .filter(|(_, members)| !members.is_empty())
    .collect()
}

/// For each path in `towers` (one per session standing there): its index
/// among those at the same tower, and how many stand there, in the order
/// given.
pub fn fan(towers: &[PathBuf]) -> Vec<(usize, usize)> {
    towers
        .iter()
        .enumerate()
        .map(|(i, tower)| {
            let same = |t: &&PathBuf| *t == tower;
            (
                towers[..i].iter().filter(same).count(),
                towers.iter().filter(same).count(),
            )
        })
        .collect()
}

/// "now" (< 60s), "N min" (< 1h), "N h" (< 1d), "N d"; "-" with no
/// timestamp.
pub fn ago(since: Option<u64>, now: u64) -> String {
    let Some(since) = since else {
        return "-".into();
    };
    let s = now.saturating_sub(since);
    match s {
        0..60 => "now".into(),
        60..3600 => format!("{} min", s / 60),
        3600..86400 => format!("{} h", s / 3600),
        _ => format!("{} d", s / 86400),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use cellworks_map::{
        Declaration, Directories, Floor, Item, ItemKind, ProjectMap, default_prefix_rule,
    };

    use super::*;

    fn floor(number: usize) -> Floor {
        Floor {
            number,
            text: String::new(),
            indent: 0,
            width: 20,
            tokens: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
        }
    }

    fn function(path: PathBuf, lines: std::ops::RangeInclusive<usize>) -> Item {
        Item {
            name: path
                .file_name()
                .expect("a named item")
                .to_string_lossy()
                .into_owned(),
            path,
            kind: ItemKind::Function,
            tests: false,
            floors: lines.map(floor).collect(),
            area: egui::Rect::NOTHING,
            base: 0.0,
            declaration: Declaration::default(),
        }
    }

    fn uses(path: PathBuf) -> Item {
        Item {
            kind: ItemKind::Uses,
            ..function(path, 1..=1)
        }
    }

    /// A map of one file "catalog/pricing.rs" holding a `use` block on line
    /// 1, a function "big" on lines 2 to 21, and a function "small" nested
    /// inside its line range, on lines 10 to 13, in source order.
    fn shop_map() -> ProjectMap {
        let root = PathBuf::from("/p");
        let file = root.join("catalog/pricing.rs");
        let mut items = BTreeMap::new();
        items.insert(
            PathBuf::from("catalog/pricing.rs"),
            vec![
                uses(file.join("uses")),
                function(file.join("big"), 2..=21),
                function(file.join("small"), 10..=13),
            ],
        );
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        ProjectMap::of_code(
            "shop",
            &root,
            &[PathBuf::from("catalog/pricing.rs")],
            &strategy,
            items,
            &BTreeMap::new(),
        )
    }

    fn session_at(events: Vec<FileEvent>) -> Session {
        Session {
            id: "s1".to_owned(),
            name: "pricing-fix".to_owned(),
            cwd: PathBuf::new(),
            status: "busy".to_owned(),
            state: "working".to_owned(),
            repo: "shop".to_owned(),
            branch: "main".to_owned(),
            summary: String::new(),
            last_active: None,
            events,
        }
    }

    fn read_event(path: PathBuf, line: Option<usize>) -> FileEvent {
        FileEvent {
            write: false,
            checkout: None,
            path,
            line,
            added: 0,
        }
    }

    #[test]
    fn a_session_stands_at_the_smallest_tower_holding_its_line() {
        let map = shop_map();
        let file = map.root.join("catalog/pricing.rs");
        let small = file.join("small");
        let big = file.join("big");

        // The smallest item whose floors hold the line wins over the wider
        // one that also covers it.
        let on_small = session_at(vec![read_event(file.clone(), Some(11))]);
        let placed = place(&map, None, &on_small).expect("line 11 lands in the city");
        let small_item = map.item(&small).expect("the small item is on the map");
        assert_eq!(placed.tower, small);
        assert_eq!(placed.centre, small_item.area.center());
        assert_eq!(placed.z, cellworks_map::city::height(small_item));
        assert!(!placed.writing);
        assert_eq!(placed.added, 0);

        // Without a line, the file's first function - not its `use` block.
        let no_line = session_at(vec![read_event(file.clone(), None)]);
        let placed = place(&map, None, &no_line).expect("a lineless event still lands");
        assert_eq!(placed.tower, big, "the first function, not the use block");

        // With a line no item covers (a top-level const, an impl header, a
        // blank line), the file's node itself - the third case of
        // requirement 5, not the first-function fallback of the second.
        let uncovered = session_at(vec![read_event(file.clone(), Some(150))]);
        let placed = place(&map, None, &uncovered).expect("the file is still in this city");
        let file_node = map.find(&file).expect("the file's node is on the map");
        assert_eq!(
            placed.tower, file,
            "the file's node, not its first function: line 150 is on no item"
        );
        assert_eq!(placed.centre, file_node.area.center());
        assert_eq!(placed.z, 0.0, "a plain node has no climbable height");

        // A file this city does not hold places nowhere.
        let elsewhere = session_at(vec![read_event(
            PathBuf::from("/elsewhere/file.rs"),
            Some(1),
        )]);
        assert_eq!(place(&map, None, &elsewhere), None);

        // A later event that cannot be placed leaves the session at the
        // last tower it could place.
        let lands_then_leaves = session_at(vec![
            read_event(file.clone(), Some(3)),
            read_event(PathBuf::from("/elsewhere/file.rs"), Some(1)),
        ]);
        let placed = place(&map, None, &lands_then_leaves)
            .expect("a file the city lacks leaves it at the last placed tower");
        assert_eq!(placed.tower, big);
    }

    fn session_full(
        id: &str,
        status: &str,
        state: &str,
        last_active: Option<u64>,
        repo: &str,
        branch: &str,
        events: Vec<FileEvent>,
    ) -> Session {
        Session {
            id: id.to_owned(),
            name: id.to_owned(),
            cwd: PathBuf::new(),
            status: status.to_owned(),
            state: state.to_owned(),
            repo: repo.to_owned(),
            branch: branch.to_owned(),
            summary: String::new(),
            last_active,
            events,
        }
    }

    #[test]
    fn groups_fan_and_ago() {
        let root = PathBuf::from("/p");
        let strategy = Directories {
            prefix: default_prefix_rule(),
        };
        let map = ProjectMap::of(
            "shop",
            &root,
            &[PathBuf::from("catalog/pricing.rs")],
            &strategy,
        );
        let in_city = |relative: &str| read_event(root.join(relative), None);

        let pricing_fix = session_full(
            "pricing-fix",
            "idle",
            "waiting",
            Some(1000),
            "shop",
            "pricing-fix-branch",
            vec![in_city("catalog/pricing.rs")],
        );
        assert_eq!(pricing_fix.session_state(), SessionState::Waiting);
        let docs_run = session_full(
            "docs-run",
            "busy",
            "working",
            Some(5000),
            "shop",
            "docs-branch",
            vec![in_city("catalog/pricing.rs")],
        );
        assert_eq!(docs_run.session_state(), SessionState::Working);
        let loyalty_proto = session_full(
            "loyalty-proto",
            "busy",
            "working",
            Some(3000),
            "shop",
            "loyalty-branch",
            vec![in_city("catalog/pricing.rs")],
        );
        let handbook_session = session_full(
            "handbook-session",
            "idle",
            "done",
            Some(4000),
            "handbook",
            "handbook-branch",
            vec![read_event(PathBuf::from("/elsewhere/intro.md"), None)],
        );
        assert_eq!(handbook_session.session_state(), SessionState::Idle);

        let sessions = vec![
            pricing_fix.clone(),
            docs_run.clone(),
            loyalty_proto.clone(),
            handbook_session.clone(),
        ];

        let all = groups(&sessions, &map, None, "");
        let names = |label: &str| -> Option<Vec<&str>> {
            all.iter()
                .find(|(group, _)| *group == label)
                .map(|(_, members)| members.iter().map(|s| s.id.as_str()).collect())
        };
        assert_eq!(names("Needs you"), Some(vec!["pricing-fix"]));
        assert_eq!(
            names("Working"),
            Some(vec!["docs-run", "loyalty-proto"]),
            "newest last_active first"
        );
        assert_eq!(
            names("Idle"),
            None,
            "no session is Idle in this city, so the group is left out entirely"
        );
        assert_eq!(names("Other projects"), Some(vec!["handbook-session"]));

        let narrowed = groups(&sessions, &map, None, "docs");
        assert_eq!(
            narrowed,
            vec![("Working", vec![&docs_run])],
            "the query narrows by name, case-insensitively, leaving empty groups out"
        );

        let by_repo = groups(&sessions, &map, None, "SHOP");
        let by_repo_names = |label: &str| -> Option<Vec<&str>> {
            by_repo
                .iter()
                .find(|(group, _)| *group == label)
                .map(|(_, members)| members.iter().map(|s| s.id.as_str()).collect())
        };
        assert_eq!(
            by_repo_names("Needs you"),
            Some(vec!["pricing-fix"]),
            "the query also matches the repo field, case-insensitively"
        );
        assert_eq!(
            by_repo_names("Working"),
            Some(vec!["docs-run", "loyalty-proto"])
        );
        assert!(
            by_repo.iter().all(|(group, _)| *group != "Other projects"),
            "handbook-session's repo does not match \"SHOP\": {by_repo:?}"
        );

        let towers = vec![
            PathBuf::from("/p/a"),
            PathBuf::from("/p/a"),
            PathBuf::from("/p/b"),
        ];
        assert_eq!(fan(&towers), vec![(0, 2), (1, 2), (0, 1)]);

        assert_eq!(ago(Some(0), 0), "now");
        assert_eq!(ago(Some(0), 90), "1 min");
        assert_eq!(ago(Some(0), 7_200), "2 h");
        assert_eq!(ago(Some(0), 172_800), "2 d");
        assert_eq!(ago(None, 0), "-");
    }
}
