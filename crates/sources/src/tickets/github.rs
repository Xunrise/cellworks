//! GitHub issues through `gh` (ADR-0003): the issues assigned to me anywhere
//! (`gh search issues --assignee @me`) and the open issues of any configured
//! repositories (`gh issue list -R`). Status is open or closed: moving a
//! ticket to done closes it, to canceled closes it as not planned, to to-do
//! reopens it. A key reads `owner/repo#12`.
//!
//! Arguments always go to `gh` as separate argv entries, never through a
//! shell, and free text from outside follows `--`.

use serde_json::Value;

use super::http::run;
use super::{Category, SourceError, SourceId, Ticket, TicketSource, day};

const SEARCH_FIELDS: &str =
    "number,title,state,labels,assignees,updatedAt,url,repository,body,author";
const LIST_FIELDS: &str =
    "number,title,state,stateReason,labels,assignees,updatedAt,url,body,author";
const VIEW_FIELDS: &str =
    "number,title,state,stateReason,labels,assignees,updatedAt,url,body,author,comments";

pub struct GitHub {
    /// `owner/repo` to list open issues of, besides those assigned to me.
    repos: Vec<String>,
}

impl GitHub {
    pub fn new(repos: Vec<String>) -> GitHub {
        GitHub { repos }
    }
}

/// `owner/repo#12` as (`owner/repo`, `12`).
fn split(key: &str) -> Result<(&str, &str), SourceError> {
    key.split_once('#')
        .filter(|(repo, number)| {
            repo.split('/').count() == 2
                && !repo.starts_with('-')
                && number.chars().all(|c| c.is_ascii_digit())
                && !number.is_empty()
        })
        .ok_or_else(|| SourceError::Other(format!("not a GitHub issue key: {key}")))
}

fn parse(json: &str, repo: Option<&str>) -> Result<Vec<Ticket>, SourceError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|_| SourceError::Other("gh: unreadable answer".into()))?;
    Ok(match &value {
        Value::Array(items) => items.iter().map(|i| ticket(i, repo)).collect(),
        one => vec![ticket(one, repo)],
    })
}

impl TicketSource for GitHub {
    fn id(&self) -> SourceId {
        SourceId::GitHub
    }

    fn list_mine(&self) -> Result<Vec<Ticket>, SourceError> {
        let mut out = parse(
            &run(
                "gh",
                &[
                    "search",
                    "issues",
                    "--assignee",
                    "@me",
                    "--state",
                    "open",
                    "--limit",
                    "100",
                    "--json",
                    SEARCH_FIELDS,
                ],
            )?,
            None,
        )?;
        for repo in &self.repos {
            let listed = parse(
                &run(
                    "gh",
                    &[
                        "issue",
                        "list",
                        "-R",
                        repo,
                        "--state",
                        "open",
                        "--limit",
                        "50",
                        "--json",
                        LIST_FIELDS,
                    ],
                )?,
                Some(repo),
            )?;
            for ticket in listed {
                if !out.iter().any(|t| t.key == ticket.key) {
                    out.push(ticket);
                }
            }
        }
        Ok(out)
    }

    fn list(&self, query: &str) -> Result<Vec<Ticket>, SourceError> {
        parse(
            &run(
                "gh",
                &[
                    "search",
                    "issues",
                    "--state",
                    "open",
                    "--limit",
                    "50",
                    "--json",
                    SEARCH_FIELDS,
                    "--",
                    query,
                ],
            )?,
            None,
        )
    }

    fn get(&self, key: &str) -> Result<Ticket, SourceError> {
        let (repo, number) = split(key)?;
        let json = run(
            "gh",
            &["issue", "view", number, "-R", repo, "--json", VIEW_FIELDS],
        )?;
        parse(&json, Some(repo))?
            .pop()
            .ok_or_else(|| SourceError::Other("gh: no issue".into()))
    }

    fn set_status(&self, key: &str, to: Category) -> Result<Ticket, SourceError> {
        let (repo, number) = split(key)?;
        let args: &[&str] = match to {
            Category::Done => &["issue", "close", number, "-R", repo],
            Category::Canceled => &[
                "issue",
                "close",
                number,
                "-R",
                repo,
                "--reason",
                "not planned",
            ],
            Category::Todo => &["issue", "reopen", number, "-R", repo],
            Category::InProgress => {
                return Err(SourceError::Other(
                    "GitHub issues are only open or closed".into(),
                ));
            }
        };
        run("gh", args)?;
        self.get(key)
    }

