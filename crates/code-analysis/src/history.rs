//! Joining `git log`'s commits to the pull request and the stories a
//! commit's message names, and building `LineHistory` from `git log` and
//! `git blame`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cellworks_map::{Commit, LineHistory};

use crate::git::LoggedCommit;

/// `LineHistory` built from `logged` (oldest first, as `GitListing::log`
/// answers) and `blamed` (per file, relative to `root`, as
/// `GitListing::blame` answers): each blamed commit id resolved to its
/// index in `logged`'s order; each file's key canonicalised under `root`,
/// the same path form `ProjectMap::of_code` keys a floor's file by (see
/// `co_change::co_changes`, which does the same `root.join(file)`).
pub(crate) fn join(
    root: &Path,
    logged: Vec<LoggedCommit>,
    blamed: BTreeMap<PathBuf, Vec<Option<String>>>,
) -> LineHistory {
    let mut commits = Vec::with_capacity(logged.len());
    let mut index_of: BTreeMap<String, usize> = BTreeMap::new();
    for entry in logged {
        let subject = entry.message.lines().next().unwrap_or("");
        index_of.insert(entry.sha.clone(), commits.len());
        commits.push(Commit {
            sha: entry.sha,
            time: entry.time,
            summary: subject.to_owned(),
            pull_request: pull_request(subject),
            stories: stories(&entry.message),
        });
    }
    let lines = blamed
        .into_iter()
        .map(|(file, shas)| {
            let indices = shas
                .into_iter()
                .map(|sha| sha.and_then(|sha| index_of.get(&sha).copied()))
                .collect();
            (root.join(file), indices)
        })
        .collect();
    LineHistory { commits, lines }
}

/// The last `(#N)` of `subject`, as a squash merge onto GitHub writes it;
/// `None` without one or for a malformed one (for example `(#12a)`).
pub(crate) fn pull_request(subject: &str) -> Option<u32> {
    let mut result = None;
    let mut i = 0;
    while let Some(found) = subject[i..].find("(#") {
        let start = i + found + 2;
        let digits_end = subject[start..]
            .find(|c: char| !c.is_ascii_digit())
            .map(|n| start + n)
            .unwrap_or(subject.len());
        if digits_end > start
            && subject[digits_end..].starts_with(')')
            && let Ok(number) = subject[start..digits_end].parse()
        {
            result = Some(number);
        }
        i = start;
    }
    result
}

/// Every `STORY-NNN` `message` names, any case, in order of first mention,
/// de-duplicated, upper case with at least three digits (`story-3` and
/// `story-003` are both `STORY-003`).
pub(crate) fn stories(message: &str) -> Vec<String> {
    let lower = message.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut offset = 0;
    while let Some(index) = lower[offset..].find("story-") {
        let start = offset + index + "story-".len();
        let digits: String = message[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if !digits.is_empty() {
            let id = format!("STORY-{digits:0>3}");
            if seen.insert(id.clone()) {
                found.push(id);
            }
        }
        offset = start;
    }
    // Jira-style keys (`SHOP-42`) count when a `Refs:` trailer names them.
    for line in message.lines() {
        let Some(refs) = line.trim().strip_prefix("Refs:") else {
            continue;
        };
        for word in refs.split(|c: char| c == ',' || c.is_whitespace()) {
            let Some((project, number)) = word.split_once('-') else {
                continue;
            };
            let key = project.starts_with(|c: char| c.is_ascii_uppercase())
                && project
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                && !number.is_empty()
                && number.chars().all(|c| c.is_ascii_digit());
            if key && !project.eq_ignore_ascii_case("story") && seen.insert(word.to_owned()) {
                found.push(word.to_owned());
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::git::LoggedCommit;

    #[test]
    fn the_pull_request_is_the_last_number_in_the_subject() {
        assert_eq!(pull_request("feature: thing (#5) (#19)"), Some(19));
        assert_eq!(
            pull_request("chore: adopt Factory v3.0.0 (#104)"),
            Some(104)
        );
        assert_eq!(pull_request("chore: before the Factory flow"), None);
        assert_eq!(pull_request("malformed (#12a)"), None);
        assert_eq!(pull_request("no parens here #5"), None);
    }

    #[test]
    fn every_story_the_message_names_is_kept() {
        // The real message of this repository's EPIC-010 merge commit
        // (831d4f1), which names seven stories.
        let message = "EPIC-010: Lenses (#80)\n\n\
Closes #37, closes #38, closes #39, closes #76, closes #77, closes #78,\n\
closes #79.\n\n\
## Changes\n\
- **STORY-015, relationship lenses:**\n\
- **STORY-052, a lens shows its lanes:**\n\
- **STORY-053, follow a building's lanes:**\n\
- **STORY-054, imports as a staircase:**\n\
- **STORY-016, the cells lens:**\n\
- **STORY-017, the custom lens (\"mine\" renamed):**\n\
- **STORY-055, each lens explains itself:**\n";

        assert_eq!(
            stories(message),
            vec![
                "STORY-015".to_owned(),
                "STORY-052".to_owned(),
                "STORY-053".to_owned(),
                "STORY-054".to_owned(),
                "STORY-016".to_owned(),
                "STORY-017".to_owned(),
                "STORY-055".to_owned(),
            ]
        );
        assert_eq!(
            stories("Refs: story-3"),
            vec!["STORY-003".to_owned()],
            "zero-padded to three digits"
        );
        assert_eq!(
            stories("Refs: STORY-003"),
            vec!["STORY-003".to_owned()],
            "already three digits"
        );
        assert_eq!(stories("no story here"), Vec::<String>::new());
        assert_eq!(
            stories("Merge SHOP-42\n\nRefs: SHOP-42, STORY-7"),
            vec!["STORY-007".to_owned(), "SHOP-42".to_owned()]
        );
        assert_eq!(stories("fix UTF-8 handling"), Vec::<String>::new());
    }

    #[test]
    fn uncommitted_lines_and_unknown_commits_have_none() {
        let logged = vec![LoggedCommit {
            sha: "c1".to_owned(),
            time: 1000,
            message: "feat: a (#1)\n\nRefs: STORY-2".to_owned(),
        }];
        let mut blamed = BTreeMap::new();
        blamed.insert(
            PathBuf::from("a.rs"),
            vec![Some("c1".to_owned()), None, Some("unknown".to_owned())],
        );

        let history = join(Path::new("/p"), logged, blamed);

        let line = |n: usize| history.of_line(Path::new("/p/a.rs"), n);
        let commit = line(1).expect("line 1 was written by c1");
        assert_eq!(commit.sha, "c1");
        assert_eq!(commit.pull_request, Some(1));
        assert_eq!(commit.stories, vec!["STORY-002".to_owned()]);
        assert_eq!(line(2), None, "an uncommitted line");
        assert_eq!(line(3), None, "blame named a commit the log never listed");
    }
}
