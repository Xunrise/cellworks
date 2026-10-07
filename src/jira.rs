// Jira through Atlassian's remote MCP server, as its own OAuth client (no model in the loop).
// Sign-in: dynamic client registration + PKCE in the browser; tokens in ~/.config/cellworks/atlassian.json (0600).
// Tickets are cached per project in ~/.cache/cellworks/jira/<site>_<KEY>.json and the map is built from the cache.
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// What the last or running sync is doing, for the top bar.
pub static STATUS: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn set_status(s: String) {
    *STATUS.lock().unwrap() = s;
}

const MCP: &str = "https://mcp.atlassian.com/v1/mcp";
const AUTH: &str = "https://mcp.atlassian.com/v1";

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Ticket {
    pub key: String,
    pub summary: String,
    pub kind: String,
    /// -1 sub-task, 0 story/task/bug, 1 epic
    pub level: i64,
    pub status: String,
    /// "new", "indeterminate" or "done"
    pub cat: String,
    pub parent: Option<String>,
    /// (link name as seen from this ticket, other ticket), e.g. ("blocks", "HACK-12")
    pub links: Vec<(String, String)>,
    pub assignee: Option<String>,
    pub updated: String,
    pub components: Vec<String>,
    pub labels: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Tokens {
    client_id: String,
    access_token: String,
    refresh_token: String,
    /// unix seconds
    expires_at: u64,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}
fn tokens_path() -> PathBuf {
    home().join(".config/cellworks/atlassian.json")
}
pub fn cache_path(site: &str, key: &str) -> PathBuf {
    home().join(format!(".cache/cellworks/jira/{site}_{key}.json"))
}
fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `jira:<site>/<KEY>` -> (site, key)
pub fn parse(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("jira:")?.split_once('/')
}

pub fn load(site: &str, key: &str) -> Option<Vec<Ticket>> {
    serde_json::from_str(&std::fs::read_to_string(cache_path(site, key)).ok()?).ok()
}

// ---------- HTTP via curl (secrets go through stdin config, never the command line) ----------

/// POSTs `body` and returns (status, headers, body).
fn post(url: &str, headers: &[(&str, &str)], content_type: &str, body: &str) -> Result<(u32, String, String)> {
    let mut cfg = format!("url = \"{url}\"\nrequest = \"POST\"\nsilent\ninclude\nheader = \"Content-Type: {content_type}\"\n");
    for (k, v) in headers {
        cfg += &format!("header = \"{k}: {v}\"\n");
    }
    // the body goes through a private temp file so it never shows up in `ps`
    let tmp = home().join(format!(".cache/cellworks/req-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(tmp.parent().unwrap())?;
    write_private(&tmp, body)?;
    cfg += &format!("data-binary = \"@{}\"\n", tmp.display());
    let mut child = Command::new("curl").args(["-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    child.stdin.take().unwrap().write_all(cfg.as_bytes())?;
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out)?;
    child.wait()?;
    let _ = std::fs::remove_file(&tmp);
    // skip "100 Continue" preambles; the last header block belongs to the real response
    let mut rest = out.as_str();
    loop {
        let (head, body) = rest.split_once("\r\n\r\n").ok_or_else(|| anyhow!("no response from {url}"))?;
        let status: u32 = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        if status == 100 {
            rest = body;
            continue;
        }
        return Ok((status, head.to_string(), body.to_string()));
    }
}

fn write_private(p: &std::path::Path, text: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(p)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match (b[i], s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())) {
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
    let enc = |s: &str| s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect::<String>();
    pairs.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&")
}

fn base64url(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut s = String::new();
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    s
}

fn sha256(text: &str) -> Result<Vec<u8>> {
    let mut child = Command::new("openssl").args(["dgst", "-sha256", "-binary"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(text.as_bytes())?;
    Ok(child.wait_with_output()?.stdout)
}

// ---------- OAuth ----------

/// Browser sign-in. Blocks until the browser comes back to the local callback (or 5 minutes pass).
pub fn sign_in() -> Result<()> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let redirect = format!("http://localhost:{}/callback", listener.local_addr()?.port());
    let reg = json!({
        "client_name": "Cellworks", "redirect_uris": [redirect], "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"], "token_endpoint_auth_method": "none",
    });
    let (_, _, body) = post(&format!("{AUTH}/register"), &[], "application/json", &reg.to_string())?;
    let client_id = serde_json::from_str::<Value>(&body)?["client_id"].as_str().map(String::from).ok_or_else(|| anyhow!("registration failed: {body}"))?;
    let verifier = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let challenge = base64url(&sha256(&verifier)?);
    let state = uuid::Uuid::new_v4().simple().to_string();
    let url = format!("{AUTH}/authorize?{}", form(&[
        ("response_type", "code"), ("client_id", &client_id), ("code_challenge", &challenge), ("code_challenge_method", "S256"),
        ("redirect_uri", &redirect), ("state", &state),
    ]));
    Command::new("open").arg(&url).status()?;
    listener.set_nonblocking(false)?;
    let deadline = now() + 300;
    let code = loop {
        if now() > deadline {
            bail!("sign-in timed out");
        }
        let (mut conn, _) = listener.accept()?;
        let mut buf = [0u8; 4096];
        let n = conn.read(&mut buf)?;
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let query = req.split_whitespace().nth(1).and_then(|p| p.split_once('?')).map(|q| q.1.to_string()).unwrap_or_default();
        let get = |k: &str| query.split('&').find_map(|kv| kv.strip_prefix(&format!("{k}="))).map(percent_decode);
        let _ = conn.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<p style='font-family:sans-serif'>Cellworks is signed in to Atlassian. You can close this tab.</p>");
        if get("state").as_deref() == Some(state.as_str()) {
            if let Some(code) = get("code") {
                break code;
            }
            bail!("sign-in refused: {query}");
        }
    };
    let (_, _, body) = post(&format!("{AUTH}/token"), &[], "application/x-www-form-urlencoded", &form(&[
        ("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", &redirect), ("client_id", &client_id), ("code_verifier", &verifier),
    ]))?;
    save(&client_id, &body)
}

fn save(client_id: &str, token_response: &str) -> Result<()> {
    let v: Value = serde_json::from_str(token_response)?;
    let t = Tokens {
        client_id: client_id.into(),
        // an error response carries no secrets, so it is safe to show
        access_token: v["access_token"].as_str().ok_or_else(|| anyhow!("token request refused: {}", token_response.chars().take(300).collect::<String>()))?.into(),
        refresh_token: v["refresh_token"].as_str().unwrap_or_default().into(),
        expires_at: now() + v["expires_in"].as_u64().unwrap_or(3600),
    };
    std::fs::create_dir_all(tokens_path().parent().unwrap())?;
    write_private(&tokens_path(), &serde_json::to_string(&t)?)
}

/// A valid access token: stored, refreshed, or (first time) from a browser sign-in.
fn token() -> Result<String> {
    let Some(t) = std::fs::read_to_string(tokens_path()).ok().and_then(|s| serde_json::from_str::<Tokens>(&s).ok()) else {
        sign_in()?;
        return token();
    };
    if t.expires_at > now() + 60 {
        return Ok(t.access_token);
    }
    let (status, _, body) = post(&format!("{AUTH}/token"), &[], "application/x-www-form-urlencoded", &form(&[
        ("grant_type", "refresh_token"), ("refresh_token", &t.refresh_token), ("client_id", &t.client_id),
    ]))?;
    if status != 200 {
        sign_in()?; // refresh token expired or revoked
    } else {
        save(&t.client_id, &body)?;
    }
    let t: Tokens = serde_json::from_str(&std::fs::read_to_string(tokens_path())?)?;
    Ok(t.access_token)
}

// ---------- MCP ----------

struct Session {
    token: String,
    id: String,
    next: u64,
}

impl Session {
    fn open() -> Result<Self> {
        let token = token()?;
        let mut s = Session { token, id: String::new(), next: 1 };
        let (head, _) = s.rpc("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "cellworks", "version": "0.1"}}))?;
        s.id = head.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id")).map(|(_, v)| v.trim().to_string())).unwrap_or_default();
        s.notify("notifications/initialized")?;
        Ok(s)
    }

    fn headers(&self) -> Vec<(&str, String)> {
        let mut h = vec![("Authorization", format!("Bearer {}", self.token)), ("Accept", "application/json, text/event-stream".into())];
        if !self.id.is_empty() {
            h.push(("Mcp-Session-Id", self.id.clone()));
        }
        h
    }

    fn notify(&mut self, method: &str) -> Result<()> {
        let h = self.headers();
        let h: Vec<(&str, &str)> = h.iter().map(|(k, v)| (*k, v.as_str())).collect();
        post(MCP, &h, "application/json", &json!({"jsonrpc": "2.0", "method": method}).to_string())?;
        Ok(())
    }

    /// One JSON-RPC request; the answer comes as plain JSON or as server-sent events.
    fn rpc(&mut self, method: &str, params: Value) -> Result<(String, Value)> {
        let id = self.next;
        self.next += 1;
        let h = self.headers();
        let h: Vec<(&str, &str)> = h.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (status, head, body) = post(MCP, &h, "application/json", &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string())?;
        if status == 401 {
            let _ = std::fs::remove_file(tokens_path());
            bail!("Atlassian sign-in expired; sync again to sign in");
        }
        let msg = body.lines().filter_map(|l| l.strip_prefix("data:")).chain(std::iter::once(body.as_str()))
            .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok()).find(|v| v["id"] == id)
            .ok_or_else(|| anyhow!("{method}: HTTP {status}: {}", body.chars().take(300).collect::<String>()))?;
        if let Some(e) = msg.get("error") {
            bail!("{method}: {e}");
        }
        Ok((head, msg["result"].clone()))
    }

    fn tool(&mut self, name: &str, args: Value) -> Result<Value> {
        let (_, r) = self.rpc("tools/call", json!({"name": name, "arguments": args}))?;
        let text = r["content"][0]["text"].as_str().unwrap_or_default();
        if r["isError"] == true {
            bail!("{name}: {text}");
        }
        serde_json::from_str(text).with_context(|| format!("{name}: {}", text.chars().take(300).collect::<String>()))
    }
}

/// Fetches every ticket of the project and caches it. Returns how many.
pub fn sync(site: &str, key: &str, progress: &dyn Fn(usize)) -> Result<usize> {
    let mut s = Session::open()?;
    let mut out: Vec<Ticket> = vec![];
    let mut page: Option<String> = None;
    loop {
        let mut args = json!({
            "cloudId": site, "jql": format!("project = {key} ORDER BY updated DESC"), "maxResults": 100,
            "fields": ["summary", "status", "issuetype", "parent", "issuelinks", "assignee", "updated", "labels", "components"],
        });
        if let Some(p) = &page {
            args["nextPageToken"] = p.clone().into();
        }
        let r = s.tool("searchJiraIssuesUsingJql", args)?;
        // two answer shapes: plain Jira REST ({issues: [..], isLast, nextPageToken}) or {issues: {nodes, pageInfo}}
        let nodes = r["issues"].as_array().or_else(|| r["issues"]["nodes"].as_array());
        if out.is_empty() && nodes.is_none_or(|n| n.is_empty()) {
            bail!("no tickets in the answer: {}", r.to_string().chars().take(400).collect::<String>());
        }
        for n in nodes.into_iter().flatten() {
            out.push(ticket(n));
        }
        progress(out.len());
        page = match r["nextPageToken"].as_str() {
            Some(t) if r["isLast"] != true => Some(t.to_string()),
            _ => r["issues"]["pageInfo"]["endCursor"].as_str().filter(|_| r["issues"]["pageInfo"]["hasNextPage"] == true).map(String::from),
        };
        if page.is_none() {
            break;
        }
    }
    let p = cache_path(site, key);
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_string(&out)?)?;
    Ok(out.len())
}

fn ticket(n: &Value) -> Ticket {
    let f = &n["fields"];
    let s = |v: &Value| v.as_str().unwrap_or_default().to_string();
    Ticket {
        key: s(&n["key"]),
        summary: s(&f["summary"]).trim().to_string(),
        kind: s(&f["issuetype"]["name"]),
        level: f["issuetype"]["hierarchyLevel"].as_i64().unwrap_or(0),
        status: s(&f["status"]["name"]),
        cat: s(&f["status"]["statusCategory"]["key"]),
        parent: f["parent"]["key"].as_str().map(String::from),
        links: f["issuelinks"].as_array().into_iter().flatten().filter_map(|l| {
            match (l.get("outwardIssue"), l.get("inwardIssue")) {
                (Some(o), _) => Some((s(&l["type"]["outward"]), s(&o["key"]))),
                (_, Some(i)) => Some((s(&l["type"]["inward"]), s(&i["key"]))),
                _ => None,
            }
        }).collect(),
        assignee: f["assignee"]["displayName"].as_str().map(String::from),
        updated: s(&f["updated"]).chars().take(10).collect(),
        components: f["components"].as_array().into_iter().flatten().map(|c| s(&c["name"])).collect(),
        labels: f["labels"].as_array().into_iter().flatten().map(s).collect(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn percent_decoding() {
        assert_eq!(super::percent_decode("a%3Ab%2Fc+d"), "a:b/c d");
    }

    #[test]
    fn base64url_matches_rfc4648() {
        assert_eq!(super::base64url(b"foobar"), "Zm9vYmFy");
        assert_eq!(super::base64url(b"fo"), "Zm8");
        assert_eq!(super::base64url(&[0xfb, 0xff]), "-_8");
    }
}
