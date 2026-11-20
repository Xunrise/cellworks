//! Linear over its GraphQL API (`https://api.linear.app/graphql`) through
//! curl, with a personal API key in the `Authorization` header. Without a key
//! the facade does not build this source at all.
//!
//! Moving a ticket picks the team's workflow state of the matching type:
//! unstarted (else backlog) for to-do, started, completed, canceled.

use serde_json::{Value, json};

use super::http::request;
use super::{Category, SourceError, SourceId, Ticket, TicketSource, day};

const API: &str = "https://api.linear.app/graphql";

const FIELDS: &str = "fragment F on Issue { identifier title description url priorityLabel updatedAt \
    state { name type } assignee { name } creator { name } team { key name } \
    labels { nodes { name } } \
    comments(first: 5) { nodes { body createdAt user { name } } } \
    relations { nodes { type relatedIssue { identifier title } } } }";

pub struct Linear {
    key: String,
}

impl Linear {
    pub fn new(api_key: &str) -> Linear {
        Linear {
            key: api_key.trim().to_owned(),
        }
    }

    /// One GraphQL call; answers its `data`.
    fn graphql(&self, query: &str, variables: Value) -> Result<Value, SourceError> {
        let body =
            json!({ "query": format!("{query} {FIELDS}"), "variables": variables }).to_string();
        let r = request(
            API,
            &[("Authorization", &self.key)],
            Some(("application/json", &body)),
            20,
        )?;
        let value: Value = serde_json::from_str(&r.body)
            .map_err(|_| SourceError::Other(format!("Linear: HTTP {}", r.status)))?;
        if let Some(message) = value["errors"][0]["message"].as_str() {
            return Err(SourceError::Other(format!("Linear: {message}")));
        }
        if r.status == 401 || r.status == 403 {
            return Err(SourceError::Other("Linear: the API key was refused".into()));
        }
        Ok(value["data"].clone())
    }

    fn issue(&self, key: &str) -> Result<Ticket, SourceError> {
        let data = self.graphql(
            "query($id: String!) { issue(id: $id) { ...F } }",
            json!({ "id": key }),
        )?;
        Ok(ticket(&data["issue"]))
    }
}

impl TicketSource for Linear {
    fn id(&self) -> SourceId {
        SourceId::Linear
    }

    fn list_mine(&self) -> Result<Vec<Ticket>, SourceError> {
        let data = self.graphql(
            "query { viewer { assignedIssues(first: 50, filter: { state: { type: { nin: [\"completed\", \"canceled\"] } } }) { nodes { ...F } } } }",
            json!({}),
        )?;
        Ok(nodes(&data["viewer"]["assignedIssues"]))
    }

    fn list(&self, query: &str) -> Result<Vec<Ticket>, SourceError> {
        let data = self.graphql(
            "query($q: String!) { issues(first: 30, filter: { searchableContent: { contains: $q } }) { nodes { ...F } } }",
            json!({ "q": query }),
        )?;
        Ok(nodes(&data["issues"]))
    }

    fn get(&self, key: &str) -> Result<Ticket, SourceError> {
        self.issue(key)
    }

    fn set_status(&self, key: &str, to: Category) -> Result<Ticket, SourceError> {
        let data = self.graphql(
            "query($id: String!) { issue(id: $id) { id team { states { nodes { id name type position } } } } }",
            json!({ "id": key }),
        )?;
        let id = data["issue"]["id"].as_str().unwrap_or_default();
        let state = pick_state(&data["issue"]["team"]["states"], to).ok_or_else(|| {
            SourceError::Other(format!("Linear: no {} state in the team", to.name()))
        })?;
        self.graphql(
            "mutation($id: String!, $state: String!) { issueUpdate(id: $id, input: { stateId: $state }) { success } }",
            json!({ "id": id, "state": state }),
        )?;
        self.issue(key)
    }

    fn claim(&self, key: &str) -> Result<Ticket, SourceError> {
        let me = self.graphql("query { viewer { id } }", json!({}))?;
        self.graphql(
            "mutation($id: String!, $me: String!) { issueUpdate(id: $id, input: { assigneeId: $me }) { success } }",
            json!({ "id": key, "me": me["viewer"]["id"] }),
        )?;
        self.issue(key)
    }

