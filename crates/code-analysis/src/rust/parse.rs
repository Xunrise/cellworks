//! One Rust file read with tree-sitter: its functions, types and `use` blocks
//! with their line ranges, the token positions on every line, the calls each
//! function makes as written, and the names its `use` statements bring in.
//!
//! Pure: a string in, plain data out. Resolving a call to the item it names
//! needs the other files and is [`super::resolve`]'s job.

use std::collections::BTreeMap;

use cellworks_map::{ItemKind, Token};
use tree_sitter::{Node, Parser};

/// What one file holds.
#[derive(Debug, Default)]
pub(crate) struct FileCode {
    pub items: Vec<ParsedItem>,
    /// Per module inside the file (empty for the file's own module): what its
    /// `use` statements bring in.
    pub uses: BTreeMap<Vec<String>, Uses>,
    /// Per line, from 0: the tokens on it.
    pub tokens: Vec<Vec<Token>>,
    /// The file's lines.
    pub lines: Vec<String>,
}

/// The names `use` statements bring into one module.
#[derive(Debug, Default)]
pub(crate) struct Uses {
    /// A name and the path it stands for.
    pub names: BTreeMap<String, Vec<String>>,
    /// Paths imported with `::*`.
    pub globs: Vec<Vec<String>>,
}

/// A function, type or `use` block, as written.
#[derive(Debug)]
pub(crate) struct ParsedItem {
    pub name: String,
    pub kind: ItemKind,
    /// The modules inside the file it sits in, outermost first.
    pub module: Vec<String>,
    /// The type of the `impl` block it sits in.
    pub owner: Option<String>,
    /// First and last line, from 0, inclusive.
    pub start: usize,
    pub end: usize,
    /// Each call it makes: the line, from 0, and what is called.
    pub calls: Vec<(usize, Call)>,
    /// Whether it belongs to the tests: a test, or anything inside a test
    /// module (`#[cfg(test)]`, or a module called `tests`).
    pub tests: bool,
}

impl ParsedItem {
    /// Its name inside the file's module: the inner modules, the `impl`
    /// type and its own name.
    pub fn qualified(&self) -> Vec<String> {
        let mut path = self.module.clone();
        path.extend(self.owner.clone());
        path.push(self.name.clone());
        path
    }
}

/// A call as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Call {
    /// `f()`, `a::b::f()`, `Self::f()`: the path's segments.
    Path(Vec<String>),
    /// `self.f()`.
    Method(String),
}

/// The name an item gets for a block of `use` statements.
pub(crate) const USES: &str = "use";

/// Columns a tab counts as.
const TAB: usize = 4;

/// Reads `source`. A file tree-sitter cannot parse at all holds nothing; one
/// with syntax errors still yields what parsed.
pub(crate) fn parse(source: &str) -> FileCode {
    let mut parser = Parser::new();
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let tree = parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .ok()
        .and_then(|()| parser.parse(source, None));
    let Some(tree) = tree else {
        return FileCode {
            lines,
            ..FileCode::default()
        };
    };
    let mut code = FileCode {
        tokens: vec![Vec::new(); lines.len()],
        lines,
        ..FileCode::default()
    };
    leaves(tree.root_node(), source, &mut code);
    let mut reader = Reader {
        source,
        code: &mut code,
        in_tests: false,
    };
    reader.declarations(tree.root_node(), &[], None);
    code
}

/// The column `byte` of `line` is at, a tab counting as [`TAB`].
pub(crate) fn column(line: &str, byte: usize) -> usize {
    line.get(..byte)
        .unwrap_or(line)
        .chars()
        .map(|c| if c == '\t' { TAB } else { 1 })
        .sum()
}

/// Records every leaf of the tree as a token on the line it starts on.
fn leaves(root: Node<'_>, source: &str, code: &mut FileCode) {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if node.child_count() == 0 && node.end_byte() > node.start_byte() {
            let start = node.start_position();
            if let (Some(line), Some(tokens)) =
                (code.lines.get(start.row), code.tokens.get_mut(start.row))
            {
                let first = column(line, start.column);
                let text = source.get(node.byte_range()).unwrap_or_default();
                let len = text.lines().next().map_or(0, |t| column(t, t.len()));
                tokens.push(Token {
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
                return;
            }
        }
    }
}

