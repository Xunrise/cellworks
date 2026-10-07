// Scans a project into a world: context cells and code, laid out as factories.
use crate::ast;
use eframe::egui::{Pos2, Rect, Vec2, pos2, vec2};
#[allow(unused_imports)]
use std::f32::consts::TAU;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Cell,
    Code,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tag {
    Source,
    Ours,
    Open,
    Code,
    None,
}

/// Index of a section: (factory, section).
pub type SecId = (usize, usize);

pub struct Item {
    pub term: String,
    pub text: String,
    pub concept: bool,
    pub uses: Vec<SecId>,
    pub cx: u32,
    pub line: u32,
    pub loc: u32,
    /// position inside its section
    pub off: Vec2,
    pub rect: Rect,
}

pub struct Metrics {
    pub lang: &'static str,
    pub loc: u32,
    pub funcs: u32,
    pub cx_max: u32,
    pub cx_sum: u32,
}

pub struct Link {
    pub to: (usize, Option<usize>),
    pub why: String,
}

/// A file a servitor is writing: its lines appear one by one; `done_at` is the animation time it finished.
pub struct Build {
    pub lines: Vec<String>,
    pub progress: f32,
    pub done_at: Option<f32>,
}

pub struct Section {
    pub title: String,
    pub tag: Tag,
    pub items: Vec<Item>,
    pub body: String,
    pub open: bool,
    pub path: Option<String>,
    pub rel: Option<String>,
    pub lines: usize,
    pub metrics: Option<Metrics>,
    pub deps: Vec<SecId>,
    pub dependents: Vec<SecId>,
    pub calls_out: Vec<(SecId, u32)>,
    pub calls_in: Vec<(SecId, u32)>,
    pub links: Vec<Link>,
    /// position inside its factory, animated towards `goal_off`
    pub off: Vec2,
    pub vel: Vec2,
    pub rect: Rect,
    pub heat: f32,
    /// being written by a servitor (a construction site)
    pub build: Option<Build>,
    /// lines a servitor just wrote into this existing file
    pub edit: Option<Build>,
    /// for tickets: 0 to do, 1 in progress, 2 done; for workloads: 0 idle, 1 degraded, 2 healthy
    pub state: Option<u8>,
    /// line to open the file at, when the section is a block inside a bigger file
    pub line: u32,
    /// folder with the code this stands for, in whichever project holds it
    pub code: Option<String>,
}

pub struct Fac {
    pub id: String,
    pub kind: Kind,
    pub title: String,
    pub path: String,
    pub status: Option<String>,
    pub age: i64,
    pub lines: usize,
    pub sections: Vec<Section>,
    pub refs: Vec<usize>,
    pub links: Vec<Link>,
    pub rect: Rect,
    pub vel: Vec2,
    pub pinned: bool,
    /// temperature of the file layout inside this factory
    pub alpha: f32,
    /// files pulled to the middle: the focused file first, then its relatives
    pub focus_secs: Vec<usize>,
    /// file-to-file springs inside this factory
    pub inner: Vec<(usize, usize, f32)>,
    /// index into `World::districts`
    pub district: usize,
    /// name relative to its district, for labels
    pub short: String,
    /// index into `World::subdistricts`, or `usize::MAX` when the district is not split
    pub sub: usize,
    /// a folder that only exists because a servitor is creating files in it
    pub building: bool,
}

pub struct World {
    pub repo: String,
    pub facs: Vec<Fac>,
    pub belts: Vec<(usize, usize)>,
    /// code factory -> code factory it imports from, with the number of file imports
    pub code_belts: Vec<(usize, usize, u32)>,
    /// cell factory -> code factory, with the number of links
    pub pipes: Vec<(usize, usize, u32)>,
    /// absolute path -> (factory, section)
    pub by_path: HashMap<String, (usize, Option<usize>)>,
    pub n_cells: usize,
    /// simulation temperature: forces scale with it, the layout rests at 0
    pub alpha: f32,
    pub focus: Option<usize>,
    pub near: HashSet<usize>,
    /// file being edited: it and its factory hold still
    pub frozen: Option<SecId>,
    /// top-level regions of the map: folders below the common prefix, plus the context cells
    pub districts: Vec<String>,
    /// home position of each district; its factories are pulled towards it
    pub anchors: Vec<Pos2>,
    /// home of each sub-district relative to its district's anchor; index `subdistricts.len() + d` holds
    /// the folders of district `d` that are in no sub-district
    pub sub_home: Vec<Vec2>,
    /// (district, folder name) of the sub-regions inside large districts
    pub subdistricts: Vec<(usize, String)>,
    /// grouping currently laid out: 0 folders, 1 connections, 2 git changes
    pub organize: u8,
}

fn section(title: String, tag: Tag, body: String) -> Section {
    Section {
        lines: body.lines().count(), title, tag, items: vec![], body, open: false, path: None, rel: None, metrics: None,
        deps: vec![], dependents: vec![], calls_out: vec![], calls_in: vec![], links: vec![], off: Vec2::ZERO, vel: Vec2::ZERO, rect: Rect::NOTHING, heat: 0.0, build: None, edit: None, state: None, line: 0, code: None,
    }
}

fn item(term: String, text: String) -> Item {
    Item { term, text, concept: false, uses: vec![], cx: 0, line: 0, loc: 0, off: Vec2::ZERO, rect: Rect::NOTHING }
}

const SKIP: &[&str] = &[".git", "node_modules", ".venv", "dist", "build", ".next", "__pycache__", "target", "worktrees", "vendor"];
const BINARY: &[&str] = &["png", "jpg", "jpeg", "gif", "ico", "ttf", "woff", "woff2", "pdf", "zip", "webp", "mp4", "icns", "svg"];
const STOP_CALLS: &[&str] = &[
    "get", "set", "map", "push", "log", "then", "init", "run", "main", "new", "toString", "filter", "join", "split", "len", "print", "format",
    "append", "error", "warn", "info", "debug", "keys", "values", "has", "add", "find", "test", "describe", "expect", "it", "echo", "cd", "require",
];

fn skipped(rel: &str, ignore: &[String]) -> bool {
    rel.split('/').any(|c| ignore.iter().any(|i| i == c)) || skipped_always(rel)
}

fn skipped_always(rel: &str) -> bool {
    rel.split('/').any(|c| SKIP.contains(&c)) || rel.starts_with(".delta/")
}

fn list_files(repo: &Path, ignore: &[String]) -> Vec<String> {
    if let Ok(out) = Command::new("git").arg("-C").arg(repo).args(["ls-files", "-co", "--exclude-standard"]).output() {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).lines().filter(|r| !skipped(r, ignore)).map(String::from).collect();
        }
    }
    let mut out = Vec::new(); // not a git repo
    walk(repo, repo, ignore, &mut out);
    out
}

fn walk(root: &Path, dir: &Path, ignore: &[String], out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        if out.len() > 3000 {
            return;
        }
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !SKIP.contains(&name.as_str()) && !ignore.contains(&name) {
                walk(root, &p, ignore, out);
            }
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_string_lossy().to_string());
        }
    }
}

fn find_cells_dir(dir: &Path, depth: usize) -> Option<PathBuf> {
    if depth > 5 {
        return None;
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if SKIP.contains(&name.as_str()) || name == ".claude" || name == ".delta" {
            continue;
        }
        if name == "context-cells" || (name == "contexts" && p.parent().is_some_and(|d| d.ends_with("domain"))) {
            return Some(p);
        }
        if let Some(found) = find_cells_dir(&p, depth + 1) {
            return Some(found);
        }
    }
    None
}

pub fn days_since(date: &str) -> Option<i64> {
    let mut it = date.split('-').map(|x| x.parse::<i64>());
    let (y, m, d) = (it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?);
    // days from civil (Howard Hinnant)
    let civil = |y: i64, m: i64, d: i64| {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy
    };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64 / 86400;
    Some(now - (civil(y, m, d) - civil(1970, 1, 1)))
}

fn words(s: &str) -> HashSet<&str> {
    s.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':')).filter(|w| !w.is_empty()).collect()
}

/// Lowercase words of a text, splitting identifiers at camelCase and snake_case boundaries.
fn word_seq(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        cur.extend(c.to_lowercase());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// A glossary term and the section that defines it.
struct Term {
    term: String,
    def: String,
    at: SecId,
}

fn parse_cells(dir: &Path) -> (Vec<Fac>, Vec<Term>) {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("cell-")) && p.extension().is_some_and(|x| x == "md")).collect())
        .unwrap_or_default();
    files.sort();
    let ids: Vec<String> = files.iter().map(|p| p.file_stem().unwrap().to_string_lossy().to_string()).collect();
    let status_re = Regex::new(r"Status: (\d{4}-\d{2}-\d{2})").unwrap();
    let ref_re = Regex::new(r"cell-[a-z-]+").unwrap();
    let mut glossary: Vec<Term> = Vec::new();
    let mut facs = Vec::new();
    for (fi, (p, id)) in files.iter().zip(&ids).enumerate() {
        let text = std::fs::read_to_string(p).unwrap_or_default();
        let status = status_re.captures(&text).map(|c| c[1].to_string());
        let mut sections = Vec::new();
        for (si, block) in text.split("\n## ").skip(1).enumerate() {
            let (head, body) = block.split_once('\n').unwrap_or((block, ""));
            let mut s = section(head.trim().to_string(), Tag::None, body.trim().to_string());
            for line in body.lines().filter_map(|l| l.strip_prefix("- ")) {
                let (term, rest) = match line.split_once(": ") {
                    Some((t, r)) if t.len() <= 45 => {
                        if id.contains("glossary") {
                            for t in t.split(", ") {
                                glossary.push(Term { term: t.split(" (").next().unwrap().trim().to_string(), def: r.to_string(), at: (fi, si) });
                            }
                        }
                        (t.to_string(), r.to_string())
                    }
                    _ => (line.split_whitespace().take(5).collect::<Vec<_>>().join(" ") + "…", line.to_string()),
                };
                s.items.push(item(term.trim_matches(|c| c == '*' || c == '`').to_string(), rest));
            }
            s.tag = if head.contains("OPEN") { Tag::Open } else if head.contains("OURS") { Tag::Ours } else if head.contains("SOURCE") { Tag::Source } else { Tag::None };
            s.open = head.to_lowercase().starts_with("open");
            sections.push(s);
        }
        let mut refs: Vec<usize> = ref_re.find_iter(&text).filter_map(|m| ids.iter().position(|i| i == m.as_str())).filter(|&i| ids[i] != *id).collect();
        refs.sort();
        refs.dedup();
        let title = text.lines().find_map(|l| l.strip_prefix("# ")).unwrap_or(id).to_string();
        facs.push(Fac {
            id: id.clone(), kind: Kind::Cell, title, path: p.to_string_lossy().to_string(), age: status.as_deref().and_then(days_since).unwrap_or(999), status,
            lines: text.lines().count(), sections, refs, links: vec![], rect: Rect::NOTHING, vel: Vec2::ZERO, pinned: false, alpha: 0.6, focus_secs: vec![], inner: vec![], district: 0, short: String::new(), sub: usize::MAX, building: false,
        });
    }
    link_cells(&mut facs, &glossary);
    (facs, glossary)
}

