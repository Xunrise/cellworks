//! The CodeAnalysis bounded context: scanning code projects with tree-sitter
//! and git into a graph, and the file and function facts DomainKnowledge reads.
//!
//! Defined by docs/domain/contexts/CodeAnalysis/CodeAnalysis.md. So far it
//! finds a project's included files and hands them to the Map: in a git
//! repository git says which files it shows, elsewhere the folder is walked,
//! and hidden folders are left out either way. Rust files are read with
//! tree-sitter into items: functions, types and `use` blocks, line by line,
//! with the calls they make resolved to the item called. Every file is told
//! by its kind, and any other file holding text is read in the parts its
//! kind has: a document's sections, a configuration's keys, a feature
//! file's scenarios; else one tower of its lines. With ignored files switched on,
//! the folder is walked into hidden folders too, only `.git` folders are left
//! out, and a large ignored folder stays closed: one node with its file count.
//! In a git repository, every visible file is blamed and joined to the pull
//! request and the stories its commit names, into `ProjectMap::history`.
//! [`load`] is an ordinary blocking function and
//! [`start_load`] is the thread around it.

mod closed;
mod co_change;
mod event;
mod files;
mod git;
mod history;
mod kinds;
mod plain;
mod rust;

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use cellworks_map::{Directories, NodeKind, PrefixRule, ProjectMap};

pub use event::CodeAnalysisEvent;
pub use git::{GitCli, GitListing, GitUnavailable};

/// Reads the project at `root` and maps it, with what its git history says
/// changed together, and says what it met on the way.
///
/// Blocking: called on the background thread [`start_load`] spawns, and
/// directly from a test. [`CodeAnalysisEvent::ProjectMapped`] comes last, and
/// not at all when the project folder itself cannot be read. With `ignored`,
/// git-ignored files and hidden folders are on the map too. Per-line history
/// is read the same way co-change is: nothing fetched from GitHub, empty
/// when the folder is not a repository or the log fails.
pub fn load(
    root: &Path,
    name: &str,
    prefix: &'static PrefixRule,
    git: &dyn GitListing,
    ignored: bool,
) -> Vec<CodeAnalysisEvent> {
    let project = root.to_path_buf();
    let mut events = Vec::new();

    if let Err(error) = std::fs::read_dir(root) {
        events.push(CodeAnalysisEvent::ProjectFolderSkipped {
            project: project.clone(),
            path: project,
            reason: error.to_string(),
        });
        return events;
    }

    // A repository exactly when `.git` is there, of either kind: a directory,
    // or the file a linked worktree has.
    let is_repository = root.join(".git").symlink_metadata().is_ok();
    let listed = if is_repository {
        git.visible_files(root)
    } else {
        Err(GitUnavailable(String::new()))
    };

    let mut skipped = Vec::new();
    let visible = match listed {
        Ok(files) => files,
        Err(GitUnavailable(reason)) => {
            if is_repository {
                events.push(CodeAnalysisEvent::GitExcludesUnavailable {
                    project: project.clone(),
                    reason,
                });
            }
            let (files, unread) = files::walk(root, false);
            skipped = unread;
            files
        }
    };
    let visible: Vec<PathBuf> = visible
        .into_iter()
        .filter(|file| !files::in_hidden_folder(file))
        .collect();

    // ponytail: a closed folder is still walked in full to count its files;
    // stop counting at the limit if reading `node_modules` is slow.
    let (files, closed) = if ignored {
        let (all, unread) = files::walk(root, true);
        skipped = unread;
        closed::close(all, &visible.iter().cloned().collect())
    } else {
        (visible, Default::default())
    };
    events.extend(skipped.into_iter().map(|(path, reason)| {
        CodeAnalysisEvent::ProjectFolderSkipped {
            project: project.clone(),
            path,
            reason,
        }
    }));

    let mut items = rust::read(root, &files);
    // A file no parser read stands as one tower of its lines.
    let unread: Vec<&PathBuf> = files.iter().filter(|f| !items.contains_key(*f)).collect();
    items.extend(plain::read(root, unread));
    let kinds = files
        .iter()
        .map(|file| (file.clone(), kinds::kind_of(file)))
        .filter(|(_, kind)| *kind != NodeKind::Code)
        .collect();
    let strategy = Directories { prefix };
    let mut map = ProjectMap::of_code(name, root, &files, &strategy, items, &kinds);
    map.close(&closed);
    // Without history the co-change lens is empty; git saying why has been
    // reported with the listing already.
    if is_repository && let Ok(commits) = git.history(root) {
        map.co_changes = co_change::co_changes(root, &files, &commits);
    }
    // Per-line provenance: nothing is fetched from GitHub, the same as
    // above, and a failing log leaves it empty the same way.
    // ponytail: blaming every visible file adds about 1s on this repository
    // and grows linearly with file count; blame lazily per opened district
    // if that ever becomes the ceiling.
    if is_repository && let Ok(logged) = git.log(root) {
        let blamed = git.blame(root, &files);
        map.history = history::join(root, logged, blamed);
    }
    events.push(CodeAnalysisEvent::ProjectMapped {
        project,
        map: Box::new(map),
    });
    events
}