struct Reader<'s, 'c> {
    source: &'s str,
    code: &'c mut FileCode,
    /// Whether the declarations being read lie inside a test module.
    in_tests: bool,
}

impl Reader<'_, '_> {
    fn text(&self, node: Node<'_>) -> &str {
        self.source.get(node.byte_range()).unwrap_or_default()
    }

    /// The items among the children of `parent`, a file or a module body or an
    /// `impl` body. Adjacent `use` statements form one block.
    fn declarations(&mut self, parent: Node<'_>, module: &[String], owner: Option<&str>) {
        let mut cursor = parent.walk();
        let children: Vec<Node<'_>> = parent.named_children(&mut cursor).collect();
        let mut uses: Option<(usize, usize)> = None;
        let mut test_attribute = false;
        let mut cfg_test = false;
        for node in children {
            let is_use = matches!(node.kind(), "use_declaration" | "extern_crate_declaration")
                || (node.kind() == "mod_item" && node.child_by_field_name("body").is_none());
            if is_use {
                if node.kind() == "use_declaration"
                    && let Some(tree) = node.child_by_field_name("argument")
                {
                    self.use_tree(tree, Vec::new(), module);
                }
                let (start, end) = (node.start_position().row, node.end_position().row);
                uses = Some(uses.map_or((start, end), |(first, _)| (first, end)));
                continue;
            }
            if node.kind() == "attribute_item" {
                test_attribute |= self.is_test_attribute(node);
                cfg_test |= self.is_cfg_test(node);
                continue;
            }
            if node.kind().ends_with("comment") {
                continue;
            }
            self.close_uses(&mut uses, module);
            let is_test = std::mem::take(&mut test_attribute);
            let is_cfg_test = std::mem::take(&mut cfg_test);
            match node.kind() {
                "function_item" => self.function(node, module, owner, is_test),
                "struct_item" | "enum_item" | "union_item" | "trait_item" | "type_item" => {
                    self.named(node, ItemKind::Type, module, None, Vec::new());
                }
                "impl_item" => {
                    let owner = node
                        .child_by_field_name("type")
                        .map(|ty| type_name(self.text(ty)));
                    if let Some(body) = node.child_by_field_name("body") {
                        self.declarations(body, module, owner.as_deref());
                    }
                }
                "mod_item" => {
                    if let (Some(name), Some(body)) = (
                        node.child_by_field_name("name"),
                        node.child_by_field_name("body"),
                    ) {
                        let name = self.text(name).to_owned();
                        let outer = self.in_tests;
                        self.in_tests |= is_cfg_test || name == "tests";
                        let mut inner = module.to_vec();
                        inner.push(name);
                        self.declarations(body, &inner, None);
                        self.in_tests = outer;
                    }
                }
                _ => {}
            }
        }
        self.close_uses(&mut uses, module);
    }

    fn close_uses(&mut self, uses: &mut Option<(usize, usize)>, module: &[String]) {
        if let Some((start, end)) = uses.take() {
            self.code.items.push(ParsedItem {
                name: USES.to_owned(),
                kind: ItemKind::Uses,
                module: module.to_vec(),
                owner: None,
                start,
                end,
                calls: Vec::new(),
                tests: self.in_tests,
            });
        }
    }

    fn is_test_attribute(&self, node: Node<'_>) -> bool {
        let text = self.text(node);
        let inner = text.trim_start_matches("#[").trim_end_matches(']').trim();
        inner == "test" || inner.ends_with("::test")
    }

    fn is_cfg_test(&self, node: Node<'_>) -> bool {
        let text: String = self
            .text(node)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        text == "#[cfg(test)]"
    }

