// Data structures as a map, read statically from a backend monorepo (no database connection):
// Postgres tables by replaying the Knex migrations, Mongo collections, gRPC proto messages and services, DTOs,
// and the services that use them. Project path: `data:<repo>`.
use crate::otc::Node;
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub fn parse(path: &str) -> Option<&str> {
    path.strip_prefix("data:")
}

#[derive(Default, Clone)]
struct Table {
    columns: Vec<(String, String)>,
    refs: HashSet<String>,
    file: String,
    line: u32,
}

fn files(dir: &Path, ext: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !matches!(name.as_str(), "node_modules" | "dist" | "target" | ".git" | "build") {
                files(&p, ext, out);
            }
        } else if p.extension().is_some_and(|x| ext.contains(&x.to_string_lossy().as_ref())) {
            out.push(p);
        }
    }
}

fn bare(name: &str) -> String {
    name.rsplit('.').next().unwrap_or(name).to_string()
}

/// Replays the `up` half of every migration in order and returns the tables that exist at the end.
fn replay(dir: &Path) -> BTreeMap<String, Table> {
    let mut paths: Vec<PathBuf> = vec![];
    files(dir, &["js", "ts"], &mut paths);
    paths.sort();
    let q = r#"['"`]([\w.]+)['"`]"#;
    let ops: Vec<(&str, Regex)> = vec![
        ("create", Regex::new(&format!(r"\bcreateTable(?:IfNotExists)?\(\s*{q}")).unwrap()),
        ("alter", Regex::new(&format!(r"(?:\.table|\balterTable)\(\s*{q}")).unwrap()),
        ("drop", Regex::new(&format!(r"\bdropTable(?:IfExists)?\(\s*{q}")).unwrap()),
        ("rename", Regex::new(&format!(r"\brenameTable\(\s*{q}\s*,\s*{q}")).unwrap()),
        ("column", Regex::new(&format!(r"\b\w+\.(increments|bigIncrements|string|text|integer|bigInteger|bigint|float|double|decimal|boolean|date|datetime|timestamp|timestamptz|time|json|jsonb|uuid|enu|enum|specificType|binary|geometry|geography|point|tinyint|smallint)\(\s*(?:{q})?")).unwrap()),
        ("dropcol", Regex::new(&format!(r"\bdropColumns?\(([^)]*)\)")).unwrap()),
        ("renamecol", Regex::new(&format!(r"\brenameColumn\(\s*{q}\s*,\s*{q}")).unwrap()),
        ("ref", Regex::new(&format!(r"\breferences\(\s*{q}\s*\)(?:\s*\.inTable\(\s*{q})?")).unwrap()),
        ("rawcreate", Regex::new(r#"(?i)CREATE TABLE (?:IF NOT EXISTS )?"?(?:\w+"?\.)?"?(\w+)"#).unwrap()),
        ("rawalter", Regex::new(r#"(?i)ALTER TABLE (?:IF EXISTS )?(?:ONLY )?"?(?:\w+"?\.)?"?(\w+)"#).unwrap()),
        ("rawdrop", Regex::new(r#"(?i)DROP TABLE (?:IF EXISTS )?"?(?:\w+"?\.)?"?(\w+)"#).unwrap()),
        ("rawcol", Regex::new(r#"(?i)ADD COLUMN (?:IF NOT EXISTS )?"?(\w+)"?\s+(\w+)"#).unwrap()),
        ("rawref", Regex::new(r#"(?i)REFERENCES "?(?:\w+"?\.)?"?(\w+)"#).unwrap()),
    ];
    let quoted = Regex::new(q).unwrap();
    let mut tables: BTreeMap<String, Table> = BTreeMap::new();
    for p in paths {
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        // only the up half: everything before the down function
        let up = ["exports.down", "export async function down", "export function down", "down:", "down ="].iter().filter_map(|m| text.find(m)).min().map_or(text.as_str(), |i| &text[..i]);
        let mut events: Vec<(usize, &str, regex::Captures)> = vec![];
        for (kind, re) in &ops {
            for c in re.captures_iter(up) {
                events.push((c.get(0).unwrap().start(), kind, c));
            }
        }
        events.sort_by_key(|e| e.0);
        let file = p.to_string_lossy().to_string();
        let mut cur: Option<String> = None;
        for (pos, kind, c) in events {
            let g = |i: usize| c.get(i).map(|m| m.as_str().to_string());
            let line = up[..pos].lines().count() as u32;
            match kind {
                "create" | "rawcreate" => {
                    let t = bare(&g(1).unwrap());
                    tables.insert(t.clone(), Table { file: file.clone(), line, ..Default::default() });
                    cur = Some(t);
                }
                "alter" | "rawalter" => {
                    let t = bare(&g(1).unwrap());
                    tables.entry(t.clone()).or_insert_with(|| Table { file: file.clone(), line, ..Default::default() });
                    cur = Some(t);
                }
                "drop" | "rawdrop" => {
                    tables.remove(&bare(&g(1).unwrap()));
                }
                "rename" => {
                    if let Some(t) = tables.remove(&bare(&g(1).unwrap())) {
                        tables.insert(bare(&g(2).unwrap()), t);
                    }
                }
                _ => {
                    let Some(t) = cur.as_ref().and_then(|c| tables.get_mut(c)) else { continue };
                    match kind {
                        "column" | "rawcol" => {
                            let (ty, name) = if kind == "column" { (g(1).unwrap(), g(2)) } else { (g(2).unwrap(), g(1)) };
                            let name = name.unwrap_or_else(|| if ty.contains("ncrements") { "id".into() } else { String::new() });
                            if !name.is_empty() && !t.columns.iter().any(|c| c.0 == name) {
                                t.columns.push((name, ty));
                            }
                        }
                        "dropcol" => {
                            for m in quoted.captures_iter(&g(1).unwrap()) {
                                t.columns.retain(|c| c.0 != m[1]);
                            }
                        }
                        "renamecol" => {
                            let (a, b) = (g(1).unwrap(), g(2).unwrap());
                            for col in t.columns.iter_mut().filter(|c| c.0 == a) {
                                col.0 = b.clone();
                            }
                        }
                        "ref" => {
                            // references('table.id') or references('id').inTable('table')
                            let target = g(2).unwrap_or_else(|| g(1).unwrap().split('.').rev().nth(1).unwrap_or("").to_string());
                            if !target.is_empty() {
                                t.refs.insert(bare(&target));
                            }
                        }
                        "rawref" => {
                            t.refs.insert(g(1).unwrap());
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    tables
}

/// (name, file, line, fields as (name, type), type names it refers to) of blocks opened by `head` and closed by a
/// line starting with `}` at the block's own indentation.
fn blocks(path: &Path, head: &Regex, field: &Regex) -> Vec<(String, String, u32, Vec<(String, String)>)> {
    let Ok(text) = std::fs::read_to_string(path) else { return vec![] };
    let lines: Vec<&str> = text.lines().collect();
    let mut out = vec![];
    for (i, l) in lines.iter().enumerate() {
        let Some(c) = head.captures(l) else { continue };
        let indent = l.len() - l.trim_start().len();
        let mut fields = vec![];
        for m in lines.iter().skip(i + 1) {
            if m.trim_start().starts_with('}') && m.len() - m.trim_start().len() <= indent {
                break;
            }
            if let Some(f) = field.captures(m) {
                fields.push((f["name"].to_string(), f["ty"].trim().to_string()));
            }
        }
        out.push((c["name"].to_string(), path.to_string_lossy().to_string(), i as u32 + 1, fields));
    }
    out
}

pub fn load(repo: &str) -> (Vec<Node>, Vec<(usize, usize)>) {
    let root = Path::new(repo);
    let mut nodes: Vec<Node> = vec![];
    let mut links: Vec<(usize, usize)> = vec![];
    let node = |district: &str, folder: &str, title: String, body: String, path: Option<String>, line: u32, items: Vec<(String, String)>| Node {
        district: district.into(), folder: folder.into(), title, body, state: None, path, line, items, code: None,
    };
    // ---- Postgres: one folder per database (migration folder), split by name prefix when large ----
    let mut table_at: HashMap<String, usize> = HashMap::new();
    let mut mig_dirs: Vec<PathBuf> = std::fs::read_dir(root.join("migration")).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("migrations") && !n.to_string_lossy().contains("mongo"))).collect()).unwrap_or_default();
    mig_dirs.sort();
    for dir in mig_dirs {
        let db = dir.file_name().unwrap().to_string_lossy().trim_start_matches("migrations").trim_start_matches('-').to_string();
        let db = if db.is_empty() { "main".to_string() } else { db };
        let tables = replay(&dir);
        for (name, t) in &tables {
            let folder = db.clone(); // big databases are split by the services using them, below
            table_at.insert(name.clone(), nodes.len());
            nodes.push(node("postgres", &folder, name.clone(), format!("table in {db}: {} columns", t.columns.len()), Some(t.file.clone()), t.line, t.columns.clone()));
        }
        for (name, t) in &tables {
            for r in &t.refs {
                if let (Some(&a), Some(&b)) = (table_at.get(name), table_at.get(r)) {
                    links.push((a, b));
                }
            }
        }
    }
    // ---- Mongo: collections named in the Mongo migrations or in service code ----
    let coll_re = Regex::new(r#"['"`](v\d+\.\d+\.\d+-[\w-]+)['"`]"#).unwrap();
    let mut colls: BTreeMap<String, (String, u32)> = BTreeMap::new();
    let mut mongo_files = vec![];
    files(&root.join("migration/migrations-mongo"), &["js", "ts"], &mut mongo_files);
    files(&root.join("services"), &["ts"], &mut mongo_files);
    for f in &mongo_files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for (i, l) in text.lines().enumerate() {
            for c in coll_re.captures_iter(l) {
                colls.entry(c[1].to_string()).or_insert((f.to_string_lossy().to_string(), i as u32 + 1));
            }
        }
    }
    let mut coll_at: HashMap<String, usize> = HashMap::new();
    for (name, (file, line)) in &colls {
        coll_at.insert(name.clone(), nodes.len());
        nodes.push(node("mongo", "collections", name.clone(), "MongoDB collection".into(), Some(file.clone()), *line, vec![]));
    }
    // ---- gRPC contracts: messages and services per proto package ----
    let mut protos = vec![];
    files(&root.join("contract/proto"), &["proto"], &mut protos);
    protos.sort();
    let msg_re = Regex::new(r"^\s*(?:message|enum) (?P<name>\w+)\s*\{").unwrap();
    let field_re = Regex::new(r"^\s*(?:repeated |optional )?(?P<ty>[\w.]+)\s+(?P<name>\w+)\s*=\s*\d+").unwrap();
    let svc_re = Regex::new(r"^\s*service (?P<name>\w+)\s*\{").unwrap();
    let rpc_re = Regex::new(r"^\s*rpc (?P<name>\w+)\s*\(\s*(?P<ty>(?:stream )?[\w.]+\s*\)\s*returns\s*\(\s*(?:stream )?[\w.]+)").unwrap();
    let mut msg_at: HashMap<String, usize> = HashMap::new();
    let mut proto_refs: Vec<(usize, Vec<String>)> = vec![];
    let mut proto_services: Vec<(String, usize)> = vec![];
    let word = Regex::new(r"[A-Za-z_]\w*").unwrap();
    for p in &protos {
        let pkg = p.parent().and_then(|d| d.file_name()).map_or("proto".into(), |n| n.to_string_lossy().to_string());
        for (name, file, line, fields) in blocks(p, &msg_re, &field_re) {
            let i = nodes.len();
            msg_at.entry(name.clone()).or_insert(i);
            proto_refs.push((i, fields.iter().map(|f| f.1.rsplit('.').next().unwrap_or(&f.1).to_string()).collect()));
            nodes.push(node("contracts", &pkg, name, format!("proto message in {pkg}"), Some(file), line, fields.into_iter().map(|(n, t)| (n, t)).collect()));
        }
        for (name, file, line, rpcs) in blocks(p, &svc_re, &rpc_re) {
            let i = nodes.len();
            proto_refs.push((i, rpcs.iter().flat_map(|r| word.find_iter(&r.1).map(|m| m.as_str().to_string()).collect::<Vec<_>>()).collect()));
            proto_services.push((name.clone(), i));
            nodes.push(node("contracts", &pkg, name, format!("gRPC service in {pkg}"), Some(file), line, rpcs));
        }
    }
    for (i, refs) in proto_refs {
        for r in refs {
            if let Some(&t) = msg_at.get(&r).filter(|&&t| t != i) {
                if !links.contains(&(i, t)) {
                    links.push((i, t));
                }
            }
        }
    }
    // ---- DTOs per service, and the services themselves ----
    let dto_re = Regex::new(r"^\s*export (?:abstract )?(?:class|interface) (?P<name>\w+(?:Dto|DTO))\b").unwrap();
    let prop_re = Regex::new(r"^\s*(?:readonly\s+|public\s+)?(?P<name>\w+)[?!]?\s*:\s*(?P<ty>[^;=]+)").unwrap();
    let mut dto_at: HashMap<String, usize> = HashMap::new();
    let mut dto_refs: Vec<(usize, String)> = vec![];
    let mut services: Vec<PathBuf> = std::fs::read_dir(root.join("services")).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
    services.sort();
    let quoted = Regex::new(r#"['"`]([\w.-]+)['"`]|(?i:from|join|into|update)\s+"?(\w+)"#).unwrap();
    let mut service_at: Vec<(usize, HashSet<String>, HashSet<String>)> = vec![];
    for svc in &services {
        let name = svc.file_name().unwrap().to_string_lossy().to_string();
        let mut src = vec![];
        files(svc, &["ts", "rs"], &mut src);
        src.sort();
        let (mut strings, mut words) = (HashSet::new(), HashSet::new());
        for f in &src {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            for c in quoted.captures_iter(&text) {
                if let Some(m) = c.get(1).or(c.get(2)) {
                    strings.insert(m.as_str().to_string());
                }
            }
            words.extend(word.find_iter(&text).map(|m| m.as_str().to_string()));
            for (dto, file, line, fields) in blocks(f, &dto_re, &prop_re) {
                let i = nodes.len();
                dto_at.entry(dto.clone()).or_insert(i);
                dto_refs.extend(fields.iter().map(|fl| (i, fl.1.clone())));
                nodes.push(node("dtos", &name, dto, format!("DTO in {name}"), Some(file), line, fields));
            }
        }
        let i = nodes.len();
        service_at.push((i, strings, words));
        let mut n = node("services", "services", name.clone(), format!("service code: services/{name}"), None, 0, vec![]);
        n.code = Some(svc.to_string_lossy().to_string());
        nodes.push(n);
    }
    for (i, ty) in dto_refs {
        for m in word.find_iter(&ty) {
            if let Some(&t) = dto_at.get(m.as_str()).filter(|&&t| t != i) {
                if !links.contains(&(i, t)) {
                    links.push((i, t));
                }
            }
        }
    }
    // a service uses the tables and collections its code names, and the gRPC services it mentions
    for (i, strings, words) in &service_at {
        let mut used: Vec<usize> = strings.iter().filter_map(|s| table_at.get(s).or(coll_at.get(s)).copied()).collect();
        used.extend(proto_services.iter().filter(|(n, _)| words.contains(n)).map(|(_, j)| *j));
        used.sort();
        used.dedup();
        links.extend(used.into_iter().map(|t| (*i, t)));
    }
    // tables of a big database sit with the one service using them, in "shared", or where no service names them
    let mut by_db: HashMap<String, Vec<usize>> = HashMap::new();
    for t in (0..nodes.len()).filter(|&t| nodes[t].district == "postgres") {
        by_db.entry(nodes[t].folder.clone()).or_default().push(t);
    }
    for (db, ts) in by_db.into_iter().filter(|(_, ts)| ts.len() > 30) {
        for t in ts {
            let users: Vec<String> = links.iter().filter(|l| l.1 == t && nodes[l.0].district == "services").map(|l| nodes[l.0].title.clone()).collect();
            nodes[t].folder = match users.as_slice() {
                [] => format!("{db} · not found in services"),
                [one] => format!("{db} · {one}"),
                _ => format!("{db} · shared"),
            };
            nodes[t].body += &format!("\nused by: {}", if users.is_empty() { "no service".into() } else { users.join(", ") });
        }
    }
    (nodes, links)
}

#[cfg(test)]
mod tests {
    #[test]
    fn replays_migrations() {
        let dir = std::env::temp_dir().join(format!("cellworks-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("001_a.js"), "exports.up = k => k.schema.createTable('users', t => { t.increments(); t.string('name'); })\n.createTable('cars', t => { t.integer('owner').references('id').inTable('users'); t.text('old'); });\nexports.down = k => k.schema.dropTable('cars');").unwrap();
        std::fs::write(dir.join("002_b.js"), "exports.up = k => k.schema.table('cars', t => { t.dropColumn('old'); t.renameColumn('owner', 'user_id'); });\nexports.down = () => {};").unwrap();
        let t = super::replay(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(t["users"].columns.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["id", "name"]);
        assert_eq!(t["cars"].columns.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["user_id"]);
        assert!(t["cars"].refs.contains("users"));
    }

    #[test]
    #[ignore] // needs ~/Projects/Accounts/acme
    fn acme_data_loads() {
        let (nodes, links) = super::load(&format!("{}/Projects/Accounts/acme/shopfront-api", std::env::var("HOME").unwrap()));
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for n in &nodes {
            *by.entry(format!("{}/{}", n.district, n.folder)).or_default() += 1;
        }
        let districts: std::collections::BTreeMap<&str, usize> = nodes.iter().fold(Default::default(), |mut m, n| {
            *m.entry(n.district.as_str()).or_default() += 1;
            m
        });
        println!("{districts:?}, {} folders, {} links", by.len(), links.len());
        let svc_links = links.iter().filter(|(a, _)| nodes[*a].district == "services").count();
        println!("service usage links: {svc_links}");
        for (f, n) in by.iter().filter(|(f, _)| f.starts_with("postgres")) {
            println!("  {f}: {n}");
        }
        assert!(districts.get("postgres").is_some_and(|&n| n > 50) && svc_links > 0);
    }
}
