//! STORY-021: a black-box pass through the pull-request source, against fake
//! `Forge` and `Checkout` adapters so no `gh` or `git` process, and no
//! network, is ever touched.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cellworks_map::{
    Declaration, Directories, Floor, Item, ItemKind, ProjectMap, default_prefix_rule,
};
use cellworks_sources::pull_requests::*;

fn pull_request(number: u32, state: PullRequestState, title: &str, branch: &str) -> PullRequest {
    PullRequest {
        number,
        state,
        title: title.to_owned(),
        branch: branch.to_owned(),
        head: "headcommit".to_owned(),
        base: "basecommit".to_owned(),
        author: "octocat".to_owned(),
        story: None,
        additions: 0,
        deletions: 0,
        changed_files: 0,
        files: Vec::new(),
        flags: BTreeMap::new(),
    }
}

/// Records whether it was asked, and answers once with what it was given.
struct FakeForge {
    open: Mutex<Option<Result<Vec<PullRequest>, ForgeUnavailable>>>,
    merged: Mutex<Option<Result<Vec<PullRequest>, ForgeUnavailable>>>,
}

impl FakeForge {
    fn new(
        open: Result<Vec<PullRequest>, ForgeUnavailable>,
        merged: Result<Vec<PullRequest>, ForgeUnavailable>,
    ) -> FakeForge {
        FakeForge {
            open: Mutex::new(Some(open)),
            merged: Mutex::new(Some(merged)),
        }
    }
}

impl Forge for FakeForge {
    fn open_pull_requests(&self, _repo: &Path) -> Result<Vec<PullRequest>, ForgeUnavailable> {
        self.open
            .lock()
            .expect("lock")
            .take()
            .expect("open_pull_requests asked more than once")
    }

    fn merged_pull_requests(
        &self,
        _repo: &Path,
        _limit: usize,
    ) -> Result<Vec<PullRequest>, ForgeUnavailable> {
        self.merged
            .lock()
            .expect("lock")
            .take()
            .expect("merged_pull_requests asked more than once")
    }

    fn story_issues(
        &self,
        _repo: &Path,
        _search: &str,
    ) -> Result<Vec<cellworks_sources::pull_requests::forge::Issue>, ForgeUnavailable> {
        unimplemented!("STORY-021's own tests never read a story's issue")
    }
}

/// A fake checkout: answers `diff` once with what it was given, and records
/// whether a worktree was added or removed.
struct FakeCheckout {
    diff: RefCell<Option<Result<String, ForgeUnavailable>>>,
    add_worktree_called: Cell<bool>,
    remove_worktree_called: Cell<bool>,
}

impl FakeCheckout {
    fn returning(diff: Result<String, ForgeUnavailable>) -> FakeCheckout {
        FakeCheckout {
            diff: RefCell::new(Some(diff)),
            add_worktree_called: Cell::new(false),
            remove_worktree_called: Cell::new(false),
        }
    }
}

impl Checkout for FakeCheckout {
    fn diff(&self, _repo: &Path, _pull_request: &PullRequest) -> Result<String, ForgeUnavailable> {
        self.diff
            .borrow_mut()
            .take()
            .expect("diff asked more than once")
    }

    fn add_worktree(
        &self,
        _repo: &Path,
        _commit: &str,
        _into: &Path,
    ) -> Result<(), ForgeUnavailable> {
        self.add_worktree_called.set(true);
        Ok(())
    }

    fn remove_worktree(&self, _repo: &Path, _into: &Path) -> Result<(), ForgeUnavailable> {
        self.remove_worktree_called.set(true);
        Ok(())
    }
}

fn floor(number: usize) -> Floor {
    Floor {
        number,
        text: String::new(),
        indent: 0,
        width: 0,
        tokens: Vec::new(),
        calls: Vec::new(),
        imports: Vec::new(),
    }
}