/// Prose sections get the glossary concepts they mention as crates, and every term learns which other
/// sections mention it.
fn link_cells(facs: &mut [Fac], glossary: &[Term]) {
    for f in facs.iter_mut() {
        for s in &mut f.sections {
            if s.items.is_empty() {
                let w = words(&s.body);
                s.items = glossary.iter().filter(|g| g.term.len() > 2 && (w.contains(g.term.as_str()) || (g.term.contains(' ') && s.body.contains(g.term.as_str()))))
                    .take(16).map(|g| Item { concept: true, ..item(g.term.clone(), g.def.clone()) }).collect();
            }
        }
    }
    // usages: every other section whose text mentions the term
    let bodies: Vec<Vec<(String, HashSet<String>)>> = facs.iter().map(|f| f.sections.iter().map(|s| (s.body.clone(), words(&s.body).into_iter().map(String::from).collect())).collect()).collect();
    for (fi, f) in facs.iter_mut().enumerate() {
        for (si, s) in f.sections.iter_mut().enumerate() {
            for it in &mut s.items {
                if it.term.len() < 3 || it.term.ends_with('…') {
                    continue;
                }
                for (fj, secs) in bodies.iter().enumerate() {
                    for (sj, (body, w)) in secs.iter().enumerate() {
                        if (fj, sj) != (fi, si) && (w.contains(&it.term) || (it.term.contains(' ') && body.contains(&it.term))) {
                            it.uses.push((fj, sj));
                        }
                    }
                }
            }
        }
    }
}

/// Rows of the markdown tables in `md` as (first column, second column), without header and separator rows.
fn table_rows(md: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = md.lines().collect();
    let mut out = vec![];
    for (i, l) in lines.iter().enumerate() {
        let l = l.trim();
        if !l.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = l.trim_matches('|').split('|').map(|c| c.trim().trim_matches('`').to_string()).collect();
        let separator = cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '));
        let header = lines.get(i + 1).is_some_and(|n| n.trim().starts_with('|') && n.contains("---"));
        if separator || header || cells.len() < 2 || cells[0].is_empty() {
            continue;
        }
        out.push((cells[0].clone(), cells[1].clone()));
    }
    out
}

/// YYYY-MM-DD of a day number since 1970-01-01 (civil from days, Howard Hinnant).
fn date_of(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

/// A jarvis domain model (`docs/domain/contexts/<Name>/<Name>.md` plus glossaries) as cells: one per bounded
/// context, its `##` headings as sections, its glossary table as a section of terms. Freshness is the file age.
/// Contexts are either folders (`<Name>/<Name>.md` with a `glossary.md` beside it) or flat `<name>.md`
/// files; a shared `glossary.md` next to `contexts/` becomes a cell of its own.
fn parse_jarvis(dir: &Path) -> (Vec<Fac>, Vec<Term>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    entries.sort();
    // (id, main file, glossary file)
    let mut cells: Vec<(String, PathBuf, PathBuf)> = entries.iter().filter_map(|p| {
        if p.is_dir() {
            let id = p.file_name()?.to_string_lossy().to_string();
            Some((id.clone(), p.join(format!("{id}.md")), p.join("glossary.md")))
        } else if p.extension().is_some_and(|e| e == "md") && !p.file_stem().is_some_and(|s| s.eq_ignore_ascii_case("readme")) {
            Some((p.file_stem()?.to_string_lossy().to_string(), p.clone(), PathBuf::new()))
        } else {
            None
        }
    }).collect();
    if let Some(shared) = dir.parent().map(|d| d.join("glossary.md")).filter(|g| g.is_file()) {
        cells.push(("glossary".into(), shared.clone(), shared));
    }
    let ids: Vec<String> = cells.iter().map(|c| c.0.clone()).collect();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64 / 86_400);
    let (mut facs, mut glossary) = (vec![], vec![]);
    for (id, main, gloss) in &cells {
        let Ok(text) = std::fs::read_to_string(main) else { continue };
        let fi = facs.len();
        let mut sections = Vec::new();
        for block in text.split("\n## ").skip(1) {
            let (head, body) = block.split_once('\n').unwrap_or((block, ""));
            let mut s = section(head.trim().to_string(), Tag::None, body.trim().to_string());
            for line in body.lines().filter_map(|l| l.trim_start().strip_prefix("- ")) {
                let (term, rest) = match line.split_once(" — ").or_else(|| line.split_once(": ")) {
                    Some((t, r)) if t.len() <= 45 => (t.to_string(), r.to_string()),
                    _ => (line.split_whitespace().take(5).collect::<Vec<_>>().join(" ") + "…", line.to_string()),
                };
                s.items.push(item(term.trim_matches(|c| c == '*' || c == '`').to_string(), rest));
            }
            s.open = head.to_lowercase().starts_with("open");
            sections.push(s);
        }
        if let Ok(g) = std::fs::read_to_string(gloss) {
            let rows = table_rows(&g);
            if !rows.is_empty() {
                let mut s = section("Glossary".into(), Tag::Source, g.clone());
                for (term, def) in rows {
                    let term = term.trim_matches(|c| c == '*' || c == '`').to_string();
                    glossary.push(Term { term: term.clone(), def: def.clone(), at: (fi, sections.len()) });
                    s.items.push(item(term, def));
                }
                sections.push(s);
            }
        }
        let refs: Vec<usize> = ids.iter().enumerate().filter(|(j, other)| *other != id && text.contains(other.as_str()) && *j < cells.len()).map(|(j, _)| j).collect();
        let modified = std::fs::metadata(main).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(now, |d| d.as_secs() as i64 / 86_400);
        let title = text.lines().find_map(|l| l.strip_prefix("# ")).map(|t| t.split_once("Context:").map_or(t, |x| x.1).trim().to_string()).unwrap_or_else(|| id.clone());
        facs.push(Fac {
            id: id.clone(), kind: Kind::Cell, title, path: main.to_string_lossy().to_string(), status: Some(date_of(modified)), age: now - modified,
            lines: text.lines().count(), sections, refs, links: vec![], rect: Rect::NOTHING, vel: Vec2::ZERO, pinned: false, alpha: 0.6,
            focus_secs: vec![], inner: vec![], district: 0, short: String::new(), sub: usize::MAX, building: false,
        });
    }
    // refs were indices into the folder list; contexts without a file were skipped, so map them by id
    let present: Vec<String> = facs.iter().map(|f| f.id.clone()).collect();
    for f in &mut facs {
        f.refs = f.refs.iter().filter_map(|&j| present.iter().position(|id| *id == ids[j])).collect();
    }
    link_cells(&mut facs, &glossary);
    (facs, glossary)
}

/// One factory per directory, split deeper until a factory holds at most 25 files.
fn group(files: Vec<String>, depth: usize, out: &mut Vec<(String, Vec<String>)>) {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for rel in files {
        let parts: Vec<&str> = rel.split('/').collect();
        let key = if parts.len() > depth + 1 { parts[..=depth].join("/") } else if parts.len() > 1 { parts[..parts.len() - 1].join("/") } else { "(root)".into() };
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => g.1.push(rel),
            None => groups.push((key, vec![rel])),
        }
    }
    for (k, v) in groups {
        if v.len() > 25 && depth < 4 && v.iter().any(|r| r.matches('/').count() > depth + 1) {
            group(v, depth + 1, out);
        } else {
            out.push((k, v));
        }
    }
}

fn strip_ext(rel: &str) -> &str {
    match rel.rfind('.') {
        Some(i) if !rel[i..].contains('/') && i > rel.rfind('/').map_or(0, |j| j + 1) => &rel[..i],
        _ => rel,
    }
}

/// Resolves an import specifier written in `from` to a file of the repo.
fn resolve_import(spec: &str, from: &str, lang: &str, by_stem: &HashMap<String, SecId>, by_last: &HashMap<String, Vec<String>>) -> Option<SecId> {
    let dir = from.rsplit_once('/').map_or("", |x| x.0);
    let join = |base: &str, rest: &str| {
        let mut parts: Vec<&str> = if base.is_empty() { vec![] } else { base.split('/').collect() };
        for p in rest.split('/') {
            match p {
                "." | "" => {}
                ".." => { parts.pop(); }
                p => parts.push(p),
            }
        }
        parts.join("/")
    };
    let lookup = |p: &str| {
        let p = strip_ext(p);
        [p.to_string(), format!("{p}/index"), format!("{p}/mod"), format!("{p}/__init__")].into_iter().find_map(|k| by_stem.get(&k).copied())
    };
    if spec.starts_with("./") || spec.starts_with("../") {
        return lookup(&join(dir, spec));
    }
    if lang == "py" && spec.starts_with('.') {
        let ups = spec.chars().take_while(|&c| c == '.').count();
        let mut base = dir.to_string();
        for _ in 1..ups {
            base = base.rsplit_once('/').map_or(String::new(), |x| x.0.to_string());
        }
        return lookup(&join(&base, &spec[ups..].replace('.', "/")));
    }
    let mut path = spec.trim_start_matches('\\').replace("::", "/").replace('\\', "/");
    if lang == "py" || lang == "lua" {
        path = path.replace('.', "/");
    }
    if lang == "rs" {
        if let Some(rest) = path.strip_prefix("self/") {
            return lookup(&join(dir, rest)).or_else(|| lookup(&join(&format!("{dir}/{}", from.rsplit('/').next().map(strip_ext).unwrap_or("")), rest)));
        }
        path = path.trim_start_matches("crate/").trim_start_matches("super/").to_string();
    }
    if lang == "sh" {
        return lookup(&join(dir, &path)).or_else(|| lookup(&path));
    }
    // suffix match, dropping trailing segments (`use crate::a::Thing` names a.rs)
    let mut segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty() && *s != "{" ).collect();
    while !segs.is_empty() {
        let p = segs.join("/");
        let p = strip_ext(&p);
        let last = p.rsplit('/').next().unwrap_or(p);
        let hit = by_last.get(last).into_iter().flatten().filter(|cand| cand.as_str() == p || cand.ends_with(&format!("/{p}"))).min_by_key(|c| c.len());
        if let Some(c) = hit {
            return by_stem.get(c).copied();
        }
        if let Some(h) = lookup(p) {
            return Some(h);
        }
        segs.pop();
    }
    None
}