    fn function(&mut self, node: Node<'_>, module: &[String], owner: Option<&str>, test: bool) {
        let mut calls = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            self.calls(body, &mut calls);
        }
        let kind = if test {
            ItemKind::Test
        } else {
            ItemKind::Function
        };
        self.named(node, kind, module, owner, calls);
    }

    fn named(
        &mut self,
        node: Node<'_>,
        kind: ItemKind,
        module: &[String],
        owner: Option<&str>,
        calls: Vec<(usize, Call)>,
    ) {
        let Some(name) = node.child_by_field_name("name") else {
            return;
        };
        let name = self.text(name).to_owned();
        self.code.items.push(ParsedItem {
            name,
            kind,
            module: module.to_vec(),
            owner: owner.map(str::to_owned),
            start: node.start_position().row,
            end: node.end_position().row,
            calls,
            tests: self.in_tests || kind == ItemKind::Test,
        });
    }

    /// Every call below `node`, as written.
    fn calls(&self, node: Node<'_>, into: &mut Vec<(usize, Call)>) {
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
            && let Some(call) = self.callee(function)
        {
            into.push((function.start_position().row, call));
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            // A nested function's calls are its own.
            if child.kind() != "function_item" {
                self.calls(child, into);
            }
        }
    }

    fn callee(&self, function: Node<'_>) -> Option<Call> {
        match function.kind() {
            "identifier" | "scoped_identifier" => Some(Call::Path(segments(self.text(function)))),
            "generic_function" => self.callee(function.child_by_field_name("function")?),
            "field_expression" => {
                let value = function.child_by_field_name("value")?;
                let field = function.child_by_field_name("field")?;
                (self.text(value) == "self").then(|| Call::Method(self.text(field).to_owned()))
            }
            _ => None,
        }
    }

    /// Records the names one `use` tree brings into `module`.
    fn use_tree(&mut self, node: Node<'_>, prefix: Vec<String>, module: &[String]) {
        match node.kind() {
            "scoped_use_list" => {
                let mut inner = prefix;
                if let Some(path) = node.child_by_field_name("path") {
                    inner.extend(segments(self.text(path)));
                }
                if let Some(list) = node.child_by_field_name("list") {
                    self.use_tree(list, inner, module);
                }
            }
            "use_list" => {
                let mut cursor = node.walk();
                let children: Vec<Node<'_>> = node.named_children(&mut cursor).collect();
                for child in children {
                    self.use_tree(child, prefix.clone(), module);
                }
            }
            "use_as_clause" => {
                let (Some(path), Some(alias)) = (
                    node.child_by_field_name("path"),
                    node.child_by_field_name("alias"),
                ) else {
                    return;
                };
                let mut full = prefix;
                full.extend(segments(self.text(path)));
                let alias = self.text(alias).to_owned();
                self.code
                    .uses
                    .entry(module.to_vec())
                    .or_default()
                    .names
                    .insert(alias, full);
            }
            "use_wildcard" => {
                let mut full = prefix;
                let mut cursor = node.walk();
                if let Some(path) = node.named_children(&mut cursor).next() {
                    full.extend(segments(self.text(path)));
                }
                self.code
                    .uses
                    .entry(module.to_vec())
                    .or_default()
                    .globs
                    .push(full);
            }
            "identifier" | "scoped_identifier" | "crate" | "self" | "super" => {
                let mut full = prefix;
                full.extend(segments(self.text(node)));
                if full.last().is_some_and(|last| last == "self") {
                    full.pop();
                }
                if let Some(name) = full.last().cloned() {
                    self.code
                        .uses
                        .entry(module.to_vec())
                        .or_default()
                        .names
                        .insert(name, full);
                }
            }
            _ => {}
        }
    }
}

