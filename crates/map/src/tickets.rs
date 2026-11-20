//! The neutral ticket model the case and the Tickets panel draw: one
//! [`Ticket`] whatever tracker it came from, and the [`TicketDesk`] the UI
//! asks for tickets.
//!
//! The model lives here, not in `cellworks-sources`, because the Map's UI
//! renders it and the Sources crate already depends on this one. Sources owns
//! the trackers: it fills the model and implements [`TicketDesk`] (see
//! `cellworks_sources::tickets`). Nothing in this module does I/O except
//! [`clock`].

use std::fmt;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Where a ticket lives.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "lowercase")]
pub enum SourceId {
    #[default]
    Jira,
    Cockpit,
    GitHub,
    Linear,
}

impl SourceId {
    pub const ALL: [SourceId; 4] = [
        SourceId::Jira,
        SourceId::Cockpit,
        SourceId::GitHub,
        SourceId::Linear,
    ];

    /// The lowercase id used in `<source>:<key>` and in cache folders.
    pub fn id(self) -> &'static str {
        match self {
            SourceId::Jira => "jira",
            SourceId::Cockpit => "cockpit",
            SourceId::GitHub => "github",
            SourceId::Linear => "linear",
        }
    }

    /// The name shown to the engineer.
    pub fn name(self) -> &'static str {
        match self {
            SourceId::Jira => "Jira",
            SourceId::Cockpit => "Cockpit",
            SourceId::GitHub => "GitHub",
            SourceId::Linear => "Linear",
        }
    }

    fn from_id(id: &str) -> Option<SourceId> {
        SourceId::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// Where a ticket stands, the same four things in every tracker.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    #[default]
    Todo,
    InProgress,
    Done,
    Canceled,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Todo,
        Category::InProgress,
        Category::Done,
        Category::Canceled,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Category::Todo => "To do",
            Category::InProgress => "In progress",
            Category::Done => "Done",
            Category::Canceled => "Canceled",
        }
    }

    /// Whether the ticket still wants work.
    pub fn is_open(self) -> bool {
        matches!(self, Category::Todo | Category::InProgress)
    }
}

/// One ticket, as the case and the panel show it.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Ticket {
    pub source: SourceId,
    /// The display key: SHOP-42, TIC-2, owner/repo#12, ENG-7.
    pub key: String,
    pub title: String,
    /// The description as plain text.
    pub body: String,
    /// Bug, story, task...: whatever the tracker calls it; may be empty.
    pub kind: String,
    /// The tracker's own status name.
    pub status: String,
    pub category: Category,
    pub priority: String,
    pub assignee: Option<String>,
    pub reporter: Option<String>,
    pub labels: Vec<String>,
    pub url: String,
    /// The project or team the tracker groups it under.
    pub project: String,
    /// The repository it concerns, a path or a name, when the tracker knows.
    pub repo: Option<String>,
    /// The acceptance criteria, one per entry, and where they were found.
    pub criteria: Vec<String>,
    pub criteria_from: String,
    /// (how the link reads from this ticket, the other key, its title).
    pub links: Vec<(String, String, String)>,
    /// The latest comments, newest last: (author, created, body).
    pub comments: Vec<(String, String, String)>,
    /// "YYYY-MM-DD".
    pub updated: String,
}

impl Ticket {
    pub fn reference(&self) -> TicketRef {
        TicketRef {
            source: self.source,
            key: self.key.clone(),
        }
    }
}

/// A ticket's identity: its source and its display key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TicketRef {
    pub source: SourceId,
    pub key: String,
}

impl fmt::Display for TicketRef {
    /// The stable form, `<source>:<key>`, that case files are named by.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.source.id(), self.key)
    }
}

impl TicketRef {
    /// Reads `<source>:<key>`, or a bare key: `owner/repo#12` is GitHub,
    /// anything else Jira (so the keys older case files carry still load).
    pub fn parse(text: &str) -> TicketRef {
        let text = text.trim();
        let (named, key) = match text.split_once(':') {
            Some((prefix, key)) => match SourceId::from_id(&prefix.to_lowercase()) {
                Some(source) => (Some(source), key),
                None => (None, text),
            },
            None => (None, text),
        };
        let source = named.unwrap_or(if key.contains('#') {
            SourceId::GitHub
        } else {
            SourceId::Jira
        });
        let key = if source == SourceId::GitHub {
            key.to_owned()
        } else {
            key.to_uppercase()
        };
        TicketRef { source, key }
    }