pub fn scan(repo: &str, ignore: &[String]) -> World {
    if let Some(root) = crate::otc::parse(repo) {
        let (nodes, links) = crate::otc::load(root);
        return graph_world(repo, &nodes, &links);
    }
    if let Some(root) = crate::data::parse(repo) {
        let (nodes, links) = crate::data::load(root);
        return graph_world(repo, &nodes, &links);
    }
    if let Some((site, key)) = crate::jira::parse(repo) {
        return jira_world(repo, &crate::jira::load(site, key).unwrap_or_default());
    }
    let repo_p = Path::new(repo);
    let knowledge = find_cells_dir(repo_p, 0);
    let (mut facs, glossary) = match &knowledge {
        Some(d) if d.ends_with("context-cells") => parse_cells(d),
        Some(d) => parse_jarvis(d),
        None => Default::default(),
    };
    // the knowledge files are cells, not code
    let knowledge_rel = knowledge.as_ref().and_then(|d| d.strip_prefix(repo_p).ok()).map(|d| format!("{}/", d.to_string_lossy()));
    let n_cells = facs.len();
    let cell_paths: HashSet<String> = facs.iter().map(|f| f.path.clone()).collect();
    let mut texts: HashMap<String, String> = HashMap::new();
    for rel in list_files(repo_p, ignore) {
        let p = repo_p.join(&rel);
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        if cell_paths.contains(&p.to_string_lossy().to_string()) || knowledge_rel.as_ref().is_some_and(|k| rel.starts_with(k.as_str())) || BINARY.contains(&ext.as_str()) || rel.contains("lock") {
            continue;
        }
        if std::fs::metadata(&p).is_ok_and(|m| m.is_file() && m.len() < 400_000) {
            if let Ok(t) = std::fs::read_to_string(&p) {
                texts.insert(rel, t);
            }
        }
    }

    // ---- code factories: one per directory, one machine per file, crates = functions from the AST ----
    let mut rels: Vec<String> = texts.keys().cloned().collect();
    rels.sort();
    let mut groups = Vec::new();
    group(rels, 0, &mut groups);
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    let mut asts: Vec<(SecId, ast::FileAst)> = Vec::new();
    for (g, rels) in groups {
        let fi = facs.len();
        let mut sections = Vec::new();
        for rel in rels {
            let t = &texts[&rel];
            let ext = rel.rsplit('.').next().unwrap_or("").to_lowercase();
            let mut s = section(rel.rsplit('/').next().unwrap().to_string(), Tag::Code, t.chars().take(4000).collect());
            s.lines = t.lines().count();
            s.path = Some(repo_p.join(&rel).to_string_lossy().to_string());
            s.rel = Some(rel.clone());
            if let Some(a) = ast::analyze(&ext, t) {
                let mut funcs: Vec<&ast::Func> = a.funcs.iter().collect();
                funcs.sort_by_key(|f| std::cmp::Reverse(f.cx));
                s.items = funcs.iter().take(48).map(|f| Item {
                    cx: f.cx, line: f.line, loc: f.loc, ..item(f.name.clone(), format!("line {} · {} lines · complexity {} · {} params", f.line, f.loc, f.cx, f.params))
                }).collect();
                s.metrics = Some(Metrics {
                    lang: a.lang, loc: t.lines().filter(|l| !l.trim().is_empty()).count() as u32, funcs: a.funcs.len() as u32,
                    cx_max: a.funcs.iter().map(|f| f.cx).max().unwrap_or(0), cx_sum: a.funcs.iter().map(|f| f.cx).sum(),
                });
                asts.push(((fi, sections.len()), a));
            }
            sections.push(s);
        }
        let lines = sections.iter().map(|s| s.lines).sum::<usize>() / 4;
        let path = if g == "(root)" { repo.to_string() } else { repo_p.join(&g).to_string_lossy().to_string() };
        facs.push(Fac { id: g.clone(), kind: Kind::Code, title: g, path, status: None, age: 0, lines, sections, refs: vec![], links: vec![], rect: Rect::NOTHING, vel: Vec2::ZERO, pinned: false, alpha: 0.6, focus_secs: vec![], inner: vec![], district: 0, short: String::new(), sub: usize::MAX, building: false });
    }
    let mut by_path = HashMap::new();
    let mut by_stem: HashMap<String, SecId> = HashMap::new();
    let mut by_last: HashMap<String, Vec<String>> = HashMap::new();
    for (fi, f) in facs.iter().enumerate() {
        by_path.insert(f.path.clone(), (fi, None));
        for (si, s) in f.sections.iter().enumerate() {
            if let Some(p) = &s.path {
                by_path.insert(p.clone(), (fi, Some(si)));
            }
            if let Some(r) = &s.rel {
                let stem = strip_ext(r).to_string();
                by_last.entry(stem.rsplit('/').next().unwrap().to_string()).or_default().push(stem.clone());
                by_stem.insert(stem, (fi, si));
            }
        }
    }

    // ---- dependencies: imports resolved to files ----
    for ((fi, si), a) in &asts {
        let rel = facs[*fi].sections[*si].rel.clone().unwrap();
        let mut deps: Vec<SecId> = a.imports.iter().filter_map(|i| resolve_import(i, &rel, a.lang, &by_stem, &by_last)).filter(|d| d != &(*fi, *si)).collect();
        deps.sort();
        deps.dedup();
        facs[*fi].sections[*si].deps = deps;
    }
    // ---- interactions: calls to functions defined in another file ----
    let mut defs: HashMap<String, Vec<SecId>> = HashMap::new();
    for (id, a) in &asts {
        for f in &a.funcs {
            let last = f.name.rsplit('.').next().unwrap_or(&f.name).to_string();
            let v = defs.entry(last).or_default();
            if !v.contains(id) {
                v.push(*id);
            }
        }
    }
    let mut item_uses: Vec<(SecId, String, SecId)> = Vec::new();
    for (id, a) in &asts {
        let deps = facs[id.0].sections[id.1].deps.clone();
        let mut out: HashMap<SecId, u32> = HashMap::new();
        for (name, n) in &a.calls {
            if name.len() < 3 || STOP_CALLS.contains(&name.as_str()) {
                continue;
            }
            let Some(cands) = defs.get(name) else { continue };
            if cands.contains(id) {
                continue;
            }
            let imported: Vec<&SecId> = cands.iter().filter(|c| deps.contains(c)).collect();
            let target = match (imported.as_slice(), cands.as_slice()) {
                ([one], _) => **one,
                ([], [one]) => *one,
                _ => continue,
            };
            *out.entry(target).or_default() += n;
            item_uses.push((target, name.clone(), *id));
        }
        let mut out: Vec<(SecId, u32)> = out.into_iter().collect();
        out.sort_by_key(|x| std::cmp::Reverse(x.1));
        facs[id.0].sections[id.1].calls_out = out;
    }
    for (target, name, caller) in item_uses {
        for it in facs[target.0].sections[target.1].items.iter_mut().filter(|it| it.term.rsplit('.').next() == Some(name.as_str())) {
            if !it.uses.contains(&caller) {
                it.uses.push(caller);
            }
        }
    }
    let n = facs.len();
    for fi in 0..n {
        for si in 0..facs[fi].sections.len() {
            for d in facs[fi].sections[si].deps.clone() {
                facs[d.0].sections[d.1].dependents.push((fi, si));
            }
            for (d, c) in facs[fi].sections[si].calls_out.clone() {
                facs[d.0].sections[d.1].calls_in.push(((fi, si), c));
            }
        }
    }

    // ---- links between code and context cells ----
    if n_cells > 0 {
        let cell_re = Regex::new(r"cell-[a-z-]+").unwrap();
        let adr_re = Regex::new(r"ADR-0*(\d+)").unwrap();
        let path_re = Regex::new(r"[\w@./-]+\.(?:ts|tsx|js|jsx|py|php|rs|sh|lua|go|sql|ya?ml|json)\b").unwrap();
        let mut adrs: HashMap<u32, SecId> = HashMap::new();
        for (fi, f) in facs[..n_cells].iter().enumerate() {
            for (si, s) in f.sections.iter().enumerate() {
                if let Some(c) = adr_re.captures(&s.title).filter(|c| c.get(0).unwrap().start() == 0) {
                    adrs.insert(c[1].parse().unwrap_or(0), (fi, si));
                }
            }
        }
        // terms match as whole word sequences (cardState, card_state and "card state" all match "Card state");
        // terms found in more than a quarter of the files are too generic to link
        let seqs: Vec<(SecId, Vec<String>)> = facs.iter().enumerate().skip(n_cells)
            .flat_map(|(fi, f)| f.sections.iter().enumerate().map(move |(si, s)| ((fi, si), s.rel.clone().unwrap())))
            .map(|(id, rel)| (id, word_seq(&texts[&rel]))).collect();
        let mut seen_terms = HashSet::new();
        let term_hits: Vec<(&Term, Vec<SecId>)> = glossary.iter().filter(|g| g.term.chars().count() >= 3 && seen_terms.insert(g.term.clone())).map(|g| {
            let tw = word_seq(&g.term);
            (g, seqs.iter().filter(|(_, sq)| !tw.is_empty() && sq.windows(tw.len()).any(|win| win == tw.as_slice())).map(|(id, _)| *id).collect())
        }).collect();
        let max_df = (seqs.len() / 4).max(3);
        let mut code_links: Vec<(SecId, (usize, Option<usize>), String)> = Vec::new();
        let mut per_file: HashMap<SecId, HashSet<SecId>> = HashMap::new();
        for (g, files) in &term_hits {
            // one-word terms ("File", "User") are common words in code: they must be rarer to count as a link
            let limit = if g.term.split_whitespace().count() == 1 { (seqs.len() / 20).max(3) } else { max_df };
            if files.len() > limit {
                continue;
            }
            for id in files {
                let seen = per_file.entry(*id).or_default();
                if seen.len() < 12 && seen.insert(g.at) {
                    code_links.push((*id, (g.at.0, Some(g.at.1)), format!("term “{}”", g.term)));
                }
            }
        }
        for (fi, f) in facs.iter().enumerate().skip(n_cells) {
            for (si, s) in f.sections.iter().enumerate() {
                let raw = &texts[s.rel.as_ref().unwrap()];
                let seen = per_file.entry((fi, si)).or_default();
                for m in cell_re.find_iter(raw) {
                    if let Some(cf) = facs[..n_cells].iter().position(|c| c.id == m.as_str()) {
                        if seen.insert((cf, usize::MAX)) {
                            code_links.push(((fi, si), (cf, None), format!("mentions {}", m.as_str())));
                        }
                    }
                }
                for c in adr_re.captures_iter(raw) {
                    if let Some(&at) = c[1].parse().ok().and_then(|k: u32| adrs.get(&k)) {
                        if seen.insert(at) {
                            code_links.push(((fi, si), (at.0, Some(at.1)), format!("mentions ADR-{}", &c[1])));
                        }
                    }
                }
            }
        }
        // cells that name a file of the repo
        for (cf, c) in facs[..n_cells].iter().enumerate() {
            for (cs, s) in c.sections.iter().enumerate() {
                for m in path_re.find_iter(&s.body) {
                    let p = strip_ext(m.as_str().trim_start_matches("./"));
                    let last = p.rsplit('/').next().unwrap_or(p);
                    if let Some(c) = by_last.get(last).into_iter().flatten().find(|c| c.as_str() == p || c.ends_with(&format!("/{p}"))) {
                        code_links.push((by_stem[c], (cf, Some(cs)), format!("named in {}", facs[cf].id)));
                    }
                }
            }
        }
        let mut pipes: HashMap<(usize, usize), u32> = HashMap::new();
        for ((fi, si), (cf, cs), why) in code_links {
            *pipes.entry((cf, fi)).or_default() += 1;
            let back = Link { to: (fi, Some(si)), why: why.clone() };
            match cs {
                Some(cs) => {
                    let sec = &mut facs[cf].sections[cs];
                    if !sec.links.iter().any(|l| l.to == back.to) {
                        sec.links.push(back);
                    }
                    // concept crates trace into the code that uses the term
                    let term = why.strip_prefix("term “").and_then(|t| t.strip_suffix('”')).map(String::from);
                    if let Some(term) = term {
                        for f in facs[..n_cells].iter_mut() {
                            for s in &mut f.sections {
                                for it in s.items.iter_mut().filter(|it| it.term == term) {
                                    if !it.uses.contains(&(fi, si)) {
                                        it.uses.push((fi, si));
                                    }
                                }
                            }
                        }
                    }
                }
                None => facs[cf].links.push(back),
            }
            facs[fi].sections[si].links.push(Link { to: (cf, cs), why });
        }
        let mut w = World { repo: repo.into(), facs, belts: vec![], code_belts: vec![], pipes: pipes.into_iter().map(|((a, b), c)| (a, b, c)).collect(), by_path, n_cells, alpha: 0.6, focus: None, near: HashSet::new(), frozen: None, districts: vec![], anchors: vec![], sub_home: vec![], subdistricts: vec![], organize: 0 };
        finish(&mut w, n_cells);
        return w;
    }
    let mut w = World { repo: repo.into(), facs, belts: vec![], code_belts: vec![], pipes: vec![], by_path, n_cells, alpha: 0.6, focus: None, near: HashSet::new(), frozen: None, districts: vec![], anchors: vec![], sub_home: vec![], subdistricts: vec![], organize: 0 };
    finish(&mut w, n_cells);
    w
}

