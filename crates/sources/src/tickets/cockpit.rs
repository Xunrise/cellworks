//! Cockpit, the local ticket tracker: an HTTP daemon on 127.0.0.1:4747.
//!
//! `GET /api/state` lists every ticket and the repos table (name to path);
//! `POST /api/tools/<name>` with a JSON body and the header `x-tk: 1` calls a
//! tool. Statuses are backlog, todo, doing, done and canceled. Without the
//! daemon the same tools run through `cockpit tool NAME '{json}'`.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::http::{request, run};
use super::{Category, SourceError, SourceId, Ticket, TicketSource, day};

pub struct Cockpit {
    base: String,
}

impl Cockpit {
    pub fn new(url: &str) -> Cockpit {
        Cockpit {
            base: url.trim_end_matches('/').to_owned(),
        }
    }

    /// One tool call: through the daemon, else through the `cockpit` binary.
    fn call(&self, tool: &str, args: &Value) -> Result<Value, SourceError> {
        let body = args.to_string();
        let text = match request(
            &format!("{}/api/tools/{tool}", self.base),
            &[("x-tk", "1")],
            Some(("application/json", &body)),
            10,
        ) {
            Ok(r) if r.status == 200 => r.body,
            Ok(r) => {
                return Err(SourceError::Other(format!(
                    "cockpit {tool}: HTTP {} {}",
                    r.status,
                    r.body.chars().take(200).collect::<String>()
                )));
            }
            // The daemon is not there: the binary talks to the same store.
            Err(_) => run("cockpit", &["tool", tool, &body])?,
        };
        let value: Value = serde_json::from_str(&text)
            .map_err(|_| SourceError::Other(format!("cockpit {tool}: unreadable answer")))?;
        match value["error"].as_str() {
            Some(error) => Err(SourceError::Other(format!("cockpit {tool}: {error}"))),
            None => Ok(value),
        }
    }

    /// Refs (`jira:KEY`...) of the tracker tickets the hub already mirrors.
    pub fn hub_refs(&self) -> Result<Vec<String>, SourceError> {
        let rows = self.call("ticket.list", &json!({}))?;
        let rows = if rows.is_array() {
            &rows
        } else {
            &rows["tickets"]
        };
        Ok(array(rows)
            .iter()
            .filter_map(|r| r["ref"].as_str())
            .filter(|r| !r.starts_with("cockpit:"))
            .map(str::to_owned)
            .collect())
    }

    /// Brings `t` into the hub: `ticket.pull`, else `ticket.mirror` when the
    /// hub lacks the tracker's sign-in. Answers "TIC-5 created" or "... updated".
    pub fn send(&self, t: &Ticket) -> Result<String, SourceError> {
        let r = format!("{}:{}", t.source.id(), t.key);
        let answer = match self.call("ticket.pull", &json!({ "ref": r })) {
            Ok(a) => a,
            Err(SourceError::Other(e)) if lacks_sign_in(&e) => {
                let status = status_name(t.category);
                self.call(
                    "ticket.mirror",
                    &json!({"source": t.source.id(), "external_key": t.key, "title": t.title,
                        "status": status, "external_status": t.status, "body": t.body,
                        "url": t.url, "updated": t.updated}),
                )?
            }
            Err(e) => return Err(e),
        };
        Ok(format!(
            "{} {}",
            answer["key"].as_str().unwrap_or("?"),
            if answer["created"].as_bool() == Some(true) {
                "created"
            } else {
                "updated"
            }
        ))
    }

    /// The daemon's state: every ticket and the repos table.
    fn state(&self) -> Result<Value, String> {
        let r = request(&format!("{}/api/state", self.base), &[], None, 10)?;
        if r.status != 200 {
            return Err(format!("cockpit: HTTP {}", r.status));
        }
        serde_json::from_str(&r.body).map_err(|_| "cockpit: unreadable state".to_owned())
    }

    /// Repo name to path; empty when the daemon does not say.
    fn repos(&self) -> BTreeMap<String, String> {
        self.state()
            .map(|state| repos_of(&state))
            .unwrap_or_default()
    }

    /// Every ticket, from the state, else from each project's overview.
    fn all(&self) -> Result<Vec<Ticket>, SourceError> {
        match self.state() {
            Ok(state) => {
                let repos = repos_of(&state);
                Ok(array(&state["tickets"])
                    .iter()
                    .map(|t| ticket(t, &repos))
                    .collect())
            }
            Err(_) => {
                let mut out = Vec::new();
                for project in array(&self.call("project.list", &json!({}))?) {
                    if project["tickets"].as_u64().unwrap_or(0) == 0 {
                        continue;
                    }
                    let name = project["name"].as_str().unwrap_or_default();
                    let overview = self.call("project.overview", &json!({ "project": name }))?;
                    out.extend(overview_tickets(&overview));
                }
                Ok(out)
            }
        }
    }
}

