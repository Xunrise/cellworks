//! Pull requests, their stories, sizes, flags and tower placements: the types
//! `PullRequestEvent` reports and the blocking and threaded functions that
//! build them.

mod critical;
mod diff;
pub mod forge;
mod placement;

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

pub use diff::{Diff, Removal, Size};
pub use forge::{Checkout, Forge, ForgeUnavailable, GhCli, GitCli};
pub use placement::{FilePlacement, TowerPlacement, place};

/// A pull request as the forge lists it: a diff and where its lines land on
/// the city (a [`Review`]) are still lazy, read only when it is opened
/// (requirement 4); its size, file list and critical-change flags come with
/// the listing itself (STORY-022 requirement 2) since `gh pr list` already
/// carries them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequest {
    pub number: u32,
    pub state: PullRequestState,
    pub title: String,
    pub branch: String,
    pub head: String,
    pub base: String,
    pub author: String,
    pub story: Option<String>,
    /// Lines the listing says this pull request adds.
    pub additions: usize,
    /// Lines the listing says this pull request removes.
    pub deletions: usize,
    /// Files the listing says this pull request touches.
    pub changed_files: usize,
    /// The paths the listing names, capped at 100 by `gh` itself: enough to
    /// compute the list row's own flags, not necessarily every file the pull
    /// request touches (STORY-022 requirement 2); [`open`]/[`check_out`]
    /// recompute flags from the diff's own complete file list.
    pub files: Vec<String>,
    /// The critical-change flags this pull request's listed `files` raise,
    /// filled once by [`list`] (never by a `Forge` implementation itself, so
    /// a fake handing back an empty map still gets it filled in).
    pub flags: std::collections::BTreeMap<String, Vec<String>>,
}

/// Whether a pull request is still open or was merged; a closed, unmerged
/// pull request is never listed (requirement 1) so has no variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Merged,
}

/// What opening a pull request yields: its size, the critical-change flags it
/// raises with the files that raised each, and where its lines land on the
/// city at its head.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub size: diff::Size,
    pub flags: std::collections::BTreeMap<String, Vec<String>>,
    pub placements: Vec<placement::FilePlacement>,
}

/// What listing, opening or checking a pull request out reports, over the
/// channel [`start_list`], [`start_open`] or [`start_check_out`] returns.
#[derive(Debug, Clone, PartialEq)]
pub enum PullRequestEvent {
    Listed {
        pull_requests: Vec<PullRequest>,
    },
    ListFailed {
        reason: String,
    },
    Opened {
        number: u32,
        review: Review,
    },
    /// [`check_out`] succeeded: `head` is the worktree's own folder (not a
    /// commit hash, unlike [`PullRequest::head`]), kept open rather than
    /// mapped and removed, so the engineer can browse and edit it as an
    /// ordinary project for the life of the review (STORY-022 requirement 3).
    CheckedOut {
        number: u32,
        head: PathBuf,
        diff: diff::Diff,
        size: diff::Size,
        flags: std::collections::BTreeMap<String, Vec<String>>,
    },
    OpenFailed {
        number: u32,
        reason: String,
    },
}

/// The story a title or branch name carries: the first `"story-"`, matched
/// case-insensitively, immediately followed by one or more digits, read as
/// `STORY-<digits>`. `None` when `"story-"` does not occur, or occurs with no
/// digit right after it (requirement 2).
fn story_of(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find("story-")?;
    let digits_start = index + "story-".len();
    let digits: String = text[digits_start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        None
    } else {
        Some(format!("STORY-{digits}"))
    }
}

/// Lists `repo`'s pull requests: every open one, then the last 30 merged, each
/// list in the order `forge` returned it (requirement 1). Stops at the first
/// failure without calling the second method. Each pull request's story is
/// (re)computed from its title, else its branch, here rather than trusted
/// from the forge: a fake `Forge` in a test may hand back pull requests with
/// no story set at all, so `list` itself must not depend on the forge having
/// already done it. Likewise `flags` is filled here, once, from `repo`'s own
/// `sdlc/critical.yaml` matched against each pull request's own `files`
/// (STORY-022 requirement 2): a `Forge` implementation never fills it itself.
pub fn list(repo: &Path, forge: &dyn Forge) -> PullRequestEvent {
    let open_ones = match forge.open_pull_requests(repo) {
        Ok(pull_requests) => pull_requests,
        Err(ForgeUnavailable(reason)) => return PullRequestEvent::ListFailed { reason },
    };
    let merged_ones = match forge.merged_pull_requests(repo, 30) {
        Ok(pull_requests) => pull_requests,
        Err(ForgeUnavailable(reason)) => return PullRequestEvent::ListFailed { reason },
    };
    let flags_defined = critical::read_flags(repo);
    let pull_requests = open_ones
        .into_iter()
        .chain(merged_ones)
        .map(|mut pull_request| {
            pull_request.story =
                story_of(&pull_request.title).or_else(|| story_of(&pull_request.branch));
            let paths: Vec<&str> = pull_request.files.iter().map(String::as_str).collect();
            pull_request.flags = critical::flags_raised(&flags_defined, &paths);
            pull_request
        })
        .collect();
    PullRequestEvent::Listed { pull_requests }
}

