//! Reading the settings file: where it lives, and the folders it adds.
//!
//! Read-only by construction. This module exposes a path builder and a
//! reader and no write function at all — no `fs::write`, no `File::create`,
//! no `OpenOptions`, no `remove_file`. Until EPIC-008 brings the interface for
//! adding a folder, hand-editing this file is the only way to add one, so it
//! is the user's data and nothing here touches it.
//!
//! Parsing is tolerant: a key this story does not understand is ignored rather
//! than rejected. A file that cannot be parsed at all is reported, because
//! treating it as "no added folders" would look to the engineer exactly like
//! their projects had been deleted.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::event::WorkspaceEvent;

/// The key the settings file carries, and the only one this story reads.
const ADDED_FOLDERS: &str = "added_folders";

/// Which convention names the settings file's directory.
///
/// This is the one place a platform difference is encoded, and it encodes
/// *where a file lives* — not how two paths compare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Platform {
    MacOs,
    Xdg,
}

/// Where the settings file lives, with the environment passed in rather than
/// read — which is what lets both rows of the table be asserted on either
/// runner without a test reaching the real directory.
pub(crate) fn settings_file(
    platform: Platform,
    home: &Path,
    xdg_config_home: Option<&Path>,
) -> PathBuf {
    match platform {
        Platform::MacOs => home
            .join("Library")
            .join("Application Support")
            .join("Cellworks")
            .join("settings.toml"),
        Platform::Xdg => xdg_config_home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| home.join(".config"))
            .join("cellworks")
            .join("settings.toml"),
    }
}

/// What one settings file added, and what it could not.
#[derive(Debug, Default)]
pub(crate) struct AddedFolders {
    /// The canonical path of every entry that resolved to a readable
    /// directory.
    pub(crate) folders: Vec<PathBuf>,
    /// One event per entry that did not, plus one for a file that could not be
    /// read or parsed at all.
    pub(crate) events: Vec<WorkspaceEvent>,
}

/// Reads the folders `settings_file` adds.
///
/// An absent file means no added folders and is not an error. A file that
/// cannot be read or parsed is reported and adds nothing; the projects found
/// under the projects folder are unaffected.
pub(crate) fn read(settings_file: &Path) -> AddedFolders {
    let mut added = AddedFolders::default();

    let text = match fs::read_to_string(settings_file) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return added,
        Err(error) => {
            added.events.push(unreadable(settings_file, &error));
            return added;
        }
    };

    let table = match text.parse::<toml::Table>() {
        Ok(table) => table,
        Err(error) => {
            added.events.push(unreadable(settings_file, &error));
            return added;
        }
    };

    // Tolerant: a missing key, a key of the wrong shape, and every key this
    // story does not know are all simply "no added folders from here".
    let entries = table
        .get(ADDED_FOLDERS)
        .and_then(toml::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    for entry in entries.iter().filter_map(toml::Value::as_str) {
        match resolve(Path::new(entry)) {
            Ok(canonical) => added.folders.push(canonical),
            Err(reason) => added.events.push(WorkspaceEvent::AddedFolderSkipped {
                path: PathBuf::from(entry),
                reason,
            }),
        }
    }

    added
}

/// The canonical path of an added folder, or why it is not one.
///
/// `~` is not expanded and a relative entry is refused rather than resolved
/// against whatever the working directory happens to be.
fn resolve(entry: &Path) -> Result<PathBuf, String> {
    if !entry.is_absolute() {
        return Err("the path is not absolute".to_owned());
    }

    let canonical = fs::canonicalize(entry).map_err(|error| error.to_string())?;
    let metadata = fs::metadata(&canonical).map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err("the path is not a directory".to_owned());
    }

    Ok(canonical)
}

