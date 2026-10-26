//! PROTOTYPE: Jira, read-only, through Atlassian's remote MCP server, with
//! Cellworks as its own OAuth client (dynamic client registration and PKCE
//! in the browser; no model in the loop). Only the read tools are called:
//! `getAccessibleAtlassianResources`, `getJiraIssue` and
//! `searchJiraIssuesUsingJql`. Nothing here writes to Jira.
//!
//! Tokens are kept in `~/.config/cellworks/atlassian.json` (0600). Each issue
//! is cached in `~/.cache/cellworks/jira/issues/<KEY>.json` and shown from
//! the cache until a fetch on a background thread brings it fresh; results
//! reach the UI thread as [`JiraEvent`]s over `std::sync::mpsc`.
//!
//! `CELLWORKS_JIRA_OFFLINE` keeps every call off the network, and
//! `CELLWORKS_JIRA_CACHE` moves the cache, so the screenshot harness runs on
//! a fixture.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MCP: &str = "https://mcp.atlassian.com/v1/mcp";
const AUTH: &str = "https://mcp.atlassian.com/v1";

/// The JQL behind "my open tickets".
pub const MINE: &str = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

/// One issue, as the case shows it.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Issue {
    pub key: String,
    pub summary: String,
    pub kind: String,
    pub status: String,
    /// "new", "indeterminate" or "done".
    pub category: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub reporter: Option<String>,
    pub updated: String,
    /// The description as plain text.
    pub description: String,
    /// The acceptance criteria, one per entry, and where they were found:
    /// the name of a custom field, or "description".
    pub criteria: Vec<String>,
    pub criteria_from: String,
    /// (how the link reads from this issue, the other key, its summary).
    pub links: Vec<(String, String, String)>,
    /// The latest comments, newest last: (author, created, body).
    pub comments: Vec<(String, String, String)>,
    pub url: String,
}

/// One row of a search.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Row {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub updated: String,
}

/// What a background call brought back.
pub enum JiraEvent {
    IssueFetched { key: String, issue: Box<Issue> },
    MineListed(Vec<Row>),
    FetchFailed { key: String, reason: String },
    SignInNeeded,
    SignedIn,
    SignInFailed(String),
}

/// When the shown copy was synced: this session, or only the cache's age.
#[derive(Clone, Copy, Debug)]
pub enum Synced {
    Live(SystemTime),
    Cached(SystemTime),
}

impl Synced {
    /// "synced 14:02" or "cached 2026-10-21 09:13", in local time.
    pub fn label(self) -> String {
        match self {
            Synced::Live(at) => format!("synced {}", clock(at, false)),
            Synced::Cached(at) => format!("cached {}", clock(at, true)),
        }
    }
}

/// Where sign-in stands.
#[derive(Clone, Debug, PartialEq)]
pub enum SignIn {
    Unknown,
    Needed,
    /// The browser was opened; waiting for it to come back.
    Pending,
    Failed(String),
    Done,
}

/// The issues known this session, the fetches under way, and sign-in.
pub struct Jira {
    tx: Sender<JiraEvent>,
    rx: Receiver<JiraEvent>,
    issues: BTreeMap<String, (Issue, Synced)>,
    asked: BTreeSet<String>,
    pending: BTreeSet<String>,
    failed: BTreeMap<String, String>,
    mine: Option<(Vec<Row>, Synced)>,
    mine_asked: bool,
    pub sign_in: SignIn,
}

impl Default for Jira {
    fn default() -> Self {
        Jira::new()
    }
}

impl Jira {
    pub fn new() -> Jira {
        let (tx, rx) = channel();
        Jira {
            tx,
            rx,
            issues: BTreeMap::new(),
            asked: BTreeSet::new(),
            pending: BTreeSet::new(),
            failed: BTreeMap::new(),
            mine: None,
            mine_asked: false,
            sign_in: SignIn::Unknown,
        }
    }

