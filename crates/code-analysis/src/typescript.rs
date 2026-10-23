//! The TypeScript and JavaScript files of a project read into map items:
//! functions, classes and their methods, type declarations, constants and
//! import blocks, with each file's tests — `describe`, `it` and `test`,
//! including `.only`, `.skip` and `.each` — read into the cellar. A file
//! whose syntax does not parse is left to the plain-text fallback.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cellworks_map::{Declaration, Declares, Floor, Item, ItemKind};
use tree_sitter::{Language, Node, Parser};

use crate::items;

/// Files larger than this are left unread: generated code, not code to walk.
const LARGEST: u64 = 2 * 1024 * 1024;

/// The items of every TypeScript or JavaScript file among `files`, relative
/// to `root`, keyed by the file. A file this reader does not want — the
/// wrong extension, too large, or one whose syntax does not parse — has no
/// entry, left to the plain-text fallback.
pub(crate) fn read(root: &Path, files: &[PathBuf]) -> BTreeMap<PathBuf, Vec<Item>> {
    files
        .iter()
        .filter_map(|file| {
            let language = grammar_of(file)?;
            let full = root.join(file);
            if std::fs::metadata(&full).ok()?.len() > LARGEST {
                return None;
            }
            let source = std::fs::read_to_string(&full).ok()?;
            let tests_file = file
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".test.") || name.contains(".spec."));
            Some((file.clone(), items(&full, &source, language, tests_file)?))
        })
        .collect()
}

/// The grammar `file`'s extension reads with: the TypeScript grammar for
/// `.ts`/`.mts`/`.cts`, the TSX grammar for the rest of the family — `.tsx`
/// and the whole plain-JavaScript family, since React code often holds
/// markup in a plain `.js` file. `None` when the extension is not this
/// reader's family at all.
fn grammar_of(file: &Path) -> Option<Language> {
    match file.extension()?.to_str()? {
        "ts" | "mts" | "cts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        _ => None,
    }
}

/// A function, type, constant, import block or test, as written, before it
/// gets a final, unique map path.
struct Found {
    name: String,
    /// Fed to [`items::unique_paths`]: a method's is `"Class::method"`.
    path_name: String,
    kind: ItemKind,
    /// The index in the same list of the class it is a method of.
    owner: Option<usize>,
    /// First and last line, from 0, inclusive.
    start: usize,
    end: usize,
    tests: bool,
    public: bool,
    declares: Declares,
}

impl Found {
    fn new(
        name: &str,
        kind: ItemKind,
        start: usize,
        end: usize,
        tests: bool,
        public: bool,
    ) -> Found {
        Found {
            name: name.to_owned(),
            path_name: name.to_owned(),
            kind,
            owner: None,
            start,
            end,
            tests,
            public,
            declares: Declares::Plain,
        }
    }
}

/// A recognised `describe`/`it`/`test` call, with `.only`, `.skip` and
/// `.each` variants folded in.
enum Framework {
    /// `describe(...)`: no item of its own, its body read as a nested file.
    Suite,
    /// `it(...)`/`test(...)`: a [`ItemKind::Test`] named by its label.
    Test(String),
}

/// The pure per-file reader: parses `source` with `language`, bails out on a
/// syntax error, and walks its top-level statements into items with their
/// floors and a final, unique path each.
fn items(full: &Path, source: &str, language: Language, tests_file: bool) -> Option<Vec<Item>> {
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(source, None)?;
    if tree.root_node().has_error() {
        return None;
    }
    let lines: Vec<String> = source.lines().map(str::to_owned).collect();
    let tokens = items::tokens(tree.root_node(), source, &lines);

    let mut found = Vec::new();
    statements(tree.root_node(), source, tests_file, &mut found);

    let paths = items::unique_paths(full, found.iter().map(|f| (f.path_name.clone(), f.start)));

    Some(
        found
            .into_iter()
            .enumerate()
            .map(|(index, f)| {
                let floors: Vec<Floor> = (f.start..=f.end)
                    .filter_map(|row| items::floor(&lines, &tokens, row))
                    .collect();
                Item {
                    name: f.name,
                    path: paths[index].clone(),
                    kind: f.kind,
                    tests: f.tests,
                    floors,
                    area: egui::Rect::NOTHING,
                    base: 0.0,
                    declaration: Declaration {
                        declares: f.declares,
                        public: f.public,
                        part_of: f.owner.map(|owner| paths[owner].clone()),
                        implements: Vec::new(),
                        holds: Vec::new(),
                    },
                }
            })
            .collect(),
    )
}

