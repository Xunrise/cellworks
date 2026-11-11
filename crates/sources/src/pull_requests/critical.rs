//! Critical-change flags: a project's `sdlc/critical.yaml` names flags, each
//! with the globs a changed file's lowercased path is matched against. `*`
//! matches any run of characters, `/` included. A missing file means no
//! flags; a line that cannot be read (a glob before any flag name, say) is
//! skipped rather than panicking.

use std::collections::BTreeMap;
use std::path::Path;

/// One flag of `sdlc/critical.yaml`, named, with the globs that raise it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    pub name: String,
    pub globs: Vec<String>,
}

/// The flags of `repo`'s `sdlc/critical.yaml`, in the order they appear.
/// A missing file means no flags; a glob line before any flag name is an
/// orphan and is skipped.
pub fn read_flags(repo: &Path) -> Vec<Flag> {
    let Ok(text) = std::fs::read_to_string(repo.join("sdlc/critical.yaml")) else {
        return Vec::new();
    };
    let mut flags: Vec<Flag> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(glob) = trimmed.strip_prefix("- ") {
            if let Some(flag) = flags.last_mut() {
                flag.globs.push(glob.trim().trim_matches('"').to_owned());
            }
        } else if let Some(name) = trimmed.strip_suffix(':')
            && name != "flags"
        {
            flags.push(Flag {
                name: name.to_owned(),
                globs: Vec::new(),
            });
        }
    }
    flags
}

/// Whether `glob` matches the whole of `path`. `*` matches any run of
/// characters, `/` included: the glob is split on `*` and each part is
/// looked for in order, the first as a required prefix, the last as a
/// required suffix, the rest found in sequence between them.
pub fn glob_matches(glob: &str, path: &str) -> bool {
    let parts: Vec<&str> = glob.split('*').collect();
    let last = parts.len() - 1;
    if last == 0 {
        return path == parts[0];
    }
    let (prefix, suffix) = (parts[0], parts[last]);
    if !path.starts_with(prefix) || !path.ends_with(suffix) {
        return false;
    }
    if path.len() < prefix.len() + suffix.len() {
        return false;
    }
    let mut rest = &path[prefix.len()..path.len() - suffix.len()];
    for part in &parts[1..last] {
        let Some(at) = rest.find(part) else {
            return false;
        };
        rest = &rest[at + part.len()..];
    }
    true
}

/// Each flag the paths raise, with the files (in their original case) that
/// raised it, each listed once. A flag no path raises is absent.
pub fn flags_raised(flags: &[Flag], paths: &[&str]) -> BTreeMap<String, Vec<String>> {
    let mut raised: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in paths {
        let lower = path.to_lowercase();
        for flag in flags {
            if flag.globs.iter().any(|glob| glob_matches(glob, &lower)) {
                raised
                    .entry(flag.name.clone())
                    .or_default()
                    .push((*path).to_owned());
            }
        }
    }
    raised
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_critical_yaml_gives_no_flags() {
        let project = tempfile::tempdir().expect("a temporary directory");

        let flags = read_flags(project.path());

        assert!(flags.is_empty(), "{flags:?}");
    }

    #[test]
    fn a_glob_line_before_any_flag_name_is_skipped_rather_than_panicking() {
        let project = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(project.path().join("sdlc")).expect("create sdlc");
        std::fs::write(
            project.path().join("sdlc/critical.yaml"),
            "flags:\n  - \"orphan.txt\"\n  dependencies:\n    - \"cargo.lock\"\n",
        )
        .expect("write critical.yaml");

        let flags = read_flags(project.path());

        assert_eq!(
            flags,
            vec![Flag {
                name: "dependencies".to_owned(),
                globs: vec!["cargo.lock".to_owned()],
            }],
            "the orphan glob line names no flag and must not panic or appear: {flags:?}"
        );
    }

    #[test]
    fn a_project_with_two_flags_reads_both_with_their_globs() {
        let project = tempfile::tempdir().expect("a temporary directory");
        std::fs::create_dir(project.path().join("sdlc")).expect("create sdlc");
        std::fs::write(
            project.path().join("sdlc/critical.yaml"),
            "flags:\n  dependencies:\n    - \"cargo.lock\"\n    - \"**/cargo.toml\"\n  security:\n    - \"*.pem\"\n",
        )
        .expect("write critical.yaml");

        let flags = read_flags(project.path());

        assert_eq!(
            flags,
            vec![
                Flag {
                    name: "dependencies".to_owned(),
                    globs: vec!["cargo.lock".to_owned(), "**/cargo.toml".to_owned()],
                },
                Flag {
                    name: "security".to_owned(),
                    globs: vec!["*.pem".to_owned()],
                },
            ],
            "{flags:?}"
        );
    }

    #[test]
    fn a_star_matches_any_run_of_characters_including_a_slash() {
        assert!(glob_matches("*.rs", "crates/sources/src/lib.rs"));
        assert!(!glob_matches("*.rs", "crates/sources/src/lib.txt"));
        assert!(glob_matches("**/cargo.toml", "crates/sources/cargo.toml"));
    }

    #[test]
    fn a_flag_is_raised_when_the_lowercased_path_matches_but_the_original_case_is_kept() {
        let flags = vec![Flag {
            name: "dependencies".to_owned(),
            globs: vec!["cargo.lock".to_owned()],
        }];

        let raised = flags_raised(&flags, &["Cargo.lock"]);

        assert_eq!(
            raised.get("dependencies"),
            Some(&vec!["Cargo.lock".to_owned()]),
            "the match is case-insensitive but the file is recorded in its original case: {raised:?}"
        );
    }

    #[test]
    fn a_flag_raised_by_more_than_one_file_names_every_file_that_raised_it() {
        let flags = vec![Flag {
            name: "dependencies".to_owned(),
            globs: vec!["cargo.lock".to_owned(), "cargo.toml".to_owned()],
        }];

        let raised = flags_raised(&flags, &["Cargo.lock", "Cargo.toml", "src/lib.rs"]);

        assert_eq!(
            raised.get("dependencies"),
            Some(&vec!["Cargo.lock".to_owned(), "Cargo.toml".to_owned()]),
            "{raised:?}"
        );
    }

    #[test]
    fn a_flag_no_path_raises_is_absent_from_the_result() {
        let flags = vec![
            Flag {
                name: "dependencies".to_owned(),
                globs: vec!["cargo.lock".to_owned()],
            },
            Flag {
                name: "security".to_owned(),
                globs: vec!["*.pem".to_owned()],
            },
        ];

        let raised = flags_raised(&flags, &["Cargo.lock"]);

        assert!(
            !raised.contains_key("security"),
            "no path raised the security flag: {raised:?}"
        );
        assert_eq!(raised.keys().collect::<Vec<_>>(), vec!["dependencies"]);
    }
}