impl TicketSource for Cockpit {
    fn id(&self) -> SourceId {
        SourceId::Cockpit
    }

    fn list_mine(&self) -> Result<Vec<Ticket>, SourceError> {
        self.all()
    }

    fn list(&self, query: &str) -> Result<Vec<Ticket>, SourceError> {
        let query = query.to_lowercase();
        Ok(self
            .all()?
            .into_iter()
            .filter(|t| {
                format!("{} {} {}", t.key, t.title, t.body)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect())
    }

    fn get(&self, key: &str) -> Result<Ticket, SourceError> {
        let raw = self.call("ticket.get", &json!({ "key": key }))?;
        Ok(ticket(&raw, &self.repos()))
    }

    fn set_status(&self, key: &str, to: Category) -> Result<Ticket, SourceError> {
        self.call(
            "ticket.update",
            &json!({ "key": key, "status": status_name(to) }),
        )?;
        self.get(key)
    }

    /// Claiming is starting: the ticket moves to doing.
    fn claim(&self, key: &str) -> Result<Ticket, SourceError> {
        self.set_status(key, Category::InProgress)
    }

    fn writable(&self) -> bool {
        true
    }
}

/// Whether a `ticket.pull` error says the hub has no sign-in or key for the tracker.
fn lacks_sign_in(error: &str) -> bool {
    let e = error.to_lowercase();
    e.contains("sign") || e.contains("api key")
}

fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or_default()
}

fn repos_of(state: &Value) -> BTreeMap<String, String> {
    array(&state["repos"])
        .iter()
        .filter_map(|r| {
            Some((
                r["name"].as_str()?.to_owned(),
                r["path"].as_str()?.to_owned(),
            ))
        })
        .collect()
}

/// Every ticket a project overview lists, whatever its buckets are called.
fn overview_tickets(overview: &Value) -> Vec<Ticket> {
    let none = BTreeMap::new();
    overview
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(_, bucket)| array(bucket))
        .filter(|t| t["key"].is_string() && t["title"].is_string())
        .map(|t| ticket(t, &none))
        .collect()
}

pub(super) fn category(status: &str) -> Category {
    match status {
        "doing" => Category::InProgress,
        "done" => Category::Done,
        "canceled" => Category::Canceled,
        _ => Category::Todo,
    }
}

fn status_name(category: Category) -> &'static str {
    match category {
        Category::Todo => "todo",
        Category::InProgress => "doing",
        Category::Done => "done",
        Category::Canceled => "canceled",
    }
}

