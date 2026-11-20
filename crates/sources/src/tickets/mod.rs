//! First-class tickets from four pluggable trackers: Jira, Cockpit, GitHub
//! issues and Linear.
//!
//! The neutral model ([`Ticket`], [`TicketRef`], [`Category`]) is defined in
//! `cellworks_map::tickets`, because the Map's case and panel render it; it is
//! re-exported here. Each tracker is a blocking [`TicketSource`] (HTTP through
//! `curl`, GitHub through `gh`, ADR-0003) and the [`Tickets`] facade owns the
//! enabled ones: it calls them on worker threads, caches what they answer and
//! hands the results to the UI thread as [`TicketEvent`]s over
//! `std::sync::mpsc`, drained once per frame by [`TicketDesk::poll`].
//!
//! `CELLWORKS_JIRA_OFFLINE` keeps every call off the network (only the Jira
//! cache is read), and `CELLWORKS_JIRA_CACHE` moves Jira's cache, so the demo
//! runs on a fixture.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub use cellworks_map::tickets::{
    Category, SignIn, SourceId, SourceState, Synced, Ticket, TicketDesk, TicketRef,
};

pub mod cockpit;
pub mod github;
mod http;
pub mod jira;
pub mod linear;

/// Why a source could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// Not signed in (Jira).
    SignIn,
    /// The source cannot do this (writing to Jira, say).
    Unsupported,
    /// The message to show the engineer.
    Other(String),
}

impl From<String> for SourceError {
    fn from(reason: String) -> SourceError {
        SourceError::Other(reason)
    }
}

/// One tracker. Every call blocks, so the facade only calls them on worker
/// threads; every test passes a fake or a recorded answer.
pub trait TicketSource: Send + Sync {
    fn id(&self) -> SourceId;
    /// My open tickets.
    fn list_mine(&self) -> Result<Vec<Ticket>, SourceError>;
    /// Tickets matching `query`, in the tracker's own search language: JQL
    /// for Jira, free text elsewhere.
    fn list(&self, query: &str) -> Result<Vec<Ticket>, SourceError>;
    /// One ticket by its display key, read in full.
    fn get(&self, key: &str) -> Result<Ticket, SourceError>;
    /// Moves the ticket to `to`; answers it as it is afterwards.
    fn set_status(&self, _key: &str, _to: Category) -> Result<Ticket, SourceError> {
        Err(SourceError::Unsupported)
    }
    /// Assigns the ticket to me (or starts it, where that is what claiming
    /// means); answers it as it is afterwards.
    fn claim(&self, _key: &str) -> Result<Ticket, SourceError> {
        Err(SourceError::Unsupported)
    }
    /// Signs in through the browser; only sources that need it.
    fn sign_in(&self) -> Result<(), String> {
        Err("this source needs no sign-in".into())
    }
    /// Whether [`TicketSource::set_status`] works.
    fn writable(&self) -> bool {
        false
    }
}

/// Which sources are on, and how each reaches its tracker. The settings
/// window fills it through [`TicketsConfig::from_settings`];
/// [`TicketsConfig::from_env`] reads the environment alone.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketsConfig {
    pub jira: bool,
    /// Part of the site's address to pick when a sign-in reaches several.
    pub jira_site: Option<String>,
    pub cockpit: bool,
    pub cockpit_url: String,
    pub github: bool,
    /// `owner/repo` to list open issues of, besides those assigned to me.
    pub github_repos: Vec<String>,
    pub linear: bool,
    /// Without a key the Linear source is off.
    pub linear_api_key: Option<String>,
    /// No network or process calls: only the Jira cache is read.
    pub offline: bool,
}

impl Default for TicketsConfig {
    fn default() -> Self {
        TicketsConfig {
            jira: true,
            jira_site: None,
            cockpit: true,
            cockpit_url: "http://127.0.0.1:4747".to_owned(),
            github: true,
            github_repos: Vec::new(),
            linear: true,
            linear_api_key: None,
            offline: false,
        }
    }
}