fn text<'s>(node: Node<'_>, source: &'s str) -> &'s str {
    source.get(node.byte_range()).unwrap_or_default()
}

fn name_of(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|name| text(name, source).to_owned())
}

/// A string or template-string literal's text, its surrounding quotes or
/// backticks stripped; anything else inside — a `.each` placeholder among
/// them — is left exactly as written.
fn label_text(node: Node<'_>, source: &str) -> String {
    let mut chars = text(node, source).chars();
    chars.next();
    chars.next_back();
    chars.as_str().to_owned()
}

/// The items among the named children of `parent`, a file or a suite's
/// function body. Adjacent `import` statements form one block.
fn statements(parent: Node<'_>, source: &str, tests: bool, out: &mut Vec<Found>) {
    let mut cursor = parent.walk();
    let children: Vec<Node<'_>> = parent.named_children(&mut cursor).collect();
    let mut imports: Option<(usize, usize)> = None;
    for node in children {
        if node.kind() == "import_statement" {
            let (start, end) = (node.start_position().row, node.end_position().row);
            imports = Some(imports.map_or((start, end), |(first, _)| (first, end)));
            continue;
        }
        if node.kind() == "comment" {
            continue;
        }
        if let Some((start, end)) = imports.take() {
            out.push(Found::new(
                "imports",
                ItemKind::Uses,
                start,
                end,
                tests,
                false,
            ));
        }
        declaration(node, source, tests, false, out);
    }
    if let Some((start, end)) = imports {
        out.push(Found::new(
            "imports",
            ItemKind::Uses,
            start,
            end,
            tests,
            false,
        ));
    }
}

/// One top-level statement: a declaration becomes an item, `export` makes it
/// public, and a test-framework call is read by [`framework`].
fn declaration(node: Node<'_>, source: &str, tests: bool, public: bool, out: &mut Vec<Found>) {
    let (start, end) = (node.start_position().row, node.end_position().row);
    match node.kind() {
        "export_statement" => {
            if let Some(inner) = node.child_by_field_name("declaration") {
                declaration(inner, source, tests, true, out);
            } else if let Some(value) = node.child_by_field_name("value") {
                declaration(value, source, tests, true, out);
            }
        }
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name) = name_of(node, source) {
                out.push(Found::new(
                    &name,
                    ItemKind::Function,
                    start,
                    end,
                    tests,
                    public,
                ));
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            class(node, source, tests, public, out);
        }
        "interface_declaration" | "type_alias_declaration" | "enum_declaration" => {
            if let Some(name) = name_of(node, source) {
                out.push(Found::new(&name, ItemKind::Type, start, end, tests, public));
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            variables(node, source, tests, public, out);
        }
        "expression_statement" => expression(node, source, out),
        _ => {}
    }
}

/// A class's own item, kept down to the line before its first method (or the
/// whole class when it has none), and each method as its own item, named
/// after it and `part_of` the class's own, final item.
fn class(node: Node<'_>, source: &str, tests: bool, public: bool, out: &mut Vec<Found>) {
    let Some(name) = name_of(node, source) else {
        return;
    };
    let start = node.start_position().row;
    let end = node.end_position().row;
    let index = out.len();
    out.push(Found::new(&name, ItemKind::Type, start, end, tests, public));
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();
    let methods: Vec<Node<'_>> = body
        .named_children(&mut cursor)
        .filter(|member| member.kind() == "method_definition")
        .collect();
    if let Some(first) = methods.first() {
        out[index].end = first.start_position().row.saturating_sub(1).max(start);
    }
    for method in methods {
        let Some(method_name) = name_of(method, source) else {
            continue;
        };
        out.push(Found {
            name: method_name.clone(),
            path_name: format!("{name}::{method_name}"),
            kind: ItemKind::Function,
            owner: Some(index),
            start: method.start_position().row,
            end: method.end_position().row,
            tests,
            public,
            declares: Declares::Plain,
        });
    }
}