/// Removes the temporary worktree it was built for, however [`open`] returns,
/// including when mapping the project fails (requirement 6). A cleanup
/// failure here has nothing useful to do with it, so it is discarded rather
/// than panicking in `Drop`.
struct WorktreeGuard<'a> {
    checkout: &'a dyn Checkout,
    repo: &'a Path,
    worktree: PathBuf,
}

impl Drop for WorktreeGuard<'_> {
    fn drop(&mut self) {
        let _ = self.checkout.remove_worktree(self.repo, &self.worktree);
    }
}

/// A worktree path under the OS temp directory, hard to guess: `git worktree
/// add` accepts an existing *empty* directory, so a predictable name (pid and
/// pull request number alone) would let another local user on a shared
/// machine pre-stage one at the same path. [`forge::GitCli::add_worktree`]
/// also creates its target exclusively, so even a guessed name cannot be
/// raced into use; this only keeps a guess from being worth attempting.
fn unique_worktree_path(pull_request_number: u32) -> PathBuf {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let nonce = RandomState::new().build_hasher().finish();
    std::env::temp_dir().join(format!(
        "cellworks-review-{}-{pull_request_number}-{nonce:016x}",
        std::process::id(),
    ))
}

/// Opens `pull_request` of `repo`: its diff, size, critical-change flags and
/// where its lines land on the city at its head, built in a temporary
/// detached worktree under the OS temp directory that is always removed
/// before this function returns.
pub fn open(
    repo: &Path,
    pull_request: &PullRequest,
    checkout: &dyn Checkout,
    project_map: &dyn Fn(&Path) -> Option<cellworks_map::ProjectMap>,
) -> PullRequestEvent {
    let text = match checkout.diff(repo, pull_request) {
        Ok(text) => text,
        Err(ForgeUnavailable(reason)) => {
            return PullRequestEvent::OpenFailed {
                number: pull_request.number,
                reason,
            };
        }
    };

    let diff = diff::parse_diff(&text);
    let size = diff::size(&diff);
    let flags_defined = critical::read_flags(repo);
    let paths: Vec<&str> = diff.files.keys().map(String::as_str).collect();
    let flags = critical::flags_raised(&flags_defined, &paths);

    let worktree_path = unique_worktree_path(pull_request.number);

    if let Err(ForgeUnavailable(reason)) =
        checkout.add_worktree(repo, &pull_request.head, &worktree_path)
    {
        return PullRequestEvent::OpenFailed {
            number: pull_request.number,
            reason,
        };
    }

    let _guard = WorktreeGuard {
        checkout,
        repo,
        worktree: worktree_path.clone(),
    };

    let Some(map) = project_map(&worktree_path) else {
        return PullRequestEvent::OpenFailed {
            number: pull_request.number,
            reason: "the pull request's head could not be mapped".to_owned(),
        };
    };

    let placements = placement::place(&diff, &map, &worktree_path);

    PullRequestEvent::Opened {
        number: pull_request.number,
        review: Review {
            size,
            flags,
            placements,
        },
    }
}

/// [`list`] on a background thread, its one event sent over the returned
/// channel; a receiver gone before the send ends the thread quietly (mirrors
/// `crates/code-analysis/src/lib.rs`'s `start_load`).
pub fn start_list(
    repo: PathBuf,
    forge: Arc<dyn Forge + Send + Sync>,
) -> mpsc::Receiver<PullRequestEvent> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let _ = sender.send(list(&repo, forge.as_ref()));
    });

    receiver
}

/// [`open`] on a background thread, its one event sent over the returned
/// channel; a receiver gone before the send ends the thread quietly (mirrors
/// `crates/code-analysis/src/lib.rs`'s `start_load`).
#[allow(clippy::type_complexity)]
pub fn start_open(
    repo: PathBuf,
    pull_request: PullRequest,
    checkout: Arc<dyn Checkout + Send + Sync>,
    project_map: Arc<dyn Fn(&Path) -> Option<cellworks_map::ProjectMap> + Send + Sync + 'static>,
) -> mpsc::Receiver<PullRequestEvent> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let event = open(
            &repo,
            &pull_request,
            checkout.as_ref(),
            project_map.as_ref(),
        );
        let _ = sender.send(event);
    });

    receiver
}