/// A Jira project as a map: components are districts, epics are folders, tickets are machines (sub-tasks their
/// crates), Jira links are lanes. Done tickets older than 30 days are left out.
pub fn jira_world(path: &str, tickets: &[crate::jira::Ticket]) -> World {
    use crate::jira::Ticket;
    let keep = |t: &Ticket| t.cat != "done" || days_since(&t.updated).is_some_and(|d| d <= 30);
    let by_key: HashMap<&str, &Ticket> = tickets.iter().map(|t| (t.key.as_str(), t)).collect();
    let is_epic = |k: &str| by_key.get(k).is_some_and(|t| t.level >= 1);
    let clean = |s: &str| s.replace('/', "∕");
    let comp = |t: &Ticket| t.components.first().cloned().unwrap_or_else(|| "no component".into());
    // tickets per folder: their epic, or "(no epic)" in their component
    let mut folders: Vec<(String, Option<&Ticket>, Vec<&Ticket>)> = vec![];
    let mut slot: HashMap<String, usize> = HashMap::new();
    let mut items: HashMap<&str, Vec<&Ticket>> = HashMap::new();
    for t in tickets.iter().filter(|t| keep(t)) {
        let parent_ticket = t.parent.as_deref().filter(|p| !is_epic(p) && by_key.get(p).is_some_and(|p| keep(p)));
        if let (Some(p), true) = (parent_ticket, t.level < 0) {
            items.entry(by_key[p].key.as_str()).or_default().push(t); // a sub-task is a crate of its ticket
            continue;
        }
        let key = match (t.level >= 1, t.parent.as_deref().filter(|p| is_epic(p))) {
            (true, _) => t.key.clone(),
            (false, Some(e)) => e.to_string(),
            _ => format!("(no epic) {}", comp(t)),
        };
        let i = *slot.entry(key.clone()).or_insert_with(|| {
            folders.push((key.clone(), by_key.get(key.as_str()).copied().filter(|e| e.level >= 1), vec![]));
            folders.len() - 1
        });
        if t.level >= 1 {
            folders[i].2.insert(0, t); // the epic's own card comes first
        } else {
            folders[i].2.push(t);
        }
    }
    let state = |t: &Ticket| match t.cat.as_str() {
        "done" => 2,
        "indeterminate" => 1,
        _ => 0,
    };
    let mut facs = vec![];
    let mut at: HashMap<String, SecId> = HashMap::new();
    for (fi, (key, epic, ts)) in folders.iter().enumerate() {
        // an epic sits in the component most of its tickets belong to
        let mut votes: HashMap<String, usize> = HashMap::new();
        for t in ts.iter().chain(epic) {
            *votes.entry(comp(t)).or_default() += 1;
        }
        let district = epic.filter(|e| !e.components.is_empty()).map(|e| comp(e))
            .or_else(|| votes.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|v| v.0)).unwrap_or_default();
        let title = epic.map_or(key.clone(), |e| format!("{} {}", e.key, e.summary));
        let sections: Vec<Section> = ts.iter().enumerate().map(|(si, t)| {
            at.insert(t.key.clone(), (fi, si));
            let mut body = format!("{} · {} · {}\n", t.kind, t.status, t.assignee.as_deref().unwrap_or("unassigned"));
            if !t.labels.is_empty() {
                body += &format!("labels: {}\n", t.labels.join(", "));
            }
            for (how, other) in &t.links {
                body += &format!("{how} {other}\n");
            }
            let mut s = section(format!("{} {}", t.key, t.summary), Tag::Code, body);
            s.state = Some(state(t));
            s.lines = 30;
            s.items = items.get(t.key.as_str()).into_iter().flatten().map(|st| {
                item(format!("{} {}", st.key, st.summary), format!("{} · {}", st.status, st.assignee.as_deref().unwrap_or("unassigned")))
            }).collect();
            s
        }).collect();
        facs.push(Fac {
            id: format!("{}/{}", clean(&district), clean(&title)), kind: Kind::Code, title: clean(&title), path: format!("{path}/{key}"),
            status: None, age: 0, lines: sections.len() * 30, sections, refs: vec![], links: vec![], rect: Rect::NOTHING, vel: Vec2::ZERO,
            pinned: false, alpha: 0.6, focus_secs: vec![], inner: vec![], district: 0, short: String::new(), sub: usize::MAX, building: false,
        });
    }
    // Jira links become dependencies both ways, so lanes and relation lines show them
    for t in tickets {
        let Some(&a) = at.get(&t.key) else { continue };
        for (_, other) in &t.links {
            if let Some(&b) = at.get(other) {
                if a != b && !facs[a.0].sections[a.1].deps.contains(&b) {
                    facs[a.0].sections[a.1].deps.push(b);
                    facs[b.0].sections[b.1].dependents.push(a);
                }
            }
        }
    }
    let mut w = World { repo: path.into(), facs, belts: vec![], code_belts: vec![], pipes: vec![], by_path: HashMap::new(), n_cells: 0, alpha: 0.6, focus: None, near: HashSet::new(), frozen: None, districts: vec![], anchors: vec![], sub_home: vec![], subdistricts: vec![], organize: 0 };
    finish(&mut w, 0);
    w
}

/// Any graph as a map: nodes grouped into district/folder, links as lanes.
pub fn graph_world(path: &str, nodes: &[crate::otc::Node], links: &[(usize, usize)]) -> World {
    let mut folders: Vec<((String, String), Vec<usize>)> = vec![];
    for (i, n) in nodes.iter().enumerate() {
        let key = (n.district.clone(), n.folder.clone());
        match folders.iter_mut().find(|f| f.0 == key) {
            Some(f) => f.1.push(i),
            None => folders.push((key, vec![i])),
        }
    }
    let mut at: Vec<SecId> = vec![(0, 0); nodes.len()];
    let mut facs = vec![];
    for (fi, ((district, folder), members)) in folders.iter().enumerate() {
        let sections: Vec<Section> = members.iter().enumerate().map(|(si, &i)| {
            at[i] = (fi, si);
            let n = &nodes[i];
            let mut s = section(n.title.clone(), Tag::Code, n.body.clone());
            s.state = n.state;
            s.path = n.path.clone();
            s.line = n.line;
            s.code = n.code.clone();
            s.lines = 30;
            s.items = n.items.iter().map(|(t, x)| item(t.clone(), x.clone())).collect();
            s
        }).collect();
        facs.push(Fac {
            id: format!("{district}/{folder}"), kind: Kind::Code, title: folder.clone(), path: format!("{path}/{district}/{folder}"),
            status: None, age: 0, lines: sections.len() * 30, sections, refs: vec![], links: vec![], rect: Rect::NOTHING, vel: Vec2::ZERO,
            pinned: false, alpha: 0.6, focus_secs: vec![], inner: vec![], district: 0, short: String::new(), sub: usize::MAX, building: false,
        });
    }
    for &(a, b) in links {
        let (a, b) = (at[a], at[b]);
        if a != b && !facs[a.0].sections[a.1].deps.contains(&b) {
            facs[a.0].sections[a.1].deps.push(b);
            facs[b.0].sections[b.1].dependents.push(a);
        }
    }
    let mut w = World { repo: path.into(), facs, belts: vec![], code_belts: vec![], pipes: vec![], by_path: HashMap::new(), n_cells: 0, alpha: 0.6, focus: None, near: HashSet::new(), frozen: None, districts: vec![], anchors: vec![], sub_home: vec![], subdistricts: vec![], organize: 0 };
    finish(&mut w, 0);
    w
}