/// Runs [`load`] on a `std::thread` and hands back the receiving end.
///
/// A send that fails is the UI having gone away; the thread ends quietly.
pub fn start_load(
    root: PathBuf,
    name: String,
    prefix: &'static PrefixRule,
    git: Arc<dyn GitListing + Send + Sync>,
    ignored: bool,
) -> mpsc::Receiver<CodeAnalysisEvent> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        for event in load(&root, &name, prefix, git.as_ref(), ignored) {
            if sender.send(event).is_err() {
                return;
            }
        }
    });

    receiver
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::fs;

    use cellworks_map::default_prefix_rule;

    use super::*;
    use crate::git::LoggedCommit;

    /// A recorded answer from git, and whether it was asked.
    struct Recorded {
        answer: Result<Vec<PathBuf>, GitUnavailable>,
        asked: Cell<bool>,
    }

    impl Recorded {
        fn listing(files: &[&str]) -> Recorded {
            Recorded {
                answer: Ok(files.iter().map(PathBuf::from).collect()),
                asked: Cell::new(false),
            }
        }
    }

    impl GitListing for Recorded {
        fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
            self.asked.set(true);
            self.answer.clone()
        }
    }

    /// A recorded listing that also answers `log` and `blame`.
    struct WithHistory {
        files: Vec<PathBuf>,
        commits: Vec<LoggedCommit>,
        blamed: BTreeMap<PathBuf, Vec<Option<String>>>,
    }

    impl GitListing for WithHistory {
        fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
            Ok(self.files.clone())
        }

        fn log(&self, _: &Path) -> Result<Vec<LoggedCommit>, GitUnavailable> {
            Ok(self.commits.clone())
        }

        fn blame(&self, _: &Path, _: &[PathBuf]) -> BTreeMap<PathBuf, Vec<Option<String>>> {
            self.blamed.clone()
        }
    }

    fn tree(files: &[&str]) -> tempfile::TempDir {
        let tree = tempfile::tempdir().expect("a temporary directory");
        for file in files {
            let path = tree.path().join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, "").expect("write the file");
        }
        tree
    }

    fn map_of(events: &[CodeAnalysisEvent]) -> &ProjectMap {
        match events.last() {
            Some(CodeAnalysisEvent::ProjectMapped { map, .. }) => map,
            other => panic!("expected the map last, got {other:?}"),
        }
    }

    fn district_names(events: &[CodeAnalysisEvent]) -> Vec<&str> {
        map_of(events)
            .districts
            .iter()
            .map(|d| d.name.as_str())
            .collect()
    }

    fn read(root: &Path, git: &dyn GitListing) -> Vec<CodeAnalysisEvent> {
        load(root, "shop", default_prefix_rule(), git, false)
    }

    #[test]
    fn with_ignored_files_on_only_dot_git_is_left_out() {
        let tree = tree(&[
            ".git/HEAD",
            ".github/workflows/ci.yml",
            "out/output.o",
            "src/catalog/item.rs",
        ]);
        let git = Recorded::listing(&["src/catalog/item.rs"]);

        let events = load(tree.path(), "shop", default_prefix_rule(), &git, true);

        assert_eq!(district_names(&events), [".github", "out", "src"]);
    }

    #[test]
    fn a_dot_git_directory_or_file_makes_a_repository_and_git_is_asked() {
        for kind in ["directory", "file", "none"] {
            let tree = tree(&["src/a.rs"]);
            match kind {
                "directory" => fs::create_dir(tree.path().join(".git")).expect("create .git"),
                "file" => fs::write(tree.path().join(".git"), "gitdir: x").expect("write .git"),
                _ => {}
            }
            let git = Recorded::listing(&["src/a.rs"]);

            read(tree.path(), &git);

            assert_eq!(git.asked.get(), kind != "none", "{kind}");
        }
    }

    #[test]
    fn each_node_carries_the_kind_of_its_file() {
        let tree = tree(&["README.md", "src/main.rs", "deploy/main.tf"]);

        let events = read(tree.path(), &Recorded::listing(&[]));

        let map = map_of(&events);
        let kind = |file: &str| {
            map.districts
                .iter()
                .flat_map(|d| {
                    d.nodes
                        .iter()
                        .chain(d.sub_districts.iter().flat_map(|s| &s.nodes))
                })
                .find(|node| node.path.ends_with(file))
                .map(|node| node.kind)
        };
        assert_eq!(kind("README.md"), Some(NodeKind::Document));
        assert_eq!(kind("src/main.rs"), Some(NodeKind::Code));
        assert_eq!(kind("deploy/main.tf"), Some(NodeKind::Infrastructure));
    }

    #[test]
    fn a_file_git_does_not_list_is_left_out() {
        let tree = tree(&[".git/HEAD", "build/output.o", "src/catalog/item.rs"]);
        let git = Recorded::listing(&["src/catalog/item.rs"]);

        assert_eq!(district_names(&read(tree.path(), &git)), ["catalog"]);
    }

    #[test]
    fn hidden_folders_are_left_out_even_when_git_lists_them() {
        let tree = tree(&[".git/HEAD"]);
        let git = Recorded::listing(&[".cache/stale.bin", ".github/workflows/ci.yml", "src/a.rs"]);

        assert_eq!(district_names(&read(tree.path(), &git)), ["src"]);
    }

    #[test]
    fn outside_a_repository_only_hidden_folders_are_left_out() {
        let tree = tree(&[".cache/x", "build/x"]);
        let git = Recorded::listing(&[]);

        assert_eq!(district_names(&read(tree.path(), &git)), ["build"]);
    }

    #[test]
    fn when_git_cannot_answer_the_folder_is_walked_and_that_is_said() {
        let tree = tree(&[".git/HEAD", "build/output.o"]);
        let git = Recorded {
            answer: Err(GitUnavailable("git: not found".to_owned())),
            asked: Cell::new(false),
        };

        let events = read(tree.path(), &git);

        assert!(
            matches!(&events[0], CodeAnalysisEvent::GitExcludesUnavailable { reason, .. }
                if reason == "git: not found"),
            "{events:?}"
        );
        assert_eq!(district_names(&events), ["build"]);
    }

    #[test]
    fn an_empty_folder_draws_nothing() {
        let tree = tree(&["src/catalog/item.rs"]);
        fs::create_dir_all(tree.path().join("docs")).expect("create docs");

        let events = read(tree.path(), &Recorded::listing(&[]));

        assert_eq!(district_names(&events), ["catalog"]);
    }

    #[test]
    fn a_folder_that_cannot_be_read_is_skipped_and_the_rest_is_mapped() {
        use std::os::unix::fs::PermissionsExt;

        let tree = tree(&["src/catalog/item.rs", "src/closed/x.rs"]);
        let closed = tree.path().join("src/closed");
        fs::set_permissions(&closed, fs::Permissions::from_mode(0o000))
            .expect("take the permissions off the folder");

        let readable_anyway = fs::read_dir(&closed).is_ok();
        let events = read(tree.path(), &Recorded::listing(&[]));

        fs::set_permissions(&closed, fs::Permissions::from_mode(0o755))
            .expect("give the permissions back, or the temporary directory cannot be removed");
        assert!(
            !readable_anyway,
            "this test needs a non-root user: mode 0o000 did not stop the read"
        );

        assert!(
            events.iter().any(|event| matches!(event,
                CodeAnalysisEvent::ProjectFolderSkipped { path, .. } if path == &closed)),
            "{events:?}"
        );
        assert_eq!(district_names(&events), ["catalog"]);
    }

    #[test]
    fn a_project_folder_that_has_gone_away_is_reported_and_not_mapped() {
        let tree = tree(&[]);
        let gone = tree.path().join("gone");

        let events = read(&gone, &Recorded::listing(&[]));

        assert!(
            matches!(events.as_slice(),
                [CodeAnalysisEvent::ProjectFolderSkipped { path, .. }] if path == &gone),
            "{events:?}"
        );
    }

    #[test]
    fn start_load_reports_over_the_channel() {
        let tree = tree(&["src/a.rs"]);

        let events = start_load(
            tree.path().to_path_buf(),
            "shop".to_owned(),
            default_prefix_rule(),
            Arc::new(GitCli),
            false,
        );
        let reported = events
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("the read thread reports");

        assert!(matches!(reported, CodeAnalysisEvent::ProjectMapped { .. }));
    }

    #[test]
    fn each_floor_knows_its_commit_pull_request_and_stories() {
        let tree = tree(&["a.txt"]);
        // `load` only asks git for history inside a repository.
        fs::create_dir(tree.path().join(".git")).expect("create .git");
        fs::write(tree.path().join("a.txt"), "first\nsecond\n").expect("write a.txt");
        let mut blamed = BTreeMap::new();
        blamed.insert(PathBuf::from("a.txt"), vec![Some("c1".to_owned()), None]);
        let git = WithHistory {
            files: vec![PathBuf::from("a.txt")],
            commits: vec![LoggedCommit {
                sha: "c1".to_owned(),
                time: 1000,
                message: "feat: a thing (#9)\n\nRefs: STORY-4".to_owned(),
            }],
            blamed,
        };

        let events = read(tree.path(), &git);

        let map = map_of(&events);
        let node = tree.path().join("a.txt");
        let items = map.items_of(&node).expect("a.txt has items");
        let tower = &items[0];
        // Looked up from the loaded map's own item, not a hand-built path: a
        // mismatch in how `history.lines` keys its files would make this (and
        // every real lookup) silently find nothing, while still passing any
        // test that hard-codes the expected path instead.
        let file = tower
            .declaration
            .part_of
            .as_deref()
            .unwrap_or(&tower.path)
            .parent()
            .expect("a file");

        let first = map
            .history
            .of_line(file, tower.floors[0].number)
            .expect("line 1 has a commit");
        assert_eq!(first.sha, "c1");
        assert_eq!(first.pull_request, Some(9));
        assert_eq!(first.stories, vec!["STORY-004".to_owned()]);
        assert_eq!(
            map.history.of_line(file, tower.floors[1].number),
            None,
            "line 2 is uncommitted"
        );
        assert_eq!(map.history.commits.len(), 1);
    }

    #[test]
    fn a_folder_without_git_has_no_history() {
        let tree = tree(&["a.txt"]);

        let events = read(tree.path(), &Recorded::listing(&[]));

        let map = map_of(&events);
        assert!(map.history.commits.is_empty());
        assert!(map.history.lines.is_empty());
    }
}
