//! The files no parser reads, read as text, in the parts their kind has
//! ([`kind_of`]): a document a section per heading, a configuration an item
//! per top-level key with one more for each level nested under it, a feature
//! file its header and an item per scenario, Terraform an item per top-level
//! block; any other file one tower of its lines. The lines of secrets are
//! never held: only how wide each is. An image has no lines to read. So no
//! folder of the city looks empty because its code is in a language
//! Cellworks does not parse.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use cellworks_map::{Declaration, Floor, Item, ItemKind, NodeKind, Token};

use crate::kinds::kind_of;
use crate::rust::column;

/// Files larger than this are left bare: data or generated text, not
/// something to read line by line.
const LARGEST: u64 = 512 * 1024;

/// The most floors a text tower has; longer files are cut off.
const TALLEST: usize = 400;

/// The most levels nested under a configuration key that stand as items of
/// their own; deeper lines join the deepest.
const LEVELS: usize = 4;

/// One line of a file: its index from 0, and its text.
type Row<'a> = (usize, &'a str);

/// The items of each of `files`, relative to `root`, that holds text, keyed
/// by the file: the parts its kind has. A file that cannot be read, is too
/// large, is empty, is an image or is not text (not UTF-8, or holding a NUL)
/// has none and stays a bare block.
pub(crate) fn read<'a>(
    root: &Path,
    files: impl IntoIterator<Item = &'a PathBuf>,
) -> BTreeMap<PathBuf, Vec<Item>> {
    files
        .into_iter()
        .filter_map(|file| Some((file.clone(), items(root, file)?)))
        .collect()
}

/// One part of a file: its name, its kind, its lines, and the index of the
/// part it is part of.
struct Part<'a> {
    name: String,
    kind: ItemKind,
    rows: Vec<Row<'a>>,
    of: Option<usize>,
}

impl<'a> Part<'a> {
    fn new(name: String, rows: Vec<Row<'a>>) -> Part<'a> {
        Part {
            name,
            kind: ItemKind::Text,
            rows,
            of: None,
        }
    }
}

fn items(root: &Path, file: &Path) -> Option<Vec<Item>> {
    let kind = kind_of(file);
    if kind == NodeKind::Image {
        return None;
    }
    let path = root.join(file);
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > LARGEST {
        return None;
    }
    let text = fs::read_to_string(&path).ok()?;
    if text.contains('\0') {
        return None;
    }
    let lines: Vec<Row> = text.lines().take(TALLEST).enumerate().collect();
    if lines.is_empty() {
        return None;
    }
    let name = file.file_name()?.to_string_lossy().into_owned();
    let mut parts = match kind {
        NodeKind::Document => sections(&lines),
        NodeKind::Configuration => keys(&lines, file.extension().is_some_and(|e| e == "toml")),
        NodeKind::Specification => scenarios(&lines, &name),
        NodeKind::Infrastructure => blocks(&lines),
        _ => Vec::new(),
    };
    if parts.is_empty() {
        parts.push(Part::new(name, lines));
    }
    let sealed = matches!(kind, NodeKind::Secrets | NodeKind::SecretsExample);
    let paths = part_paths(&path, &parts);
    Some(
        parts
            .iter()
            .zip(&paths)
            .map(|(part, at)| Item {
                name: part.name.clone(),
                path: at.clone(),
                kind: part.kind,
                tests: false,
                floors: part
                    .rows
                    .iter()
                    .map(|(row, line)| floor(*row, line, sealed))
                    .collect(),
                area: egui::Rect::NOTHING,
                base: 0.0,
                declaration: Declaration {
                    part_of: part.of.map(|of| paths[of].clone()),
                    ..Declaration::default()
                },
            })
            .collect(),
    )
}

/// The path of each of `parts` of the file at `file`: the file's path joined
/// with its name, a slash in it written as a division slash; a name met
/// before gets the line it starts on, `@12`.
fn part_paths(file: &Path, parts: &[Part]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    parts
        .iter()
        .map(|part| {
            let name = part.name.replace('/', "\u{2215}");
            let name = if seen.insert(name.clone()) {
                name
            } else {
                let line = part.rows.first().map_or(0, |(row, _)| row + 1);
                format!("{name}@{line}")
            };
            file.join(name)
        })
        .collect()
}

/// `lines` cut before every line `start` names, each part named so: the
/// lines before the first such line, and the parts.
fn cut<'a>(
    lines: &[Row<'a>],
    mut start: impl FnMut(usize, &str) -> Option<String>,
) -> (Vec<Row<'a>>, Vec<Part<'a>>) {
    let mut leading = Vec::new();
    let mut parts: Vec<Part> = Vec::new();
    for &(row, line) in lines {
        if let Some(name) = start(row, line) {
            parts.push(Part::new(name, vec![(row, line)]));
        } else if let Some(part) = parts.last_mut() {
            part.rows.push((row, line));
        } else {
            leading.push((row, line));
        }
    }
    (leading, parts)
}

/// The lines before the first part joined to it.
fn joined<'a>((mut leading, mut parts): (Vec<Row<'a>>, Vec<Part<'a>>)) -> Vec<Part<'a>> {
    if let Some(first) = parts.first_mut() {
        leading.append(&mut first.rows);
        first.rows = leading;
    }
    parts
}

/// A document's sections, one at each Markdown heading outside a fenced
/// code block.
fn sections<'a>(lines: &[Row<'a>]) -> Vec<Part<'a>> {
    let mut fenced = false;
    let heading = |_: usize, line: &str| {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let rest = line.trim_start_matches('#');
        let hashes = line.len() - rest.len();
        let is_heading =
            !fenced && (1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with(' '));
        is_heading.then(|| rest.trim().to_owned())
    };
    let mut parts = joined(cut(lines, heading));
    for part in &mut parts {
        part.kind = ItemKind::Section;
    }
    parts
}

