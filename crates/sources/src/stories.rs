//! A story's text, read from the issue whose title names it: `As a`, `I
//! want` and `So that`, for the review panel's story card (STORY-022
//! requirement 9, promised to EPIC-011 for STORY-019).
//!
//! The story's issue is the one among a forge's `story_issues(repo, id)`
//! whose title, trimmed, starts with `"<id>:"`, matched case-insensitively to
//! match this crate's own [`crate::pull_requests::story_of`] convention
//! (which lowercases before matching). A body with no `As a`/`I want`/`So
//! that` lines, or an id whose issue is not found, is not an error: it is
//! `Ok(None)`, left for the caller (the panel) to show as "the story's issue
//! was not found".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, mpsc};

use crate::pull_requests::forge::{Forge, ForgeUnavailable, GhCli, Issue};

/// A story's text, read from its issue: the title with the `"<id>: "` prefix
/// stripped, and its `As a`/`I want`/`So that` lines with their leading
/// keyword stripped too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryText {
    pub id: String,
    pub issue: u32,
    pub title: String,
    pub as_a: String,
    pub i_want: String,
    pub so_that: String,
}

/// What [`start_read`]/[`start_read_with`] report, one event per id, over the
/// channel they return. An id whose issue is not found, or whose body has no
/// `As a`/`I want`/`So that` lines, sends neither event: finding nothing is
/// not a failure (it is the caller's job to decide what "not found" shows),
/// and [`StoryEvent`] names no variant for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoryEvent {
    Read { id: String, story: StoryText },
    ReadFailed { id: String, reason: String },
}

/// Found stories only, cached per repository and story id for the process:
/// a miss or a failure is cheap enough (and common enough, for an id with no
/// matching issue yet) that caching it would need its own invalidation, and a
/// later issue created for a once-missing id would never be found (design
/// decision, STORY-022).
static CACHE: LazyLock<Mutex<HashMap<(PathBuf, String), StoryText>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// [`read_with`] through [`GhCli`], the real forge.
pub fn read(repository: &Path, id: &str) -> Result<Option<StoryText>, ForgeUnavailable> {
    read_with(repository, id, &GhCli)
}

/// [`start_read_with`] through [`GhCli`], the real forge.
pub fn start_read(repository: PathBuf, ids: Vec<String>) -> mpsc::Receiver<StoryEvent> {
    start_read_with(repository, ids, Arc::new(GhCli))
}

/// Reads the story named `id` of `repository`: `Ok(Some(story))` found
/// (cached afterwards), `Ok(None)` when no issue of `repository` has a title
/// starting with `"<id>:"`, or when it does but its body carries no `As
/// a`/`I want`/`So that` lines, `Err` when `forge` itself could not answer.
/// A cache hit never calls `forge`.
pub fn read_with(
    repository: &Path,
    id: &str,
    forge: &dyn Forge,
) -> Result<Option<StoryText>, ForgeUnavailable> {
    let key = (repository.to_path_buf(), id.to_owned());
    if let Some(story) = CACHE
        .lock()
        .expect("the story cache is not poisoned")
        .get(&key)
    {
        return Ok(Some(story.clone()));
    }

    let issues = forge.story_issues(repository, id)?;
    let Some(issue) = find_issue(&issues, id) else {
        return Ok(None);
    };
    let Some(story) = parse_story(id, issue) else {
        return Ok(None);
    };

    CACHE
        .lock()
        .expect("the story cache is not poisoned")
        .insert(key, story.clone());
    Ok(Some(story))
}

/// [`read_with`] for every id of `ids`, in order, on a background thread: one
/// [`StoryEvent::Read`] or [`StoryEvent::ReadFailed`] per id that resolves to
/// one, sent as it is found so the panel need not wait for every id to answer
/// before showing the first. A receiver gone before an id's event is sent
/// ends the thread quietly (mirrors [`crate::pull_requests::start_list`]).
pub fn start_read_with(
    repository: PathBuf,
    ids: Vec<String>,
    forge: Arc<dyn Forge + Send + Sync>,
) -> mpsc::Receiver<StoryEvent> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        for id in ids {
            let event = match read_with(&repository, &id, forge.as_ref()) {
                Ok(Some(story)) => StoryEvent::Read { id, story },
                Ok(None) => continue,
                Err(ForgeUnavailable(reason)) => StoryEvent::ReadFailed { id, reason },
            };
            if sender.send(event).is_err() {
                return;
            }
        }
    });

    receiver
}

/// The issue among `issues` whose title, trimmed, starts with `"<id>:"`,
/// case-insensitively: a mere mention of `id` elsewhere in the title does not
/// match.
fn find_issue<'a>(issues: &'a [Issue], id: &str) -> Option<&'a Issue> {
    let prefix = format!("{id}:");
    issues.iter().find(|issue| {
        let trimmed = issue.title.trim();
        // `.get(..)` rather than indexing: an issue's title is free text an
        // outside author wrote, and `prefix.len()` (a byte count of ASCII
        // characters) need not land on a char boundary of a title that opens
        // with a multi-byte character — indexing there would panic.
        trimmed
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(&prefix))
    })
}

