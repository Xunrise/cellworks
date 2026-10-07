// Tree-sitter analysis of one source file: functions with cyclomatic complexity, imports and calls.
use std::collections::HashMap;
use tree_sitter::{Language, Node, Parser};

pub struct Func {
    pub name: String,
    pub line: u32,
    pub loc: u32,
    pub cx: u32,
    pub params: u32,
}

pub struct FileAst {
    pub lang: &'static str,
    pub funcs: Vec<Func>,
    pub imports: Vec<String>,
    pub calls: HashMap<String, u32>,
}

pub fn language(ext: &str) -> Option<(&'static str, Language)> {
    Some(match ext {
        "ts" | "mts" | "cts" => ("ts", tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => ("tsx", tree_sitter_typescript::LANGUAGE_TSX.into()),
        "js" | "jsx" | "mjs" | "cjs" => ("js", tree_sitter_javascript::LANGUAGE.into()),
        "py" => ("py", tree_sitter_python::LANGUAGE.into()),
        "php" => ("php", tree_sitter_php::LANGUAGE_PHP.into()),
        "rs" => ("rs", tree_sitter_rust::LANGUAGE.into()),
        "lua" => ("lua", tree_sitter_lua::LANGUAGE.into()),
        "sh" | "bash" | "zsh" => ("sh", tree_sitter_bash::LANGUAGE.into()),
        _ => return None,
    })
}

const FUNCS: &[&str] = &[
    "function_declaration", "function_definition", "method_definition", "method_declaration", "function_item", "arrow_function",
    "function_expression", "generator_function_declaration", "function", "local_function", "function_signature_item",
];
const CLASSES: &[&str] = &["class_declaration", "class_definition", "impl_item", "trait_item", "interface_declaration", "trait_declaration", "abstract_class_declaration"];
const DECISIONS: &[&str] = &[
    "if_statement", "if_expression", "elif_clause", "else_if_clause", "elseif_statement", "for_statement", "for_in_statement", "for_expression",
    "foreach_statement", "while_statement", "while_expression", "do_statement", "loop_expression", "repeat_statement", "c_style_for_statement",
    "switch_case", "case_statement", "case_item", "match_arm", "catch_clause", "except_clause", "conditional_expression", "ternary_expression",
    "list_comprehension", "conditional_type",
];
const CALLS: &[&str] = &["call_expression", "call", "function_call", "function_call_expression", "member_call_expression", "scoped_call_expression", "nullsafe_member_call_expression", "command"];

fn text<'a>(n: Node, src: &'a [u8]) -> &'a str {
    n.utf8_text(src).unwrap_or("")
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string()
}

/// Name of a function node, looking at the declarator it is assigned to when it is anonymous.
fn func_name(n: Node, src: &[u8]) -> Option<String> {
    if let Some(name) = n.child_by_field_name("name") {
        return Some(text(name, src).to_string());
    }
    let p = n.parent()?;
    let field = match p.kind() {
        "variable_declarator" | "public_field_definition" | "field_definition" => "name",
        "pair" => "key",
        "assignment_expression" | "assignment" => "left",
        _ => return None,
    };
    p.child_by_field_name(field).map(|x| text(x, src).to_string())
}

