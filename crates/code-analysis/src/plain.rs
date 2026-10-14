//! The files no parser reads, read as text: each stands as one tower of its
//! lines, so no folder of the city looks empty because its code is in a
//! language Cellworks does not parse.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use cellworks_map::{Floor, Item, ItemKind, Token};

use crate::rust::column;

/// Files larger than this are left bare: data or generated text, not
/// something to read line by line.
const LARGEST: u64 = 512 * 1024;

/// The most floors a text tower has; longer files are cut off.
const TALLEST: usize = 400;

/// One tower for each of `files`, relative to `root`, that holds text, keyed
/// by the file. A file that cannot be read, is too large, is empty or is not
/// text (not UTF-8, or holding a NUL) has none and stays a bare block.
pub(crate) fn read<'a>(
    root: &Path,
    files: impl IntoIterator<Item = &'a PathBuf>,
) -> BTreeMap<PathBuf, Vec<Item>> {
    files
        .into_iter()
        .filter_map(|file| {
            let item = tower(root, file)?;
            Some((file.clone(), vec![item]))
        })
        .collect()
}

fn tower(root: &Path, file: &Path) -> Option<Item> {
    let path = root.join(file);
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > LARGEST {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    if text.contains('\0') {
        return None;
    }
    let floors: Vec<Floor> = text
        .lines()
        .take(TALLEST)
        .enumerate()
        .map(|(row, line)| floor(row, line))
        .collect();
    if floors.is_empty() {
        return None;
    }
    let name = file.file_name()?.to_string_lossy().into_owned();
    Some(Item {
        path: path.join(&name),
        name,
        kind: ItemKind::Text,
        tests: false,
        floors,
        area: egui::Rect::NOTHING,
        base: 0.0,
    })
}

/// Line `row` of a text file, its words as tokens.
fn floor(row: usize, line: &str) -> Floor {
    let trimmed = line.trim_end();
    let indent = column(line, line.len() - line.trim_start().len());
    let tokens = trimmed
        .split_whitespace()
        .map(|word| {
            // The word's byte offset within the line, as a column.
            let byte = word.as_ptr() as usize - line.as_ptr() as usize;
            Token {
                start: column(line, byte),
                len: word.chars().count(),
            }
        })
        .collect();
    Floor {
        number: row + 1,
        text: line.to_owned(),
        indent: if trimmed.is_empty() { 0 } else { indent },
        width: column(trimmed, trimmed.len()),
        tokens,
        calls: Vec::new(),
        imports: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(files: &[(&str, &[u8])]) -> (tempfile::TempDir, Vec<PathBuf>) {
        let root = tempfile::tempdir().expect("a folder");
        let mut paths = Vec::new();
        for (name, bytes) in files {
            fs::write(root.path().join(name), bytes).expect("written");
            paths.push(PathBuf::from(name));
        }
        (root, paths)
    }

    #[test]
    fn a_text_file_is_one_tower_with_a_floor_per_line() {
        let (root, files) = written(&[("app.ts", b"const a = 1;\n\tlet b = a;\n")]);

        let read = read(root.path(), &files);

        let items = &read[Path::new("app.ts")];
        assert_eq!(items.len(), 1);
        let tower = &items[0];
        assert_eq!(tower.name, "app.ts");
        assert_eq!(tower.kind, ItemKind::Text);
        assert_eq!(tower.path, root.path().join("app.ts").join("app.ts"));
        assert_eq!(tower.floors.len(), 2);
        let second = &tower.floors[1];
        assert_eq!(
            (second.number, second.indent),
            (2, 4),
            "a tab counts as four"
        );
        let starts: Vec<(usize, usize)> = second.tokens.iter().map(|t| (t.start, t.len)).collect();
        assert_eq!(starts, [(4, 3), (8, 1), (10, 1), (12, 2)]);
    }

    #[test]
    fn a_long_file_is_cut_off_at_the_tallest_tower() {
        let text = "line\n".repeat(TALLEST + 100);
        let (root, files) = written(&[("long.txt", text.as_bytes())]);

        let read = read(root.path(), &files);

        assert_eq!(read[Path::new("long.txt")][0].floors.len(), TALLEST);
    }

    #[test]
    fn a_file_that_is_not_text_too_large_or_empty_has_no_tower() {
        let large = vec![b'a'; LARGEST as usize + 1];
        let (root, files) = written(&[
            ("logo.png", &[0x89, b'P', b'N', b'G', 0xff, 0x00]),
            ("nul.txt", b"a\0b"),
            ("large.txt", &large),
            ("empty.txt", b""),
        ]);

        let read = read(root.path(), &files);

        assert!(read.is_empty(), "{:?}", read.keys().collect::<Vec<_>>());
    }
}
