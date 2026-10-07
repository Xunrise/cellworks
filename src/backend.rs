// Everything outside the world model: transcript feed, projects, chats, servitor processes, git, Herdr.
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

pub enum Event {
    Tool { agent: String, parent: Option<String>, cwd: Option<String>, tool: String, input: Value },
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}
pub fn projects_root() -> PathBuf {
    home().join("Projects")
}
fn claude_projects() -> PathBuf {
    home().join(".claude/projects")
}
pub fn slug(p: &str) -> String {
    p.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}
fn mtime(p: &Path) -> SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH)
}
fn jsonl_in(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "jsonl")).collect()).unwrap_or_default()
}
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default()
}

// ---------- transcript tailer: tool calls of every Claude session on this machine ----------
pub fn start_tailer(tx: Sender<Event>, repaint: eframe::egui::Context) {
    std::thread::spawn(move || {
        let mut offsets: HashMap<PathBuf, u64> = HashMap::new();
        loop {
            let now = SystemTime::now();
            let mut files = Vec::new();
            for pd in subdirs(&claude_projects()) {
                files.extend(jsonl_in(&pd).into_iter().map(|f| (f, None)));
                for sd in subdirs(&pd) {
                    let parent = sd.file_name().unwrap().to_string_lossy().to_string();
                    files.extend(jsonl_in(&sd.join("subagents")).into_iter().map(|f| (f, Some(parent.clone()))));
                }
            }
            let mut sent = false;
            for (f, parent) in files {
                let Ok(meta) = std::fs::metadata(&f) else { continue };
                let size = meta.len();
                let fresh = now.duration_since(meta.modified().unwrap_or(now)).unwrap_or_default() < Duration::from_secs(3600);
                if !fresh && !offsets.contains_key(&f) {
                    continue;
                }
                let off = *offsets.entry(f.clone()).or_insert(size.saturating_sub(40_000));
                if size <= off {
                    continue;
                }
                let mut buf = Vec::new();
                let Ok(mut fh) = std::fs::File::open(&f) else { continue };
                if fh.seek(SeekFrom::Start(off)).is_err() || fh.read_to_end(&mut buf).is_err() {
                    continue;
                }
                let end = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
                offsets.insert(f.clone(), off + end as u64);
                let agent = f.file_stem().unwrap().to_string_lossy().to_string();
                for line in buf[..end].split(|&b| b == b'\n') {
                    let Ok(d) = serde_json::from_slice::<Value>(line) else { continue }; // first line after a mid-file seek
                    let Some(content) = d.pointer("/message/content").and_then(Value::as_array) else { continue };
                    for c in content.iter().filter(|c| c["type"] == "tool_use") {
                        sent = true;
                        let _ = tx.send(Event::Tool {
                            agent: agent.clone(), parent: parent.clone(), cwd: d["cwd"].as_str().map(String::from),
                            tool: c["name"].as_str().unwrap_or("?").into(), input: c["input"].clone(),
                        });
                    }
                }
            }
            if sent {
                repaint.request_repaint();
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

// ---------- projects ----------
pub struct Project {
    pub path: String,
    pub name: String,
    pub last: SystemTime,
}

fn cwd_of(transcript: &Path) -> Option<String> {
    let f = std::fs::File::open(transcript).ok()?;
    BufReader::new(f).lines().take(40).flatten().find_map(|l| serde_json::from_str::<Value>(&l).ok()?["cwd"].as_str().map(String::from))
}

pub fn projects() -> Vec<Project> {
    let root = projects_root();
    let mut found: Vec<String> = Vec::new();
    for d in subdirs(&root).into_iter().flat_map(|d| std::iter::once(d.clone()).chain(subdirs(&d))) {
        if d.join(".git").exists() {
            found.push(d.to_string_lossy().to_string());
        }
    }
    let cutoff = SystemTime::now() - Duration::from_secs(30 * 86400);
    for sd in subdirs(&claude_projects()) {
        if let Some(latest) = jsonl_in(&sd).into_iter().filter(|t| mtime(t) > cutoff).max_by_key(|t| mtime(t)) {
            if let Some(cwd) = cwd_of(&latest) {
                if cwd.starts_with(&format!("{}/", root.display())) && !cwd.contains("/worktrees/") && Path::new(&cwd).is_dir() && !found.contains(&cwd) {
                    found.push(cwd);
                }
            }
        }
    }
    let mut out: Vec<Project> = found.into_iter().map(|p| {
        let last = jsonl_in(&claude_projects().join(slug(&p))).iter().map(|t| mtime(t)).max().unwrap_or(SystemTime::UNIX_EPOCH);
        let name = p.strip_prefix(&format!("{}/", root.display())).unwrap_or(&p).to_string();
        Project { path: p, name, last }
    }).collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.last));
    out
}

// ---------- chats (transcripts) ----------
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub mtime: SystemTime,
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter(|c| c["type"] == "text").filter_map(|c| c["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn is_prompt(d: &Value) -> Option<String> {
    if d["type"] != "user" || d["isMeta"] == true {
        return None;
    }
    let t = text_of(&d["message"]["content"]).trim().to_string();
    (!t.is_empty() && !t.starts_with('<')).then_some(t)
}

pub fn sessions(repo: &str) -> Vec<SessionInfo> {
    let mut files = jsonl_in(&claude_projects().join(slug(repo)));
    files.sort_by_key(|f| std::cmp::Reverse(mtime(f)));
    files.into_iter().take(60).map(|f| {
        let title = std::fs::File::open(&f).ok().and_then(|fh| {
            BufReader::new(fh).lines().take(300).flatten().find_map(|l| is_prompt(&serde_json::from_str(&l).ok()?))
        });
        SessionInfo { id: f.file_stem().unwrap().to_string_lossy().to_string(), title: title.unwrap_or("(no prompt)".into()).chars().take(120).collect(), mtime: mtime(&f) }
    }).collect()
}

pub fn short_input(v: &Value) -> String {
    for k in ["file_path", "command", "pattern", "path", "description", "prompt", "url", "query"] {
        if let Some(s) = v[k].as_str() {
            return s.replace(&format!("{}/", projects_root().display()), "").chars().take(160).collect();
        }
    }
    v.to_string().chars().take(160).collect()
}

// ---------- git ----------
pub fn git(repo: &str, args: &[&str]) -> String {
    match Command::new("git").arg("-C").arg(repo).args(args).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string() + &if o.status.success() { String::new() } else { String::from_utf8_lossy(&o.stderr).to_string() },
        Err(e) => e.to_string(),
    }
}

#[derive(Default)]
pub struct GitState {
    pub branch: String,
    pub status: Vec<(String, String)>,
    pub ahead_behind: String,
    pub log: Vec<String>,
    /// (path, branch) of every worktree of the repository
    pub worktrees: Vec<(String, String)>,
    /// local branches, most recently committed first
    pub branches: Vec<String>,
    /// lines added and removed per changed file, against HEAD (untracked files count as all added)
    pub numstat: HashMap<String, (u32, u32)>,
}

pub fn git_state(repo: &str) -> GitState {
    GitState {
        branch: git(repo, &["branch", "--show-current"]).trim().into(),
        status: git(repo, &["status", "--porcelain", "-uall"]).lines().filter(|l| l.len() > 3).map(|l| (l[..2].to_string(), l[3..].to_string())).collect(),
        ahead_behind: git(repo, &["rev-list", "--left-right", "--count", "@{u}...HEAD"]).trim().into(),
        log: git(repo, &["log", "--oneline", "-30"]).lines().map(String::from).collect(),
        worktrees: git(repo, &["worktree", "list", "--porcelain"]).split("\n\n").filter_map(|b| {
            let path = b.lines().find_map(|l| l.strip_prefix("worktree "))?;
            let branch = b.lines().find_map(|l| l.strip_prefix("branch refs/heads/")).unwrap_or("(detached)");
            Some((path.to_string(), branch.to_string()))
        }).collect(),
        branches: git(repo, &["for-each-ref", "--sort=-committerdate", "--format=%(refname:short)", "refs/heads"]).lines().map(String::from).collect(),
        numstat: numstat(repo),
    }
}

fn numstat(repo: &str) -> HashMap<String, (u32, u32)> {
    let mut out: HashMap<String, (u32, u32)> = git(repo, &["diff", "HEAD", "--numstat"]).lines().filter_map(|l| {
        let mut it = l.split('\t');
        let (a, d, f) = (it.next()?, it.next()?, it.next()?);
        Some((f.to_string(), (a.parse().unwrap_or(0), d.parse().unwrap_or(0))))
    }).collect();
    for f in git(repo, &["ls-files", "--others", "--exclude-standard"]).lines() {
        let lines = std::fs::read_to_string(Path::new(repo).join(f)).map(|t| t.lines().count() as u32).unwrap_or(0);
        out.insert(f.to_string(), (lines, 0));
    }
    out
}

pub fn git_diff(repo: &str, file: &str) -> String {
    let d = git(repo, &["diff", "HEAD", "--", file]);
    if d.trim().is_empty() {
        if let Ok(t) = std::fs::read_to_string(Path::new(repo).join(file)) {
            return format!("(untracked)\n{}", t.lines().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n"));
        }
    }
    d
}

// ---------- terminals via Herdr ----------
/// Opens a Herdr tab in `cwd`, optionally running `cmd` in it. Returns Herdr's output.
pub fn herdr_tab(cwd: &str, label: &str, cmd: Option<&str>) -> String {
    let out = Command::new("herdr").args(["tab", "create", "--cwd", cwd, "--label", label, "--focus"]).output();
    let Ok(out) = out else { return "herdr not found".into() };
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if let (Some(cmd), Ok(v)) = (cmd, serde_json::from_str::<Value>(&text)) {
        fn find_tab(v: &Value) -> Option<String> {
            match v {
                Value::Object(m) => m.get("tab_id").and_then(Value::as_str).map(String::from).or_else(|| m.values().find_map(find_tab)),
                Value::Array(a) => a.iter().find_map(find_tab),
                _ => None,
            }
        }
        let panes = Command::new("herdr").args(["pane", "list"]).output().ok().and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok());
        let pane = find_tab(&v).and_then(|tab| {
            panes?["result"]["panes"].as_array()?.iter().find(|p| p["tab_id"] == tab.as_str())?["pane_id"].as_str().map(String::from)
        });
        if let Some(pane) = pane {
            let r = Command::new("herdr").args(["pane", "run", &pane]).args(cmd.split_whitespace()).output();
            return r.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
        }
        return format!("opened tab, could not find its pane id to run `{cmd}`:\n{text}");
    }
    text
}
