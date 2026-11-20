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
mod tests {
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

    #[test]
    fn statuses_round_trip() {
        for c in Category::ALL {
            assert_eq!(category(status_name(c)), c);
        }
        assert_eq!(category("backlog"), Category::Todo);
    }
}