    /// Takes what the background calls brought back; once per frame.
    pub fn poll(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                JiraEvent::IssueFetched { key, issue } => {
                    self.pending.remove(&key);
                    self.failed.remove(&key);
                    self.issues
                        .insert(key, (*issue, Synced::Live(SystemTime::now())));
                    self.sign_in = SignIn::Done;
                }
                JiraEvent::MineListed(rows) => {
                    self.pending.remove(MINE);
                    self.mine = Some((rows, Synced::Live(SystemTime::now())));
                    self.sign_in = SignIn::Done;
                }
                JiraEvent::FetchFailed { key, reason } => {
                    self.pending.remove(&key);
                    self.failed.insert(key, reason);
                }
                JiraEvent::SignInNeeded => {
                    self.pending.clear();
                    if self.sign_in != SignIn::Pending {
                        self.sign_in = SignIn::Needed;
                    }
                }
                JiraEvent::SignedIn => {
                    self.sign_in = SignIn::Done;
                    // Ask again for everything that waited on sign-in.
                    self.asked.clear();
                    self.mine_asked = false;
                    self.failed.clear();
                }
                JiraEvent::SignInFailed(reason) => self.sign_in = SignIn::Failed(reason),
            }
        }
    }

    /// The issue `key`, from this session or the cache; asks for it fresh
    /// once per session.
    pub fn issue(&mut self, key: &str) -> Option<&(Issue, Synced)> {
        if !self.issues.contains_key(key)
            && let Some(cached) = load::<Issue>(&cache_file(key))
        {
            self.issues.insert(key.to_owned(), cached);
        }
        if self.asked.insert(key.to_owned()) && !offline() && self.sign_in != SignIn::Pending {
            self.pending.insert(key.to_owned());
            let (tx, key) = (self.tx.clone(), key.to_owned());
            std::thread::spawn(move || {
                let event = match fetch_issue(&key) {
                    Ok(issue) => {
                        save_cache(&cache_file(&key), &issue);
                        JiraEvent::IssueFetched {
                            key,
                            issue: Box::new(issue),
                        }
                    }
                    Err(Failure::SignIn) => JiraEvent::SignInNeeded,
                    Err(Failure::Other(reason)) => JiraEvent::FetchFailed { key, reason },
                };
                let _ = tx.send(event);
            });
        }
        self.issues.get(key)
    }

    /// My open tickets, from this session or the cache; asks once.
    pub fn mine(&mut self) -> Option<&(Vec<Row>, Synced)> {
        if self.mine.is_none() {
            self.mine = load::<Vec<Row>>(&cache_file("mine"));
        }
        if !self.mine_asked && !offline() && self.sign_in != SignIn::Pending {
            self.mine_asked = true;
            self.pending.insert(MINE.to_owned());
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let event = match search(MINE) {
                    Ok(rows) => {
                        save_cache(&cache_file("mine"), &rows);
                        JiraEvent::MineListed(rows)
                    }
                    Err(Failure::SignIn) => JiraEvent::SignInNeeded,
                    Err(Failure::Other(reason)) => JiraEvent::FetchFailed {
                        key: MINE.to_owned(),
                        reason,
                    },
                };
                let _ = tx.send(event);
            });
        }
        self.mine.as_ref()
    }

    pub fn is_fetching(&self, key: &str) -> bool {
        self.pending.contains(key)
    }

    pub fn failure(&self, key: &str) -> Option<&str> {
        self.failed.get(key).map(String::as_str)
    }

    /// Opens the browser to sign in, on a thread of its own; under
    /// `CELLWORKS_DRY_LAUNCH` or offline nothing opens and sign-in stays
    /// pending.
    pub fn start_sign_in(&mut self) {
        self.sign_in = SignIn::Pending;
        if offline() || std::env::var_os("CELLWORKS_DRY_LAUNCH").is_some() {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(match sign_in() {
                Ok(()) => JiraEvent::SignedIn,
                Err(reason) => JiraEvent::SignInFailed(reason),
            });
        });
    }
}

