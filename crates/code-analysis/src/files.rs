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

/// Every file below `root`, relative to it, skipping hidden folders and not
/// following symbolic links; plus every folder that could not be read and why.
pub(crate) fn walk(root: &Path) -> (Vec<PathBuf>, Vec<(PathBuf, String)>) {
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
            if is_dir {
                if entry.file_name().as_encoded_bytes().first() != Some(&b'.') {
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

    #[test]
    fn the_walk_skips_hidden_folders_only() {
        let tree = tempfile::tempdir().expect("a temporary directory");
        for file in [".cache/stale.bin", "build/output.o", ".env"] {
            let path = tree.path().join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, "").expect("write the file");
        }

        let (mut files, skipped) = walk(tree.path());
        files.sort();

        assert_eq!(
            files,
            [PathBuf::from(".env"), PathBuf::from("build/output.o")]
        );
        assert!(skipped.is_empty());
    }
}