fn finish(w: &mut World, n_cells: usize) {
    for (i, f) in w.facs.iter().enumerate() {
        for &r in &f.refs {
            if !w.belts.contains(&(r, i)) {
                w.belts.push((i, r));
            }
        }
    }
    let mut cb: HashMap<(usize, usize), u32> = HashMap::new();
    for (fi, f) in w.facs.iter().enumerate() {
        for s in &f.sections {
            for d in &s.deps {
                if d.0 != fi {
                    *cb.entry((fi, d.0)).or_default() += 1;
                }
            }
        }
    }
    w.code_belts = cb.into_iter().map(|((a, b), c)| (a, b, c)).collect();
    for (fi, f) in w.facs.iter_mut().enumerate() {
        let mut inner: HashMap<(usize, usize), f32> = HashMap::new();
        for (si, s) in f.sections.iter().enumerate() {
            let pairs = s.deps.iter().map(|d| (*d, 1.0))
                .chain(s.calls_out.iter().map(|(d, c)| (*d, 1.0 + (*c as f32).ln())))
                .chain(s.items.iter().flat_map(|it| it.uses.iter().map(|d| (*d, 0.3))));
            for ((df, ds), w) in pairs {
                if df == fi && ds != si {
                    *inner.entry((si.min(ds), si.max(ds))).or_default() += w;
                }
            }
        }
        for (pair, w) in affinity(f) {
            *inner.entry(pair).or_default() += w;
        }
        f.inner = strongest(inner, 6).into_iter().map(|((a, b), w)| (a, b, w.min(4.0))).collect();
    }
    w.code_belts.sort();
    assign_districts(w, n_cells);
    layout(w);
}

/// Groups factories into districts: the first folder below the prefix all code shares, one level deeper when
/// that leaves a single district. Factory labels become relative to their district.
fn assign_districts(w: &mut World, n_cells: usize) {
    let segs: Vec<Vec<String>> = w.facs[n_cells..].iter().map(|f| if f.id == "(root)" { vec![] } else { f.id.split('/').map(String::from).collect() }).collect();
    let nonroot: Vec<&Vec<String>> = segs.iter().filter(|s| !s.is_empty()).collect();
    let mut common = 0;
    while nonroot.len() > 1 && nonroot.iter().all(|s| s.len() > common + 1 && s[common] == nonroot[0][common]) {
        common += 1;
    }
    let key = |s: &Vec<String>, depth: usize| if s.is_empty() { "(root)".to_string() } else { s[..(depth + 1).min(s.len())].join("/") };
    let mut depth = common;
    while depth < common + 3 && segs.iter().map(|s| key(s, depth)).collect::<HashSet<_>>().len() < 2 && segs.iter().any(|s| s.len() > depth + 1) {
        depth += 1;
    }
    let mut names: Vec<String> = vec![];
    if n_cells > 0 {
        names.push("context cells".into());
    }
    for (f, s) in w.facs[n_cells..].iter_mut().zip(&segs) {
        let k = key(s, depth);
        let idx = names.iter().position(|n| *n == k).unwrap_or_else(|| {
            names.push(k.clone());
            names.len() - 1
        });
        f.district = idx;
        f.short = if s.len() > depth + 1 { s[depth + 1..].join("/") } else { s.last().cloned().unwrap_or_else(|| "(root)".into()) };
    }
    for f in &mut w.facs[..n_cells] {
        f.district = 0;
        f.short = f.title.clone();
    }
    // large districts split once more, by the next folder, when that gives at least two groups
    let mut subs: Vec<(usize, String)> = vec![];
    for d in 0..names.len() {
        let members: Vec<usize> = (n_cells..w.facs.len()).filter(|&i| w.facs[i].district == d).collect();
        if members.len() < 6 {
            continue;
        }
        let first = |s: &str| s.split('/').next().unwrap_or(s).to_string();
        let keys: HashSet<String> = members.iter().map(|&i| first(&w.facs[i].short)).collect();
        if keys.len() < 2 || keys.len() == members.len() {
            continue;
        }
        for i in members {
            let f = &mut w.facs[i];
            let k = first(&f.short);
            let idx = subs.iter().position(|(sd, n)| *sd == d && *n == k).unwrap_or_else(|| {
                subs.push((d, k.clone()));
                subs.len() - 1
            });
            f.sub = idx;
            if let Some((_, rest)) = f.short.split_once('/') {
                f.short = rest.to_string();
            }
        }
    }
    w.subdistricts = subs;
    w.districts = names;
}

const NAME_STOP: &[&str] = &[
    "get", "add", "update", "delete", "create", "remove", "set", "list", "by", "id", "dto", "ts", "tsx", "js", "py", "rs", "php", "base", "spec", "test", "index", "mod",
    "the", "of", "and", "to", "for", "all", "new", "old", "util", "utils", "helper", "helpers", "type", "types",
];

/// Belongs-together weights between files of one folder that do not import each other:
/// used by the same file, importing the same files, and sharing name words.
fn affinity(f: &Fac) -> HashMap<(usize, usize), f32> {
    let n = f.sections.len();
    let mut out: HashMap<(usize, usize), f32> = HashMap::new();
    if n < 2 || n > 400 {
        return out;
    }
    let words: Vec<HashSet<String>> = f.sections.iter().map(|s| {
        s.title.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 1 && !NAME_STOP.contains(w)).map(String::from).collect()
    }).collect();
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&f.sections[i], &f.sections[j]);
            let shared_users = a.dependents.iter().filter(|x| b.dependents.contains(x)).count();
            let shared_deps = a.deps.iter().filter(|x| b.deps.contains(x)).count();
            let common = words[i].intersection(&words[j]).count();
            let union = words[i].union(&words[j]).count().max(1);
            let mut w = 0.7 * shared_users.min(3) as f32 + 0.4 * shared_deps.min(3) as f32;
            if common > 0 {
                w += 1.2 * common as f32 / union as f32;
            }
            if w >= 0.35 {
                out.insert((i, j), w);
            }
        }
    }
    out
}

/// Keeps, for every node, only its `k` strongest edges (an edge survives if either end keeps it).
fn strongest(edges: HashMap<(usize, usize), f32>, k: usize) -> Vec<((usize, usize), f32)> {
    let mut per: HashMap<usize, Vec<(f32, (usize, usize))>> = HashMap::new();
    for (&e, &w) in &edges {
        per.entry(e.0).or_default().push((w, e));
        per.entry(e.1).or_default().push((w, e));
    }
    let mut keep: HashSet<(usize, usize)> = HashSet::new();
    for v in per.values_mut() {
        v.sort_by(|a, b| b.0.total_cmp(&a.0));
        keep.extend(v.iter().take(k).map(|x| x.1));
    }
    edges.into_iter().filter(|(e, _)| keep.contains(e)).collect()
}

/// Grid slot for each of `n` nodes so that related nodes are neighbours: the most connected node takes the middle,
/// then each next node is the one most related to those placed, put in the free slot nearest its relatives.
/// Nodes without relations fill the remaining slots last.
fn grid_order(n: usize, cols: usize, rows: usize, edges: &[(usize, usize, f32)]) -> Vec<usize> {
    let mut adj: Vec<Vec<(usize, f32)>> = vec![vec![]; n];
    for &(a, b, w) in edges {
        if a < n && b < n {
            adj[a].push((b, w));
            adj[b].push((a, w));
        }
    }
    if edges.is_empty() || n > 400 {
        return (0..n).collect();
    }
    let pos = |k: usize| vec2((k % cols) as f32, (k / cols) as f32);
    let centre = vec2((cols as f32 - 1.0) * 0.5, (rows as f32 - 1.0) * 0.5);
    let mut slot_of = vec![usize::MAX; n];
    let mut free: Vec<bool> = vec![true; cols * rows];
    let total: Vec<f32> = adj.iter().map(|v| v.iter().map(|x| x.1).sum()).collect();
    let mut pull = vec![0.0f32; n]; // affinity to already placed nodes
    for _ in 0..n {
        let next = (0..n).filter(|&i| slot_of[i] == usize::MAX).max_by(|&a, &b| (pull[a], total[a]).partial_cmp(&(pull[b], total[b])).unwrap()).unwrap();
        let placed_rel: Vec<(Vec2, f32)> = adj[next].iter().filter(|(o, _)| slot_of[*o] != usize::MAX).map(|(o, w)| (pos(slot_of[*o]), *w)).collect();
        let target = if placed_rel.is_empty() {
            // unrelated to anything placed: connected nodes start in the middle, loners go to the edge
            if total[next] > 0.0 { centre } else { centre * 3.0 }
        } else {
            placed_rel.iter().fold(Vec2::ZERO, |a, (p, w)| a + *p * *w) / placed_rel.iter().map(|x| x.1).sum::<f32>()
        };
        let k = (0..cols * rows).filter(|&k| free[k]).min_by(|&a, &b| (pos(a) - target).length_sq().total_cmp(&(pos(b) - target).length_sq())).unwrap();
        free[k] = false;
        slot_of[next] = k;
        for &(o, w) in &adj[next] {
            pull[o] += w;
        }
    }
    slot_of
}

fn size_of(f: &Fac) -> f32 {
    (300.0 + f.lines as f32 * 1.2).min(900.0)
}