    fn writable(&self) -> bool {
        true
    }
}

fn nodes(connection: &Value) -> Vec<Ticket> {
    connection["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(ticket)
        .collect()
}

/// The id of the team's workflow state that `to` means: the first (by
/// position) of the matching type.
fn pick_state(states: &Value, to: Category) -> Option<String> {
    let wanted: &[&str] = match to {
        Category::Todo => &["unstarted", "backlog"],
        Category::InProgress => &["started"],
        Category::Done => &["completed"],
        Category::Canceled => &["canceled"],
    };
    let all = states["nodes"].as_array()?;
    wanted.iter().find_map(|kind| {
        all.iter()
            .filter(|s| s["type"] == *kind)
            .min_by(|a, b| {
                a["position"]
                    .as_f64()
                    .partial_cmp(&b["position"].as_f64())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|s| s["id"].as_str().map(str::to_owned))
    })
}

fn category(kind: &str) -> Category {
    match kind {
        "started" => Category::InProgress,
        "completed" => Category::Done,
        "canceled" => Category::Canceled,
        _ => Category::Todo,
    }
}

fn ticket(v: &Value) -> Ticket {
    let text = |p: &Value| p.as_str().unwrap_or_default().to_owned();
    let priority = text(&v["priorityLabel"]);
    Ticket {
        source: SourceId::Linear,
        key: text(&v["identifier"]),
        title: text(&v["title"]),
        body: text(&v["description"]),
        status: text(&v["state"]["name"]),
        category: category(v["state"]["type"].as_str().unwrap_or_default()),
        priority: if priority == "No priority" {
            String::new()
        } else {
            priority
        },
        assignee: v["assignee"]["name"].as_str().map(str::to_owned),
        reporter: v["creator"]["name"].as_str().map(str::to_owned),
        labels: v["labels"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l["name"].as_str().map(str::to_owned))
            .collect(),
        url: text(&v["url"]),
        project: text(&v["team"]["name"]),
        links: v["relations"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|r| {
                (
                    text(&r["type"]),
                    text(&r["relatedIssue"]["identifier"]),
                    text(&r["relatedIssue"]["title"]),
                )
            })
            .collect(),
        comments: v["comments"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                (
                    text(&c["user"]["name"]),
                    day(&text(&c["createdAt"])),
                    text(&c["body"]),
                )
            })
            .collect(),
        updated: day(&text(&v["updatedAt"])),
        ..Ticket::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        let path = format!(
            "{}/tests/fixtures/tickets/linear_assigned.json",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn assigned_issues_become_tickets() {
        let tickets = nodes(&fixture()["data"]["viewer"]["assignedIssues"]);
        assert_eq!(tickets.len(), 2);
        let t = &tickets[0];
        assert_eq!((t.key.as_str(), t.source), ("ENG-7", SourceId::Linear));
        assert_eq!(t.category, Category::InProgress);
        assert_eq!(t.status, "In Progress");
        assert_eq!(t.priority, "High");
        assert_eq!(t.project, "Engineering");
        assert_eq!(t.labels, ["backend"]);
        assert_eq!(t.updated, "2026-11-10");
        assert_eq!(
            t.links[0],
            ("blocks".into(), "ENG-9".into(), "Cache layer".into())
        );
        assert_eq!(t.comments[0].0, "Grace");
        // "No priority" is no priority.
        assert_eq!(tickets[1].priority, "");
        assert_eq!(tickets[1].category, Category::Todo);
    }

    #[test]
    fn a_status_change_picks_the_teams_state_of_that_type() {
        let states = &fixture()["data"]["issue"]["team"]["states"];
        assert_eq!(
            pick_state(states, Category::Todo).as_deref(),
            Some("s-todo")
        );
        assert_eq!(
            pick_state(states, Category::InProgress).as_deref(),
            Some("s-doing")
        );
        assert_eq!(
            pick_state(states, Category::Done).as_deref(),
            Some("s-done")
        );
        assert_eq!(
            pick_state(states, Category::Canceled).as_deref(),
            Some("s-cancel")
        );
        assert_eq!(
            pick_state(
                &json!({"nodes": [{"id": "b", "type": "backlog", "position": 1}]}),
                Category::Todo
            )
            .as_deref(),
            Some("b")
        );
    }
}