/// A feature file's header, its feature and background, and an item for
/// each scenario or rule, part of the header.
fn scenarios<'a>(lines: &[Row<'a>], file: &str) -> Vec<Part<'a>> {
    let start = |_: usize, line: &str| {
        let line = line.trim();
        let starts = line.starts_with("Scenario")
            || line.starts_with("Rule:")
            || line.starts_with("Example:");
        starts.then(|| line.to_owned())
    };
    let (header, mut parts) = cut(lines, start);
    if header.is_empty() {
        return parts;
    }
    let name = header
        .iter()
        .map(|(_, line)| line.trim())
        .find(|line| line.starts_with("Feature:"))
        .unwrap_or(file)
        .to_owned();
    for part in &mut parts {
        part.of = Some(0);
    }
    parts.insert(0, Part::new(name, header));
    parts
}

/// Terraform's top-level blocks: each starts on an unindented line that
/// opens a brace.
fn blocks<'a>(lines: &[Row<'a>]) -> Vec<Part<'a>> {
    let start = |_: usize, line: &str| {
        let opens = !line.starts_with(char::is_whitespace) && line.trim_end().ends_with('{');
        opens.then(|| line.trim_end().trim_end_matches('{').trim().to_owned())
    };
    joined(cut(lines, start))
}

/// A configuration's top-level keys, each an item with its own lines, and
/// for each level nested under it, at most [`LEVELS`], an item of that
/// level's lines, part of the key. In TOML a table's header is the key and
/// its lines are nested under it.
fn keys<'a>(lines: &[Row<'a>], toml: bool) -> Vec<Part<'a>> {
    let holds = |line: &str| {
        let line = line.trim();
        !line.is_empty()
            && !line.starts_with('#')
            && !line.starts_with("//")
            && line != "---"
            && !line
                .chars()
                .all(|c| matches!(c, '{' | '}' | '[' | ']' | ','))
    };
    // How deep each line is nested: its indentation, and in TOML one more
    // inside a table.
    let mut in_table = false;
    let depths: BTreeMap<usize, usize> = lines
        .iter()
        .filter(|(_, line)| holds(line))
        .map(|&(row, line)| {
            let indent = column(line, line.len() - line.trim_start().len());
            if toml && line.trim_start().starts_with('[') {
                in_table = true;
                return (row, indent);
            }
            (row, indent + usize::from(toml && in_table))
        })
        .collect();
    let Some(&top) = depths.values().min() else {
        return Vec::new();
    };
    let key = |row: usize, line: &str| {
        (depths.get(&row) == Some(&top)).then(|| {
            let line = line.trim().trim_start_matches(['"', '\'', '[', '-', ' ']);
            let name = line
                .split([':', '=', '"', '\'', ']'])
                .next()
                .unwrap_or(line);
            match name.trim() {
                "" => "key".to_owned(),
                name => name.to_owned(),
            }
        })
    };
    let mut parts = Vec::new();
    for key in joined(cut(lines, key)) {
        let mut nested: Vec<usize> = key
            .rows
            .iter()
            .filter_map(|(row, _)| depths.get(row).copied().filter(|depth| *depth > top))
            .collect();
        nested.sort_unstable();
        nested.dedup();
        let level = |row: usize| {
            let depth = depths.get(&row)?;
            let rank = nested.iter().position(|d| d == depth)?;
            Some(rank.min(LEVELS - 1) + 1)
        };
        let levels = nested.len().min(LEVELS);
        let at = parts.len();
        let mut own = Part::new(key.name.clone(), Vec::new());
        let mut deeper: Vec<Part> = (1..=levels)
            .map(|n| Part {
                of: Some(at),
                ..Part::new(format!("{}.{n}", key.name), Vec::new())
            })
            .collect();
        for (row, line) in key.rows {
            match level(row) {
                Some(n) => deeper[n - 1].rows.push((row, line)),
                None => own.rows.push((row, line)),
            }
        }
        parts.push(own);
        parts.extend(deeper);
    }
    parts
}