    /// A file name for this ticket: bare for Jira (existing case files),
    /// `<source>-<key>` with path characters replaced for the rest.
    pub fn file_stem(&self) -> String {
        let safe: String = self
            .key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        match self.source {
            SourceId::Jira => safe,
            other => format!("{}-{safe}", other.id()),
        }
    }
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

/// Where a source's sign-in stands (only Jira signs in).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum SignIn {
    #[default]
    Unknown,
    Needed,
    /// The browser was opened; waiting for it to come back.
    Pending,
    Failed(String),
    Done,
}

/// What the panel shows about one source.
#[derive(Clone, Debug)]
pub struct SourceState {
    pub id: SourceId,
    pub synced: Option<Synced>,
    pub fetching: bool,
    pub failure: Option<String>,
    pub sign_in: SignIn,
    /// Whether tickets of this source can change status from here.
    pub writable: bool,
}

/// What the case and the Tickets panel ask for tickets. `cellworks_sources`
/// implements it over the enabled trackers; calls never block.
pub trait TicketDesk {
    /// Takes what the background calls brought back; once per frame.
    fn poll(&mut self);
    /// The ticket, from this session or the cache; asks for it fresh once.
    fn ticket(&mut self, which: &TicketRef) -> Option<(Ticket, Synced)>;
    /// My tickets across the enabled sources, from this session or the cache;
    /// asks each source once.
    fn all(&mut self) -> Vec<(Ticket, Synced)>;
    /// The known tickets whose repository hint names `repo`; never asks a
    /// source, so it is cheap enough for every frame.
    fn for_repo(&mut self, repo: &Path) -> Vec<Ticket>;
    /// The enabled sources and how they stand.
    fn sources(&self) -> Vec<SourceState>;
    /// Asks every enabled source for its list again.
    fn refresh(&mut self);
    fn is_fetching(&self, which: &TicketRef) -> bool;
    fn failure(&self, which: &TicketRef) -> Option<String>;
    /// Moves the ticket to `category` at its tracker, on a background thread.
    fn set_status(&mut self, which: &TicketRef, category: Category);
    fn sign_in(&self, source: SourceId) -> SignIn;
    fn start_sign_in(&mut self, source: SourceId);
    /// Whether the Cockpit hub answered at the last look.
    fn cockpit_up(&self) -> bool;
    /// Whether the hub already mirrors this tracker ticket.
    fn in_cockpit(&self, which: &TicketRef) -> bool;
    /// What the last "send" said ("TIC-5 created", or the error).
    fn sent(&self, which: &TicketRef) -> Option<Result<String, String>>;
    /// Brings the ticket into Cockpit, on a background thread.
    fn send_to_cockpit(&mut self, which: &TicketRef);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_keys_are_jira_and_prefixed_ones_name_their_source() {
        let jira = TicketRef::parse("shop-42");
        assert_eq!(
            (jira.source, jira.key.as_str()),
            (SourceId::Jira, "SHOP-42")
        );
        assert_eq!(jira.to_string(), "jira:SHOP-42");
        assert_eq!(TicketRef::parse(&jira.to_string()), jira);
        let gh = TicketRef::parse("acme/api#12");
        assert_eq!(
            (gh.source, gh.key.as_str()),
            (SourceId::GitHub, "acme/api#12")
        );
        assert_eq!(TicketRef::parse(&gh.to_string()), gh);
        assert_eq!(TicketRef::parse("cockpit:tic-2").source, SourceId::Cockpit);
        assert_eq!(TicketRef::parse("Linear:eng-7").key, "ENG-7");
    }

    #[test]
    fn jira_case_files_keep_their_bare_names() {
        assert_eq!(TicketRef::parse("SHOP-42").file_stem(), "SHOP-42");
        assert_eq!(
            TicketRef::parse("github:acme/api#12").file_stem(),
            "github-acme_api_12"
        );
    }
}