/// Each declarator of a `const`/`let`/`var` statement: a plain identifier
/// holding an arrow function or function expression is a `Function`, a
/// plain identifier holding anything else is a `Constant`, a destructuring
/// pattern yields nothing.
fn variables(node: Node<'_>, source: &str, tests: bool, public: bool, out: &mut Vec<Found>) {
    let start = node.start_position().row;
    let end = node.end_position().row;
    let mut cursor = node.walk();
    for declarator in node.named_children(&mut cursor) {
        let Some(name_node) = declarator.child_by_field_name("name") else {
            continue;
        };
        if name_node.kind() != "identifier" {
            continue;
        }
        let name = text(name_node, source).to_owned();
        let is_function = declarator
            .child_by_field_name("value")
            .is_some_and(|value| {
                matches!(
                    value.kind(),
                    "arrow_function" | "function_expression" | "generator_function"
                )
            });
        let kind = if is_function {
            ItemKind::Function
        } else {
            ItemKind::Type
        };
        let mut item = Found::new(&name, kind, start, end, tests, public);
        if !is_function {
            item.declares = Declares::Constant;
        }
        out.push(item);
    }
}

/// An `expression_statement` holding a call: a test-framework call read by
/// [`framework`], else nothing.
fn expression(node: Node<'_>, source: &str, out: &mut Vec<Found>) {
    let Some(call) = node
        .named_child(0)
        .filter(|c| c.kind() == "call_expression")
    else {
        return;
    };
    match framework(call, source) {
        Some(Framework::Suite) => {
            if let Some(body) = call
                .child_by_field_name("arguments")
                .and_then(|arguments| arguments.named_child(1))
                .and_then(|function| function.child_by_field_name("body"))
            {
                statements(body, source, true, out);
            }
        }
        Some(Framework::Test(label)) => {
            let (start, end) = (node.start_position().row, node.end_position().row);
            out.push(Found::new(&label, ItemKind::Test, start, end, true, false));
        }
        None => {}
    }
}

/// `describe`/`it`/`test`, its `.only`/`.skip` suffix, or its `.each(table)`
/// call-then-call (also written as a tagged template, which parses the same
/// way: a nested call whose own `function` is the `X.each` member
/// expression). Recognised only when the outermost call's first argument is
/// a string or template-string literal.
fn framework(call: Node<'_>, source: &str) -> Option<Framework> {
    let callee = call.child_by_field_name("function")?;
    let base = match callee.kind() {
        "identifier" | "member_expression" => base_name(callee, source)?,
        "call_expression" => {
            let inner = callee.child_by_field_name("function")?;
            let (base, modifier) = base_and_modifier(inner, source)?;
            (modifier == Some("each")).then_some(base)?
        }
        _ => return None,
    };
    if !matches!(base.as_str(), "describe" | "it" | "test") {
        return None;
    }
    let arguments = call.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    let first = arguments.named_children(&mut cursor).next()?;
    if !matches!(first.kind(), "string" | "template_string") {
        return None;
    }
    if base == "describe" {
        Some(Framework::Suite)
    } else {
        Some(Framework::Test(label_text(first, source)))
    }
}

/// `describe`/`it`/`test` on its own, or with an `.only`/`.skip`/`.each`
/// suffix.
fn base_name(node: Node<'_>, source: &str) -> Option<String> {
    base_and_modifier(node, source).map(|(base, _)| base)
}