/// `issue`'s title with its `"<id>: "` prefix stripped, trimmed.
fn title_without_prefix(title: &str, id: &str) -> String {
    let trimmed = title.trim();
    let prefix_len = id.len() + 1; // the id and its colon
    trimmed
        .get(prefix_len..)
        .unwrap_or(trimmed)
        .trim()
        .to_owned()
}

/// `issue`'s `As a`/`I want`/`So that` lines, or `None` when its body carries
/// fewer than all three.
fn parse_story(id: &str, issue: &Issue) -> Option<StoryText> {
    let as_a = first_labelled_line(&issue.body, &["As an", "As a"])?;
    let i_want = first_labelled_line(&issue.body, &["I want"])?;
    let so_that = first_labelled_line(&issue.body, &["So that"])?;
    Some(StoryText {
        id: id.to_owned(),
        issue: issue.number,
        title: title_without_prefix(&issue.title, id),
        as_a,
        i_want,
        so_that,
    })
}

/// The rest of the first line of `body`, trimmed, whose own trimmed text
/// starts (case-insensitively) with one of `labels`. `labels` is checked in
/// order, so a longer label that is itself a prefix of a shorter match (`"As
/// an"` against `"As a"`) is tried first.
fn first_labelled_line(body: &str, labels: &[&str]) -> Option<String> {
    body.lines().find_map(|line| strip_label(line, labels))
}