/// A cockpit ticket; its repo is the path the repos table gives, else the name.
fn ticket(t: &Value, repos: &BTreeMap<String, String>) -> Ticket {
    let text = |k: &str| t[k].as_str().unwrap_or_default().to_owned();
    let repo = t["repo"].as_str().filter(|r| !r.is_empty());
    let status = text("status");
    Ticket {
        source: SourceId::Cockpit,
        key: text("key"),
        title: text("title"),
        body: text("body"),
        category: category(&status),
        status,
        priority: t["priority"]
            .as_i64()
            .map(|p| format!("P{p}"))
            .unwrap_or_default(),
        labels: t["cluster"]
            .as_str()
            .map(|c| vec![c.to_owned()])
            .unwrap_or_default(),
        project: text("project"),
        repo: repo.map(|r| repos.get(r).cloned().unwrap_or_else(|| r.to_owned())),
        updated: day(&text("updated")),
        ..Ticket::default()
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!(
            "{}/tests/fixtures/tickets/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn the_live_state_lists_tickets_with_their_repo_paths() {
        let state = fixture("cockpit_state.json");
        let repos = repos_of(&state);
        let tickets: Vec<Ticket> = array(&state["tickets"])
            .iter()
            .map(|t| ticket(t, &repos))
            .collect();
        assert_eq!(tickets.len(), 2);
        let two = tickets.iter().find(|t| t.key == "TIC-2").unwrap();
        assert_eq!(two.category, Category::Todo);
        assert_eq!(two.priority, "P3");
        assert_eq!(two.project, "ticket-cockpit");
        assert_eq!(two.updated, "2026-11-18");
        assert!(two.repo.as_deref().unwrap().ends_with("/ticket-cockpit"));
        let one = tickets.iter().find(|t| t.key == "TIC-1").unwrap();
        assert_eq!(one.category, Category::Done);
    }

    #[test]
    fn a_ticket_get_and_an_overview_read_the_same_way() {
        let got = ticket(&fixture("cockpit_ticket.json"), &BTreeMap::new());
        assert_eq!(
            (got.key.as_str(), got.repo.as_deref()),
            ("TIC-2", Some("ticket-cockpit"))
        );
        let listed = overview_tickets(&fixture("cockpit_overview.json"));
        let mut keys: Vec<&str> = listed.iter().map(|t| t.key.as_str()).collect();
        keys.sort();
        assert_eq!(keys, ["TIC-1", "TIC-2"]);
    }

    /// A one-thread HTTP stub: answers `POST /api/tools/<name>` with the
    /// canned body for `name` (else an error body) and records the calls.
    pub(in crate::tickets) fn stub(
        answers: &[(&str, &str)],
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let answers: BTreeMap<String, String> = answers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = calls.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let tool = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|p| p.strip_prefix("/api/tools/"))
                    .unwrap_or_default()
                    .to_owned();
                let mut length = 0;
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                log.lock()
                    .unwrap()
                    .push(format!("{tool} {}", String::from_utf8_lossy(&body)));
                let answer = answers
                    .get(&tool)
                    .cloned()
                    .unwrap_or_else(|| r#"{"error":"no such tool"}"#.to_owned());
                let _ = write!(
                    reader.get_mut(),
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                    answer.len()
                );
            }
        });
        (url, calls)
    }

    fn linear() -> Ticket {
        Ticket {
            source: SourceId::Linear,
            key: "ENG-7".into(),
            title: "Fix it".into(),
            body: "details".into(),
            status: "In Review".into(),
            category: Category::InProgress,
            url: "https://linear.app/x/ENG-7".into(),
            updated: "2026-11-19".into(),
            ..Ticket::default()
        }
    }

    #[test]
    fn send_pulls_and_reports_created_or_updated() {
        let (url, calls) = stub(&[(
            "ticket.pull",
            r#"{"key":"ENG-7","created":false,"ticket":{}}"#,
        )]);
        let done = Cockpit::new(&url).send(&linear()).unwrap();
        assert_eq!(done, "ENG-7 updated");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].starts_with("ticket.pull ") && calls[0].contains("linear:ENG-7"));
    }

    #[test]
    fn send_mirrors_what_cellworks_holds_when_the_hub_has_no_sign_in() {
        let (url, calls) = stub(&[
            ("ticket.pull", r#"{"error":"Linear needs an API key"}"#),
            (
                "ticket.mirror",
                r#"{"key":"linear-ENG-7","created":true,"ticket":{}}"#,
            ),
        ]);
        let done = Cockpit::new(&url).send(&linear()).unwrap();
        assert_eq!(done, "linear-ENG-7 created");
        let calls = calls.lock().unwrap();
        let mirror: Value =
            serde_json::from_str(calls[1].strip_prefix("ticket.mirror ").unwrap()).unwrap();
        assert_eq!(
            (
                mirror["source"].as_str(),
                mirror["external_key"].as_str(),
                mirror["status"].as_str(),
                mirror["external_status"].as_str(),
                mirror["title"].as_str(),
            ),
            (
                Some("linear"),
                Some("ENG-7"),
                Some("doing"),
                Some("In Review"),
                Some("Fix it")
            )
        );
    }

    #[test]
    fn send_shows_other_errors_without_mirroring() {
        let (url, calls) = stub(&[("ticket.pull", r#"{"error":"Issue does not exist"}"#)]);
        let error = Cockpit::new(&url).send(&linear()).unwrap_err();
        assert!(error.to_string().contains("Issue does not exist"));
        assert_eq!(calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn hub_refs_lists_mirrors_but_not_cockpit_own_tickets() {
        let (url, _) = stub(&[(
            "ticket.list",
            r#"[{"ref":"cockpit:TIC-2"},{"ref":"jira:SHOP-42"},{"ref":"github:a/b#1"}]"#,
        )]);
        assert_eq!(
            Cockpit::new(&url).hub_refs().unwrap(),
            ["jira:SHOP-42", "github:a/b#1"]
        );
    }

    #[test]
    fn a_missing_sign_in_is_recognised() {
        assert!(lacks_sign_in("Jira: not signed in"));
        assert!(lacks_sign_in("Linear needs an API key"));
        assert!(!lacks_sign_in("Issue does not exist"));
    }

    #[test]
    fn statuses_round_trip() {
        for c in Category::ALL {
            assert_eq!(category(status_name(c)), c);
        }
        assert_eq!(category("backlog"), Category::Todo);
    }
}