    fn claim(&self, key: &str) -> Result<Ticket, SourceError> {
        let (repo, number) = split(key)?;
        run(
            "gh",
            &["issue", "edit", number, "-R", repo, "--add-assignee", "@me"],
        )?;
        self.get(key)
    }

    fn writable(&self) -> bool {
        true
    }
}

/// One issue of `gh`'s JSON; `repo` is the repository when the listing does
/// not say (`gh issue list`).
fn ticket(v: &Value, repo: Option<&str>) -> Ticket {
    let text = |p: &Value| p.as_str().unwrap_or_default().to_owned();
    let repo = v["repository"]["nameWithOwner"]
        .as_str()
        .or(repo)
        .unwrap_or_default()
        .to_owned();
    let state = text(&v["state"]).to_lowercase();
    let reason = text(&v["stateReason"]).to_lowercase();
    let category = match (state.as_str(), reason.as_str()) {
        ("closed", "not_planned") => Category::Canceled,
        ("closed", _) => Category::Done,
        _ => Category::Todo,
    };
    let number = v["number"].as_u64().unwrap_or(0);
    Ticket {
        source: SourceId::GitHub,
        key: format!("{repo}#{number}"),
        title: text(&v["title"]),
        body: text(&v["body"]).replace("\r\n", "\n"),
        status: state,
        category,
        assignee: v["assignees"][0]["login"].as_str().map(str::to_owned),
        reporter: v["author"]["login"].as_str().map(str::to_owned),
        labels: v["labels"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l["name"].as_str().map(str::to_owned))
            .collect(),
        url: text(&v["url"]),
        project: repo.clone(),
        comments: v["comments"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                (
                    text(&c["author"]["login"]),
                    day(&text(&c["createdAt"])),
                    text(&c["body"]),
                )
            })
            .collect(),
        updated: day(&text(&v["updatedAt"])),
        repo: Some(repo).filter(|r| !r.is_empty()),
        ..Ticket::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recorded_search_becomes_tickets() {
        let json = include_str!("../../tests/fixtures/tickets/github_search.json");
        let tickets = parse(json, None).unwrap();
        let first = &tickets[0];
        assert_eq!(first.source, SourceId::GitHub);
        assert_eq!(first.key, "acme/FAN_app#5");
        assert_eq!(first.repo.as_deref(), Some("acme/FAN_app"));
        assert_eq!(first.category, Category::Todo);
        assert_eq!(first.assignee.as_deref(), Some("dev"));
        assert_eq!(first.updated, "2023-04-16");
    }

    #[test]
    fn a_listing_without_repositories_takes_the_one_asked() {
        let json = r#"[{"number": 9, "title": "t", "state": "CLOSED", "stateReason": "NOT_PLANNED",
            "labels": [{"name": "bug"}], "assignees": [], "updatedAt": "2026-01-02T03:04:05Z", "url": "u", "body": "b"}]"#;
        let t = parse(json, Some("acme/api")).unwrap().remove(0);
        assert_eq!(t.key, "acme/api#9");
        assert_eq!(t.category, Category::Canceled);
        assert_eq!(t.labels, ["bug"]);
        assert_eq!(t.assignee, None);
    }

    #[test]
    fn a_viewed_issue_carries_its_comments() {
        let json = r#"{"number": 3, "title": "t", "state": "OPEN", "stateReason": "REOPENED",
            "comments": [{"author": {"login": "ada"}, "createdAt": "2026-02-01T00:00:00Z", "body": "me too"}]}"#;
        let t = parse(json, Some("acme/api")).unwrap().remove(0);
        assert_eq!(
            t.comments,
            [("ada".into(), "2026-02-01".into(), "me too".into())]
        );
    }

    #[test]
    fn keys_split_into_repository_and_number_or_are_refused() {
        assert_eq!(split("acme/api#12").unwrap(), ("acme/api", "12"));
        for bad in ["api#12", "acme/api#", "acme/api#1x", "-x/api#1", "SHOP-42"] {
            assert!(split(bad).is_err(), "{bad}");
        }
    }
}
