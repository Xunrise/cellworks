//! A unified diff, read with no size limit: which lines each file gains and
//! loses.
//!
//! `git diff --no-color --no-ext-diff -U0 <base>...<head>` is parsed line by
//! line, one file at a time; a deleted file is named only on its `--- ` line,
//! an added file only on its `+++ ` line, and a binary file by neither, only
//! by "Binary files a/<path> and b/<path> differ". Paths come from `strip_prefix("a/")`/
//! `("b/")`, applied once, so a path like `a/a/x` keeps its own leading `a/`.
//!
//! A `--- `/`+++ ` line is a file header only between hunks: inside a hunk
//! body the same text is content, a removed `-- …` or an added `++ …`. The
//! line counts in each `@@` header say how long that body is, so the two are
//! told apart by position, not by prefix.

use std::collections::BTreeMap;

/// Every file a diff touches, keyed by its path (the new path, or the old
/// path for a deletion).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diff {
    pub files: BTreeMap<String, FileChange>,
}

/// The lines one file gained and lost.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileChange {
    /// New-side line numbers this file gained.
    pub added: Vec<usize>,
    /// Lines this file lost, oldest first.
    pub removed: Vec<Removal>,
}

/// One removed line: its number on the old side, and the new-side line
/// number it sat just before (0 when it was the file's first line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub old_line: usize,
    pub before_new_line: usize,
}

/// The totals [`size`] reads off a [`Diff`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    pub added: usize,
    pub removed: usize,
    pub files: usize,
}

/// Parses `git diff --no-color --no-ext-diff -U0 <base>...<head>` output into
/// one [`FileChange`] per file it touches.
///
/// A deletion is named only on its `--- ` line (`+++ /dev/null`), an addition
/// only on its `+++ ` line (`--- /dev/null`); either way the file still gets
/// an entry, even with no hunks, so a binary file (named instead by
/// "Binary files a/<path> and b/<path> differ") counts toward [`Size::files`]
/// too.
pub fn parse_diff(text: &str) -> Diff {
    let mut diff = Diff::default();
    // The file currently named by the most recent "---"/"+++" line, and the
    // running old/new line counters the current hunk header set.
    let mut current: Option<String> = None;
    let mut old_line = 0usize;
    let mut new_line = 0usize;
    // How many old-side and new-side lines the current hunk still owes, from
    // the counts in its header. A line is a header only outside a hunk body:
    // a removed line whose own content starts with "-- " arrives as "--- …"
    // and a prefix test alone cannot tell it from the "--- " file header.
    let mut old_owed = 0usize;
    let mut new_owed = 0usize;

    for line in text.lines() {
        if (old_owed > 0 || new_owed > 0) && !is_hunk_body_line(line) {
            // The header's counts and the text disagree (a truncated hunk):
            // trust the text and read what follows as file headers again.
            old_owed = 0;
            new_owed = 0;
        }
        let in_hunk = old_owed > 0 || new_owed > 0;

        if in_hunk && line.starts_with('\\') {
            // "\ No newline at end of file" annotates the line before it and
            // is an old or new line of neither side.
        } else if !in_hunk && let Some(path) = line.strip_prefix("--- ") {
            // A deletion is named here, never on the "+++" line that follows.
            if path != "/dev/null"
                && let Some(path) = strip_side_prefix(path)
            {
                diff.files.entry(path.clone()).or_default();
                current = Some(path);
            }
        } else if !in_hunk && let Some(path) = line.strip_prefix("+++ ") {
            // An addition, or the new side of a modification, is named here.
            if path != "/dev/null"
                && let Some(path) = strip_side_prefix(path)
            {
                diff.files.entry(path.clone()).or_default();
                current = Some(path);
            }
        } else if !in_hunk && let Some(rest) = line.strip_prefix("Binary files ") {
            if let Some(path) = binary_new_path(rest) {
                diff.files.entry(path).or_default();
            }
        } else if let Some(header) = line.strip_prefix("@@ ") {
            // Only a header can carry a raw "@@ ": a removed line holding one
            // arrives as "-@@ …".
            if let Some(hunk) = parse_hunk_header(header) {
                old_line = hunk.old_start;
                new_line = hunk.new_start;
                old_owed = hunk.old_count;
                new_owed = hunk.new_count;
            }
        } else if line.starts_with('-') {
            // Inside a hunk body every '-' line is a removal, including one
            // whose own content starts with '-': a removed Markdown bullet
            // "- Works offline" arrives as "-- Works offline", a removed Lua
            // comment "-- x" as "--- x". Skipping either would undercount the
            // removals and leave old_line behind for the rest of the hunk.
            if let Some(path) = &current
                && let Some(change) = diff.files.get_mut(path)
            {
                change.removed.push(Removal {
                    old_line,
                    before_new_line: new_line,
                });
            }
            old_line += 1;
            old_owed = old_owed.saturating_sub(1);
        } else if line.starts_with('+') {
            // Likewise for '+': content beginning with '+' or "++ " is an
            // addition like any other.
            if let Some(path) = &current
                && let Some(change) = diff.files.get_mut(path)
            {
                change.added.push(new_line);
            }
            new_line += 1;
            new_owed = new_owed.saturating_sub(1);
        } else if line.starts_with(' ') {
            // -U0 ought to leave no context lines, but a line is still
            // accounted for if one shows up.
            old_line += 1;
            new_line += 1;
            old_owed = old_owed.saturating_sub(1);
            new_owed = new_owed.saturating_sub(1);
        }
    }

    diff
}

