//! `GitCli` against a real repository it creates; skips when git is absent.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cellworks_code_analysis::{GitCli, GitListing};

fn git(repository: &Path, arguments: &[&str]) -> bool {
    Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn git_lists_tracked_and_untracked_files_but_not_ignored_or_deleted_ones() {
    let tree = tempfile::tempdir().expect("a temporary directory");
    let repository = tree.path();
    if !git(repository, &["init", "-q"]) {
        println!("git is absent; skipping");
        return;
    }
    for (file, text) in [
        (".gitignore", "build/\n"),
        ("build/output.o", ""),
        ("src/tracked.rs", ""),
        ("src/gone.rs", ""),
        ("src/untracked.rs", ""),
    ] {
        let path = repository.join(file);
        fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        fs::write(path, text).expect("write the file");
    }
    assert!(git(
        repository,
        &["add", ".gitignore", "src/tracked.rs", "src/gone.rs"]
    ));
    fs::remove_file(repository.join("src/gone.rs")).expect("delete the file");

    let files = GitCli.visible_files(repository).expect("git answers");

    assert_eq!(
        files,
        [".gitignore", "src/tracked.rs", "src/untracked.rs"]
            .map(PathBuf::from)
            .to_vec()
    );
}

/// A repository's own `core.fsmonitor` names a program; `GitCli` must not run
/// it. The control is `git ls-files --cached --others --exclude-standard`,
/// the command `GitCli` runs minus the hardening, which does run it.
#[test]
fn git_does_not_run_the_fsmonitor_a_repository_names() {
    use std::os::unix::fs::PermissionsExt;

    let tree = tempfile::tempdir().expect("a temporary directory");
    let repository = tree.path().join("repo");
    fs::create_dir(&repository).expect("create the repository");
    if !git(&repository, &["init", "-q"]) {
        println!("git is absent; skipping");
        return;
    }
    let marker = tree.path().join("marker");
    let script = tree.path().join("fsmonitor.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .expect("write script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("make it executable");
    fs::write(repository.join("a.rs"), "").expect("write a file");
    assert!(git(&repository, &["add", "a.rs"]));
    assert!(git(
        &repository,
        &["config", "core.fsmonitor", &script.display().to_string()]
    ));

    GitCli.visible_files(&repository).expect("git answers");
    let _ = GitCli.history(&repository);
    let _ = GitCli.branch(&repository);
    assert!(!marker.exists(), "GitCli ran the repository's fsmonitor");

    assert!(git(
        &repository,
        &["ls-files", "--cached", "--others", "--exclude-standard"]
    ));
    assert!(
        marker.exists(),
        "control: plain ls-files runs the fsmonitor"
    );
}
