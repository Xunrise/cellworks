//! The AgentSessions bounded context: discovering sessions from transcripts,
//! placing them on the graph, and creating and steering them.
//!
//! Defined by docs/domain/contexts/AgentSessions/AgentSessions.md.

mod checkout;
mod claude;
mod paint;
mod terminal;
mod transcript;
pub mod ui;
pub mod units;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::Duration;

use cellworks_map::ProjectMap;
use egui::Pos2;

use crate::terminal::Terminal;

pub use claude::{Agent, AgentList, ClaudeCli, ClaudeUnavailable, parse_agents};

/// What a tool call does where it lands: the unit's work animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActionKind {
    #[default]
    Read,
    /// Edit, MultiEdit, Write, NotebookEdit.
    Write,
    /// A shell command.
    Run,
    /// Grep, Glob, or a searching shell command.
    Search,
    /// A subagent started with Task/Agent.
    Delegate,
    /// WebFetch, WebSearch: off the map.
    Fetch,
}

/// One tool call a session made, placed relative to its own checkout so
/// the open project's own city can decide whether it is in it. A file for
/// the file tools; a folder for a command or search; an empty path for a
/// call that leaves the map.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileEvent {
    pub kind: ActionKind,
    /// The checkout's git common dir (shared by every worktree of one
    /// repository); `None` when the checkout is not a git repository.
    pub checkout: Option<PathBuf>,
    /// Relative to its own checkout root when `checkout` is `Some`;
    /// otherwise the absolute path (a path prefix then decides whether it's
    /// in a given project's city).
    pub path: PathBuf,
    pub line: Option<usize>,
    pub added: usize,
    /// The command, pattern, task or URL; empty for the file tools.
    pub detail: String,
    /// The `tool_use` id, unique within a session; empty in hand-made
    /// sessions.
    pub id: String,
    /// `None` while the call runs; then whether it succeeded.
    pub outcome: Option<bool>,
}

impl FileEvent {
    pub fn write(&self) -> bool {
        self.kind == ActionKind::Write
    }

    /// The verb a unit's label shows: "editing", "testing", ...
    pub fn verb(&self) -> &'static str {
        match self.kind {
            ActionKind::Read => "reading",
            ActionKind::Write => "editing",
            ActionKind::Run if self.detail.contains("test") => "testing",
            ActionKind::Run => "running",
            ActionKind::Search => "searching",
            ActionKind::Delegate => "delegating",
            ActionKind::Fetch => "fetching",
        }
    }
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
    /// The short id `claude attach`/`claude stop` take, carried over from
    /// `Agent.id`; `None` for an interactive session, which has none.
    pub attach_id: Option<String>,
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

/// What a background watch, or a [`SessionPane`], reports to the UI thread,
/// once per drain. Crate-private: it carries `transcript::Entry`, itself
/// `pub(crate)` since the conversation's shape is this crate's own concern,
/// not part of its public API.
#[derive(Clone, Debug)]
pub(crate) enum SessionEvent {
    Listed(Vec<Session>),
    ListingFailed(String),
    /// A pane's conversation, re-read from its transcript.
    ConversationRead(Vec<transcript::Entry>),
    /// The new attach id of a background copy `continue_in_background`
    /// started.
    CopyStarted(String),
    /// At most 200 characters of a failed `continue_in_background`'s output.
    CopyFailed(String),
    Stopped,
    /// At most 200 characters of a failed `stop`'s error.
    StopFailed(String),
}