fn unreadable(path: &Path, reason: &dyn std::fmt::Display) -> WorkspaceEvent {
    WorkspaceEvent::SettingsUnreadable {
        path: path.to_path_buf(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_tree::{TestTree, canonical};

    #[test]
    fn on_macos_the_settings_file_is_in_application_support() {
        let path = settings_file(Platform::MacOs, Path::new("/Users/engineer"), None);

        assert_eq!(
            path,
            Path::new("/Users/engineer/Library/Application Support/Cellworks/settings.toml")
        );
    }

    #[test]
    fn with_xdg_config_home_set_the_settings_file_is_below_it() {
        let path = settings_file(
            Platform::Xdg,
            Path::new("/home/engineer"),
            Some(Path::new("/home/engineer/elsewhere")),
        );

        assert_eq!(
            path,
            Path::new("/home/engineer/elsewhere/cellworks/settings.toml")
        );
    }

    #[test]
    fn without_xdg_config_home_the_settings_file_is_below_dot_config() {
        let path = settings_file(Platform::Xdg, Path::new("/home/engineer"), None);

        assert_eq!(
            path,
            Path::new("/home/engineer/.config/cellworks/settings.toml")
        );
    }

    #[test]
    fn an_absent_settings_file_adds_nothing_and_is_not_an_error() {
        let tree = TestTree::new();

        let added = read(&tree.sources().settings_file);

        assert!(added.folders.is_empty());
        assert!(added.events.is_empty());
    }

    #[test]
    fn an_entry_that_is_not_a_readable_directory_is_skipped_and_reported() {
        let tree = TestTree::new();
        let work = tree.plain_folder("work/infra");
        let regular_file = tree.file("work/notes.txt", "a file, not a folder\n");
        let absent = tree.path().join("work/gone");
        tree.file(
            "settings.toml",
            &format!(
                "added_folders = [\n  \"{}\",\n  \"{}\",\n  \"relative/path\",\n  \"{}\",\n]\n",
                absent.display(),
                regular_file.display(),
                work.display(),
            ),
        );

        let added = read(&tree.sources().settings_file);

        assert_eq!(added.folders, vec![canonical(&work)]);
        let skipped: Vec<&PathBuf> = added
            .events
            .iter()
            .filter_map(|event| match event {
                WorkspaceEvent::AddedFolderSkipped { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(
            skipped,
            vec![&absent, &regular_file, &PathBuf::from("relative/path")],
            "each entry that did not resolve is reported, naming itself"
        );
        assert_eq!(added.events.len(), 3, "and nothing else is reported");
    }

    #[test]
    fn a_settings_file_that_cannot_be_parsed_is_reported() {
        let tree = TestTree::new();
        let settings = tree.file("settings.toml", "added_folders = [\n");

        let added = read(&settings);

        assert!(added.folders.is_empty());
        assert!(
            matches!(
                added.events.as_slice(),
                [WorkspaceEvent::SettingsUnreadable { path, .. }] if path == &settings
            ),
            "expected one SettingsUnreadable naming the file, got {:?}",
            added.events
        );
    }

    #[test]
    fn a_key_this_story_does_not_understand_is_ignored_rather_than_rejected() {
        let tree = TestTree::new();
        let work = tree.plain_folder("work/infra");
        let settings = tree.file(
            "settings.toml",
            &format!(
                "theme = \"dark\"\nadded_folders = [\"{}\"]\n\n[window]\nwidth = 1280\n",
                work.display()
            ),
        );

        let added = read(&settings);

        assert_eq!(added.folders, vec![canonical(&work)]);
        assert!(added.events.is_empty(), "{:?}", added.events);
    }

    #[test]
    fn a_non_string_entry_is_ignored_rather_than_rejected() {
        let tree = TestTree::new();
        let work = tree.plain_folder("work/infra");
        let settings = tree.file(
            "settings.toml",
            &format!("added_folders = [17, \"{}\"]\n", work.display()),
        );

        let added = read(&settings);

        assert_eq!(added.folders, vec![canonical(&work)]);
        assert!(added.events.is_empty(), "{:?}", added.events);
    }
}
