//! What Rust's reader and the plain-text reader share: the column a byte
//! sits at, the tree-sitter token walk, a line's [`Floor`] and giving items a
//! unique map path.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cellworks_map::{Floor, Token};
use tree_sitter::Node;

/// Columns a tab counts as.
pub(crate) const TAB: usize = 4;

/// The column `byte` of `line` is at, a tab counting as [`TAB`].
pub(crate) fn column(line: &str, byte: usize) -> usize {
    line.get(..byte)
        .unwrap_or(line)
        .chars()
        .map(|c| if c == '\t' { TAB } else { 1 })
        .sum()
}

/// Every leaf of the tree as a token on the line it starts on.
pub(crate) fn tokens(root: Node<'_>, source: &str, lines: &[String]) -> Vec<Vec<Token>> {
    let mut tokens = vec![Vec::new(); lines.len()];
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.child_count() == 0 && node.end_byte() > node.start_byte() {
            let start = node.start_position();
            if let (Some(line), Some(line_tokens)) =
                (lines.get(start.row), tokens.get_mut(start.row))
            {
                let first = column(line, start.column);
                let text = source.get(node.byte_range()).unwrap_or_default();
                let len = text.lines().next().map_or(0, |t| column(t, t.len()));
                line_tokens.push(Token {
                    start: first,
                    len: len.max(1),
                });
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return tokens;
            }
        }
    }
}

/// The floor line `row` of `lines` draws, its tokens taken from `tokens`;
/// `calls` and `imports` are filled in later by the caller.
pub(crate) fn floor(lines: &[String], tokens: &[Vec<Token>], row: usize) -> Option<Floor> {
    let text = lines.get(row)?;
    let trimmed = text.trim_end();
    let indent = column(text, text.len() - text.trim_start().len());
    Some(Floor {
        number: row + 1,
        text: text.clone(),
        indent: if trimmed.is_empty() { 0 } else { indent },
        width: column(trimmed, trimmed.len()),
        tokens: tokens.get(row).cloned().unwrap_or_default(),
        calls: Vec::new(),
        imports: Vec::new(),
    })
}

/// The map path of each of `items`, named `(name, first_row)`: `file` joined
/// with the name (a `/` in it written as a division slash), and the first
/// row as well, `@12`, when a path repeats.
pub(crate) fn unique_paths(
    file: &Path,
    items: impl IntoIterator<Item = (String, usize)>,
) -> Vec<PathBuf> {
    let mut taken = BTreeMap::<PathBuf, usize>::new();
    items
        .into_iter()
        .map(|(name, first_row)| {
            let name = name.replace('/', "\u{2215}");
            let path = file.join(name);
            let seen = taken.entry(path.clone()).or_default();
            *seen += 1;
            if *seen == 1 {
                path
            } else {
                PathBuf::from(format!("{}@{}", path.display(), first_row + 1))
            }
        })
        .collect()
}
