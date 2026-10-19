//! The files of a folder that is not a git repository, and the hidden-folder
//! rule every source shares.

use std::fs;
use std::path::{Path, PathBuf};

/// Whether any folder on `relative`'s way down begins with `.`. The file's own
/// name does not count: hidden files are kept, hidden folders are not.
pub(crate) fn in_hidden_folder(relative: &Path) -> bool {
    let mut folders = relative.iter().collect::<Vec<_>>();
    folders.pop();
    folders
        .iter()
        .any(|name| name.as_encoded_bytes().first() == Some(&b'.'))
}

/// Every file below `root`, relative to it, not following symbolic links; plus
/// every folder that could not be read and why. Hidden folders are skipped
/// unless `with_hidden`, and `.git` is skipped either way, folder or file: it
/// is git's, not a file of the project.
pub(crate) fn walk(root: &Path, with_hidden: bool) -> (Vec<PathBuf>, Vec<(PathBuf, String)>) {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                skipped.push((directory, error.to_string()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    skipped.push((directory.clone(), error.to_string()));
                    continue;
                }
            };
            // `file_type` rather than `metadata`: a symlink is not followed.
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            let path = entry.path();
            let name = entry.file_name();
            // The folder, or the pointer file a linked worktree has.
            if name == ".git" {
                continue;
            }
            if is_dir {
                let hidden = name.as_encoded_bytes().first() == Some(&b'.');
                if with_hidden || !hidden {
                    pending.push(path);
                }
            } else if let Ok(relative) = path.strip_prefix(root) {
                files.push(relative.to_path_buf());
            }
        }
    }

    (files, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_in_a_hidden_folder_is_hidden_and_a_hidden_file_is_not() {
        assert!(in_hidden_folder(Path::new(".github/workflows/ci.yml")));
        assert!(in_hidden_folder(Path::new("src/.cache/x")));
        assert!(!in_hidden_folder(Path::new(".gitignore")));
        assert!(!in_hidden_folder(Path::new("src/main.rs")));
    }

    fn tree() -> tempfile::TempDir {
        let tree = tempfile::tempdir().expect("a temporary directory");
        for file in [
            ".cache/stale.bin",
            "build/output.o",
            ".env",
            ".git/HEAD",
            "nested/.git/HEAD",
            "worktree/.git",
        ] {
            let path = tree.path().join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, "").expect("write the file");
        }
        tree
    }

    fn walked(root: &Path, with_hidden: bool) -> Vec<PathBuf> {
        let (mut files, skipped) = walk(root, with_hidden);
        assert!(skipped.is_empty());
        files.sort();
        files
    }

    #[test]
    fn the_walk_skips_hidden_folders_only() {
        let tree = tree();

        assert_eq!(
            walked(tree.path(), false),
            [PathBuf::from(".env"), PathBuf::from("build/output.o")]
        );
    }

    #[test]
    fn with_hidden_the_walk_enters_hidden_folders_but_never_git() {
        let tree = tree();

        assert_eq!(
            walked(tree.path(), true),
            [
                PathBuf::from(".cache/stale.bin"),
                PathBuf::from(".env"),
                PathBuf::from("build/output.o")
            ]
        );
    }
}