fn offline() -> bool {
    std::env::var_os("CELLWORKS_JIRA_OFFLINE").is_some()
}

// ---------------------------------------------------------------- files

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

fn tokens_path() -> PathBuf {
    home().join(".config/cellworks/atlassian.json")
}

fn cache_dir() -> PathBuf {
    std::env::var_os("CELLWORKS_JIRA_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache/cellworks/jira/issues"))
}

fn cache_file(name: &str) -> PathBuf {
    cache_dir().join(format!("{name}.json"))
}

fn load<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<(T, Synced)> {
    let text = std::fs::read_to_string(path).ok()?;
    let at = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some((serde_json::from_str(&text).ok()?, Synced::Cached(at)))
}

fn save_cache<T: Serialize>(path: &Path, value: &T) {
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

/// `at` in local time, through `date`, which knows the time zone.
pub(crate) fn clock(at: SystemTime, with_date: bool) -> String {
    let secs = at.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let format = if with_date {
        "+%Y-%m-%d %H:%M"
    } else {
        "+%H:%M"
    };
    Command::new("date")
        .args(["-r", &secs.to_string(), format])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| secs.to_string())
}

// ---------------------------------------------------------------- HTTP

enum Failure {
    SignIn,
    Other(String),
}

impl From<String> for Failure {
    fn from(reason: String) -> Failure {
        Failure::Other(reason)
    }
}

