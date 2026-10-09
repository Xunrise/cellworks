//! `GitCli` against a real repository it creates. Ignored by default, because
//! the acceptance suite never runs real git; skips when git is absent.
//!
//! cargo test --workspace --release -- --ignored --nocapture

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
#[ignore = "runs real git"]
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