impl TicketsConfig {
    /// The defaults with `CELLWORKS_JIRA_SITE`, `CELLWORKS_JIRA_OFFLINE`,
    /// `CELLWORKS_COCKPIT_URL`, `CELLWORKS_GITHUB_REPOS` (comma separated) and
    /// `CELLWORKS_LINEAR_API_KEY` applied.
    pub fn from_env() -> TicketsConfig {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let default = TicketsConfig::default();
        TicketsConfig {
            jira_site: var("CELLWORKS_JIRA_SITE"),
            offline: std::env::var_os("CELLWORKS_JIRA_OFFLINE").is_some(),
            cockpit_url: var("CELLWORKS_COCKPIT_URL").unwrap_or(default.cockpit_url.clone()),
            github_repos: var("CELLWORKS_GITHUB_REPOS")
                .map(|repos| repos.split(',').map(|r| r.trim().to_owned()).collect())
                .unwrap_or_default(),
            linear_api_key: var("CELLWORKS_LINEAR_API_KEY"),
            ..default
        }
    }

    /// The config of `settings` (already merged with the environment, see
    /// `Settings::effective`) and the Linear key, from the environment or the
    /// keychain. Without a key the Linear source is off.
    pub fn from_settings(
        settings: &cellworks_workspace::Settings,
        linear_api_key: Option<String>,
    ) -> TicketsConfig {
        TicketsConfig {
            jira: settings.jira.enabled,
            jira_site: Some(settings.jira.site.trim().to_owned()).filter(|s| !s.is_empty()),
            cockpit: settings.cockpit.enabled,
            cockpit_url: settings.cockpit.url.trim().to_owned(),
            github: settings.github.enabled,
            github_repos: settings.github.repos.clone(),
            linear: settings.linear_enabled,
            linear_api_key,
            offline: settings.jira.offline,
        }
    }

    fn sources(&self) -> Vec<Arc<dyn TicketSource>> {
        let mut sources: Vec<Arc<dyn TicketSource>> = Vec::new();
        if self.jira {
            sources.push(Arc::new(jira::Jira::new(self.jira_site.clone())));
        }
        if self.offline {
            return sources;
        }
        if self.cockpit {
            sources.push(Arc::new(cockpit::Cockpit::new(&self.cockpit_url)));
        }
        if self.github {
            sources.push(Arc::new(github::GitHub::new(self.github_repos.clone())));
        }
        if let (true, Some(key)) = (self.linear, &self.linear_api_key) {
            sources.push(Arc::new(linear::Linear::new(key)));
        }
        sources
    }
}

/// Reads my tickets from one source of `config`, once, to see whether it
/// answers: the number it listed, or why it did not. Blocks; read-only.
pub fn test_connection(config: &TicketsConfig, source: SourceId) -> Result<usize, String> {
    let source: Arc<dyn TicketSource> = match source {
        SourceId::Jira => return Err("Jira is checked by signing in".to_owned()),
        SourceId::Cockpit => Arc::new(cockpit::Cockpit::new(&config.cockpit_url)),
        SourceId::GitHub => Arc::new(github::GitHub::new(config.github_repos.clone())),
        SourceId::Linear => match &config.linear_api_key {
            Some(key) => Arc::new(linear::Linear::new(key)),
            None => return Err("no API key yet".to_owned()),
        },
    };
    match source.list_mine() {
        Ok(tickets) => Ok(tickets.len()),
        Err(SourceError::SignIn) => Err("not signed in".to_owned()),
        Err(SourceError::Unsupported) => Err("not supported".to_owned()),
        Err(SourceError::Other(reason)) => Err(reason),
    }
}

/// What a background call brought back.
pub enum TicketEvent {
    Listed {
        source: SourceId,
        tickets: Vec<Ticket>,
    },
    Fetched(Box<Ticket>),
    ListFailed {
        source: SourceId,
        reason: String,
    },
    FetchFailed {
        which: TicketRef,
        reason: String,
    },
    SignInNeeded(SourceId),
    SignedIn(SourceId),
    SignInFailed(SourceId, String),
}