/// POSTs `body` through curl and returns (status, headers, body). Headers
/// and body go through curl's config on stdin and a private file, so no
/// token shows in the process list.
fn post(
    url: &str,
    headers: &[(&str, &str)],
    content_type: &str,
    body: &str,
) -> Result<(u32, String, String), String> {
    let mut config = format!(
        "url = \"{url}\"\nrequest = \"POST\"\nsilent\ninclude\nheader = \"Content-Type: {content_type}\"\n"
    );
    for (name, value) in headers {
        config += &format!("header = \"{name}: {value}\"\n");
    }
    let dir = home().join(".cache/cellworks");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("req-{}.tmp", random_hex(8)));
    write_private(&tmp, body)?;
    config += &format!("data-binary = \"@{}\"\n", tmp.display());
    let run = || -> Result<String, String> {
        let mut child = Command::new("curl")
            .args(["-K", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("curl: {e}"))?;
        child
            .stdin
            .take()
            .ok_or("curl: no stdin")?
            .write_all(config.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut out = String::new();
        child
            .stdout
            .take()
            .ok_or("curl: no stdout")?
            .read_to_string(&mut out)
            .map_err(|e| e.to_string())?;
        child.wait().map_err(|e| e.to_string())?;
        Ok(out)
    };
    let out = run();
    let _ = std::fs::remove_file(&tmp);
    let out = out?;
    // Skip "100 Continue": the last header block is the response's.
    let mut rest = out.as_str();
    loop {
        let (head, body) = rest
            .split_once("\r\n\r\n")
            .ok_or_else(|| format!("no response from {url}"))?;
        let status: u32 = head
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if status == 100 {
            rest = body;
            continue;
        }
        return Ok((status, head.to_owned(), body.to_owned()));
    }
}

fn write_private(path: &Path, text: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .map_err(|e| e.to_string())
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
        let _ = file.read_exact(&mut buf);
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decodes a URL-encoded value exactly once.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match (
            b[i],
            s.get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok()),
        ) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn form(pairs: &[(&str, &str)]) -> String {
    let encode = |s: &str| {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>()
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn base64url(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    s
}

fn sha256(text: &str) -> Result<Vec<u8>, String> {
    let mut child = Command::new("openssl")
        .args(["dgst", "-sha256", "-binary"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("openssl: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("openssl: no stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(child.wait_with_output().map_err(|e| e.to_string())?.stdout)
}

// ---------------------------------------------------------------- OAuth

#[derive(Serialize, Deserialize)]
struct Tokens {
    client_id: String,
    access_token: String,
    refresh_token: String,
    /// Unix seconds.
    expires_at: u64,
}

/// Browser sign-in: registers Cellworks as a client, opens the browser and
/// waits up to five minutes for it to come back to a local callback.
fn sign_in() -> Result<(), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://localhost:{port}/callback");
    let registration = json!({
        "client_name": "Cellworks", "redirect_uris": [redirect],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"], "token_endpoint_auth_method": "none",
    });
    let (_, _, body) = post(
        &format!("{AUTH}/register"),
        &[],
        "application/json",
        &registration.to_string(),
    )?;
    let client_id = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v["client_id"].as_str().map(str::to_owned))
        .ok_or("client registration refused")?;
    let verifier = random_hex(32);
    let challenge = base64url(&sha256(&verifier)?);
    let state = random_hex(16);
    let url = format!(
        "{AUTH}/authorize?{}",
        form(&[
            ("response_type", "code"),
            ("client_id", &client_id),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("redirect_uri", &redirect),
            ("state", &state),
        ])
    );
    Command::new("open")
        .arg(&url)
        .status()
        .map_err(|e| e.to_string())?;
    let deadline = now() + 300;
    let code = loop {
        if now() > deadline {
            return Err("sign-in timed out".into());
        }
        let (mut conn, _) = listener.accept().map_err(|e| e.to_string())?;
        let mut buf = [0u8; 4096];
        let n = conn.read(&mut buf).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let query = request
            .split_whitespace()
            .nth(1)
            .and_then(|p| p.split_once('?'))
            .map(|q| q.1.to_owned())
            .unwrap_or_default();
        let get = |k: &str| {
            query
                .split('&')
                .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
                .map(percent_decode)
        };
        let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<p style='font-family:sans-serif'>Cellworks is signed in to Atlassian. You can close this tab.</p>");
        if get("state").as_deref() == Some(state.as_str()) {
            match get("code") {
                Some(code) => break code,
                None => return Err("sign-in refused".into()),
            }
        }
    };
    let (_, _, body) = post(
        &format!("{AUTH}/token"),
        &[],
        "application/x-www-form-urlencoded",
        &form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("client_id", &client_id),
            ("code_verifier", &verifier),
        ]),
    )?;
    save_tokens(&client_id, &body)
}

fn save_tokens(client_id: &str, response: &str) -> Result<(), String> {
    let v: Value = serde_json::from_str(response).map_err(|_| "token response unreadable")?;
    let tokens = Tokens {
        client_id: client_id.into(),
        access_token: v["access_token"]
            .as_str()
            .ok_or("token request refused")?
            .into(),
        refresh_token: v["refresh_token"].as_str().unwrap_or_default().into(),
        expires_at: now() + v["expires_in"].as_u64().unwrap_or(3600),
    };
    let path = tokens_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    write_private(
        &path,
        &serde_json::to_string(&tokens).map_err(|e| e.to_string())?,
    )
}

/// A valid access token, stored or refreshed. Never signs in by itself:
/// without one the caller is told sign-in is needed.
fn token() -> Result<String, Failure> {
    let stored = std::fs::read_to_string(tokens_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Tokens>(&s).ok())
        .ok_or(Failure::SignIn)?;
    if stored.expires_at > now() + 60 {
        return Ok(stored.access_token);
    }
    let (status, _, body) = post(
        &format!("{AUTH}/token"),
        &[],
        "application/x-www-form-urlencoded",
        &form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &stored.refresh_token),
            ("client_id", &stored.client_id),
        ]),
    )?;
    if status != 200 {
        return Err(Failure::SignIn);
    }
    save_tokens(&stored.client_id, &body)?;
    let fresh: Tokens = std::fs::read_to_string(tokens_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or(Failure::SignIn)?;
    Ok(fresh.access_token)
}

// ---------------------------------------------------------------- MCP