/// The temporary detached worktree [`check_out`] built for a review, kept
/// open (unlike [`open`]'s own [`WorktreeGuard`], which removes its worktree
/// before returning) for as long as the review it belongs to is shown.
///
/// Unlike `WorktreeGuard<'a>`, which borrows its `Checkout` and so cannot
/// outlive the thread that built it, this owns an `Arc` so it can cross the
/// channel [`start_check_out`] returns and be held on the UI thread for the
/// life of a review (design decision, STORY-022). Its [`Drop`] removes the
/// worktree on a background thread it spawns itself, never blocking the
/// thread it is dropped on (`docs/services/cellworks/guidelines/architecture.md`,
/// "Threads and events": a guard that runs a process when dropped is never
/// dropped on the UI thread); a cleanup failure has nothing useful to do with
/// it, so it is discarded rather than panicking in `Drop`, same as
/// `WorktreeGuard`.
pub struct HeadCheckout {
    checkout: Arc<dyn Checkout + Send + Sync>,
    repo: PathBuf,
    worktree: PathBuf,
}

impl Drop for HeadCheckout {
    fn drop(&mut self) {
        let checkout = Arc::clone(&self.checkout);
        let repo = self.repo.clone();
        let worktree = self.worktree.clone();
        std::thread::spawn(move || {
            let _ = checkout.remove_worktree(&repo, &worktree);
        });
    }
}

/// Checks `pull_request`'s head out of `repo` into a new temporary detached
/// worktree, built the same way [`open`] builds its own (fetching first when
/// needed, computing `diff`, `size` and `flags` from the diff's own complete
/// file list, never the listing's capped one). Unlike [`open`], this does not
/// map the project and does not remove the worktree before returning: the
/// worktree is handed back open, as a [`HeadCheckout`], so the head can be
/// opened as an ordinary project and kept for the life of the review
/// (STORY-022 requirement 3, design decision "the head is kept as a
/// checkout"). On any failure the worktree (if one was even started) is
/// never left dangling beyond what [`HeadCheckout`]'s own `Drop` would have
/// cleaned up, because no [`HeadCheckout`] is returned until `add_worktree`
/// has already succeeded.
pub fn check_out(
    repo: &Path,
    pull_request: &PullRequest,
    checkout: Arc<dyn Checkout + Send + Sync>,
) -> (PullRequestEvent, Option<HeadCheckout>) {
    let text = match checkout.diff(repo, pull_request) {
        Ok(text) => text,
        Err(ForgeUnavailable(reason)) => {
            return (
                PullRequestEvent::OpenFailed {
                    number: pull_request.number,
                    reason,
                },
                None,
            );
        }
    };

    let diff = diff::parse_diff(&text);
    let size = diff::size(&diff);
    let flags_defined = critical::read_flags(repo);
    let paths: Vec<&str> = diff.files.keys().map(String::as_str).collect();
    let flags = critical::flags_raised(&flags_defined, &paths);

    let worktree_path = unique_worktree_path(pull_request.number);

    if let Err(ForgeUnavailable(reason)) =
        checkout.add_worktree(repo, &pull_request.head, &worktree_path)
    {
        return (
            PullRequestEvent::OpenFailed {
                number: pull_request.number,
                reason,
            },
            None,
        );
    }

    let head_checkout = HeadCheckout {
        checkout,
        repo: repo.to_path_buf(),
        worktree: worktree_path.clone(),
    };

    (
        PullRequestEvent::CheckedOut {
            number: pull_request.number,
            head: worktree_path,
            diff,
            size,
            flags,
        },
        Some(head_checkout),
    )
}