#[derive(Default)]
struct Slot {
    /// The tickets of the latest list, and when it was taken.
    listed: Option<(Vec<TicketRef>, Synced)>,
    list_asked: bool,
    fetching: bool,
    failure: Option<String>,
    sign_in: SignIn,
}

/// The tickets known this session, the calls under way, and every enabled
/// source's state.
pub struct Tickets {
    sources: Vec<Arc<dyn TicketSource>>,
    tx: Sender<TicketEvent>,
    rx: Receiver<TicketEvent>,
    slots: BTreeMap<SourceId, Slot>,
    store: BTreeMap<TicketRef, (Ticket, Synced)>,
    asked: BTreeSet<TicketRef>,
    /// Tickets whose own cache file was read: a list row in the store must not
    /// hide the full copy cached beside it.
    read: BTreeSet<TicketRef>,
    pending: BTreeSet<TicketRef>,
    failed: BTreeMap<TicketRef, String>,
    offline: bool,
    cache_root: PathBuf,
    jira_cache: PathBuf,
}

impl Default for Tickets {
    fn default() -> Self {
        Tickets::new(TicketsConfig::from_env())
    }
}

impl Tickets {
    pub fn new(config: TicketsConfig) -> Tickets {
        let cache = http::home().join(".cache/cellworks");
        let mut tickets = Tickets::with_sources(config.sources(), cache.join("tickets"));
        tickets.offline = config.offline;
        tickets.jira_cache = std::env::var_os("CELLWORKS_JIRA_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|| cache.join("jira/issues"));
        tickets
    }

    /// Starts over with the sources `config` names; what the old sources had
    /// brought is read again from the cache.
    pub fn reconfigure(&mut self, config: TicketsConfig) {
        *self = Tickets::new(config);
    }

    /// The facade over `sources`, caching under `cache_root/<source>`.
    pub fn with_sources(sources: Vec<Arc<dyn TicketSource>>, cache_root: PathBuf) -> Tickets {
        let (tx, rx) = channel();
        Tickets {
            slots: sources.iter().map(|s| (s.id(), Slot::default())).collect(),
            sources,
            tx,
            rx,
            store: BTreeMap::new(),
            asked: BTreeSet::new(),
            read: BTreeSet::new(),
            pending: BTreeSet::new(),
            failed: BTreeMap::new(),
            offline: false,
            jira_cache: cache_root.join("jira"),
            cache_root,
        }
    }

    fn dir(&self, source: SourceId) -> PathBuf {
        match source {
            SourceId::Jira => self.jira_cache.clone(),
            other => self.cache_root.join(other.id()),
        }
    }

    fn source(&self, id: SourceId) -> Option<Arc<dyn TicketSource>> {
        self.sources.iter().find(|s| s.id() == id).cloned()
    }

    /// Runs `work` on a thread of its own; its event reaches [`poll`].
    fn spawn(&self, work: impl FnOnce() -> TicketEvent + Send + 'static) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(work());
        });
    }

    fn put(&mut self, ticket: Ticket, synced: Synced) {
        let which = ticket.reference();
        let merged = merge(ticket, self.store.get(&which).map(|(old, _)| old));
        self.store.insert(which, (merged, synced));
    }

    /// Reads the cached list of `source` into the store, once.
    fn load_list(&mut self, source: SourceId) {
        if self.slots.get(&source).is_none_or(|s| s.listed.is_some()) {
            return;
        }
        if let Some((tickets, synced)) = load::<Vec<Ticket>>(&self.dir(source).join("mine.json")) {
            let refs = tickets.iter().map(Ticket::reference).collect();
            for ticket in tickets {
                self.put(ticket, synced);
            }
            if let Some(slot) = self.slots.get_mut(&source) {
                slot.listed = Some((refs, synced));
            }
        }
    }

    /// The tickets of every list known so far; with `ask`, asks each source
    /// for its list once.
    fn listed(&mut self, ask: bool) -> Vec<(Ticket, Synced)> {
        let ids: Vec<SourceId> = self.sources.iter().map(|s| s.id()).collect();
        let mut out = Vec::new();
        for id in ids {
            self.load_list(id);
            if ask {
                self.ask_list(id);
            }
            if let Some((refs, _)) = self.slots[&id].listed.as_ref() {
                out.extend(refs.iter().filter_map(|r| self.store.get(r).cloned()));
            }
        }
        out
    }

    fn ask_list(&mut self, source: SourceId) {
        let Some(src) = self.source(source) else {
            return;
        };
        let dir = self.dir(source);
        let Some(slot) = self.slots.get_mut(&source) else {
            return;
        };
        if slot.list_asked || self.offline || slot.sign_in == SignIn::Pending {
            return;
        }
        slot.list_asked = true;
        slot.fetching = true;
        self.spawn(move || match src.list_mine() {
            Ok(tickets) => {
                save(&dir.join("mine.json"), &tickets);
                TicketEvent::Listed { source, tickets }
            }
            Err(SourceError::SignIn) => TicketEvent::SignInNeeded(source),
            Err(SourceError::Unsupported) => TicketEvent::ListFailed {
                source,
                reason: "not supported".into(),
            },
            Err(SourceError::Other(reason)) => TicketEvent::ListFailed { source, reason },
        });
    }
}