struct Session {
    token: String,
    id: String,
    next: u64,
}

/// The read-only tools this client may call; anything else is refused
/// before it leaves the machine.
const READ_TOOLS: [&str; 3] = [
    "getAccessibleAtlassianResources",
    "getJiraIssue",
    "searchJiraIssuesUsingJql",
];

impl Session {
    fn open() -> Result<Session, Failure> {
        let mut session = Session {
            token: token()?,
            id: String::new(),
            next: 1,
        };
        let (head, _) = session.rpc(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "cellworks", "version": "0.1"}}),
        )?;
        session.id = head
            .lines()
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
                    .map(|(_, v)| v.trim().to_owned())
            })
            .unwrap_or_default();
        let headers = session.headers();
        let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        post(
            MCP,
            &headers,
            "application/json",
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        )?;
        Ok(session)
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("Authorization", format!("Bearer {}", self.token)),
            ("Accept", "application/json, text/event-stream".into()),
        ];
        if !self.id.is_empty() {
            headers.push(("Mcp-Session-Id", self.id.clone()));
        }
        headers
    }

    /// One JSON-RPC request; the answer comes as JSON or as server-sent
    /// events.
    fn rpc(&mut self, method: &str, params: Value) -> Result<(String, Value), Failure> {
        let id = self.next;
        self.next += 1;
        let headers = self.headers();
        let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let (status, head, body) = post(MCP, &headers, "application/json", &request.to_string())?;
        if status == 401 {
            return Err(Failure::SignIn);
        }
        let message = body
            .lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .chain(std::iter::once(body.as_str()))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
            .find(|v| v["id"] == id)
            .ok_or_else(|| format!("{method}: HTTP {status}"))?;
        if let Some(error) = message.get("error") {
            return Err(Failure::Other(format!("{method}: {error}")));
        }
        Ok((head, message["result"].clone()))
    }

    fn tool(&mut self, name: &str, args: Value) -> Result<Value, Failure> {
        if !READ_TOOLS.contains(&name) {
            return Err(Failure::Other(format!("{name} is not a read tool")));
        }
        let (_, result) = self.rpc("tools/call", json!({"name": name, "arguments": args}))?;
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        if result["isError"] == true {
            return Err(Failure::Other(format!(
                "{name}: {}",
                text.chars().take(300).collect::<String>()
            )));
        }
        serde_json::from_str(text).map_err(|_| {
            Failure::Other(format!(
                "{name}: {}",
                text.chars().take(300).collect::<String>()
            ))
        })
    }

    /// The cloud id and the site's address: the first site this sign-in
    /// reaches, or the one named in `CELLWORKS_JIRA_SITE`.
    fn site(&mut self) -> Result<(String, String), Failure> {
        let wanted = std::env::var("CELLWORKS_JIRA_SITE").ok();
        let resources = self.tool("getAccessibleAtlassianResources", json!({}))?;
        resources
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| {
                wanted
                    .as_deref()
                    .is_none_or(|w| r["url"].as_str().unwrap_or_default().contains(w))
            })
            .and_then(|r| Some((r["id"].as_str()?.to_owned(), r["url"].as_str()?.to_owned())))
            .ok_or_else(|| Failure::Other("no Atlassian site reachable".into()))
    }
}

fn fetch_issue(key: &str) -> Result<Issue, Failure> {
    let mut session = Session::open()?;
    let (cloud, site) = session.site()?;
    let raw = session.tool(
        "getJiraIssue",
        json!({"cloudId": cloud, "issueIdOrKey": key, "expand": "names"}),
    )?;
    Ok(issue(&raw, &site))
}