/// Whether a line can belong to a hunk body: a removal, an addition, a
/// context line, or the "\ No newline at end of file" marker. Anything else
/// inside a hunk means the header's counts were wrong.
fn is_hunk_body_line(line: &str) -> bool {
    line.starts_with('-')
        || line.starts_with('+')
        || line.starts_with(' ')
        || line.starts_with('\\')
}

/// `added.len()` and `removed.len()` summed across every file, plus the
/// number of files.
pub fn size(diff: &Diff) -> Size {
    diff.files.values().fold(
        Size {
            added: 0,
            removed: 0,
            files: diff.files.len(),
        },
        |mut totals, change| {
            totals.added += change.added.len();
            totals.removed += change.removed.len();
            totals
        },
    )
}

/// Strips a diff path's leading `a/` or `b/` exactly once, with `strip_prefix`
/// rather than `trim_start_matches`, which would wrongly eat both leading
/// segments of a path like `a/a/x`. A path with neither prefix (unexpected
/// from git) is kept as is.
fn strip_side_prefix(path: &str) -> Option<String> {
    Some(
        path.strip_prefix("a/")
            .or_else(|| path.strip_prefix("b/"))
            .unwrap_or(path)
            .to_string(),
    )
}

/// The new-side path out of `a/<old> and b/<new> differ`, prefix stripped
/// once.
fn binary_new_path(rest: &str) -> Option<String> {
    let rest = rest.strip_suffix(" differ")?;
    let (_, new) = rest.split_once(" and ")?;
    strip_side_prefix(new)
}

/// What a `@@ -<old>[,<count>] +<new>[,<count>] @@` header says: where each
/// side starts and how many lines of it the hunk body holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
}