/// [`check_out`] on a background thread, its one event (with the
/// [`HeadCheckout`] a success carries) sent over the returned channel; a
/// receiver gone before the send ends the thread quietly (mirrors
/// `crates/code-analysis/src/lib.rs`'s `start_load`).
pub fn start_check_out(
    repo: PathBuf,
    pull_request: PullRequest,
    checkout: Arc<dyn Checkout + Send + Sync>,
) -> mpsc::Receiver<(PullRequestEvent, Option<HeadCheckout>)> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let outcome = check_out(&repo, &pull_request, checkout);
        let _ = sender.send(outcome);
    });

    receiver
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use super::forge::Issue;
    use super::*;

    /// A minimal, valid pull request for tests that do not care about its
    /// size, files or flags.
    fn sample_pull_request(number: u32) -> PullRequest {
        PullRequest {
            number,
            state: PullRequestState::Open,
            title: "Sample".to_owned(),
            branch: "feature/sample".to_owned(),
            head: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b".to_owned(),
            base: "9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e".to_owned(),
            author: "xunrise".to_owned(),
            story: None,
            additions: 0,
            deletions: 0,
            changed_files: 0,
            files: Vec::new(),
            flags: std::collections::BTreeMap::new(),
        }
    }

    /// A [`Forge`] that lists `open` and answers no issues; `merged` is
    /// always empty.
    struct FixedForge {
        open: Vec<PullRequest>,
    }

    impl Forge for FixedForge {
        fn open_pull_requests(&self, _repo: &Path) -> Result<Vec<PullRequest>, ForgeUnavailable> {
            Ok(self.open.clone())
        }

        fn merged_pull_requests(
            &self,
            _repo: &Path,
            _limit: usize,
        ) -> Result<Vec<PullRequest>, ForgeUnavailable> {
            Ok(Vec::new())
        }

        fn story_issues(
            &self,
            _repo: &Path,
            _search: &str,
        ) -> Result<Vec<Issue>, ForgeUnavailable> {
            Ok(Vec::new())
        }
    }

    /// A [`Checkout`] double recording every worktree it was asked to add or
    /// remove, and optionally failing `diff` or `add_worktree`.
    #[derive(Default)]
    struct RecordingCheckout {
        diff_text: String,
        fail_diff: bool,
        fail_add_worktree: bool,
        added: Mutex<Vec<PathBuf>>,
        removed: Mutex<Vec<PathBuf>>,
    }

    impl RecordingCheckout {
        fn new(diff_text: &str) -> RecordingCheckout {
            RecordingCheckout {
                diff_text: diff_text.to_owned(),
                ..RecordingCheckout::default()
            }
        }

        fn removed(&self) -> Vec<PathBuf> {
            self.removed.lock().expect("not poisoned").clone()
        }

        fn added(&self) -> Vec<PathBuf> {
            self.added.lock().expect("not poisoned").clone()
        }
    }

    impl Checkout for RecordingCheckout {
        fn diff(
            &self,
            _repo: &Path,
            _pull_request: &PullRequest,
        ) -> Result<String, ForgeUnavailable> {
            if self.fail_diff {
                Err(ForgeUnavailable("git: pull request not found".to_owned()))
            } else {
                Ok(self.diff_text.clone())
            }
        }

        fn add_worktree(
            &self,
            _repo: &Path,
            _commit: &str,
            into: &Path,
        ) -> Result<(), ForgeUnavailable> {
            if self.fail_add_worktree {
                return Err(ForgeUnavailable("git: worktree add failed".to_owned()));
            }
            self.added
                .lock()
                .expect("not poisoned")
                .push(into.to_owned());
            Ok(())
        }

        fn remove_worktree(&self, _repo: &Path, into: &Path) -> Result<(), ForgeUnavailable> {
            self.removed
                .lock()
                .expect("not poisoned")
                .push(into.to_owned());
            Ok(())
        }
    }

    #[test]
    fn list_fills_flags_once_from_the_repos_critical_yaml_and_each_pull_requests_own_files() {
        let project = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(project.path().join("sdlc")).expect("create sdlc");
        std::fs::write(
            project.path().join("sdlc/critical.yaml"),
            "flags:\n  dependencies:\n    - \"cargo.lock\"\n",
        )
        .expect("write critical.yaml");

        let mut pull_request = sample_pull_request(41);
        pull_request.files = vec!["Cargo.lock".to_owned()];
        let forge = FixedForge {
            open: vec![pull_request],
        };

        let event = list(project.path(), &forge);

        let PullRequestEvent::Listed { pull_requests } = event else {
            panic!("expected Listed, got {event:?}");
        };
        assert_eq!(
            pull_requests[0].flags.get("dependencies"),
            Some(&vec!["Cargo.lock".to_owned()]),
            "list() fills flags from the pull request's own files, a fake Forge need not: {:?}",
            pull_requests[0].flags
        );
    }

    #[test]
    fn head_checkout_removes_its_worktree_on_drop_without_blocking() {
        let checkout = Arc::new(RecordingCheckout::new(""));
        let worktree = PathBuf::from("/tmp/cellworks-review-test-worktree");

        drop(HeadCheckout {
            checkout: Arc::clone(&checkout) as Arc<dyn Checkout + Send + Sync>,
            repo: PathBuf::from("/repo"),
            worktree: worktree.clone(),
        });

        // Not asserted synchronously: a drop that blocked the thread it ran
        // on would still pass an immediate assertion, so the point is polling
        // for a removal that happens on a background thread instead.
        let started = Instant::now();
        while checkout.removed().is_empty() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(checkout.removed(), vec![worktree]);
    }

    #[test]
    fn check_out_builds_a_worktree_and_its_head_checkout_removes_it_once_dropped() {
        let diff_text = "\
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+new
";
        let checkout = Arc::new(RecordingCheckout::new(diff_text));
        let pull_request = sample_pull_request(41);

        let (event, head_checkout) = check_out(
            Path::new("/repo"),
            &pull_request,
            Arc::clone(&checkout) as Arc<dyn Checkout + Send + Sync>,
        );

        let (number, head, size) = match event {
            PullRequestEvent::CheckedOut {
                number, head, size, ..
            } => (number, head, size),
            other => panic!("expected CheckedOut, got {other:?}"),
        };
        assert_eq!(number, 41);
        assert_eq!(size.added, 1);
        assert_eq!(size.removed, 1);
        assert_eq!(
            checkout.added(),
            vec![head.clone()],
            "check_out adds exactly the worktree it reports as head"
        );

        let head_checkout = head_checkout.expect("add_worktree succeeded");
        assert!(
            checkout.removed().is_empty(),
            "check_out itself does not remove the worktree it just built"
        );

        drop(head_checkout);

        let started = Instant::now();
        while checkout.removed().is_empty() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            checkout.removed(),
            vec![head],
            "dropping the review's HeadCheckout removes the worktree check_out already added"
        );
    }

    #[test]
    fn check_out_fails_without_a_head_checkout_when_the_diff_cannot_be_read() {
        let checkout = Arc::new(RecordingCheckout {
            fail_diff: true,
            ..RecordingCheckout::default()
        });
        let pull_request = sample_pull_request(7);

        let (event, head_checkout) = check_out(
            Path::new("/repo"),
            &pull_request,
            checkout as Arc<dyn Checkout + Send + Sync>,
        );

        assert!(head_checkout.is_none());
        match event {
            PullRequestEvent::OpenFailed { number, reason } => {
                assert_eq!(number, 7);
                assert_eq!(reason, "git: pull request not found");
            }
            other => panic!("expected OpenFailed, got {other:?}"),
        }
    }

    #[test]
    fn check_out_fails_without_a_head_checkout_when_the_worktree_cannot_be_added() {
        let checkout = Arc::new(RecordingCheckout {
            fail_add_worktree: true,
            ..RecordingCheckout::default()
        });
        let pull_request = sample_pull_request(8);

        let (event, head_checkout) = check_out(
            Path::new("/repo"),
            &pull_request,
            Arc::clone(&checkout) as Arc<dyn Checkout + Send + Sync>,
        );

        assert!(head_checkout.is_none());
        assert!(
            checkout.removed().is_empty(),
            "nothing was added, so nothing needs removing"
        );
        match event {
            PullRequestEvent::OpenFailed { number, .. } => assert_eq!(number, 8),
            other => panic!("expected OpenFailed, got {other:?}"),
        }
    }

    #[test]
    fn a_story_in_the_title_wins_even_when_the_branch_also_names_one() {
        let title = "STORY-058: Pull-request source with diffs";
        let branch = "feature/1-story-012-other-slug";

        let story = story_of(title).or_else(|| story_of(branch));

        assert_eq!(story.as_deref(), Some("STORY-058"));
    }

    #[test]
    fn a_story_named_only_in_the_branch_is_still_found() {
        let title = "Fix a typo in the README";
        let branch = "feature/83-story-058-slug";

        let story = story_of(title).or_else(|| story_of(branch));

        assert_eq!(story.as_deref(), Some("STORY-058"));
    }

    #[test]
    fn neither_an_epic_title_nor_its_branch_names_a_story() {
        let title = "EPIC-024: Architecture";
        let branch = "chore/promote-test-to-main";

        let story = story_of(title).or_else(|| story_of(branch));

        assert_eq!(story, None);
    }

    #[test]
    fn story_dash_with_no_digits_after_it_names_no_story() {
        assert_eq!(story_of("the story-driven approach to testing"), None);
    }

    #[test]
    fn story_is_matched_case_insensitively() {
        assert_eq!(story_of("Story-058: thing").as_deref(), Some("STORY-058"));
        assert_eq!(story_of("STORY-058: thing").as_deref(), Some("STORY-058"));
    }

    #[test]
    fn a_story_prefixed_branch_name_gives_its_story() {
        assert_eq!(
            story_of("feature/83-story-058-slug").as_deref(),
            Some("STORY-058")
        );
    }
}