/// The rest of `line`, trimmed, after whichever of `labels` its own trimmed
/// text starts with (case-insensitively); `None` when it starts with none of
/// them.
fn strip_label(line: &str, labels: &[&str]) -> Option<String> {
    let trimmed = line.trim_start();
    labels.iter().find_map(|label| {
        // `.get(..)` rather than indexing, for the same reason as
        // `find_issue`: a body line is free text an outside author wrote, and
        // `label.len()` need not land on one of its char boundaries.
        let head = trimmed.get(..label.len())?;
        if head.eq_ignore_ascii_case(label) {
            Some(trimmed[label.len()..].trim().to_owned())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex as StdMutex;
    use std::time::{Duration, Instant};

    use super::*;

    const BODY: &str = "\
Feature: Give engineers a price breakdown
  As a shopper comparing prices
  I want to see the discount and the total together
  So that I know what I am really paying
";

    /// A [`Forge`] double that answers a fixed list of issues and counts how
    /// many times [`Forge::story_issues`] was called (to prove a cache hit
    /// never calls it again). `open_pull_requests`/`merged_pull_requests` are
    /// never exercised by these tests and simply answer nothing.
    struct FakeForge {
        issues: Vec<Issue>,
        calls: StdMutex<usize>,
        fails: bool,
    }

    impl FakeForge {
        fn new(issues: Vec<Issue>) -> FakeForge {
            FakeForge {
                issues,
                calls: StdMutex::new(0),
                fails: false,
            }
        }

        fn failing() -> FakeForge {
            FakeForge {
                issues: Vec::new(),
                calls: StdMutex::new(0),
                fails: true,
            }
        }

        fn call_count(&self) -> usize {
            *self.calls.lock().expect("not poisoned")
        }
    }

    impl Forge for FakeForge {
        fn open_pull_requests(
            &self,
            _repo: &Path,
        ) -> Result<Vec<crate::pull_requests::PullRequest>, ForgeUnavailable> {
            Ok(Vec::new())
        }

        fn merged_pull_requests(
            &self,
            _repo: &Path,
            _limit: usize,
        ) -> Result<Vec<crate::pull_requests::PullRequest>, ForgeUnavailable> {
            Ok(Vec::new())
        }

        fn story_issues(
            &self,
            _repo: &Path,
            _search: &str,
        ) -> Result<Vec<Issue>, ForgeUnavailable> {
            *self.calls.lock().expect("not poisoned") += 1;
            if self.fails {
                Err(ForgeUnavailable("gh could not be started".to_owned()))
            } else {
                Ok(self.issues.clone())
            }
        }
    }

    /// A repository path unique to each test, so the process-wide cache of
    /// one test never answers another's.
    fn unique_repo(name: &str) -> PathBuf {
        PathBuf::from(format!("/repo/{name}-{:?}", std::thread::current().id()))
    }

    #[test]
    fn a_title_starting_with_the_id_is_matched_and_its_body_parsed() {
        let forge = FakeForge::new(vec![Issue {
            number: 83,
            title: "STORY-012: Give engineers a price breakdown".to_owned(),
            body: BODY.to_owned(),
        }]);

        let story = read_with(&unique_repo("title-match"), "STORY-012", &forge)
            .expect("the forge answers")
            .expect("the issue is found");

        assert_eq!(story.id, "STORY-012");
        assert_eq!(story.issue, 83);
        assert_eq!(story.title, "Give engineers a price breakdown");
        assert!(story.as_a.contains("shopper"), "{}", story.as_a);
        assert!(
            story.i_want.contains("discount and the total"),
            "{}",
            story.i_want
        );
        assert!(
            story.so_that.contains("what I am really paying"),
            "{}",
            story.so_that
        );
    }

    #[test]
    fn a_title_or_body_opening_with_a_multi_byte_character_never_panics() {
        // Neither `prefix.len()` nor a label's own length need land on a
        // char boundary of free text an outside author wrote; a title or
        // body line opening with a multi-byte character must not panic the
        // background read, it must simply not match.
        let forge = FakeForge::new(vec![Issue {
            number: 1,
            title: "日本語の話 STORY-012".to_owned(),
            body: "Feature: x\n  🎉🎉 party\n  I want nothing\n  So that nothing\n".to_owned(),
        }]);

        let story = read_with(&unique_repo("multi-byte-title"), "STORY-012", &forge)
            .expect("no panic, and the forge answers");

        assert_eq!(
            story, None,
            "the title does not start with \"STORY-012:\", it only mentions it elsewhere"
        );
    }

    #[test]
    fn a_body_opening_with_a_multi_byte_character_is_skipped_not_panicked_on() {
        let forge = FakeForge::new(vec![Issue {
            number: 2,
            title: "STORY-014: A story with a multi-byte body".to_owned(),
            body:
                "Feature: x\n  🎉🎉 not a Gherkin line\n  I want something\n  So that something\n"
                    .to_owned(),
        }]);

        let story = read_with(&unique_repo("multi-byte-body"), "STORY-014", &forge)
            .expect("no panic, and the forge answers");

        assert_eq!(
            story, None,
            "no line starts with \"As a\"/\"As an\", so the body yields no story"
        );
    }

    #[test]
    fn an_id_only_mentioned_elsewhere_in_the_title_is_not_a_match() {
        let forge = FakeForge::new(vec![Issue {
            number: 1,
            title: "Fix a flaky test for STORY-012".to_owned(),
            body: BODY.to_owned(),
        }]);

        let story = read_with(&unique_repo("mention-only"), "STORY-012", &forge)
            .expect("the forge answers");

        assert_eq!(
            story, None,
            "the title does not start with \"STORY-012:\", only mentions it"
        );
    }

    #[test]
    fn a_story_whose_issue_is_not_found_is_ok_none() {
        let forge = FakeForge::new(Vec::new());

        let story =
            read_with(&unique_repo("not-found"), "STORY-099", &forge).expect("the forge answers");

        assert_eq!(story, None);
    }

    #[test]
    fn a_body_with_no_as_a_i_want_so_that_lines_is_ok_none() {
        let forge = FakeForge::new(vec![Issue {
            number: 2,
            title: "STORY-013: A chore with no scenario shape".to_owned(),
            body: "Just some prose, no Gherkin lines here.".to_owned(),
        }]);

        let story =
            read_with(&unique_repo("no-lines"), "STORY-013", &forge).expect("the forge answers");

        assert_eq!(story, None);
    }

    #[test]
    fn a_forge_failure_surfaces_as_an_error() {
        let forge = FakeForge::failing();

        let result = read_with(&unique_repo("forge-fails"), "STORY-012", &forge);

        assert_eq!(
            result,
            Err(ForgeUnavailable("gh could not be started".to_owned()))
        );
    }

    #[test]
    fn a_cache_hit_never_calls_the_forge_again() {
        let forge = FakeForge::new(vec![Issue {
            number: 83,
            title: "STORY-012: Give engineers a price breakdown".to_owned(),
            body: BODY.to_owned(),
        }]);
        let repo = unique_repo("cache-hit");

        read_with(&repo, "STORY-012", &forge)
            .expect("the forge answers")
            .expect("the issue is found");
        assert_eq!(forge.call_count(), 1);

        read_with(&repo, "STORY-012", &forge)
            .expect("the forge answers")
            .expect("the cached story is found");

        assert_eq!(forge.call_count(), 1, "the second read must be a cache hit");
    }

    #[test]
    fn start_read_sends_one_event_per_id() {
        let forge = Arc::new(FakeForge::new(vec![
            Issue {
                number: 83,
                title: "STORY-012: Give engineers a price breakdown".to_owned(),
                body: BODY.to_owned(),
            },
            Issue {
                number: 84,
                title: "STORY-013: Another story".to_owned(),
                body: "\
Feature: Another story
  As a developer
  I want something else
  So that it is tested
"
                .to_owned(),
            },
        ]));
        let repo = unique_repo("start-read");

        let receiver = start_read_with(
            repo,
            vec!["STORY-012".to_owned(), "STORY-013".to_owned()],
            forge,
        );

        let mut events = Vec::new();
        let started = Instant::now();
        while events.len() < 2 && started.elapsed() < Duration::from_secs(5) {
            if let Ok(event) = receiver.recv_timeout(Duration::from_millis(50)) {
                events.push(event);
            }
        }

        assert_eq!(events.len(), 2, "{events:?}");
        assert!(
            events
                .iter()
                .all(|event| matches!(event, StoryEvent::Read { .. })),
            "{events:?}"
        );
    }
}