/// A tower standing on `file`, holding `lines`. Its rectangle is borrowed
/// from an empty map rather than named directly, since the layout always
/// overwrites it and this test has no other reason to depend on egui.
fn tower(name: &str, file: &str, lines: &[usize]) -> Item {
    let area = ProjectMap::of(
        "blank",
        Path::new("/blank"),
        &[],
        &Directories {
            prefix: default_prefix_rule(),
        },
    )
    .extent;
    Item {
        name: name.to_owned(),
        path: PathBuf::from(format!("{file}/{name}")),
        kind: ItemKind::Function,
        tests: false,
        floors: lines.iter().copied().map(floor).collect(),
        area,
        base: 0.0,
        declaration: Declaration::default(),
    }
}

/// A map, rooted at `root`, whose only file is `src/lib.rs`, standing two
/// towers: `new_one` on line 3 and `new_two` on line 4.
fn fixture_map(root: &Path) -> ProjectMap {
    let file = "src/lib.rs";
    let files = vec![PathBuf::from(file)];
    let mut items = BTreeMap::new();
    items.insert(
        PathBuf::from(file),
        vec![tower("new_one", file, &[3]), tower("new_two", file, &[4])],
    );
    let strategy = Directories {
        prefix: default_prefix_rule(),
    };
    ProjectMap::of_code("review", root, &files, &strategy, items, &BTreeMap::new())
}

#[test]
fn open_and_merged_pull_requests_are_listed_with_their_story() {
    let repo = tempfile::tempdir().expect("a temporary directory");
    let forge = FakeForge::new(
        Ok(vec![
            pull_request(
                10,
                PullRequestState::Open,
                "STORY-010: Add search index",
                "feature/10-add-search",
            ),
            pull_request(
                11,
                PullRequestState::Open,
                "Keep the lights on",
                "chore/11-upgrade-deps",
            ),
        ]),
        Ok(vec![
            pull_request(
                5,
                PullRequestState::Merged,
                "Housekeeping",
                "feature/5-story-021-pull-request-source",
            ),
            pull_request(
                2,
                PullRequestState::Merged,
                "EPIC-024: Architecture refresh",
                "epic/2-architecture",
            ),
        ]),
    );

    let receiver = start_list(repo.path().to_path_buf(), Arc::new(forge));
    let event = receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the list thread reports");

    let PullRequestEvent::Listed { pull_requests } = event else {
        panic!("expected Listed, got {event:?}");
    };
    assert_eq!(
        pull_requests.iter().map(|p| p.number).collect::<Vec<_>>(),
        vec![10, 11, 5, 2],
        "open pull requests first, in order, then merged ones, in order"
    );
    assert_eq!(
        pull_requests[0].story.as_deref(),
        Some("STORY-010"),
        "found in the title"
    );
    assert_eq!(
        pull_requests[1].story, None,
        "neither its title nor its branch names a story"
    );
    assert_eq!(
        pull_requests[2].story.as_deref(),
        Some("STORY-021"),
        "found in the branch, the title having none"
    );
    assert_eq!(
        pull_requests[3].story, None,
        "an epic pull request implements no story"
    );
}