/// `new` as the list or a status change brought it, with what only a full
/// read has (`old`'s body, criteria, links, comments) kept where `new` has
/// none.
fn merge(mut new: Ticket, old: Option<&Ticket>) -> Ticket {
    if let Some(old) = old {
        if new.body.is_empty() {
            new.body = old.body.clone();
        }
        if new.criteria.is_empty() {
            new.criteria = old.criteria.clone();
            new.criteria_from = old.criteria_from.clone();
        }
        if new.links.is_empty() {
            new.links = old.links.clone();
        }
        if new.comments.is_empty() {
            new.comments = old.comments.clone();
        }
        if new.kind.is_empty() {
            new.kind = old.kind.clone();
        }
        if new.reporter.is_none() {
            new.reporter = old.reporter.clone();
        }
        if new.repo.is_none() {
            new.repo = old.repo.clone();
        }
    }
    new
}

/// Whether a repository hint (a path, `owner/repo` or a bare name) names the
/// project folder `path`.
pub fn repo_matches(hint: &str, path: &Path) -> bool {
    let name = |p: &str| {
        p.trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_lowercase()
    };
    let path = path.to_string_lossy();
    !hint.is_empty() && (hint == path || name(hint) == name(&path))
}

impl TicketDesk for Tickets {
    fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            let now = Synced::Live(SystemTime::now());
            match event {
                TicketEvent::Listed { source, tickets } => {
                    let refs = tickets.iter().map(Ticket::reference).collect();
                    for ticket in tickets {
                        self.put(ticket, now);
                    }
                    if let Some(slot) = self.slots.get_mut(&source) {
                        slot.listed = Some((refs, now));
                        slot.fetching = false;
                        slot.failure = None;
                        slot.sign_in = SignIn::Done;
                    }
                }
                TicketEvent::Fetched(ticket) => {
                    let which = ticket.reference();
                    self.pending.remove(&which);
                    self.failed.remove(&which);
                    if let Some(slot) = self.slots.get_mut(&which.source) {
                        slot.sign_in = SignIn::Done;
                    }
                    self.put(*ticket, now);
                }
                TicketEvent::ListFailed { source, reason } => {
                    if let Some(slot) = self.slots.get_mut(&source) {
                        slot.fetching = false;
                        slot.failure = Some(reason);
                    }
                }
                TicketEvent::FetchFailed { which, reason } => {
                    self.pending.remove(&which);
                    self.failed.insert(which, reason);
                }
                TicketEvent::SignInNeeded(source) => {
                    self.pending.retain(|w| w.source != source);
                    if let Some(slot) = self.slots.get_mut(&source) {
                        slot.fetching = false;
                        if slot.sign_in != SignIn::Pending {
                            slot.sign_in = SignIn::Needed;
                        }
                    }
                }
                TicketEvent::SignedIn(source) => {
                    self.asked.retain(|w| w.source != source);
                    self.failed.retain(|w, _| w.source != source);
                    if let Some(slot) = self.slots.get_mut(&source) {
                        slot.sign_in = SignIn::Done;
                        slot.list_asked = false;
                        slot.failure = None;
                    }
                }
                TicketEvent::SignInFailed(source, reason) => {
                    if let Some(slot) = self.slots.get_mut(&source) {
                        slot.sign_in = SignIn::Failed(reason);
                    }
                }
            }
        }
    }

    fn ticket(&mut self, which: &TicketRef) -> Option<(Ticket, Synced)> {
        if self.read.insert(which.clone())
            && let Some((ticket, synced)) =
                load::<Ticket>(&self.dir(which.source).join(file_name(&which.key)))
        {
            self.put(ticket, synced);
        }
        if let Some(src) = self.source(which.source)
            && !self.offline
            && self.slots[&which.source].sign_in != SignIn::Pending
            && self.asked.insert(which.clone())
        {
            self.pending.insert(which.clone());
            let (dir, which) = (self.dir(which.source), which.clone());
            self.spawn(move || match src.get(&which.key) {
                Ok(ticket) => {
                    save(&dir.join(file_name(&ticket.key)), &ticket);
                    TicketEvent::Fetched(Box::new(ticket))
                }
                Err(SourceError::SignIn) => TicketEvent::SignInNeeded(which.source),
                Err(SourceError::Unsupported) => TicketEvent::FetchFailed {
                    which,
                    reason: "not supported".into(),
                },
                Err(SourceError::Other(reason)) => TicketEvent::FetchFailed { which, reason },
            });
        }
        self.store.get(which).cloned()
    }

    fn all(&mut self) -> Vec<(Ticket, Synced)> {
        self.listed(true)
    }

    /// Never asks a source: the cache and what this session already has.
    fn for_repo(&mut self, repo: &Path) -> Vec<Ticket> {
        self.listed(false)
            .into_iter()
            .map(|(ticket, _)| ticket)
            .filter(|t| {
                t.repo
                    .as_deref()
                    .is_some_and(|hint| repo_matches(hint, repo))
            })
            .collect()
    }

    fn sources(&self) -> Vec<SourceState> {
        self.sources
            .iter()
            .map(|src| {
                let slot = &self.slots[&src.id()];
                SourceState {
                    id: src.id(),
                    synced: slot.listed.as_ref().map(|(_, synced)| *synced),
                    fetching: slot.fetching,
                    failure: slot.failure.clone(),
                    sign_in: slot.sign_in.clone(),
                    writable: src.writable(),
                }
            })
            .collect()
    }

    fn refresh(&mut self) {
        for slot in self.slots.values_mut() {
            slot.list_asked = false;
        }
        self.asked.clear();
    }

    fn is_fetching(&self, which: &TicketRef) -> bool {
        self.pending.contains(which)
    }

    fn failure(&self, which: &TicketRef) -> Option<String> {
        self.failed.get(which).cloned()
    }

    fn set_status(&mut self, which: &TicketRef, category: Category) {
        let Some(src) = self.source(which.source) else {
            return;
        };
        self.pending.insert(which.clone());
        self.failed.remove(which);
        let (dir, which) = (self.dir(which.source), which.clone());
        self.spawn(move || match src.set_status(&which.key, category) {
            Ok(ticket) => {
                save(&dir.join(file_name(&ticket.key)), &ticket);
                TicketEvent::Fetched(Box::new(ticket))
            }
            Err(SourceError::SignIn) => TicketEvent::SignInNeeded(which.source),
            Err(SourceError::Unsupported) => TicketEvent::FetchFailed {
                which,
                reason: format!("{} tickets are read-only here", src.id().name()),
            },
            Err(SourceError::Other(reason)) => TicketEvent::FetchFailed { which, reason },
        });
    }

    fn sign_in(&self, source: SourceId) -> SignIn {
        self.slots
            .get(&source)
            .map(|s| s.sign_in.clone())
            .unwrap_or_default()
    }

    /// Opens the browser to sign in, on a thread of its own; under
    /// `CELLWORKS_DRY_LAUNCH` or offline nothing opens and sign-in stays
    /// pending.
    fn start_sign_in(&mut self, source: SourceId) {
        let Some(src) = self.source(source) else {
            return;
        };
        if let Some(slot) = self.slots.get_mut(&source) {
            slot.sign_in = SignIn::Pending;
        }
        if self.offline || std::env::var_os("CELLWORKS_DRY_LAUNCH").is_some() {
            return;
        }
        self.spawn(move || match src.sign_in() {
            Ok(()) => TicketEvent::SignedIn(source),
            Err(reason) => TicketEvent::SignInFailed(source, reason),
        });
    }
}