fn layout(w: &mut World) {
    // each district gets a home, packed on a golden-angle spiral by size; its factories start around it
    let nd = w.districts.len().max(1);
    let mut members: Vec<Vec<usize>> = vec![vec![]; nd];
    for (i, f) in w.facs.iter().enumerate() {
        members[f.district.min(nd - 1)].push(i);
    }
    let radius: Vec<f32> = members.iter().map(|m| m.iter().map(|&i| size_of(&w.facs[i]).powi(2)).sum::<f32>().sqrt() * 0.9 + 300.0).collect();
    let mut order: Vec<usize> = (0..nd).collect();
    order.sort_by(|&a, &b| radius[b].total_cmp(&radius[a]));
    w.anchors = vec![Pos2::ZERO; nd];
    let ns = w.subdistricts.len();
    w.sub_home = vec![Vec2::ZERO; ns + nd];
    let mut placed: Vec<(Pos2, f32)> = vec![];
    for d in order {
        let mut k = 0;
        let pos = loop {
            let (a, r) = (k as f32 * 2.399_963, 400.0 * (k as f32).sqrt());
            let pos = pos2(a.cos() * r, a.sin() * r);
            if placed.iter().all(|(p, pr)| p.distance(pos) >= pr + radius[d] + 500.0) {
                break pos;
            }
            k += 1;
        };
        w.anchors[d] = pos;
        placed.push((pos, radius[d]));
        // inside the district each sub-district gets its own home, packed the same way, so they do not mix
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for &i in &members[d] {
            groups.entry(if w.facs[i].sub < ns { w.facs[i].sub } else { ns + d }).or_default().push(i);
        }
        let mut groups: Vec<(usize, Vec<usize>, f32)> = groups.into_iter().map(|(g, m)| {
            let r = m.iter().map(|&i| size_of(&w.facs[i]).powi(2)).sum::<f32>().sqrt() * 1.2 + 250.0;
            (g, m, r)
        }).collect();
        groups.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        let mut homes: Vec<(Pos2, f32)> = vec![];
        for (g, mut ms, gr) in groups {
            let mut k = 0;
            let off = loop {
                let (a, r) = (k as f32 * 2.399_963, 250.0 * (k as f32).sqrt());
                let off = pos2(a.cos() * r, a.sin() * r);
                if homes.iter().all(|(p, pr)| p.distance(off) >= pr + gr + 300.0) {
                    break off;
                }
                k += 1;
            };
            w.sub_home[g] = off.to_vec2();
            homes.push((off, gr));
            ms.sort_by(|&a, &b| size_of(&w.facs[b]).total_cmp(&size_of(&w.facs[a])));
            let avg = ms.iter().map(|&i| size_of(&w.facs[i])).sum::<f32>() / ms.len().max(1) as f32;
            for (k, i) in ms.into_iter().enumerate() {
                let (a, r) = (k as f32 * 2.399_963, (k as f32).sqrt() * avg * 1.1);
                w.facs[i].rect = Rect::from_center_size(pos + off.to_vec2() + vec2(a.cos(), a.sin()) * r, Vec2::splat(size_of(&w.facs[i])));
            }
        }
    }
    for f in &mut w.facs {
        let n = f.sections.len().max(1);
        let cols = (n as f32).sqrt().ceil() as usize;
        let (pad, top) = (24.0, 46.0);
        let slot = (f.rect.width() - pad * (cols as f32 + 1.0)) / cols as f32;
        let ms = slot * 0.92;
        let origin = f.rect.min + Vec2::splat((slot - ms) * 0.5);
        let rows = n.div_ceil(cols);
        let slot_of = grid_order(n, cols, rows, &f.inner);
        for (j, s) in f.sections.iter_mut().enumerate() {
            let k = slot_of[j];
            let (col, row) = ((k % cols) as f32, (k / cols) as f32);
            s.rect = Rect::from_min_size(origin + vec2(pad + col * (slot + pad), top + row * (slot * 0.8 + pad)), vec2(ms, ms * 0.86));
            let ic = ((s.items.len() as f32 * 1.6).sqrt().ceil() as usize).max(3);
            let is = (ms - 16.0) / ic as f32;
            let m0 = s.rect.min;
            for (k, it) in s.items.iter_mut().enumerate() {
                it.rect = Rect::from_min_size(m0 + vec2(8.0 + (k % ic) as f32 * is, 30.0 + (k / ic) as f32 * is * 0.6), vec2(is * 0.85, is * 0.5));
                it.off = it.rect.min - m0;
            }
            s.off = s.rect.min - f.rect.min;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_imports_across_languages() {
        let files = ["src/a.ts", "src/lib/index.ts", "pkg/util.py", "src/world.rs", "src/Monolog/Logger.php"];
        let mut by_stem = HashMap::new();
        let mut by_last: HashMap<String, Vec<String>> = HashMap::new();
        for (i, f) in files.iter().enumerate() {
            let s = strip_ext(f).to_string();
            by_last.entry(s.rsplit('/').next().unwrap().to_string()).or_default().push(s.clone());
            by_stem.insert(s, (0, i));
        }
        let r = |spec: &str, from: &str, lang: &str| resolve_import(spec, from, lang, &by_stem, &by_last).map(|x| x.1);
        assert_eq!(r("./a.js", "src/b.ts", "ts"), Some(0));
        assert_eq!(r("./lib", "src/b.ts", "ts"), Some(1));
        assert_eq!(r(".util", "pkg/x.py", "py"), Some(2));
        assert_eq!(r("pkg.util", "main.py", "py"), Some(2));
        assert_eq!(r("crate::world::Kind", "src/main.rs", "rs"), Some(3));
        assert_eq!(r("self::world", "src/main.rs", "rs"), Some(3));
        assert_eq!(r("Monolog\\Logger", "src/x.php", "php"), Some(4));
        assert_eq!(r("react", "src/b.ts", "ts"), None);
    }
}

/// What the layout centres on.
pub enum Focus {
    None,
    Fac(usize),
    Sec(SecId),
}

/// Force constants for one level of the layout, scaled to the size of its nodes.
struct Params {
    charge: f32,
    gap: f32,
    rest: f32,
    near: f32,
    far: f32,
    gravity: f32,
}

impl Params {
    fn for_size(m: f32, gravity: f32) -> Self {
        let k = m / 600.0;
        Params { charge: 4.0e5 * k * k, gap: 120.0 * k, rest: 260.0 * k, near: 220.0 * k, far: 2200.0 * k, gravity }
    }
}

/// Charge, collision, spring, gravity and focus forces for nodes with centres `p` and radii `r`.
/// With `group`, nodes only push and pull nodes of their own group; groups are kept apart elsewhere.
fn forces(p: &[Pos2], r: &[f32], edges: &[(usize, usize, f32)], home: Pos2, focus: Option<usize>, near: &HashSet<usize>, prm: &Params, group: Option<&[usize]>) -> Vec<Vec2> {
    let n = p.len();
    let same = |i: usize, j: usize| group.is_none_or(|g| g[i] == g[j]);
    let mut force = vec![Vec2::ZERO; n];
    for i in 0..n {
        for j in (i + 1..n).filter(|&j| same(i, j)) {
            let d = p[j] - p[i];
            let dist = d.length().max(1.0);
            let dir = if d.length() < 0.5 { Vec2::angled(i as f32) } else { d / dist };
            let gap = r[i] + r[j] + prm.gap;
            // repulsion only reaches nearby nodes (like d3's distanceMax), so big groups do not inflate
            if dist > gap * 3.0 {
                continue;
            }
            let mut f = prm.charge / dist;
            if dist < gap {
                f += (gap - dist) * 6.0;
            }
            force[i] -= dir * f;
            force[j] += dir * f;
        }
    }
    let is_near = |i: usize| focus == Some(i) || near.contains(&i);
    for &(a, b, w) in edges.iter().filter(|e| same(e.0, e.1)) {
        let d = p[b] - p[a];
        let dist = d.length().max(1.0);
        let weight = if w < 1.0 { w } else { 1.0 + w.ln() * 0.5 }; // weights below 1 weaken a spring
        let strength = if focus.is_some() && !(is_near(a) && is_near(b)) { 0.15 } else { 1.2 } * weight;
        let f = (dist - (r[a] + r[b] + prm.rest)) * strength;
        force[a] += d / dist * f;
        force[b] -= d / dist * f;
    }
    for i in 0..n {
        force[i] += (home - p[i]) * prm.gravity;
    }
    if let Some(fc) = focus {
        for i in (0..n).filter(|&i| i != fc && same(i, fc)) {
            let d = p[i] - p[fc];
            let dist = d.length().max(1.0);
            let want = r[i] + r[fc] + if near.contains(&i) { prm.near } else { prm.far };
            if near.contains(&i) || dist < want {
                force[i] -= d / dist * (dist - want) * 2.5;
            }
        }
    }
    force
}

/// Pushes overlapping rects apart along the shallower axis and keeps them inside `bounds`.
/// `margin(i, j)` is half the clearance kept between rects `i` and `j`.
fn separate(rects: &mut [Rect], fixed: &[bool], margin: impl Fn(usize, usize) -> f32, bounds: Option<Rect>) -> bool {
    let mut moved = false;
    for _ in 0..6 {
        let mut any = false;
        for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                let m = margin(i, j);
                let (a, b) = (rects[i].expand(m), rects[j].expand(m));
                let ox = a.max.x.min(b.max.x) - a.min.x.max(b.min.x);
                let oy = a.max.y.min(b.max.y) - a.min.y.max(b.min.y);
                if ox < 1.0 || oy < 1.0 {
                    continue;
                }
                let d = b.center() - a.center();
                let push = if ox < oy { vec2(if d.x >= 0.0 { ox } else { -ox }, 0.0) } else { vec2(0.0, if d.y >= 0.0 { oy } else { -oy }) };
                let (wi, wj) = match (fixed[i], fixed[j]) {
                    (true, true) => continue,
                    (true, false) => (0.0, 1.0),
                    (false, true) => (1.0, 0.0),
                    _ => (0.5, 0.5),
                };
                rects[i] = rects[i].translate(-push * wi);
                rects[j] = rects[j].translate(push * wj);
                any = true;
            }
        }
        if let Some(b) = bounds {
            for r in rects.iter_mut() {
                let d = vec2(
                    (b.min.x - r.min.x).max(0.0) + (b.max.x - r.max.x).min(0.0),
                    (b.min.y - r.min.y).max(0.0) + (b.max.y - r.max.y).min(0.0),
                );
                if d != Vec2::ZERO {
                    *r = r.translate(d);
                }
            }
        }
        moved |= any;
        if !any {
            break;
        }
    }
    moved
}

impl World {
    pub fn set_focus(&mut self, focus: Focus) {
        for f in &mut self.facs {
            if !f.focus_secs.is_empty() {
                f.focus_secs.clear();
                f.alpha = 1.0;
            }
        }
        self.near.clear();
        self.focus = None;
        match focus {
            Focus::None => {}
            Focus::Fac(f) => {
                self.focus = Some(f);
                self.near = self.fac_edges().into_iter().filter_map(|(a, b, _)| if a == f { Some(b) } else if b == f { Some(a) } else { None }).collect();
            }
            Focus::Sec((f, si)) => {
                self.focus = Some(f);
                let s = &self.facs[f].sections[si];
                let mut related: Vec<(usize, Option<usize>)> = s.deps.iter().chain(&s.dependents).map(|&(a, b)| (a, Some(b))).collect();
                related.extend(s.calls_out.iter().chain(&s.calls_in).map(|&((a, b), _)| (a, Some(b))));
                related.extend(s.links.iter().map(|l| l.to));
                let mut by_fac: HashMap<usize, Vec<usize>> = HashMap::new();
                for (a, b) in related {
                    let v = by_fac.entry(a).or_default();
                    if let Some(b) = b.filter(|b| !v.contains(b)) {
                        v.push(b);
                    }
                }
                let own = by_fac.remove(&f).unwrap_or_default();
                self.facs[f].focus_secs = std::iter::once(si).chain(own.into_iter().filter(|&x| x != si)).collect();
                self.facs[f].alpha = 1.0;
                for (g, secs) in by_fac {
                    self.facs[g].focus_secs = secs;
                    self.facs[g].alpha = 1.0;
                    self.near.insert(g);
                }
            }
        }
        self.alpha = 1.0;
    }

    /// Factory-level edges with weights: cell references, imports, context links.
    fn fac_edges(&self) -> Vec<(usize, usize, f32)> {
        let mut e: Vec<(usize, usize, f32)> = self.belts.iter().map(|&(a, b)| (a, b, 1.0)).collect();
        e.extend(self.code_belts.iter().map(|&(a, b, c)| (a, b, c as f32)));
        e.extend(self.pipes.iter().map(|&(a, b, c)| (a, b, c as f32)));
        e
    }

