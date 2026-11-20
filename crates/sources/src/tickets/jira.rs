//! Jira, read-only, through Atlassian's remote MCP server, with Cellworks as
//! its own OAuth client (dynamic client registration and PKCE in the browser;
//! no model in the loop). Only the read tools are called:
//! `getAccessibleAtlassianResources`, `getJiraIssue` and
//! `searchJiraIssuesUsingJql`. Nothing here writes to Jira.
//!
//! Tokens are kept in `~/.config/cellworks/atlassian.json` (0600). The
//! facade (`super::Tickets`) caches what this source answers and calls it on
//! worker threads.
//!
//! The endpoints are the `/v1` ones: Atlassian's `/v2/mcp` is announced, but
//! it sits behind a different authorization server and scopes, so it is not a
//! drop-in change and stays unswitched until it can be tried against a real
//! sign-in.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::http::{home, post, random_hex, write_private};
use super::{Category, SourceError, SourceId, Ticket, TicketSource, day, now};

const MCP: &str = "https://mcp.atlassian.com/v1/mcp";
const AUTH: &str = "https://mcp.atlassian.com/v1";

/// The JQL behind "my open tickets".
pub const MINE: &str = "assignee = currentUser() AND statusCategory != Done ORDER BY updated DESC";

/// Jira over the Atlassian remote MCP server.
pub struct Jira {
    /// Part of the site's address to pick when a sign-in reaches several.
    site: Option<String>,
}

impl Jira {
    pub fn new(site: Option<String>) -> Jira {
        Jira { site }
    }
}

impl From<Failure> for SourceError {
    fn from(failure: Failure) -> SourceError {
        match failure {
            Failure::SignIn => SourceError::SignIn,
            Failure::Other(reason) => SourceError::Other(reason),
        }
    }
}

impl TicketSource for Jira {
    fn id(&self) -> SourceId {
        SourceId::Jira
    }

    fn list_mine(&self) -> Result<Vec<Ticket>, SourceError> {
        self.list(MINE)
    }

    fn list(&self, jql: &str) -> Result<Vec<Ticket>, SourceError> {
        Ok(search(jql, self.site.as_deref())?)
    }

    fn get(&self, key: &str) -> Result<Ticket, SourceError> {
        Ok(fetch_issue(key, self.site.as_deref())?)
    }

    fn sign_in(&self) -> Result<(), String> {
        sign_in()
    }
}

// ---------------------------------------------------------------- files

fn tokens_path() -> PathBuf {
    home().join(".config/cellworks/atlassian.json")
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
    /// reaches, or the one whose address contains `wanted`.
    fn site(&mut self, wanted: Option<&str>) -> Result<(String, String), Failure> {
        let resources = self.tool("getAccessibleAtlassianResources", json!({}))?;
        resources
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| wanted.is_none_or(|w| r["url"].as_str().unwrap_or_default().contains(w)))
            .and_then(|r| Some((r["id"].as_str()?.to_owned(), r["url"].as_str()?.to_owned())))
            .ok_or_else(|| Failure::Other("no Atlassian site reachable".into()))
    }
}

fn fetch_issue(key: &str, site: Option<&str>) -> Result<Ticket, Failure> {
    let mut session = Session::open()?;
    let (cloud, site) = session.site(site)?;
    let raw = session.tool(
        "getJiraIssue",
        json!({"cloudId": cloud, "issueIdOrKey": key, "expand": "names"}),
    )?;
    Ok(issue(&raw, &site))
}

fn search(jql: &str, site: Option<&str>) -> Result<Vec<Ticket>, Failure> {
    let mut session = Session::open()?;
    let (cloud, site) = session.site(site)?;
    let result = session.tool(
        "searchJiraIssuesUsingJql",
        json!({"cloudId": cloud, "jql": jql, "maxResults": 30,
               "fields": ["summary", "status", "updated", "priority", "issuetype", "labels"]}),
    )?;
    // Two answer shapes: {issues: [..], isLast, nextPageToken} or
    // {issues: {nodes, pageInfo}}.
    let nodes = result["issues"]
        .as_array()
        .or_else(|| result["issues"]["nodes"].as_array());
    Ok(nodes
        .into_iter()
        .flatten()
        .map(|n| issue(n, &site))
        .collect())
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

/// The ticket as the case shows it, from Jira's answer (a full read or a
/// search row: what is missing stays empty).
fn issue(raw: &Value, site: &str) -> Ticket {
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
    let status_key = text(&f["status"]["statusCategory"]["key"]);
    let status = text(&f["status"]["name"]);
    Ticket {
        source: SourceId::Jira,
        title: text(&f["summary"]).trim().to_owned(),
        kind: text(&f["issuetype"]["name"]),
        category: category(&status_key, &status),
        status,
        priority: text(&f["priority"]["name"]),
        assignee: f["assignee"]["displayName"].as_str().map(str::to_owned),
        reporter: f["reporter"]["displayName"].as_str().map(str::to_owned),
        labels: f["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l.as_str().map(str::to_owned))
            .collect(),
        updated: day(&text(&f["updated"])),
        project: key.split('-').next().unwrap_or_default().to_owned(),
        body: description,
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
        repo: None,
        key,
    }
}

/// Jira's status category key ("new", "indeterminate", "done"); a status
/// named "won't do" or "cancel..." in the done category is a cancellation.
fn category(key: &str, status: &str) -> Category {
    let lower = status.to_lowercase();
    match key {
        "done"
            if lower.contains("cancel") || lower.contains("won't") || lower.contains("reject") =>
        {
            Category::Canceled
        }
        "done" => Category::Done,
        "indeterminate" => Category::InProgress,
        _ => Category::Todo,
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

    #[test]
    fn a_recorded_issue_becomes_a_ticket() {
        let raw: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/tickets/jira_issue.json"))
                .unwrap();
        let ticket = issue(&raw, "https://acme.atlassian.net/");
        assert_eq!(ticket.source, SourceId::Jira);
        assert_eq!(ticket.key, "SHOP-42");
        assert_eq!(ticket.title, "Checkout drops the coupon");
        assert_eq!(ticket.category, Category::InProgress);
        assert_eq!(ticket.project, "SHOP");
        assert_eq!(ticket.url, "https://acme.atlassian.net/browse/SHOP-42");
        assert_eq!(
            ticket.criteria,
            ["Coupon survives the redirect", "Total is recomputed"]
        );
        assert_eq!(ticket.criteria_from, "description");
        assert_eq!(ticket.links[0].1, "SHOP-40");
        assert_eq!(ticket.comments.len(), 1);
    }

    #[test]
    fn a_search_row_has_what_a_search_asks_for() {
        let raw = json!({"key": "SHOP-7", "fields": {"summary": " Rows ", "updated": "2026-11-02T10:00:00.000+0100",
            "status": {"name": "To Do", "statusCategory": {"key": "new"}}}});
        let ticket = issue(&raw, "https://acme.atlassian.net");
        assert_eq!(
            (ticket.title.as_str(), ticket.updated.as_str()),
            ("Rows", "2026-11-02")
        );
        assert_eq!(ticket.category, Category::Todo);
        assert!(ticket.body.is_empty());
    }

    #[test]
    fn categories_follow_jiras_status_categories() {
        assert_eq!(category("done", "Won't Do"), Category::Canceled);
        assert_eq!(category("done", "Done"), Category::Done);
        assert_eq!(category("indeterminate", "In Review"), Category::InProgress);
    }
}
