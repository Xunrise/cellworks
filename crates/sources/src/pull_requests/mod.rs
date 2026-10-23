//! Pull requests, their stories, sizes, flags and tower placements: the types
//! `PullRequestEvent` reports and the blocking and threaded functions that
//! build them.

mod critical;
mod diff;
pub mod forge;
mod placement;

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

pub use diff::Size;
pub use forge::{Checkout, Forge, ForgeUnavailable, GhCli, GitCli};
pub use placement::{FilePlacement, TowerPlacement};

/// A pull request as the forge lists it: no diff, size, flags or placements,
/// which are all lazy, read only when it is opened (requirement 4).
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

/// What listing or opening a pull request reports, over the channel
/// [`start_list`] or [`start_open`] returns.
#[derive(Debug, Clone, PartialEq)]
pub enum PullRequestEvent {
    Listed { pull_requests: Vec<PullRequest> },
    ListFailed { reason: String },
    Opened { number: u32, review: Review },
    OpenFailed { number: u32, reason: String },
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
/// already done it.
pub fn list(repo: &Path, forge: &dyn Forge) -> PullRequestEvent {
    let open_ones = match forge.open_pull_requests(repo) {
        Ok(pull_requests) => pull_requests,
        Err(ForgeUnavailable(reason)) => return PullRequestEvent::ListFailed { reason },
    };
    let merged_ones = match forge.merged_pull_requests(repo, 30) {
        Ok(pull_requests) => pull_requests,
        Err(ForgeUnavailable(reason)) => return PullRequestEvent::ListFailed { reason },
    };
    let pull_requests = open_ones
        .into_iter()
        .chain(merged_ones)
        .map(|mut pull_request| {
            pull_request.story =
                story_of(&pull_request.title).or_else(|| story_of(&pull_request.branch));
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

#[cfg(test)]
mod tests {
    use super::*;

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