/// `node` as a bare identifier, or `identifier.property` when `property` is
/// `only`, `skip` or `each`; anything else matches nothing.
fn base_and_modifier(node: Node<'_>, source: &str) -> Option<(String, Option<&'static str>)> {
    match node.kind() {
        "identifier" => Some((text(node, source).to_owned(), None)),
        "member_expression" => {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;
            if object.kind() != "identifier" {
                return None;
            }
            let modifier = match text(property, source) {
                "only" => "only",
                "skip" => "skip",
                "each" => "each",
                _ => return None,
            };
            Some((text(object, source).to_owned(), Some(modifier)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use cellworks_map::{Declares, ItemKind};

    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let tree = tempfile::tempdir().expect("a temporary directory");
        for (file, source) in files {
            let path = tree.path().join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, source).expect("write the file");
        }
        tree
    }

    fn names(items: &[Item]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }

    fn find<'a>(items: &'a [Item], name: &str) -> &'a Item {
        items
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("no item named {name} among {:?}", names(items)))
    }

    #[test]
    fn an_exported_declaration_is_public_and_others_are_not() {
        let tree = project(&[(
            "src/cart.ts",
            "export function pub_fn() {}\nfunction priv_fn() {}\n",
        )]);
        let files = vec![PathBuf::from("src/cart.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.ts")];
        assert!(find(cart, "pub_fn").declaration.public);
        assert!(!find(cart, "priv_fn").declaration.public);
    }

    #[test]
    fn a_class_keeps_its_lines_down_to_its_first_method() {
        let tree = project(&[(
            "src/cart.ts",
            "class Cart {\n  price = 0;\n  add(item) {\n    this.price += item;\n  }\n}\n",
        )]);
        let files = vec![PathBuf::from("src/cart.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.ts")];
        let class = find(cart, "Cart");
        assert_eq!(class.kind, ItemKind::Type);
        assert_eq!(class.floors.len(), 2, "stops before the first method");
        let method = find(cart, "add");
        assert_eq!(method.kind, ItemKind::Function);
        assert_eq!(
            method.path,
            tree.path().join("src/cart.ts").join("Cart::add")
        );
        assert_eq!(method.declaration.part_of.as_ref(), Some(&class.path));
    }

    #[test]
    fn every_other_variable_is_a_constant_one_per_declarator() {
        let tree = project(&[(
            "src/cart.ts",
            "const a = 1, b = 2;\nconst { x, y } = obj;\n",
        )]);
        let files = vec![PathBuf::from("src/cart.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.ts")];
        assert_eq!(names(cart), ["a", "b"], "the destructuring yields nothing");
        for name in ["a", "b"] {
            let item = find(cart, name);
            assert_eq!(item.kind, ItemKind::Type);
            assert_eq!(item.declaration.declares, Declares::Constant);
        }
    }

    #[test]
    fn adjacent_imports_form_one_block_and_a_gap_starts_another() {
        let tree = project(&[(
            "src/cart.ts",
            "import a from 'a';\nimport b from 'b';\nconst x = 1;\nimport c from 'c';\n",
        )]);
        let files = vec![PathBuf::from("src/cart.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.ts")];
        let file = tree.path().join("src/cart.ts");
        let imports: Vec<&Item> = cart.iter().filter(|item| item.name == "imports").collect();
        assert_eq!(imports.len(), 2, "a gap starts another block");
        assert_eq!(imports[0].path, file.join("imports"));
        assert_eq!(imports[1].path, file.join("imports@4"));
        assert!(imports.iter().all(|item| item.kind == ItemKind::Uses));
    }

    #[test]
    fn in_a_test_file_every_item_is_in_the_cellar_its_imports_included() {
        let tree = project(&[("src/cart.test.ts", "import a from 'a';\nfunction f() {}\n")]);
        let files = vec![PathBuf::from("src/cart.test.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.test.ts")];
        assert!(!cart.is_empty());
        assert!(cart.iter().all(|item| item.tests), "{cart:?}");
    }

    /// Markup only the TSX grammar reads: under the TypeScript grammar the
    /// file has a syntax error and yields no items at all.
    #[test]
    fn the_js_family_and_tsx_read_with_the_tsx_grammar() {
        for extension in ["tsx", "js", "jsx", "mjs", "cjs"] {
            let file = format!("src/app.{extension}");
            let tree = project(&[(&file, "function App() { return <div>hi</div>; }\n")]);
            let files = vec![PathBuf::from(&file)];

            let items = read(tree.path(), &files);

            let app = &items[Path::new(&file)];
            assert_eq!(find(app, "App").kind, ItemKind::Function, "{file}");
        }
    }

    /// An angle-bracket assertion only the TypeScript grammar reads: the TSX
    /// grammar takes `<number>` for markup, so the file would yield no items.
    #[test]
    fn a_type_assertion_in_a_ts_file_parses() {
        for extension in ["ts", "mts", "cts"] {
            let file = format!("src/cast.{extension}");
            let tree = project(&[(&file, "const n = <number>value;\n")]);
            let files = vec![PathBuf::from(&file)];

            let items = read(tree.path(), &files);

            let cast = &items[Path::new(&file)];
            let n = find(cast, "n");
            assert_eq!(n.kind, ItemKind::Type, "{file}");
            assert_eq!(n.declaration.declares, Declares::Constant, "{file}");
        }
    }

    #[test]
    fn focused_skipped_and_table_driven_tests_are_towers() {
        let tree = project(&[(
            "src/cart.test.ts",
            "it.only('runs first', () => {});\n\
             test.skip('not yet', () => {});\n\
             test.each([[1, 2]])('adds %i and %i', (a, b) => {});\n",
        )]);
        let files = vec![PathBuf::from("src/cart.test.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.test.ts")];
        assert_eq!(names(cart), ["runs first", "not yet", "adds %i and %i"]);
        assert!(cart.iter().all(|item| item.kind == ItemKind::Test));
    }

    #[test]
    fn focused_skipped_and_table_driven_suites_are_read() {
        let tree = project(&[(
            "src/cart.test.ts",
            "describe.only('A', () => {\n  it('a1', () => {});\n});\n\
             describe.skip('B', () => {\n  it('b1', () => {});\n});\n\
             describe.each([[1]])('C %i', () => {\n  it('c1', () => {});\n});\n",
        )]);
        let files = vec![PathBuf::from("src/cart.test.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.test.ts")];
        let found = names(cart);
        for suite in ["A", "B", "C %i"] {
            assert!(!found.contains(&suite), "{found:?}");
        }
        for test in ["a1", "b1", "c1"] {
            assert_eq!(find(cart, test).kind, ItemKind::Test);
        }
    }

    #[test]
    fn other_member_calls_are_no_tests() {
        let tree = project(&[(
            "src/misc.ts",
            "it.todo('x');\nfoo.it('x', () => {});\nexpect(x).toBe(1);\n",
        )]);
        let files = vec![PathBuf::from("src/misc.ts")];

        let items = read(tree.path(), &files);

        assert!(
            items
                .get(Path::new("src/misc.ts"))
                .is_none_or(Vec::is_empty)
        );
    }

    #[test]
    fn nested_suites_are_read_and_top_level_tests_count_too() {
        let tree = project(&[(
            "src/cart.test.ts",
            "describe('Outer', () => {\n  describe('Inner', () => {\n    it('deep', () => {});\n  });\n});\n\
             it('top', () => {});\n",
        )]);
        let files = vec![PathBuf::from("src/cart.test.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.test.ts")];
        let found = names(cart);
        assert!(
            !found.contains(&"Outer") && !found.contains(&"Inner"),
            "{found:?}"
        );
        for test in ["deep", "top"] {
            let item = find(cart, test);
            assert_eq!(item.kind, ItemKind::Test);
            assert!(item.tests);
        }
    }

    #[test]
    fn two_declarations_of_one_name_get_their_line() {
        let tree = project(&[(
            "src/cart.test.ts",
            "it('does x', () => {});\nit('does x', () => {});\n",
        )]);
        let files = vec![PathBuf::from("src/cart.test.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.test.ts")];
        let file = tree.path().join("src/cart.test.ts");
        assert_eq!(cart[0].path, file.join("does x"));
        assert_eq!(cart[1].path, file.join("does x@2"));
    }

    #[test]
    fn a_test_label_with_a_slash_stays_one_path_segment() {
        let tree = project(&[("src/cart.test.ts", "it('adds a/b', () => {});\n")]);
        let files = vec![PathBuf::from("src/cart.test.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.test.ts")];
        let file = tree.path().join("src/cart.test.ts");
        assert_eq!(cart[0].path.parent(), Some(file.as_path()));
        let segment = cart[0]
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a name");
        assert!(segment.contains('\u{2215}'), "{segment}");
        assert!(!segment.contains('/'), "{segment}");
    }

    #[test]
    fn a_file_with_a_syntax_error_has_no_items() {
        let tree = project(&[("src/broken.ts", "function broken(\n  return 1\n")]);
        let files = vec![PathBuf::from("src/broken.ts")];

        let items = read(tree.path(), &files);

        assert!(!items.contains_key(Path::new("src/broken.ts")));
    }

    #[test]
    fn floors_carry_the_tokens_of_their_line() {
        let tree = project(&[("src/cart.ts", "function f() {\n  const x = 42;\n}\n")]);
        let files = vec![PathBuf::from("src/cart.ts")];

        let items = read(tree.path(), &files);

        let cart = &items[Path::new("src/cart.ts")];
        let f = find(cart, "f");
        let line = &f.floors[1];
        assert_eq!(line.text, "  const x = 42;");
        assert_eq!(line.indent, 2);
        assert!(!line.tokens.is_empty());
        assert!(line.calls.is_empty());
        assert!(line.imports.is_empty());
    }
}
