//! `GitCli` against a real repository it creates: the base and head commits
//! are already present, so `diff` fetches nothing and touches no network;
//! `add_worktree`/`remove_worktree` check a commit out and back in. Ignored
//! by default, because the acceptance suite never runs real git; skips
//! quietly when git is absent.
//!
//! cargo test --workspace --release -- --ignored --nocapture

use std::fs;
use std::path::Path;
use std::process::Command;

use cellworks_sources::pull_requests::forge::{Checkout, GitCli};
use cellworks_sources::pull_requests::{PullRequest, PullRequestState};

fn git(repository: &Path, arguments: &[&str]) -> bool {
    Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .status()
        .is_ok_and(|status| status.success())
}

fn git_output(repository: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .expect("git runs");
    String::from_utf8(output.stdout)
        .expect("git prints UTF-8")
        .trim()
        .to_owned()
}

fn pull_request(base: String, head: String) -> PullRequest {
    PullRequest {
        number: 1,
        state: PullRequestState::Open,
        title: "A pull request".to_owned(),
        branch: "feature/x".to_owned(),
        head,
        base,
        author: "octocat".to_owned(),
        story: None,
    }
}

#[test]
#[ignore = "runs real git"]
fn git_cli_diffs_two_commits_and_checks_out_a_detached_worktree() {
    let tree = tempfile::tempdir().expect("a temporary directory");
    let repository = tree.path();
    if !git(repository, &["init", "-q"]) {
        println!("git is absent; skipping");
        return;
    }
    git(repository, &["config", "user.email", "test@example.com"]);
    git(repository, &["config", "user.name", "Test"]);

    fs::create_dir_all(repository.join("src")).expect("create src");
    fs::write(repository.join("src/lib.rs"), "fn old() {}\n").expect("write lib.rs");
    assert!(git(repository, &["add", "src/lib.rs"]));
    assert!(git(repository, &["commit", "-q", "-m", "base"]));
    let base = git_output(repository, &["rev-parse", "HEAD"]);

    fs::write(
        repository.join("src/lib.rs"),
        "fn new_one() {}\nfn new_two() {}\n",
    )
    .expect("rewrite lib.rs");
    assert!(git(repository, &["commit", "-q", "-am", "head"]));
    let head = git_output(repository, &["rev-parse", "HEAD"]);

    let pr = pull_request(base, head.clone());

    let diff = GitCli
        .diff(repository, &pr)
        .expect("git diffs two commits already present locally, no fetch needed");

    assert!(diff.contains("-fn old() {}"), "{diff}");
    assert!(diff.contains("+fn new_one() {}"), "{diff}");
    assert!(diff.contains("+fn new_two() {}"), "{diff}");

    let worktree = tree.path().join("scratch-worktree");
    GitCli
        .add_worktree(repository, &head, &worktree)
        .expect("a worktree can be added at the head commit");
    assert_eq!(
        fs::read_to_string(worktree.join("src/lib.rs")).expect("read the checked-out file"),
        "fn new_one() {}\nfn new_two() {}\n"
    );

    GitCli
        .remove_worktree(repository, &worktree)
        .expect("the worktree can be removed");
    assert!(
        !worktree.exists(),
        "the worktree folder is gone once removed"
    );
}