fn search(jql: &str) -> Result<Vec<Row>, Failure> {
    let mut session = Session::open()?;
    let (cloud, _) = session.site()?;
    let result = session.tool(
        "searchJiraIssuesUsingJql",
        json!({"cloudId": cloud, "jql": jql, "maxResults": 30,
               "fields": ["summary", "status", "updated"]}),
    )?;
    // Two answer shapes: {issues: [..], isLast, nextPageToken} or
    // {issues: {nodes, pageInfo}}.
    let nodes = result["issues"]
        .as_array()
        .or_else(|| result["issues"]["nodes"].as_array());
    Ok(nodes
        .into_iter()
        .flatten()
        .map(|n| {
            let f = &n["fields"];
            Row {
                key: text(&n["key"]),
                summary: text(&f["summary"]).trim().to_owned(),
                status: text(&f["status"]["name"]),
                updated: text(&f["updated"]).chars().take(10).collect(),
            }
        })
        .collect())
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// The issue as the case shows it, from Jira's answer.
fn issue(raw: &Value, site: &str) -> Issue {
    let f = &raw["fields"];
    let key = text(&raw["key"]);
    let description = rich(&f["description"]);
    // A custom field whose name says acceptance criteria, else the
    // description's section of that name.
    let named = raw["names"].as_object().and_then(|names| {
        names.iter().find_map(|(id, name)| {
            let name = name.as_str()?;
            let lower = name.to_lowercase();
            let value = rich(&f[id]);
            (lower.contains("acceptance") && !value.trim().is_empty())
                .then(|| (name.to_owned(), value))
        })
    });
    let (criteria, criteria_from) = match named {
        Some((name, value)) => (items(&value), name),
        None => match [
            "acceptance criteria",
            "akzeptanzkriterien",
            "definition of done",
        ]
        .iter()
        .find_map(|name| section(&description, name))
        {
            Some(part) => (items(&part), "description".to_owned()),
            None => (Vec::new(), String::new()),
        },
    };
    let all = f["comment"]["comments"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let comments = all[all.len().saturating_sub(3)..]
        .iter()
        .map(|c| {
            (
                text(&c["author"]["displayName"]),
                text(&c["created"]).chars().take(10).collect(),
                rich(&c["body"]),
            )
        })
        .collect();
    Issue {
        summary: text(&f["summary"]).trim().to_owned(),
        kind: text(&f["issuetype"]["name"]),
        status: text(&f["status"]["name"]),
        category: text(&f["status"]["statusCategory"]["key"]),
        priority: text(&f["priority"]["name"]),
        assignee: f["assignee"]["displayName"].as_str().map(str::to_owned),
        reporter: f["reporter"]["displayName"].as_str().map(str::to_owned),
        updated: text(&f["updated"]).chars().take(10).collect(),
        description,
        criteria,
        criteria_from,
        links: f["issuelinks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| match (l.get("outwardIssue"), l.get("inwardIssue")) {
                (Some(o), _) => Some((
                    text(&l["type"]["outward"]),
                    text(&o["key"]),
                    text(&o["fields"]["summary"]),
                )),
                (_, Some(i)) => Some((
                    text(&l["type"]["inward"]),
                    text(&i["key"]),
                    text(&i["fields"]["summary"]),
                )),
                _ => None,
            })
            .collect(),
        comments,
        url: format!("{}/browse/{key}", site.trim_end_matches('/')),
        key,
    }
}

/// A rich text field as plain text: a string as it is, an Atlassian
/// document flattened with list items as "- " lines and headings as "# ".
fn rich(v: &Value) -> String {
    fn walk(node: &Value, out: &mut String) {
        match node["type"].as_str().unwrap_or_default() {
            "text" => out.push_str(node["text"].as_str().unwrap_or_default()),
            "hardBreak" => out.push('\n'),
            "mention" => out.push_str(node["attrs"]["text"].as_str().unwrap_or_default()),
            "inlineCard" => out.push_str(node["attrs"]["url"].as_str().unwrap_or_default()),
            kind => {
                let block = matches!(
                    kind,
                    "paragraph" | "heading" | "listItem" | "codeBlock" | "blockquote" | "rule"
                );
                if block && !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                if kind == "heading" {
                    out.push_str("# ");
                }
                if kind == "listItem" {
                    out.push_str("- ");
                }
                for child in node["content"].as_array().into_iter().flatten() {
                    walk(child, out);
                }
                if block && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
    }
    match v {
        Value::String(s) => s.clone(),
        Value::Object(_) => {
            let mut out = String::new();
            walk(v, &mut out);
            out.trim().to_owned()
        }
        _ => String::new(),
    }
}

/// The text under the heading or line that starts with `name`, up to the
/// next heading: a "#" line or a line that is bold as a whole.
fn section(text: &str, name: &str) -> Option<String> {
    let heading = |l: &str| {
        let l = l.trim();
        l.starts_with('#') || (l.len() > 4 && l.starts_with("**") && l.ends_with("**"))
    };
    let mut lines = text.lines();
    lines.find(|l| {
        heading(l)
            && l.trim_start_matches(['#', '*', ' '])
                .to_lowercase()
                .starts_with(name)
    })?;
    let part: Vec<&str> = lines.take_while(|l| !heading(l)).collect();
    let part = part.join("\n");
    (!part.trim().is_empty()).then_some(part)
}

/// The entries of a list: its "- ", "* " or numbered lines, else its
/// non-empty lines.
fn items(text: &str) -> Vec<String> {
    let strip = |l: &str| -> Option<String> {
        let l = l.trim();
        let rest = l
            .strip_prefix("- ")
            .or_else(|| l.strip_prefix("* "))
            .or_else(|| {
                let digits = l.find(|c: char| !c.is_ascii_digit())?;
                (digits > 0)
                    .then(|| l[digits..].strip_prefix(". "))
                    .flatten()
            })?;
        // Markdown task boxes, escaped or not.
        let rest = rest.replace("\\[", "[").replace("\\]", "]");
        let rest = rest.trim();
        let rest = ["[ ]", "[x]", "[X]"]
            .iter()
            .find_map(|b| rest.strip_prefix(b))
            .unwrap_or(rest);
        Some(rest.trim().to_owned())
    };
    let listed: Vec<String> = text.lines().filter_map(strip).collect();
    if !listed.is_empty() {
        return listed;
    }
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_happens_once() {
        assert_eq!(percent_decode("a%3Ab%2Fc+d%2541"), "a:b/c d%41");
    }

    #[test]
    fn criteria_come_from_the_description_section() {
        let text = "Intro\n# Acceptance criteria\n- one\n- two\n# Notes\n- not this";
        let part = section(text, "acceptance criteria").expect("a section");
        assert_eq!(items(&part), ["one", "two"]);
        let text = "**Akzeptanzkriterien**\n\n* \\[ \\] Kein `root`.\n* \\[ \\] Rollen.\n\n**Rollback**\n\n* nein";
        let part = section(text, "akzeptanzkriterien").expect("a bold section");
        assert_eq!(items(&part), ["Kein `root`.", "Rollen."]);
    }

    /// PROTOTYPE: fetches one issue for real and prints what came back.
    /// `JIRA_PROBE=SHOP-42 cargo test -p cellworks-map probe -- --ignored --nocapture`
    #[test]
    #[ignore = "network"]
    fn probe() {
        let Ok(key) = std::env::var("JIRA_PROBE") else {
            return;
        };
        match fetch_issue(&key) {
            Ok(issue) => {
                save_cache(&cache_file(&key), &issue);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&issue).unwrap_or_default()
                );
            }
            Err(Failure::SignIn) => println!("sign-in needed"),
            Err(Failure::Other(reason)) => println!("failed: {reason}"),
        }
        match search(MINE) {
            Ok(rows) => {
                println!("mine: {} rows", rows.len());
                save_cache(&cache_file("mine"), &rows);
            }
            Err(Failure::SignIn) => println!("mine: sign-in needed"),
            Err(Failure::Other(reason)) => println!("mine failed: {reason}"),
        }
    }
}