// ---------------------------------------------------------------- cache

/// The cache file of the ticket `key`: the key with path characters replaced.
fn file_name(key: &str) -> String {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.json")
}

fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<(T, Synced)> {
    let text = std::fs::read_to_string(path).ok()?;
    let at = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some((serde_json::from_str(&text).ok()?, Synced::Cached(at)))
}

fn save<T: Serialize>(path: &Path, value: &T) {
    if let (Some(dir), Ok(text)) = (path.parent(), serde_json::to_string_pretty(value)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(path, text);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// "YYYY-MM-DD" from an ISO timestamp.
fn day(timestamp: &str) -> String {
    timestamp.chars().take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// A source that answers from memory and records what it was asked.
    struct Fake {
        id: SourceId,
        tickets: Mutex<Vec<Ticket>>,
        writes: Mutex<Vec<(String, Category)>>,
        writable: bool,
    }

    fn ticket(source: SourceId, key: &str, repo: Option<&str>) -> Ticket {
        Ticket {
            source,
            key: key.to_owned(),
            title: format!("title of {key}"),
            repo: repo.map(str::to_owned),
            ..Ticket::default()
        }
    }

    impl Fake {
        fn new(id: SourceId, tickets: Vec<Ticket>, writable: bool) -> Arc<Fake> {
            Arc::new(Fake {
                id,
                tickets: Mutex::new(tickets),
                writes: Mutex::new(Vec::new()),
                writable,
            })
        }
    }

    impl TicketSource for Fake {
        fn id(&self) -> SourceId {
            self.id
        }
        fn list_mine(&self) -> Result<Vec<Ticket>, SourceError> {
            Ok(self.tickets.lock().unwrap().clone())
        }
        fn list(&self, _: &str) -> Result<Vec<Ticket>, SourceError> {
            self.list_mine()
        }
        fn get(&self, key: &str) -> Result<Ticket, SourceError> {
            let mut full = self
                .tickets
                .lock()
                .unwrap()
                .iter()
                .find(|t| t.key == key)
                .cloned()
                .ok_or(SourceError::Other("no such ticket".into()))?;
            full.body = "the full text".into();
            Ok(full)
        }
        fn set_status(&self, key: &str, to: Category) -> Result<Ticket, SourceError> {
            if !self.writable {
                return Err(SourceError::Unsupported);
            }
            self.writes.lock().unwrap().push((key.to_owned(), to));
            let mut t = self.get(key)?;
            t.category = to;
            Ok(t)
        }
        fn writable(&self) -> bool {
            self.writable
        }
    }

    /// Polls until `done` holds or two seconds have passed.
    fn settle(desk: &mut Tickets, done: impl Fn(&mut Tickets) -> bool) {
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            desk.poll();
            if done(desk) {
                return;
            }
            assert!(Instant::now() < end, "the background call never answered");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn lists_merge_across_sources_and_survive_in_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let sources: Vec<Arc<dyn TicketSource>> = vec![
            Fake::new(
                SourceId::Cockpit,
                vec![ticket(SourceId::Cockpit, "TIC-2", Some("/p/cockpit"))],
                true,
            ),
            Fake::new(
                SourceId::Linear,
                vec![ticket(SourceId::Linear, "ENG-7", None)],
                true,
            ),
        ];
        let mut desk = Tickets::with_sources(sources.clone(), dir.path().to_owned());
        assert!(desk.all().is_empty(), "nothing before the first answer");
        settle(&mut desk, |d| d.all().len() == 2);
        let keys: Vec<String> = desk.all().iter().map(|(t, _)| t.key.clone()).collect();
        assert_eq!(keys, ["TIC-2", "ENG-7"]);
        assert!(
            desk.sources()
                .iter()
                .all(|s| s.synced.is_some() && !s.fetching)
        );

        // A new session shows the cache at once, marked as cached.
        let mut again = Tickets::with_sources(sources, dir.path().to_owned());
        let shown = again.all();
        assert_eq!(shown.len(), 2);
        assert!(matches!(shown[0].1, Synced::Cached(_)));
    }

    #[test]
    fn a_full_read_keeps_what_a_later_list_lacks() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(
            SourceId::Cockpit,
            vec![ticket(SourceId::Cockpit, "TIC-2", None)],
            true,
        );
        let mut desk = Tickets::with_sources(vec![fake], dir.path().to_owned());
        let which = TicketRef::parse("cockpit:TIC-2");
        settle(&mut desk, |d| {
            d.ticket(&which)
                .is_some_and(|(t, _)| t.body == "the full text")
        });
        desk.refresh();
        settle(&mut desk, |d| {
            d.all().len() == 1 && !d.sources()[0].fetching
        });
        assert_eq!(desk.ticket(&which).unwrap().0.body, "the full text");
    }

    #[test]
    fn tickets_are_found_by_the_repository_they_name() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(
            SourceId::GitHub,
            vec![
                ticket(SourceId::GitHub, "acme/api#1", Some("acme/api")),
                ticket(SourceId::GitHub, "acme/web#2", Some("acme/web")),
            ],
            true,
        );
        let mut desk = Tickets::with_sources(vec![fake], dir.path().to_owned());
        settle(&mut desk, |d| d.all().len() == 2);
        let found = desk.for_repo(Path::new("/home/dev/Projects/API"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, "acme/api#1");
        assert!(repo_matches("/p/x", Path::new("/p/x")));
        assert!(!repo_matches("", Path::new("/p/x")));
    }

    #[test]
    fn a_status_change_reaches_the_source_and_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(
            SourceId::Cockpit,
            vec![ticket(SourceId::Cockpit, "TIC-2", None)],
            true,
        );
        let mut desk = Tickets::with_sources(vec![fake.clone()], dir.path().to_owned());
        settle(&mut desk, |d| d.all().len() == 1);
        let which = TicketRef::parse("cockpit:TIC-2");
        desk.set_status(&which, Category::Done);
        assert!(desk.is_fetching(&which));
        settle(&mut desk, |d| !d.is_fetching(&which));
        assert_eq!(desk.ticket(&which).unwrap().0.category, Category::Done);
        assert_eq!(
            *fake.writes.lock().unwrap(),
            [("TIC-2".to_owned(), Category::Done)]
        );
    }

    #[test]
    fn a_read_only_source_says_so_instead_of_writing() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(
            SourceId::Jira,
            vec![ticket(SourceId::Jira, "SHOP-1", None)],
            false,
        );
        let mut desk = Tickets::with_sources(vec![fake], dir.path().to_owned());
        assert!(!desk.sources()[0].writable);
        let which = TicketRef::parse("SHOP-1");
        desk.set_status(&which, Category::Done);
        settle(&mut desk, |d| d.failure(&which).is_some());
        assert!(desk.failure(&which).unwrap().contains("read-only"));
    }

    #[test]
    fn offline_never_calls_a_source() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::new(
            SourceId::Jira,
            vec![ticket(SourceId::Jira, "SHOP-1", None)],
            false,
        );
        let mut desk = Tickets::with_sources(vec![fake], dir.path().to_owned());
        desk.offline = true;
        assert!(desk.all().is_empty());
        assert!(desk.ticket(&TicketRef::parse("SHOP-1")).is_none());
        std::thread::sleep(Duration::from_millis(50));
        desk.poll();
        assert!(desk.all().is_empty());
    }

    #[test]
    fn the_config_turns_sources_on_and_a_missing_linear_key_keeps_it_off() {
        let mut config = TicketsConfig::default();
        let ids = |c: &TicketsConfig| c.sources().iter().map(|s| s.id()).collect::<Vec<_>>();
        assert_eq!(
            ids(&config),
            [SourceId::Jira, SourceId::Cockpit, SourceId::GitHub]
        );
        config.linear_api_key = Some("lin_key".into());
        config.cockpit = false;
        assert_eq!(
            ids(&config),
            [SourceId::Jira, SourceId::GitHub, SourceId::Linear]
        );
        config.offline = true;
        assert_eq!(ids(&config), [SourceId::Jira]);
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn thin() -> Ticket {
        Ticket {
            source: SourceId::Jira,
            key: "SHOP-42".into(),
            title: "t".into(),
            ..Ticket::default()
        }
    }

    fn full() -> Ticket {
        Ticket {
            kind: "Story".into(),
            reporter: Some("Dana".into()),
            body: "text".into(),
            criteria: vec!["one".into()],
            ..thin()
        }
    }

    fn assert_full(t: &Ticket) {
        assert_eq!(t.kind, "Story");
        assert_eq!(t.reporter.as_deref(), Some("Dana"));
        assert_eq!(t.body, "text");
        assert_eq!(t.criteria, ["one"]);
    }

    /// Offline, with a list row and the full issue cached side by side (the
    /// demo's layout), the case's ticket is the full one.
    #[test]
    fn a_cached_issue_beats_its_list_row_offline() {
        let dir = tempfile::tempdir().unwrap();
        let jira = dir.path().join("jira");
        save(&jira.join("mine.json"), &vec![thin()]);
        save(&jira.join("SHOP-42.json"), &full());
        let src: Arc<dyn TicketSource> = Arc::new(jira::Jira::new(None));
        let mut desk = Tickets::with_sources(vec![src], dir.path().to_owned());
        desk.offline = true;
        assert_eq!(desk.all().len(), 1);
        assert_full(&desk.ticket(&TicketRef::parse("SHOP-42")).unwrap().0);
    }

    struct One;
    impl TicketSource for One {
        fn id(&self) -> SourceId {
            SourceId::Jira
        }
        fn list_mine(&self) -> Result<Vec<Ticket>, SourceError> {
            Ok(vec![thin()])
        }
        fn list(&self, _: &str) -> Result<Vec<Ticket>, SourceError> {
            self.list_mine()
        }
        fn get(&self, _: &str) -> Result<Ticket, SourceError> {
            Ok(full())
        }
    }

    /// An issue fetched once comes back full on the next start, even though
    /// the cached list holds only its thin row.
    #[test]
    fn a_fetched_issue_is_full_on_the_next_start() {
        let dir = tempfile::tempdir().unwrap();
        let which = TicketRef::parse("SHOP-42");
        let mut first = Tickets::with_sources(vec![Arc::new(One)], dir.path().to_owned());
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            first.poll();
            first.all();
            if first.ticket(&which).is_some_and(|(t, _)| t.body == "text")
                && !first.sources()[0].fetching
            {
                break;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut next = Tickets::with_sources(vec![Arc::new(One)], dir.path().to_owned());
        next.offline = true;
        next.all();
        assert_full(&next.ticket(&which).unwrap().0);
    }
}