/// The segments of a path as written, without generic arguments:
/// `Vec::<u8>::new` is `["Vec", "new"]`.
fn segments(path: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut depth = 0usize;
    for c in path.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 && !c.is_whitespace() => plain.push(c),
            _ => {}
        }
    }
    plain
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The name of the type an `impl` block is for: `Cart<T>` and `crate::Cart`
/// are `Cart`.
fn type_name(written: &str) -> String {
    segments(written)
        .last()
        .cloned()
        .unwrap_or_else(|| written.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRICING: &str = "\
use std::fmt;
use crate::money::{round_price, Money as Cash};

/// A discount.
fn discount(price: u32) -> u32 {
    let reduced = price - 1;
    round_price(reduced)
}

struct Cart;

impl Cart {
    fn total(&self) -> u32 {
        self.count() + Self::fee()
    }
    fn count(&self) -> u32 { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discount_reduces() {
        assert_eq!(discount(2), 1);
    }
}
";

    fn item<'a>(code: &'a FileCode, qualified: &[&str]) -> &'a ParsedItem {
        code.items
            .iter()
            .find(|item| item.qualified() == qualified)
            .unwrap_or_else(|| panic!("no {qualified:?} in {:?}", code.items))
    }

    #[test]
    fn functions_types_and_use_blocks_are_known_with_their_lines() {
        let code = parse(PRICING);

        let names: Vec<_> = code.items.iter().map(ParsedItem::qualified).collect();
        assert_eq!(
            names,
            [
                vec!["use"],
                vec!["discount"],
                vec!["Cart"],
                vec!["Cart", "total"],
                vec!["Cart", "count"],
                vec!["tests", "use"],
                vec!["tests", "discount_reduces"],
            ]
        );
        let discount = item(&code, &["discount"]);
        assert_eq!((discount.start, discount.end), (4, 7));
        assert_eq!(item(&code, &["use"]).end, 1);
        assert_eq!(
            item(&code, &["tests", "discount_reduces"]).kind,
            ItemKind::Test
        );
        assert_eq!(item(&code, &["Cart"]).kind, ItemKind::Type);
    }

    #[test]
    fn tests_and_everything_in_a_test_module_belong_to_the_tests() {
        let code = parse(PRICING);

        let tests: Vec<_> = code
            .items
            .iter()
            .filter(|item| item.tests)
            .map(ParsedItem::qualified)
            .collect();
        assert_eq!(
            tests,
            [vec!["tests", "use"], vec!["tests", "discount_reduces"]]
        );
    }

    #[test]
    fn each_line_carries_its_token_positions() {
        let code = parse(PRICING);

        // `    let reduced = price - 1;`
        let starts: Vec<usize> = code.tokens[5].iter().map(|t| t.start).collect();
        assert_eq!(starts, [4, 8, 16, 18, 24, 26, 27]);
        assert_eq!(code.tokens[5][1].len, "reduced".len());
    }

    #[test]
    fn calls_are_recorded_as_written() {
        let code = parse(PRICING);

        assert_eq!(
            item(&code, &["discount"]).calls,
            [(6, Call::Path(vec!["round_price".to_owned()]))]
        );
        assert_eq!(
            item(&code, &["Cart", "total"]).calls,
            [
                (13, Call::Method("count".to_owned())),
                (13, Call::Path(vec!["Self".to_owned(), "fee".to_owned()])),
            ]
        );
    }

    #[test]
    fn use_statements_name_what_they_bring_in() {
        let code = parse(PRICING);

        let top = &code.uses[&Vec::new()];
        assert_eq!(top.names["round_price"], ["crate", "money", "round_price"]);
        assert_eq!(top.names["Cash"], ["crate", "money", "Money"]);
        assert_eq!(top.names["fmt"], ["std", "fmt"]);
        assert_eq!(code.uses[&vec!["tests".to_owned()]].globs, [vec!["super"]]);
    }

    #[test]
    fn generic_arguments_are_not_part_of_a_path() {
        assert_eq!(segments("Vec::<u8>::new"), ["Vec", "new"]);
        assert_eq!(type_name("crate::Cart<T>"), "Cart");
    }

    #[test]
    fn a_tab_counts_as_four_columns() {
        assert_eq!(column("\tx", 1), 4);
    }
}