/// Line `row` of a text file, its words as tokens; `sealed`, only how wide
/// it is, its text and tokens never held.
fn floor(row: usize, line: &str, sealed: bool) -> Floor {
    let trimmed = line.trim_end();
    let width = column(trimmed, trimmed.len());
    if sealed {
        return Floor {
            number: row + 1,
            text: String::new(),
            indent: 0,
            width,
            tokens: Vec::new(),
            calls: Vec::new(),
            imports: Vec::new(),
        };
    }
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
        width,
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

    /// The items `text` written as `name` is read into: each one's name,
    /// the lines it holds and the name of the item it is part of.
    fn parts(name: &str, text: &str) -> Vec<(String, Vec<usize>, Option<String>)> {
        let (root, files) = written(&[(name, text.as_bytes())]);
        let read = read(root.path(), &files);
        let items = &read[Path::new(name)];
        let file = root.path().join(name);
        items
            .iter()
            .map(|item| {
                assert_eq!(item.path.parent(), Some(file.as_path()));
                let of = item.declaration.part_of.as_ref().map(|of| {
                    items
                        .iter()
                        .find(|other| &other.path == of)
                        .map(|other| other.name.clone())
                        .expect("part of an item of the same file")
                });
                let lines = item.floors.iter().map(|floor| floor.number).collect();
                (item.name.clone(), lines, of)
            })
            .collect()
    }

    #[test]
    fn a_document_is_a_section_per_heading() {
        let text = "[badge]\n# Shop\nintro\n```sh\n# not a heading\n```\n## Usage\nrun\n#hashtag\n## Licence\n";
        let (root, files) = written(&[("README.md", text.as_bytes())]);

        let read = read(root.path(), &files);

        let items = &read[Path::new("README.md")];
        assert!(items.iter().all(|item| item.kind == ItemKind::Section));
        assert_eq!(
            parts("README.md", text),
            [
                ("Shop".to_owned(), vec![1, 2, 3, 4, 5, 6], None),
                ("Usage".to_owned(), vec![7, 8, 9], None),
                ("Licence".to_owned(), vec![10], None),
            ]
        );
    }

    #[test]
    fn a_configuration_is_an_item_per_key_and_one_per_level_nested_under_it() {
        let yaml = "# settings\nserver:\n  port: 80\ndatabase:\n  host: db\n  pool:\n    size: 5\n";

        assert_eq!(
            parts("config.yaml", yaml),
            [
                ("server".to_owned(), vec![1, 2], None),
                ("server.1".to_owned(), vec![3], Some("server".to_owned())),
                ("database".to_owned(), vec![4], None),
                (
                    "database.1".to_owned(),
                    vec![5, 6],
                    Some("database".to_owned())
                ),
                (
                    "database.2".to_owned(),
                    vec![7],
                    Some("database".to_owned())
                ),
            ]
        );
        let toml = "[package]\nname = \"shop\"\n\n[dependencies]\negui = \"0.36\"\n";
        let names: Vec<String> = parts("Cargo.toml", toml).into_iter().map(|p| p.0).collect();
        assert_eq!(
            names,
            ["package", "package.1", "dependencies", "dependencies.1"]
        );
        let json = "{\n  \"a\": {\n    \"b\": 1\n  },\n  \"c\": 2\n}\n";
        let names: Vec<String> = parts("x.json", json).into_iter().map(|p| p.0).collect();
        assert_eq!(names, ["a", "a.1", "c"]);
    }

    #[test]
    fn a_configuration_stands_at_most_four_levels_deep() {
        let yaml = "a:\n b:\n  c:\n   d:\n    e:\n     f: 1\n";

        let parts = parts("deep.yml", yaml);

        assert_eq!(parts.len(), 1 + LEVELS);
        assert_eq!(parts[LEVELS].1, [5, 6], "deeper lines join the deepest");
    }

    #[test]
    fn a_feature_file_is_its_header_and_an_item_per_scenario() {
        let text = "Feature: Checkout\n  Background:\n    Given a cart\n  Scenario: Pay\n    Then paid\n  Scenario Outline: Ship\n    Examples:\n      | a |\n";

        assert_eq!(
            parts("checkout.feature", text),
            [
                ("Feature: Checkout".to_owned(), vec![1, 2, 3], None),
                (
                    "Scenario: Pay".to_owned(),
                    vec![4, 5],
                    Some("Feature: Checkout".to_owned())
                ),
                (
                    "Scenario Outline: Ship".to_owned(),
                    vec![6, 7, 8],
                    Some("Feature: Checkout".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn terraform_is_an_item_per_top_level_block() {
        let text = "# main\nterraform {\n  x = 1\n}\n\nresource \"aws_s3_bucket\" \"logs\" {\n  bucket = \"l\"\n}\n";

        let names: Vec<(String, Vec<usize>)> = parts("main.tf", text)
            .into_iter()
            .map(|(name, lines, _)| (name, lines))
            .collect();

        assert_eq!(
            names,
            [
                ("terraform".to_owned(), vec![1, 2, 3, 4, 5]),
                (
                    "resource \"aws_s3_bucket\" \"logs\"".to_owned(),
                    vec![6, 7, 8]
                ),
            ]
        );
    }

    #[test]
    fn secrets_keep_how_wide_each_line_is_and_nothing_else() {
        let (root, files) = written(&[
            (".env", b"API_KEY=hunter2\nDB_PASSWORD=swordfish\n"),
            (".env.example", b"API_KEY=\n"),
        ]);

        let read = read(root.path(), &files);

        for file in [".env", ".env.example"] {
            let items = &read[Path::new(file)];
            assert_eq!(items.len(), 1, "{file} is one vault");
            for floor in &items[0].floors {
                assert!(floor.text.is_empty() && floor.tokens.is_empty(), "{file}");
                assert!(floor.width > 0);
            }
        }
        assert_eq!(read[Path::new(".env")][0].floors[0].width, 15);
    }

    #[test]
    fn a_part_s_path_has_no_slash_and_a_repeated_name_its_line() {
        let text = "# a/b\n# Notes\n# Notes\n";
        let (root, files) = written(&[("doc.md", text.as_bytes())]);

        let read = read(root.path(), &files);

        let file = root.path().join("doc.md");
        let paths: Vec<PathBuf> = read[Path::new("doc.md")]
            .iter()
            .map(|item| item.path.clone())
            .collect();
        assert_eq!(
            paths,
            [
                file.join("a\u{2215}b"),
                file.join("Notes"),
                file.join("Notes@3")
            ]
        );
    }

    #[test]
    fn an_image_has_no_lines_to_read() {
        let (root, files) = written(&[("icon.svg", b"<svg></svg>\n")]);

        assert!(read(root.path(), &files).is_empty());
    }
}