/// The starting line and line count of each side of a
/// `@@ -<old>[,<count>] +<new>[,<count>] @@` header. A missing count means 1.
fn parse_hunk_header(header: &str) -> Option<Hunk> {
    let header = header.strip_suffix(" @@").unwrap_or(header);
    let mut parts = header.split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let (old_start, old_count) = parse_hunk_side(old)?;
    let (new_start, new_count) = parse_hunk_side(new)?;
    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

/// One side of a hunk header, `<start>` or `<start>,<count>`.
fn parse_hunk_side(side: &str) -> Option<(usize, usize)> {
    let (start, count) = match side.split_once(',') {
        Some((start, count)) => (start, count.parse().ok()?),
        None => (side, 1),
    };
    Some((start.parse().ok()?, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modified_file_records_its_added_and_removed_lines() {
        let text = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,2 +10,1 @@
-old ten
-old eleven
+new ten
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("src/lib.rs"),
            Some(&FileChange {
                added: vec![10],
                removed: vec![
                    Removal {
                        old_line: 10,
                        before_new_line: 10
                    },
                    Removal {
                        old_line: 11,
                        before_new_line: 10
                    },
                ],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn an_added_file_is_named_only_on_the_plus_side() {
        let text = "\
diff --git a/src/new_file.rs b/src/new_file.rs
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/src/new_file.rs
@@ -0,0 +1,2 @@
+first line
+second line
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("src/new_file.rs"),
            Some(&FileChange {
                added: vec![1, 2],
                removed: vec![],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn a_deleted_file_is_named_only_on_the_minus_side() {
        let text = "\
diff --git a/src/old_file.rs b/src/old_file.rs
deleted file mode 100644
index 4444444..0000000
--- a/src/old_file.rs
+++ /dev/null
@@ -1,2 +0,0 @@
-first line
-second line
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("src/old_file.rs"),
            Some(&FileChange {
                added: vec![],
                removed: vec![
                    Removal {
                        old_line: 1,
                        before_new_line: 0
                    },
                    Removal {
                        old_line: 2,
                        before_new_line: 0
                    },
                ],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn a_binary_file_still_counts_toward_the_files_touched_with_no_lines() {
        let text = "\
diff --git a/image.png b/image.png
index 5555555..6666666 100644
Binary files a/image.png and b/image.png differ
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("image.png"),
            Some(&FileChange {
                added: vec![],
                removed: vec![],
            }),
            "a binary file is still entered so it counts toward Size::files: {diff:?}"
        );
    }

    #[test]
    fn a_removal_at_the_very_start_of_a_file_is_recorded() {
        let text = "\
diff --git a/notes.txt b/notes.txt
--- a/notes.txt
+++ b/notes.txt
@@ -1 +0,0 @@
-first line
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("notes.txt"),
            Some(&FileChange {
                added: vec![],
                removed: vec![Removal {
                    old_line: 1,
                    before_new_line: 0
                }],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn a_removal_at_the_very_end_of_a_file_is_recorded() {
        let text = "\
diff --git a/notes.txt b/notes.txt
--- a/notes.txt
+++ b/notes.txt
@@ -3 +2,0 @@
-last line
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("notes.txt"),
            Some(&FileChange {
                added: vec![],
                removed: vec![Removal {
                    old_line: 3,
                    before_new_line: 2
                }],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn a_path_with_a_literal_a_segment_keeps_it_after_stripping_the_prefix_once() {
        let text = "\
diff --git a/a/x b/a/x
--- a/a/x
+++ b/a/x
@@ -1 +1 @@
-old
+new
";

        let diff = parse_diff(text);

        assert!(
            diff.files.contains_key("a/x"),
            "stripping \"a/\" once from \"a/a/x\" must leave \"a/x\", not \"x\": {diff:?}"
        );
        assert!(
            !diff.files.contains_key("x"),
            "trim_start_matches would wrongly eat both leading segments: {diff:?}"
        );
    }

    #[test]
    fn two_hunks_in_one_file_are_both_recorded() {
        let text = "\
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -5 +5 @@
-old five
+new five
@@ -20 +20,2 @@
-old twenty
+new twenty
+extra twenty one
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("src/lib.rs"),
            Some(&FileChange {
                added: vec![5, 20, 21],
                removed: vec![
                    Removal {
                        old_line: 5,
                        before_new_line: 5
                    },
                    Removal {
                        old_line: 20,
                        before_new_line: 20
                    },
                ],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn content_lines_beginning_with_a_dash_or_a_plus_are_counted_like_any_other() {
        // Markdown bullets and column-0 YAML list items: a removed "- one"
        // arrives as "-- one", an added "+ plus" as "++ plus". Both are
        // content, not the "--- "/"+++ " headers, which are consumed earlier.
        let text = "\
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,3 +1,2 @@
-- one
-plain two
-- three
++ plus
+plain
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("README.md"),
            Some(&FileChange {
                added: vec![1, 2],
                removed: vec![
                    Removal {
                        old_line: 1,
                        before_new_line: 1
                    },
                    Removal {
                        old_line: 2,
                        before_new_line: 1
                    },
                    Removal {
                        old_line: 3,
                        before_new_line: 1
                    },
                ],
            }),
            "all three removals count, and the plain line between two dashed \
             ones keeps its own old_line: {diff:?}"
        );
        assert_eq!(diff.files.len(), 1, "no phantom file: {diff:?}");
    }

    #[test]
    fn content_lines_that_read_like_a_file_header_are_still_counted_as_content() {
        // A removed SQL/Lua/Haskell comment "-- x" arrives as "--- x", an
        // added one starting with "++ " as "+++ ". Inside a hunk body those
        // are content; only between hunks do they name a file.
        let text = "\
diff --git a/db/schema.sql b/db/schema.sql
--- a/db/schema.sql
+++ b/db/schema.sql
@@ -3,2 +3,2 @@
--- create table users
-create table users (id int)
+++ create table people
+create table people (id int)
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("db/schema.sql"),
            Some(&FileChange {
                added: vec![3, 4],
                removed: vec![
                    Removal {
                        old_line: 3,
                        before_new_line: 3
                    },
                    Removal {
                        old_line: 4,
                        before_new_line: 3
                    },
                ],
            }),
            "{diff:?}"
        );
        assert_eq!(
            diff.files.len(),
            1,
            "\"--- create table users\" must not be read as a file header: {diff:?}"
        );
        assert_eq!(
            size(&diff),
            Size {
                added: 2,
                removed: 2,
                files: 1
            },
            "{diff:?}"
        );
    }

    #[test]
    fn a_file_after_a_hunk_whose_lines_read_like_headers_is_still_found() {
        // The hunk's counts are spent on its body, so the next file's own
        // "--- "/"+++ " lines are headers again.
        let text = "\
diff --git a/db/schema.sql b/db/schema.sql
--- a/db/schema.sql
+++ b/db/schema.sql
@@ -3 +3 @@
--- create table users
+++ create table people
diff --git a/notes.txt b/notes.txt
--- a/notes.txt
+++ b/notes.txt
@@ -1 +1 @@
-old
+new
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.keys().collect::<Vec<_>>(),
            vec!["db/schema.sql", "notes.txt"],
            "{diff:?}"
        );
        assert_eq!(
            size(&diff),
            Size {
                added: 2,
                removed: 2,
                files: 2
            },
            "{diff:?}"
        );
    }

    #[test]
    fn a_no_newline_marker_owes_no_line_of_its_own() {
        let text = "\
diff --git a/notes.txt b/notes.txt
--- a/notes.txt
+++ b/notes.txt
@@ -1 +1 @@
-old
\\ No newline at end of file
+new
\\ No newline at end of file
";

        let diff = parse_diff(text);

        assert_eq!(
            diff.files.get("notes.txt"),
            Some(&FileChange {
                added: vec![1],
                removed: vec![Removal {
                    old_line: 1,
                    before_new_line: 1
                }],
            }),
            "{diff:?}"
        );
    }

    #[test]
    fn size_sums_added_removed_and_files_across_the_whole_diff() {
        let text = "\
diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -1 +1,2 @@
-old
+new one
+new two
diff --git a/src/b.rs b/src/b.rs
new file mode 100644
--- /dev/null
+++ b/src/b.rs
@@ -0,0 +1,3 @@
+one
+two
+three
";

        let diff = parse_diff(text);
        let totals = size(&diff);

        assert_eq!(
            totals,
            Size {
                added: 5,
                removed: 1,
                files: 2
            },
            "2 added + 3 added = 5, 1 removed + 0 removed = 1, over 2 files"
        );
    }
}
