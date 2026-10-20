//! A git checkout's identity, read from git's own files: no `git` process is
//! ever run for this (`git rev-parse` per checkout was rejected in the
//! design, another child process and test seam).

use std::path::{Path, PathBuf};

/// The checkout holding `path`: the nearest ancestor (including `path`
/// itself) with a `.git` entry.
pub(crate) fn checkout_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| ancestor.join(".git").symlink_metadata().is_ok())
        .map(Path::to_owned)
}

/// The git common dir of the checkout at `root`: the same for every worktree
/// of one repository. `None` when `root` is not a git checkout.
pub(crate) fn common_dir(root: &Path) -> Option<PathBuf> {
    let git_entry = root.join(".git");
    let metadata = std::fs::metadata(&git_entry).ok()?;
    if metadata.is_dir() {
        return std::fs::canonicalize(&git_entry).ok();
    }
    let gitdir = read_gitdir_file(&git_entry, root)?;
    let commondir_text = std::fs::read_to_string(gitdir.join("commondir")).ok()?;
    let common = gitdir.join(commondir_text.trim());
    std::fs::canonicalize(&common).ok()
}

/// The branch checked out at `root`; empty string for a detached HEAD or
/// when it cannot be read.
pub(crate) fn branch(root: &Path) -> String {
    let Some(gitdir) = own_git_dir(root) else {
        return String::new();
    };
    let Ok(head) = std::fs::read_to_string(gitdir.join("HEAD")) else {
        return String::new();
    };
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .unwrap_or("")
        .to_owned()
}

/// The repository's display name: the parent folder's name of `common`, or
/// `root`'s own last path component when `common` is `None`.
pub(crate) fn repo_name(root: &Path, common: Option<&Path>) -> String {
    match common.and_then(Path::parent).and_then(|p| p.file_name()) {
        Some(name) => name.to_string_lossy().into_owned(),
        None => root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.to_string_lossy().into_owned()),
    }
}

/// The checkout's own git-dir at `root`: the `.git` folder itself when it is
/// a directory, or the path parsed out of a `.git` FILE's `gitdir:` line
/// when `root` is a worktree checkout.
fn own_git_dir(root: &Path) -> Option<PathBuf> {
    let git_entry = root.join(".git");
    let metadata = std::fs::metadata(&git_entry).ok()?;
    if metadata.is_dir() {
        return Some(git_entry);
    }
    read_gitdir_file(&git_entry, root)
}

/// Reads a `.git` FILE's single `gitdir: <path>` line, resolving a relative
/// path against `root`. `None` when the file cannot be read or lacks the
/// `gitdir:` prefix.
fn read_gitdir_file(git_entry: &Path, root: &Path) -> Option<PathBuf> {
    let contents = std::fs::read_to_string(git_entry).ok()?;
    let path_text = contents.trim().strip_prefix("gitdir:")?.trim();
    let path = PathBuf::from(path_text);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("a parent folder")).expect("create the folder");
        fs::write(path, contents).expect("write the fixture file");
    }

    /// Pins how a normal checkout and one of its worktrees, built the way
    /// real git lays them out (`.git/worktrees/<name>/{HEAD,commondir}` for
    /// the private git-dir, a `.git` FILE with `gitdir: <path>` for the
    /// worktree's own checkout), resolve to the same common dir and each
    /// read their own branch and repository name, even where the branch
    /// name itself contains a slash.
    #[test]
    fn checkouts_of_one_repository_share_a_common_dir() {
        let temp = tempfile::tempdir().expect("a temporary directory");
        let root = temp.path();

        // The main checkout: a real `.git` folder, its own `HEAD`.
        let main_root = root.join("main");
        fs::create_dir_all(main_root.join(".git")).expect("create main/.git");
        write(&main_root.join(".git/HEAD"), "ref: refs/heads/main\n");

        // A worktree's private git-dir, under the main checkout's `.git`.
        let worktree_git_dir = main_root.join(".git/worktrees/feature");
        fs::create_dir_all(&worktree_git_dir).expect("create the worktree's private git dir");
        write(
            &worktree_git_dir.join("HEAD"),
            "ref: refs/heads/feature/foo\n",
        );
        write(&worktree_git_dir.join("commondir"), "../..\n");

        // The worktree's own checkout folder: a `.git` FILE pointing at its
        // private git-dir.
        let feature_checkout_root = root.join("feature-checkout");
        fs::create_dir_all(&feature_checkout_root).expect("create the worktree checkout");
        let worktree_git_dir_abs =
            fs::canonicalize(&worktree_git_dir).expect("the private git dir resolves");
        write(
            &feature_checkout_root.join(".git"),
            &format!("gitdir: {}\n", worktree_git_dir_abs.display()),
        );

        let plain_folder = root.join("plain");
        fs::create_dir_all(&plain_folder).expect("create a non-git folder");

        let expected_common = fs::canonicalize(main_root.join(".git")).expect("main/.git resolves");

        let main_common = common_dir(&main_root);
        let feature_common = common_dir(&feature_checkout_root);
        assert_eq!(main_common, Some(expected_common.clone()));
        assert_eq!(
            feature_common,
            Some(expected_common),
            "a worktree shares the main checkout's common dir"
        );

        assert_eq!(branch(&main_root), "main");
        assert_eq!(
            branch(&feature_checkout_root),
            "feature/foo",
            "a branch name may itself contain a slash"
        );

        assert_eq!(
            repo_name(&main_root, main_common.as_deref()),
            repo_name(&feature_checkout_root, feature_common.as_deref()),
            "both checkouts of one repository share its display name"
        );
        assert_eq!(repo_name(&main_root, main_common.as_deref()), "main");

        assert_eq!(
            common_dir(&plain_folder),
            None,
            "a plain folder is not a git checkout"
        );
    }
}