#[test]
fn an_opened_pull_request_has_its_size_flags_and_lines_on_towers() {
    let repo = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir(repo.path().join("sdlc")).expect("create sdlc");
    std::fs::write(
        repo.path().join("sdlc/critical.yaml"),
        "flags:\n  dependencies:\n    - \"cargo.lock\"\n",
    )
    .expect("write critical.yaml");

    let diff_text = "\
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -10 +10 @@
-version = \"1.0.0\"
+version = \"1.0.1\"
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -3 +3,2 @@
-fn old() {}
+fn new_one() {}
+fn new_two() {}
";
    let checkout = FakeCheckout::returning(Ok(diff_text.to_owned()));
    let pr = pull_request(
        117,
        PullRequestState::Open,
        "STORY-058: add two functions",
        "feature/117-story-058-add",
    );
    let project_map = |worktree: &Path| Some(fixture_map(worktree));

    let event = open(repo.path(), &pr, &checkout, &project_map);

    let PullRequestEvent::Opened { number, review } = event else {
        panic!("expected Opened, got {event:?}");
    };
    assert_eq!(number, 117);
    assert_eq!(review.size.files, 2);
    assert_eq!(review.size.added, 3, "1 on Cargo.lock + 2 on src/lib.rs");
    assert_eq!(review.size.removed, 2, "1 on Cargo.lock + 1 on src/lib.rs");
    assert_eq!(
        review.flags.get("dependencies"),
        Some(&vec!["Cargo.lock".to_owned()]),
        "{:?}",
        review.flags
    );

    let lib = review
        .placements
        .iter()
        .find(|placement| placement.file == "src/lib.rs")
        .expect("src/lib.rs was changed");
    assert_eq!(lib.outside_added, 0);
    assert_eq!(lib.outside_removed, 0);
    let named = |name: &str| {
        lib.towers
            .iter()
            .find(|t| t.tower.to_string_lossy().ends_with(name))
            .unwrap_or_else(|| panic!("no tower named {name} among {:?}", lib.towers))
    };
    assert_eq!(named("new_one").added, vec![3]);
    assert_eq!(
        named("new_one").removed,
        vec![cellworks_sources::pull_requests::Removal {
            old_line: 3,
            before_new_line: 3
        }]
    );
    assert_eq!(named("new_two").added, vec![4]);
    assert!(named("new_two").removed.is_empty());

    let lock = review
        .placements
        .iter()
        .find(|placement| placement.file == "Cargo.lock")
        .expect("Cargo.lock was changed");
    assert!(
        lock.towers.is_empty(),
        "no loader puts towers on Cargo.lock"
    );
    assert_eq!(lock.outside_added, 1);
    assert_eq!(lock.outside_removed, 1);

    assert!(checkout.add_worktree_called.get());
    assert!(checkout.remove_worktree_called.get());
}

#[test]
fn a_failing_forge_reports_the_list_as_failed() {
    let repo = tempfile::tempdir().expect("a temporary directory");
    let forge = FakeForge::new(
        Err(ForgeUnavailable("gh: command not found".to_owned())),
        Ok(Vec::new()),
    );

    let event = list(repo.path(), &forge);

    match event {
        PullRequestEvent::ListFailed { reason } => {
            assert_eq!(reason, "gh: command not found");
        }
        other => panic!("expected ListFailed, got {other:?}"),
    }
}

#[test]
fn a_failing_fetch_reports_the_open_as_failed() {
    let repo = tempfile::tempdir().expect("a temporary directory");
    let checkout = FakeCheckout::returning(Err(ForgeUnavailable(
        "git: pull request not found".to_owned(),
    )));
    let pr = pull_request(42, PullRequestState::Open, "Some title", "some-branch");
    let project_map = |_: &Path| None;

    let event = open(repo.path(), &pr, &checkout, &project_map);

    match event {
        PullRequestEvent::OpenFailed { number, reason } => {
            assert_eq!(number, 42);
            assert_eq!(reason, "git: pull request not found");
        }
        other => panic!("expected OpenFailed, got {other:?}"),
    }
    assert!(
        !checkout.add_worktree_called.get(),
        "no worktree is created when the diff itself could not be read"
    );
    assert!(!checkout.remove_worktree_called.get());
}

#[test]
fn the_temporary_worktree_is_removed_when_mapping_fails() {
    let repo = tempfile::tempdir().expect("a temporary directory");
    let checkout = FakeCheckout::returning(Ok(String::new()));
    let pr = pull_request(7, PullRequestState::Open, "A title", "a-branch");
    let project_map = |_: &Path| None;

    let event = open(repo.path(), &pr, &checkout, &project_map);

    assert!(
        checkout.add_worktree_called.get(),
        "a worktree is still created to attempt the mapping"
    );
    assert!(
        checkout.remove_worktree_called.get(),
        "the temporary worktree must be removed even when the project cannot be mapped"
    );
    // Judgement call (design.md requirement 6 does not say): a `Review`
    // promises size, flags *and* placements together, so a failed mapping
    // fails the whole open rather than handing back a review with no towers.
    match event {
        PullRequestEvent::OpenFailed { number, .. } => assert_eq!(number, 7),
        other => panic!("expected OpenFailed when mapping fails, got {other:?}"),
    }
}