/// Where a watch reads sessions from, and how often it polls: passed in so
/// a test can replace both without reading `$HOME` or running `claude`.
pub struct SessionSources {
    pub transcripts: PathBuf,
    pub agents: Arc<dyn AgentList + Send + Sync>,
    pub poll: Duration,
    /// The `claude` program a [`SessionPane`] runs to attach, stop and
    /// continue a session.
    pub claude: PathBuf,
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
            claude: PathBuf::from("claude"),
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
/// thread; never blocks the caller. Crate-private: only `ui::SessionsView`
/// calls it; made so when `SessionEvent` itself became crate-private
/// (STORY-081), since it carries `transcript::Entry`, which is not part of
/// this crate's public API.
pub(crate) fn start_watch(sources: SessionSources) -> Receiver<SessionEvent> {
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
        for (id, ok) in transcript::tool_results(&new_lines) {
            if let Some(event) = cache.events.iter_mut().rev().find(|e| e.id == id) {
                event.outcome = Some(ok);
            }
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
        attach_id: (!agent.id.is_empty()).then(|| agent.id.clone()),
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
        kind: tool_call.kind,
        checkout,
        path,
        line: tool_call.line,
        added: tool_call.added,
        detail: tool_call.detail.clone(),
        id: tool_call.id.clone(),
        outcome: None,
    }
}

/// How many cells a freshly opened pane assumes before the UI's first real
/// draw corrects it to the pane's actual size.
const INITIAL_ROWS: u16 = 24;
const INITIAL_COLS: u16 = 80;

/// How much of the end of a transcript a freshly opened read-only pane reads,
/// rather than reparsing a long-running session's whole history (design.md
/// requirement 6).
const TAIL_BYTES: u64 = 1 << 20;

/// How often a read-only conversation is re-read from its transcript.
const CONVERSATION_POLL: Duration = Duration::from_secs(1);

/// The read-only conversation's entries are kept to this many, newest last -
/// the same trimming pattern [`build_session`] already applies to a
/// session's events.
const MAX_ENTRIES: usize = 80;

/// requirement 6's banner for a session with no attach id.
const READ_ONLY_BANNER: &str = "Started outside Cellworks (interactive). Read-only here.";

/// Whether a [`SessionPane`] shows a live terminal or a read-only
/// conversation.
enum PaneKind {
    /// `None` when the attach process could not even be spawned - treated as
    /// already ended, tolerantly, rather than failing to open the pane at
    /// all. Boxed: a `Terminal` is much larger than `Vec<transcript::Entry>`,
    /// and this enum is cloned-free but still sized for its largest variant.
    Terminal(Option<Box<Terminal>>),
    /// Oldest first, trimmed to [`MAX_ENTRIES`].
    Conversation(Vec<transcript::Entry>),
}

/// The headless half of a session's pane (design.md requirement 2 onward):
/// either a live terminal attached to a background session, or a read-only,
/// periodically refreshed conversation for an interactive one, each owning
/// whatever background thread(s) it starts, all reporting on this pane's own
/// channel - not the main watch's.
pub(crate) struct SessionPane {
    session_id: String,
    name: String,
    claude: PathBuf,
    /// The attach id the pane's terminal is currently attached to, if any;
    /// updated when a background copy starts, so [`SessionPane::stop`] always
    /// stops the session actually shown.
    attach_id: Option<String>,
    rows: u16,
    cols: u16,
    kind: PaneKind,
    banner: Option<String>,
    tx: Sender<SessionEvent>,
    rx: Receiver<SessionEvent>,
}

impl SessionPane {
    /// Opens a pane on `session`: a live terminal when it has an attach id,
    /// otherwise a read-only conversation tailed from its main transcript
    /// under `transcripts_root`, re-read every second on a background
    /// thread.
    pub(crate) fn open(session: &Session, claude: &Path, transcripts_root: &Path) -> SessionPane {
        let (tx, rx) = mpsc::channel();
        let rows = INITIAL_ROWS;
        let cols = INITIAL_COLS;

        let (kind, banner) = match &session.attach_id {
            Some(id) => {
                let terminal = Terminal::spawn(claude, &["attach", id], rows, cols)
                    .ok()
                    .map(Box::new);
                (PaneKind::Terminal(terminal), None)
            }
            None => {
                if let Some(file) = main_transcript_file(transcripts_root, &session.id) {
                    spawn_conversation_reader(file, tx.clone());
                }
                (
                    PaneKind::Conversation(Vec::new()),
                    Some(READ_ONLY_BANNER.to_owned()),
                )
            }
        };

        SessionPane {
            session_id: session.id.clone(),
            name: session.name.clone(),
            claude: claude.to_owned(),
            attach_id: session.attach_id.clone(),
            rows,
            cols,
            kind,
            banner,
            tx,
            rx,
        }
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    /// Whether this pane shows a live terminal (vs. a read-only
    /// conversation) - used by the UI to decide whether to offer "Stop" vs.
    /// "Continue in the background".
    pub(crate) fn is_terminal(&self) -> bool {
        matches!(self.kind, PaneKind::Terminal(_))
    }

    /// Pumps the terminal (if any) and drains whatever the pane's own
    /// channel has reported since the last call. Never blocks.
    pub(crate) fn drain(&mut self) {
        if let PaneKind::Terminal(Some(terminal)) = &mut self.kind {
            terminal.pump();
        }

        loop {
            match self.rx.try_recv() {
                Ok(SessionEvent::ConversationRead(list)) => {
                    if let PaneKind::Conversation(entries) = &mut self.kind {
                        *entries = list;
                    }
                }
                Ok(SessionEvent::CopyStarted(id)) => {
                    let terminal =
                        Terminal::spawn(&self.claude, &["attach", &id], self.rows, self.cols)
                            .ok()
                            .map(Box::new);
                    self.kind = PaneKind::Terminal(terminal);
                    self.attach_id = Some(id);
                    self.banner = Some(format!(
                        "Background copy of {}; close the original where it runs.",
                        self.name
                    ));
                }
                Ok(SessionEvent::CopyFailed(message)) => {
                    self.banner = Some(message);
                }
                Ok(SessionEvent::Stopped) => {}
                Ok(SessionEvent::StopFailed(message)) => {
                    self.banner = Some(message);
                }
                Ok(SessionEvent::Listed(_)) | Ok(SessionEvent::ListingFailed(_)) => {}
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    /// Starts a background thread running `claude::continue_in_background`
    /// and sending `CopyStarted`/`CopyFailed` on this pane's channel. No-op
    /// if this pane is already a terminal.
    pub(crate) fn continue_in_background(&mut self) {
        if self.is_terminal() {
            return;
        }
        let tx = self.tx.clone();
        let claude = self.claude.clone();
        let session_id = self.session_id.clone();
        let name = self.name.clone();
        std::thread::spawn(move || {
            let event = match claude::continue_in_background(&claude, &session_id, &name) {
                Ok(id) => SessionEvent::CopyStarted(id),
                Err(message) => SessionEvent::CopyFailed(message),
            };
            let _ = tx.send(event);
        });
    }

    /// Starts a background thread running `claude::stop` and sending
    /// `Stopped`/`StopFailed`. No-op if this pane is a conversation (nothing
    /// to stop).
    pub(crate) fn stop(&mut self) {
        let Some(id) = (if self.is_terminal() {
            self.attach_id.clone()
        } else {
            None
        }) else {
            return;
        };
        let tx = self.tx.clone();
        let claude = self.claude.clone();
        std::thread::spawn(move || {
            let event = match claude::stop(&claude, &id) {
                Ok(()) => SessionEvent::Stopped,
                Err(message) => SessionEvent::StopFailed(message.chars().take(200).collect()),
            };
            let _ = tx.send(event);
        });
    }

    /// The vt100 screen of a terminal pane, for drawing; `None` for a
    /// conversation.
    pub(crate) fn screen(&self) -> Option<&vt100::Screen> {
        match &self.kind {
            PaneKind::Terminal(Some(terminal)) => Some(terminal.screen()),
            _ => None,
        }
    }

    /// Sends `bytes` to a terminal pane's child; no-op for a conversation.
    pub(crate) fn send(&mut self, bytes: &[u8]) {
        if let PaneKind::Terminal(Some(terminal)) = &mut self.kind {
            terminal.send(bytes);
        }
    }

    /// Resizes a terminal pane's pty; no-op for a conversation.
    pub(crate) fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = rows;
        self.cols = cols;
        if let PaneKind::Terminal(Some(terminal)) = &mut self.kind {
            terminal.resize(rows, cols);
        }
    }

    /// Whether a terminal pane's attach process has ended ("(attach ended)"
    /// in the UI); `false` for a conversation.
    pub(crate) fn has_ended(&mut self) -> bool {
        match &mut self.kind {
            PaneKind::Terminal(Some(terminal)) => terminal.has_ended(),
            PaneKind::Terminal(None) => true,
            PaneKind::Conversation(_) => false,
        }
    }

    /// The conversation's entries, oldest first, for drawing; `None` for a
    /// terminal.
    pub(crate) fn entries(&self) -> Option<&[transcript::Entry]> {
        match &self.kind {
            PaneKind::Conversation(entries) => Some(entries.as_slice()),
            PaneKind::Terminal(_) => None,
        }
    }

    /// The banner line shown above a conversation; `None` for a (never-was-
    /// a-conversation) terminal that was a background session from the
    /// start.
    pub(crate) fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }
}

/// The main transcript file `session_id` has under `transcripts_root`, if
/// any: `<transcripts_root>/<any immediate subdir>/<session_id>.jsonl` - the
/// "main file" half of [`transcript_files`], since a pane must never tail a
/// subagent's own transcript (design.md requirement 6).
fn main_transcript_file(transcripts_root: &Path, session_id: &str) -> Option<PathBuf> {
    std::fs::read_dir(transcripts_root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .map(|dir| dir.join(format!("{session_id}.jsonl")))
        .find(|file| file.is_file())
}

/// Starts the background thread a read-only conversation pane tails its
/// transcript with: reads from [`Tail::from_end`][transcript::Tail::from_end]
/// (the last [`TAIL_BYTES`]), parses new lines with
/// [`transcript::entries`], accumulates into a running list trimmed to
/// [`MAX_ENTRIES`], and sends `SessionEvent::ConversationRead(list.clone())`
/// after every read - the first one immediately, before the first sleep, so
/// the pane shows content right away.
fn spawn_conversation_reader(file: PathBuf, tx: Sender<SessionEvent>) {
    std::thread::spawn(move || {
        let mut tail = transcript::Tail::from_end(&file, TAIL_BYTES);
        let mut list: Vec<transcript::Entry> = Vec::new();
        loop {
            let new_lines = tail.read(&file);
            list.extend(transcript::entries(&new_lines));
            let cut = list.len().saturating_sub(MAX_ENTRIES);
            list.drain(..cut);

            if tx
                .send(SessionEvent::ConversationRead(list.clone()))
                .is_err()
            {
                return;
            }
            std::thread::sleep(CONVERSATION_POLL);
        }
    });
}

/// Where a session's latest placeable event stands in a city.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub tower: PathBuf,
    pub centre: Pos2,
    pub z: f32,
    pub writing: bool,
    pub added: usize,
    /// The ground the tower, building or folder stands on.
    pub area: egui::Rect,
    /// The heights of the floors the event touches, when it names a line
    /// of a tower.
    pub floor: Option<(f32, f32)>,
    /// The function or type the tower is, `None` for a file or folder.
    pub item: Option<String>,
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
pub(crate) fn city_node(
    event: &FileEvent,
    map: &ProjectMap,
    common_dir: Option<&Path>,
) -> Option<PathBuf> {
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
pub(crate) fn tower_of(map: &ProjectMap, node: &Path, event: &FileEvent) -> Option<Placement> {
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
            let floor = event.line.and_then(|line| {
                let index = item.floors.iter().position(|f| f.number == line)?;
                // `floor_z` is a floor's top above ground, its bottom in the
                // cellar.
                let z = cellworks_map::city::floor_z(item, index);
                let span = event.added.max(1) as f32 * cellworks_map::city::floor_height(item);
                Some(if cellworks_map::city::in_cellar(item) {
                    (z, z + span)
                } else {
                    (z - span, z)
                })
            });
            return Some(Placement {
                tower: item.path.clone(),
                centre: item.area.center(),
                z: cellworks_map::city::height(item),
                writing: event.write(),
                added: event.added,
                area: item.area,
                floor,
                item: Some(item.name.clone()),
            });
        }
    }

    let found = map.find(node)?;
    Some(Placement {
        tower: node.to_owned(),
        centre: found.area.center(),
        z: 0.0,
        writing: event.write(),
        added: event.added,
        area: found.area,
        floor: None,
        item: None,
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
            attach_id: None,
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
            checkout: None,
            path,
            line,
            added: 0,
            ..FileEvent::default()
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
            attach_id: None,
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