fn callee(n: Node, src: &[u8]) -> Option<String> {
    let f = n.child_by_field_name("function").or_else(|| n.child_by_field_name("name"))?;
    let f = match f.kind() {
        "member_expression" => f.child_by_field_name("property")?,
        "attribute" => f.child_by_field_name("attribute")?,
        "field_expression" => f.child_by_field_name("field")?,
        "scoped_identifier" => f.child_by_field_name("name")?,
        "method_index_expression" | "dot_index_expression" => f.child_by_field_name("method").or_else(|| f.child_by_field_name("field"))?,
        _ => f,
    };
    let t = text(f, src);
    let last = t.rsplit(['.', ':', '>']).next().unwrap_or(t);
    (!last.is_empty() && last.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')).then(|| last.to_string())
}

fn import_of(n: Node, src: &[u8]) -> Vec<String> {
    match n.kind() {
        "import_statement" | "export_statement" => {
            if let Some(s) = n.child_by_field_name("source") {
                return vec![unquote(text(s, src))]; // js/ts
            }
            let mut c = n.walk();
            n.named_children(&mut c).filter(|x| x.kind() == "dotted_name" || x.kind() == "aliased_import") // python
                .map(|x| text(x.child_by_field_name("name").unwrap_or(x), src).to_string()).collect()
        }
        "import_from_statement" => n.child_by_field_name("module_name").map(|m| vec![text(m, src).to_string()]).unwrap_or_default(),
        "use_declaration" => n.child_by_field_name("argument").map(|a| vec![text(a, src).to_string()]).unwrap_or_default(),
        "mod_item" if n.child_by_field_name("body").is_none() => n.child_by_field_name("name").map(|a| vec![format!("self::{}", text(a, src))]).unwrap_or_default(),
        "namespace_use_clause" => vec![text(n, src).split(" as ").next().unwrap_or("").trim().to_string()],
        "require_expression" | "require_once_expression" | "include_expression" | "include_once_expression" => {
            let mut c = n.walk();
            n.named_children(&mut c).find(|x| x.kind().contains("string")).map(|s| vec![unquote(text(s, src))]).unwrap_or_default()
        }
        "call_expression" | "function_call" => {
            let f = n.child_by_field_name("function").or_else(|| n.child_by_field_name("name"));
            if f.is_some_and(|f| matches!(text(f, src), "require" | "import")) {
                let args = n.child_by_field_name("arguments");
                if let Some(s) = args.and_then(|a| a.named_child(0)).filter(|s| s.kind().contains("string")) {
                    return vec![unquote(text(s, src))];
                }
            }
            vec![]
        }
        "command" => {
            let name = n.child_by_field_name("name").map(|x| text(x, src));
            if matches!(name, Some("source") | Some(".")) {
                if let Some(a) = n.child_by_field_name("argument") {
                    return vec![unquote(text(a, src))];
                }
            }
            vec![]
        }
        _ => vec![],
    }
}

fn is_decision(n: Node, src: &[u8]) -> bool {
    if DECISIONS.contains(&n.kind()) {
        return true;
    }
    if matches!(n.kind(), "binary_expression" | "boolean_operator") {
        if let Some(op) = n.child_by_field_name("operator") {
            return matches!(text(op, src), "&&" | "||" | "and" | "or" | "??");
        }
    }
    false
}

pub fn analyze(ext: &str, source: &str) -> Option<FileAst> {
    let (lang, language) = language(ext)?;
    let mut parser = Parser::new();
    parser.set_language(&language).ok()?;
    let tree = parser.parse(source, None)?;
    let src = source.as_bytes();
    let mut out = FileAst { lang, funcs: vec![], imports: vec![], calls: HashMap::new() };
    // explicit stack: (node, index of the innermost named function, class prefix)
    let mut stack: Vec<(Node, Option<usize>, String)> = vec![(tree.root_node(), None, String::new())];
    while let Some((n, mut scope, mut class)) = stack.pop() {
        let kind = n.kind();
        if CLASSES.contains(&kind) {
            let name = n.child_by_field_name("name").or_else(|| n.child_by_field_name("type")).map(|x| text(x, src).to_string()).unwrap_or_default();
            if !name.is_empty() {
                class = name;
            }
        }
        if FUNCS.contains(&kind) {
            if let Some(name) = func_name(n, src) {
                let name = if class.is_empty() || name.contains('.') { name } else { format!("{class}.{name}") };
                let params = n.child_by_field_name("parameters").map_or(0, |p| p.named_child_count() as u32);
                let (a, b) = (n.start_position().row as u32, n.end_position().row as u32);
                out.funcs.push(Func { name, line: a + 1, loc: b - a + 1, cx: 1, params });
                scope = Some(out.funcs.len() - 1);
            }
        }
        if is_decision(n, src) {
            if let Some(i) = scope {
                out.funcs[i].cx += 1;
            }
        }
        out.imports.extend(import_of(n, src));
        if CALLS.contains(&kind) {
            if let Some(c) = callee(n, src) {
                *out.calls.entry(c).or_default() += 1;
            }
        }
        let mut c = n.walk();
        let children: Vec<Node> = n.children(&mut c).collect();
        for ch in children.into_iter().rev() {
            stack.push((ch, scope, class.clone()));
        }
    }
    out.imports.retain(|i| !i.is_empty());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_functions_complexity_imports_calls() {
        let src = r#"
import { b } from "./b.js";
const fs = require("fs");
export function f(x: number, y: number) {
  if (x > 1 && y > 2) { return b(x); }
  for (const i of [1]) { g(i); }
  return x ? 1 : 2;
}
class K { m() { if (1) {} } }
const h = () => b(1);
"#;
        let a = analyze("ts", src).unwrap();
        let f = a.funcs.iter().find(|f| f.name == "f").unwrap();
        assert_eq!(f.cx, 5, "if + && + for + ternary + 1");
        assert_eq!(f.params, 2);
        assert!(a.funcs.iter().any(|f| f.name == "K.m" && f.cx == 2));
        assert!(a.funcs.iter().any(|f| f.name == "h"));
        assert_eq!(a.imports, vec!["./b.js", "fs"]);
        assert_eq!(a.calls["b"], 2);
    }

    #[test]
    fn python_and_rust_imports() {
        let py = analyze("py", "import os.path\nfrom .util import x\ndef f(a):\n    if a or a:\n        return x(a)\n").unwrap();
        assert_eq!(py.imports, vec!["os.path", ".util"]);
        assert_eq!(py.funcs[0].cx, 3);
        let rs = analyze("rs", "mod world;\nuse crate::backend::git;\nfn main() { match 1 { 1 => {}, _ => {} } git(); }").unwrap();
        assert_eq!(rs.imports, vec!["self::world", "crate::backend::git"]);
        assert_eq!(rs.funcs[0].cx, 3);
    }
}