    /// Regroups and lays the map out again: by folder (0), as one graph grouped only by connections (1),
    /// or by whether a folder has uncommitted changes (2).
    pub fn organize(&mut self, mode: u8, changed: &HashSet<String>) {
        let n_cells = self.n_cells;
        assign_districts(self, n_cells);
        if mode != 0 {
            let mut names: Vec<String> = if n_cells > 0 { vec!["context cells".into()] } else { vec![] };
            for f in &mut self.facs[n_cells..] {
                let key = if mode == 1 {
                    "code".to_string()
                } else if f.sections.iter().any(|s| s.rel.as_ref().is_some_and(|r| changed.contains(r))) {
                    "changed".to_string()
                } else {
                    "unchanged".to_string()
                };
                f.district = names.iter().position(|n| *n == key).unwrap_or_else(|| {
                    names.push(key);
                    names.len() - 1
                });
                f.sub = usize::MAX;
                f.short = f.title.clone();
            }
            self.districts = names;
            self.subdistricts.clear();
        }
        self.organize = mode;
        layout(self);
        self.alpha = 1.0;
        for f in &mut self.facs {
            f.alpha = 1.0;
        }
    }

    /// A servitor is writing `abs`: when the file is not on the map yet, a construction site appears in its folder
    /// (or a new folder appears next to its closest relative). Returns where it was placed.
    pub fn add_build(&mut self, abs: &str, content: &str) -> Option<SecId> {
        let rel = abs.strip_prefix(&format!("{}/", self.repo))?.to_string();
        if self.by_path.get(abs).is_some_and(|x| x.1.is_some()) {
            return None;
        }
        let dir = rel.rsplit_once('/').map_or("(root)".to_string(), |x| x.0.to_string());
        let fi = match self.facs.iter().position(|f| f.kind == Kind::Code && f.id == dir) {
            Some(i) => i,
            None => {
                let parent = self.facs.iter().filter(|f| f.kind == Kind::Code && dir.starts_with(&format!("{}/", f.id))).max_by_key(|f| f.id.len());
                let district = parent.map_or(self.districts.len().saturating_sub(1), |f| f.district);
                let region = self.region(false, district).0;
                let at = if region.is_positive() { region.right_center() + vec2(450.0, 0.0) } else { Pos2::ZERO };
                let path = format!("{}/{dir}", self.repo);
                self.by_path.insert(path.clone(), (self.facs.len(), None));
                self.facs.push(Fac {
                    id: dir.clone(), kind: Kind::Code, title: dir.clone(), path, status: None, age: 0, lines: 40, sections: vec![], refs: vec![], links: vec![],
                    rect: Rect::from_center_size(at, Vec2::splat(340.0)), vel: Vec2::ZERO, pinned: false, alpha: 1.0, focus_secs: vec![], inner: vec![],
                    district, short: dir.rsplit('/').next().unwrap_or(&dir).to_string(), sub: usize::MAX, building: true,
                });
                self.facs.len() - 1
            }
        };
        let f = &mut self.facs[fi];
        let size = f.sections.first().map_or(vec2(260.0, 224.0), |s| s.rect.size());
        let mut s = section(rel.rsplit('/').next().unwrap_or(&rel).to_string(), Tag::Code, content.chars().take(4000).collect());
        s.lines = content.lines().count();
        s.rel = Some(rel);
        s.path = Some(abs.to_string());
        s.off = (f.rect.size() - size) * 0.5;
        s.rect = Rect::from_min_size(f.rect.min + s.off, size);
        s.build = Some(Build { lines: content.lines().map(String::from).collect(), progress: 0.0, done_at: None });
        f.sections.push(s);
        f.alpha = 1.0;
        let si = f.sections.len() - 1;
        self.by_path.insert(abs.to_string(), (fi, Some(si)));
        Some((fi, si))
    }

    /// Advances construction and edit animations; finished sites become normal files after a short pause.
    pub fn tick_builds(&mut self, dt: f32, now: f32) {
        let advance = |b: &mut Option<Build>, speed: f32| {
            if let Some(x) = b {
                let n = x.lines.len().max(1) as f32;
                x.progress = (x.progress + dt * (n / speed).clamp(6.0, 80.0)).min(n);
                if x.progress >= n && x.done_at.is_none() {
                    x.done_at = Some(now);
                }
                if x.done_at.is_some_and(|d| now - d > 1.5) {
                    *b = None;
                }
            }
        };
        for f in &mut self.facs {
            for s in &mut f.sections {
                advance(&mut s.build, 6.0);
                advance(&mut s.edit, 2.0);
            }
            if f.building && f.sections.iter().all(|s| s.build.is_none()) {
                f.building = false;
            }
        }
    }

    /// Seconds a construction of `lines` lines takes, matching `tick_builds`.
    pub fn build_secs(lines: usize) -> f32 {
        let n = lines.max(1) as f32;
        n / (n / 6.0).clamp(6.0, 80.0) + 1.0
    }

    /// Takes over positions from an earlier scan of the same project, so a rescan does not reshuffle the map;
    /// new folders and files find their place through the physics.
    pub fn adopt_layout(&mut self, old: &World) {
        let olds: HashMap<&str, &Fac> = old.facs.iter().map(|f| (f.id.as_str(), f)).collect();
        for f in &mut self.facs {
            let Some(o) = olds.get(f.id.as_str()) else {
                f.alpha = 1.0;
                continue;
            };
            f.rect = Rect::from_center_size(o.rect.center(), f.rect.size());
            let secs: HashMap<&str, &Section> = o.sections.iter().map(|s| (s.rel.as_deref().unwrap_or(&s.title), s)).collect();
            for s in &mut f.sections {
                if let Some(os) = secs.get(s.rel.as_deref().unwrap_or(&s.title)) {
                    s.off = os.off;
                }
            }
            f.alpha = 0.3;
        }
        for (d, name) in self.districts.iter().enumerate() {
            if let Some(od) = old.districts.iter().position(|n| n == name) {
                if let (Some(a), Some(oa)) = (self.anchors.get_mut(d), old.anchors.get(od)) {
                    *a = *oa;
                }
            }
        }
        self.alpha = 0.3;
        self.sync_rects();
    }

    /// Region of a district (`sub == false`) or sub-district as drawn, and how many factories it holds.
    pub fn region(&self, sub: bool, k: usize) -> (Rect, usize) {
        let mut bb = Rect::NOTHING;
        let mut n = 0;
        for f in self.facs.iter().filter(|f| if sub { f.sub == k } else { f.district == k }) {
            bb = bb.union(f.rect);
            n += 1;
        }
        (if bb.is_positive() { bb.expand(if sub { 70.0 } else { 140.0 }) } else { bb }, n)
    }

    /// Moves a whole district (and its home) or sub-district by `d`.
    pub fn translate_group(&mut self, sub: bool, k: usize, d: Vec2) {
        for f in self.facs.iter_mut().filter(|f| if sub { f.sub == k } else { f.district == k }) {
            f.rect = f.rect.translate(d);
        }
        if sub {
            if let Some(h) = self.sub_home.get_mut(k) {
                *h += d; // its home moves along, so it stays where it was dropped
            }
        } else if let Some(a) = self.anchors.get_mut(k) {
            *a += d;
        }
        self.alpha = self.alpha.max(0.3);
    }

    pub fn translate_fac(&mut self, f: usize, d: Vec2) {
        self.facs[f].rect = self.facs[f].rect.translate(d);
        self.facs[f].pinned = true;
        self.alpha = self.alpha.max(0.3);
    }

    /// One tick of both layout levels; returns whether anything still moves.
    pub fn step(&mut self, dt: f32) -> bool {
        let mut moving = self.step_factories(dt);
        for fi in 0..self.facs.len() {
            moving |= self.step_files(fi, dt);
        }
        self.sync_rects();
        moving
    }

    fn step_factories(&mut self, dt: f32) -> bool {
        let n = self.facs.len();
        let mut moving = false;
        let frozen = self.frozen.map(|f| f.0);
        // the focused factory stays where it is (the camera follows it); its neighbours come to it
        if self.alpha > 0.005 && n > 0 {
            moving = true;
            let p: Vec<Pos2> = self.facs.iter().map(|f| f.rect.center()).collect();
            let r: Vec<f32> = self.facs.iter().map(|f| f.rect.width() * 0.72).collect();
            // no "push everything else away" at this level: in big projects it inflates the map on every click
            let prm = Params { far: 0.0, ..Params::for_size(600.0, if self.anchors.is_empty() { 0.08 } else { 0.0 }) };
            // two levels: factories only interact inside their district; districts are kept apart as wholes
            let groups: Vec<usize> = self.facs.iter().map(|f| f.district).collect();
            let mut force = forces(&p, &r, &self.fac_edges(), Pos2::ZERO, self.focus, &self.near, &prm, Some(&groups));
            // each factory is pulled to its sub-district's home inside its district's home, which keeps
            // districts, and sub-districts within them, apart and readable
            let pull = 0.3;
            let ns = self.subdistricts.len();
            for (i, f) in self.facs.iter().enumerate() {
                if let Some(a) = self.anchors.get(f.district) {
                    let g = if f.sub < ns { f.sub } else { ns + f.district };
                    force[i] += (*a + self.sub_home.get(g).copied().unwrap_or_default() - p[i]) * pull;
                }
            }
            for (i, f) in self.facs.iter_mut().enumerate() {
                if f.pinned || Some(i) == frozen || self.focus == Some(i) {
                    f.vel = Vec2::ZERO;
                    continue;
                }
                f.vel = (f.vel + force[i] * self.alpha * dt) * 0.86f32.powf(dt * 60.0);
                f.rect = f.rect.translate(f.vel * dt);
            }
            self.alpha *= 0.986f32.powf(dt * 60.0);
        }
        // sub-districts only cohere (see the pull above); a hard push-apart here strings big districts into a line
        moving |= self.separate_districts();
        let mut rects: Vec<Rect> = self.facs.iter().map(|f| f.rect).collect();
        let fixed: Vec<bool> = (0..n).map(|i| self.facs[i].pinned || self.focus == Some(i) || frozen == Some(i)).collect();
        // folders of different sub-districts keep a wider street between them, so the nesting stays visible
        let subs: Vec<usize> = self.facs.iter().map(|f| f.sub).collect();
        if separate(&mut rects, &fixed, |i, j| if subs[i] == subs[j] { 50.0 } else { 120.0 }, None) {
            moving = true;
            for (f, r) in self.facs.iter_mut().zip(rects) {
                f.rect = r;
            }
        }
        moving
    }

    /// Hard constraint one level up: district regions may not overlap; a whole district moves at once.
    fn separate_districts(&mut self) -> bool {
        let nd = self.districts.len();
        if nd < 2 {
            return false;
        }
        let mut bb = vec![Rect::NOTHING; nd];
        for f in &self.facs {
            bb[f.district] = bb[f.district].union(f.rect);
        }
        let ids: Vec<usize> = (0..nd).filter(|&d| bb[d].is_positive()).collect();
        let held = |d: usize| self.facs.iter().any(|f| f.district == d && (f.pinned || self.focus.is_some_and(|fc| self.facs[fc].district == d) || self.frozen.is_some_and(|fr| self.facs[fr.0].district == d)));
        let fixed: Vec<bool> = ids.iter().map(|&d| held(d)).collect();
        // drawn regions are padded by 140, so this leaves a clear band of about 300 between districts
        let before: Vec<Rect> = ids.iter().map(|&d| bb[d].expand(290.0)).collect();
        let mut after = before.clone();
        if !separate(&mut after, &fixed, |_, _| 0.0, None) {
            return false;
        }
        for (k, &d) in ids.iter().enumerate() {
            let delta = after[k].min - before[k].min;
            if delta != Vec2::ZERO {
                for f in self.facs.iter_mut().filter(|f| f.district == d) {
                    f.rect = f.rect.translate(delta);
                }
                if let Some(a) = self.anchors.get_mut(d) {
                    *a += delta;
                }
            }
        }
        true
    }

    /// Files inside one factory: same forces, kept inside the factory walls.
    fn step_files(&mut self, fi: usize, dt: f32) -> bool {
        if self.frozen.is_some_and(|f| f.0 == fi) {
            return false;
        }
        let focused = self.focus == Some(fi);
        let f = &mut self.facs[fi];
        let n = f.sections.len();
        if n == 0 || f.alpha < 0.003 {
            return false;
        }
        let size = f.rect.size();
        let bounds = Rect::from_min_max(pos2(10.0, 40.0), pos2(size.x - 10.0, size.y - 10.0));
        let p: Vec<Pos2> = f.sections.iter().map(|s| (s.off + s.rect.size() * 0.5).to_pos2()).collect();
        let r: Vec<f32> = f.sections.iter().map(|s| s.rect.width().max(s.rect.height()) * 0.6).collect();
        let m = f.sections.iter().map(|s| s.rect.width()).sum::<f32>() / n as f32;
        let primary = if focused { f.focus_secs.first().copied() } else { None };
        let near: HashSet<usize> = f.focus_secs.iter().copied().filter(|&s| Some(s) != primary).collect();
        let mut force = forces(&p, &r, &f.inner, bounds.center(), primary, &near, &Params::for_size(m * 1.6, 0.4), None);
        if !focused {
            for &s in &near {
                force[s] += (bounds.center() - p[s]) * 1.5; // relatives of the focused file move to the middle
            }
        }
        let mut rects: Vec<Rect> = f.sections.iter().map(|s| Rect::from_min_size(s.off.to_pos2(), s.rect.size())).collect();
        for (i, s) in f.sections.iter_mut().enumerate() {
            if Some(i) == primary {
                let d = bounds.center() - p[i];
                rects[i] = rects[i].translate(d * (1.0 - 0.002f32.powf(dt)).max(0.05));
                s.vel = Vec2::ZERO;
                continue;
            }
            s.vel = (s.vel + force[i] * f.alpha * dt) * 0.86f32.powf(dt * 60.0);
            rects[i] = rects[i].translate(s.vel * dt);
        }
        let fixed: Vec<bool> = (0..n).map(|i| Some(i) == primary).collect();
        separate(&mut rects, &fixed, |_, _| m * 0.06, Some(bounds));
        for (s, r) in f.sections.iter_mut().zip(rects) {
            s.off = r.min.to_vec2();
        }
        f.alpha *= 0.986f32.powf(dt * 60.0);
        true
    }

    pub fn sync_rects(&mut self) {
        for f in &mut self.facs {
            let o = f.rect.min;
            for s in &mut f.sections {
                s.rect = Rect::from_min_size(o + s.off, s.rect.size()); // item rows are placed by the renderer
            }
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn overlaps(w: &World) -> usize {
        let mut n = 0;
        for i in 0..w.facs.len() {
            for j in i + 1..w.facs.len() {
                if w.facs[i].rect.intersects(w.facs[j].rect) {
                    n += 1;
                }
            }
        }
        n
    }

    fn settle(w: &mut World) -> usize {
        let mut ticks = 0;
        while w.step(1.0 / 60.0) && ticks < 5000 {
            ticks += 1;
        }
        ticks
    }

    #[test]
    #[ignore] // needs ~/Projects/TheFactory/The-Factory; run with --ignored
    fn layout_settles_without_overlaps() {
        let mut w = scan(&format!("{}/Projects/TheFactory/The-Factory", std::env::var("HOME").unwrap()), &[]);
        let t = settle(&mut w);
        println!("free: settled after {t} ticks, {} overlaps", overlaps(&w));
        assert_eq!(overlaps(&w), 0);
        let nd = w.districts.len();
        let bbs: Vec<Rect> = (0..nd).map(|d| w.facs.iter().filter(|f| f.district == d).fold(Rect::NOTHING, |a, f| a.union(f.rect))).collect();
        let crossing = (0..nd).flat_map(|a| (a + 1..nd).map(move |b| (a, b))).filter(|&(a, b)| bbs[a].is_positive() && bbs[b].is_positive() && bbs[a].intersects(bbs[b])).count();
        println!("{nd} districts, {crossing} overlapping");
        assert_eq!(crossing, 0);
        // a rescan keeps every folder where it was
        let mut again = scan(&format!("{}/Projects/TheFactory/The-Factory", std::env::var("HOME").unwrap()), &[]);
        again.adopt_layout(&w);
        let moved = again.facs.iter().filter(|f| w.facs.iter().find(|o| o.id == f.id).is_some_and(|o| o.rect.center().distance(f.rect.center()) > 1.0)).count();
        println!("rescan: {moved} folders moved");
        assert_eq!(moved, 0);
        let f = w.facs.iter().position(|f| f.id == "app/src/domain").unwrap();
        let s = w.facs[f].sections.iter().position(|s| s.title == "journey.ts").unwrap();
        let before = w.facs[f].rect.center();
        w.set_focus(Focus::Sec((f, s)));
        let t = settle(&mut w);
        let c = w.facs[f].rect.center();
        println!("focus: settled after {t} ticks, {} overlaps, focus moved {:.0}, near {}", overlaps(&w), c.distance(before), w.near.len());
        assert_eq!(overlaps(&w), 0);
        assert!(c.distance(before) < 1.0, "the focused factory holds still");
        // clicking around repeatedly must not grow the map
        let size = |w: &World| w.facs.iter().fold(Rect::NOTHING, |a, f| a.union(f.rect)).size();
        let start = size(&w);
        for k in 0..12 {
            let fk = (k * 7) % w.facs.len();
            w.set_focus(if k % 2 == 0 { Focus::Fac(fk) } else { Focus::None });
            settle(&mut w);
        }
        let end = size(&w);
        println!("map size {start:?} -> {end:?} after 12 clicks");
        assert!(end.x < start.x * 1.5 && end.y < start.y * 1.5);
        w.set_focus(Focus::Sec((f, s)));
        settle(&mut w);
        let sec = &w.facs[f].sections[s];
        let mid = w.facs[f].rect.center();
        let others_closer = w.facs[f].sections.iter().filter(|o| o.rect.center().distance(mid) + 1.0 < sec.rect.center().distance(mid)).count();
        println!("focused file is the {}th closest to its factory centre", others_closer + 1);
        assert_eq!(others_closer, 0);
        let mut inner = 0;
        for f in &w.facs {
            for (i, a) in f.sections.iter().enumerate() {
                assert!(f.rect.contains_rect(a.rect), "{} sticks out of {}", a.title, f.id);
                inner += f.sections[i + 1..].iter().filter(|b| a.rect.intersects(b.rect)).count();
            }
        }
        println!("{inner} overlapping files");
        assert_eq!(inner, 0);
    }
}

#[cfg(test)]
mod acme_check {
    use super::*;

    #[test]
    #[ignore] // needs ~/Projects/Accounts/acme
    fn many_clicks_do_not_inflate_a_large_project() {
        let ignore: Vec<String> = crate::settings::DEFAULT_IGNORED.iter().map(|s| s.to_string()).collect();
        let mut w = scan(&format!("{}/Projects/Accounts/acme", std::env::var("HOME").unwrap()), &ignore);
        let size = |w: &World| w.facs.iter().fold(Rect::NOTHING, |a, f| a.union(f.rect)).size();
        let settle = |w: &mut World| {
            for _ in 0..5000 {
                if !w.step(1.0 / 60.0) {
                    break;
                }
            }
        };
        settle(&mut w);
        // sub-district boxes of one district stay apart, so the folder nesting reads at a glance
        let subs: Vec<usize> = (0..w.subdistricts.len()).filter(|&k| w.region(true, k).1 > 0).collect();
        let overlapping = subs.iter().flat_map(|&a| subs.iter().map(move |&b| (a, b))).filter(|&(a, b)| a < b && w.region(true, a).0.intersects(w.region(true, b).0)).count();
        println!("{} sub-districts, {overlapping} overlapping pairs", subs.len());
        assert!(overlapping * 20 <= subs.len() * subs.len(), "sub-districts mix: {overlapping} overlapping pairs");
        let start = size(&w);
        for k in 0..40 {
            let fk = (k * 13) % w.facs.len();
            w.set_focus(if k % 3 == 2 { Focus::None } else { Focus::Fac(fk) });
            settle(&mut w);
        }
        let end = size(&w);
        println!("acme: {} factories, {} districts, size {start:?} -> {end:?}", w.facs.len(), w.districts.len());
        assert!(end.x < start.x * 1.6 && end.y < start.y * 1.6);
        // inside a folder, files that belong together sit closer than files that do not
        let f = w.facs.iter().find(|f| f.id.ends_with("admin-gateway/src/dto")).expect("dto folder");
        let c: Vec<Pos2> = f.sections.iter().map(|s| s.rect.center()).collect();
        let linked: HashSet<(usize, usize)> = f.inner.iter().map(|e| (e.0, e.1)).collect();
        let (mut dl, mut nl, mut du, mut nu) = (0.0, 0, 0.0, 0);
        for i in 0..c.len() {
            for j in i + 1..c.len() {
                if linked.contains(&(i, j)) {
                    dl += c[i].distance(c[j]);
                    nl += 1;
                } else {
                    du += c[i].distance(c[j]);
                    nu += 1;
                }
            }
        }
        let ratio = (dl / nl.max(1) as f32) / (du / nu.max(1) as f32);
        println!("dto folder: {} files, {} springs, related/unrelated distance {ratio:.2}", c.len(), linked.len());
        assert!(ratio < 0.5);
    }
}
