//! PROTOTYPE: the case file a Claude Code session writes for one ticket,
//! `~/.cellworks/cases/<KEY>.json` (`CELLWORKS_CASES` or the `[cases] folder` setting moves the folder).
//! Cellworks only reads it, and reads it again when it changes. The schema
//! is as small as the three case screens need; the `ticket-case`
//! skill documents it field by field for the session that writes it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Where the case file for `key` (a ticket's stable `<source>:<key>` form, or
/// a bare Jira key) lives, whether or not it exists.
pub fn path(key: &str) -> PathBuf {
    let stem = crate::tickets::TicketRef::parse(key).file_stem();
    cellworks_workspace::cases_folder().join(format!("{stem}.json"))
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct CaseFile {
    pub key: String,
    /// The repositories the fix touches, each where it is worked on.
    #[serde(default)]
    pub repos: Vec<Repo>,
    /// When the case was written, "YYYY-MM-DD".
    #[serde(default)]
    pub written: String,
    /// Where the case stands: ingest, plan, approve, fix.
    #[serde(default)]
    pub status: Status,
    /// Every status the case went through, oldest first.
    #[serde(default)]
    pub history: Vec<Event>,
    #[serde(default)]
    pub problem: Problem,
    /// What ingest could not settle from the ticket and the code.
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// The fix in two to four sentences, above the steps; written by plan.
    #[serde(default, alias = "fix")]
    pub approach: String,
    #[serde(default)]
    pub systems: Vec<System>,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub lanes: Vec<Lane>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Ingested,
    /// PROTOTYPE: the iterate loop runs in the case's worktree.
    Prototyping,
    /// PROTOTYPE: the engineer froze the prototype.
    Prototyped,
    Planning,
    Planned,
    Approved,
    Fixing,
    Fixed,
    /// PROTOTYPE: the engineer reviewed the built change.
    Reviewed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub status: Status,
    /// RFC 3339, or "YYYY-MM-DD HH:MM".
    pub at: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A repository the fix touches: its root, and the worktree and branch the
/// ticket is (or is proposed to be) worked on, when there is one.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Repo {
    pub name: String,
    pub root: String,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    /// The branch the fix branch starts from, e.g. "develop".
    #[serde(default)]
    pub base_branch: Option<String>,
}

impl Repo {
    /// Where the map opens for this repository: the worktree once it exists,
    /// else the root.
    pub fn base(&self) -> PathBuf {
        self.worktree
            .as_deref()
            .map(expand)
            .filter(|w| w.is_dir())
            .unwrap_or_else(|| expand(&self.root))
    }

    fn holds(&self, root: &str) -> bool {
        let root = expand(root);
        root == expand(&self.root) || root == self.base()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    pub headline: String,
    pub plain: String,
    #[serde(default)]
    pub problems: Vec<Finding>,
    #[serde(default)]
    pub today_chain: Vec<Link>,
    /// The acceptance criteria in plain words, in Jira's order.
    #[serde(default)]
    pub criteria_notes: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub title: String,
    #[serde(rename = "where")]
    pub place: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub kind: Kind,
    pub label: String,
    #[serde(default)]
    pub danger: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Vault,
    Key,
    Workload,
    Role,
    Database,
    Doc,
    Queue,
    Service,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct System {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    #[serde(default)]
    pub group: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub title: String,
    pub summary: String,
    #[serde(rename = "where")]
    pub place: String,
    pub spots: Vec<Spot>,
    #[serde(default)]
    pub changes: Vec<Change>,
    #[serde(default)]
    pub excerpts: Vec<Excerpt>,
    #[serde(default)]
    pub risks: Vec<Risk>,
    /// The line OPEN IN NVIM jumps to; the first excerpt's first hunk
    /// otherwise.
    #[serde(default)]
    pub open_cmd_line: Option<usize>,
}

/// A file (or folder) in a repository, or an outside system by its id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Spot {
    File(FileSpot),
    System(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileSpot {
    /// The `repos[].name` it lives in; none for an outside file.
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub repo_root: String,
    pub path: String,
    #[serde(default)]
    pub lines: Option<[usize; 2]>,
}

impl Spot {
    /// Whether two spots name the same file or system; line ranges aside.
    pub fn same(&self, other: &Spot) -> bool {
        match (self, other) {
            (Spot::File(a), Spot::File(b)) => a.absolute() == b.absolute(),
            (Spot::System(a), Spot::System(b)) => a == b,
            _ => false,
        }
    }
}

impl FileSpot {
    pub fn absolute(&self) -> PathBuf {
        expand(&self.repo_root).join(&self.path)
    }
}

/// `~/...` with the home folder in front.
pub fn expand(root: &str) -> PathBuf {
    match root.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(root),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub what: String,
    pub before: String,
    pub after: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Excerpt {
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub repo_root: String,
    pub path: String,
    #[serde(default)]
    pub hunks: Vec<Hunk>,
    #[serde(default)]
    pub inserted: Vec<Inserted>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Hunk {
    pub start: usize,
    pub end: usize,
    /// Line number to mark; unmarked lines are context.
    #[serde(default)]
    pub marks: BTreeMap<String, Mark>,
}

/// A line the fix adds, which the file does not have yet.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Inserted {
    /// The line it follows; 0 puts it before the first hunk.
    pub after_line: usize,
    pub text: String,
    #[serde(default = "added")]
    pub mark: Mark,
}

fn added() -> Mark {
    Mark::Add
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    Ctx,
    Hit,
    Del,
    Add,
    /// Not in the file: a gap between hunks, or "..." among inserted lines.
    Gap,
    /// Not in the file: the heading of the next excerpt's file.
    #[serde(skip)]
    File,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Risk {
    pub level: Level,
    pub tag: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Danger,
    Warn,
    Info,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Lane {
    pub from: Spot,
    pub to: Spot,
    pub when: When,
    pub kind: LaneKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum When {
    Today,
    After,
    Both,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LaneKind {
    /// Logs in as the administrator.
    Root,
    /// Logs in as a role of its own.
    OwnRole,
    /// Is defined by (a script, a manifest).
    DefinedBy,
    /// Reads (a secret, a config).
    Reads,
}

/// One line of a step's code panel: number (0: none), text, mark.
pub type Line = (usize, String, Mark);

/// A case file as the screens use it: the file, plus what was read from
/// disk for it.
pub struct Loaded {
    pub case: CaseFile,
    pub modified: SystemTime,
    /// Per step: the excerpt lines, with the text on disk.
    pub lines: Vec<Vec<Line>>,
    /// Per step: the file OPEN IN NVIM opens and the line, when it exists.
    pub opens: Vec<Option<(PathBuf, usize)>>,
    /// Per step, once the fix is under way: the diff's lines and the range
    /// it was read from, e.g. "develop...fix/x".
    pub diffs: Vec<Option<(Vec<Line>, String)>>,
}

/// Reads the case file for `key`: None when there is none, an error when
/// it does not parse.
pub fn load(key: &str) -> Option<Result<Loaded, String>> {
    let path = path(key);
    let modified = modified(key)?;
    let text = std::fs::read_to_string(&path).ok()?;
    Some(
        serde_json::from_str::<CaseFile>(&text)
            .map_err(|e| format!("{}: {e}", path.display()))
            .map(normalised)
            .map(|case| {
                let mut loaded = Loaded {
                    diffs: case.steps.iter().map(|step| diffed(&case, step)).collect(),
                    opens: case.steps.iter().map(opens).collect(),
                    lines: Vec::new(),
                    case,
                    modified,
                };
                loaded.lines = loaded
                    .case
                    .steps
                    .iter()
                    .zip(&loaded.diffs)
                    .map(|(step, diff)| match diff {
                        Some((lines, _)) => lines.clone(),
                        None => lines(step),
                    })
                    .collect();
                loaded
            }),
    )
}

/// Fills in what a case file may leave out: a file's `repo_root` from its
/// named repository (its worktree when it has one), a file's repository from
/// its root, and, in a file without `repos`, one repository per folder the
/// files live in.
fn normalised(mut case: CaseFile) -> CaseFile {
    let mut files: Vec<(&mut Option<String>, &mut String)> = Vec::new();
    for step in &mut case.steps {
        for spot in &mut step.spots {
            if let Spot::File(file) = spot {
                files.push((&mut file.repo, &mut file.repo_root));
            }
        }
        for excerpt in &mut step.excerpts {
            files.push((&mut excerpt.repo, &mut excerpt.repo_root));
        }
    }
    for lane in &mut case.lanes {
        for spot in [&mut lane.from, &mut lane.to] {
            if let Spot::File(file) = spot {
                files.push((&mut file.repo, &mut file.repo_root));
            }
        }
    }
    if case.repos.is_empty() {
        for (_, root) in &files {
            let folder = expand(root);
            if folder.is_dir() && !case.repos.iter().any(|r| r.holds(root)) {
                case.repos.push(Repo {
                    name: folder
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| root.to_string()),
                    root: root.to_string(),
                    worktree: None,
                    branch: None,
                    base_branch: None,
                });
            }
        }
    }
    for (repo, root) in files {
        let found = match repo.as_deref() {
            Some(name) => case.repos.iter().find(|r| r.name == name),
            None => case.repos.iter().find(|r| r.holds(root)),
        };
        if let Some(found) = found {
            *repo = Some(found.name.clone());
            *root = found.base().display().to_string();
        }
    }
    case
}

/// Moves the case for `key` to `status`, with `edit` applied first and
/// the move recorded in its history. The only write Cellworks makes to a
/// case file; the sessions write everything else.
pub fn advance(
    key: &str,
    status: Status,
    note: &str,
    edit: impl FnOnce(&mut CaseFile),
) -> Result<(), String> {
    let path = path(key);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut case: CaseFile =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    edit(&mut case);
    case.status = status;
    case.history.push(Event {
        status,
        at: crate::tickets::clock(SystemTime::now(), true),
        note: note.to_owned(),
    });
    let text = serde_json::to_string_pretty(&case).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

/// Where the case for `key` stands in its lifecycle: None without a case
/// file, or with one that does not read.
// ponytail: reads the file on every call; cache by modified time if the
// picker ever lists more than a screenful.
pub fn status_of(key: &str) -> Option<Status> {
    #[derive(Deserialize)]
    struct Head {
        #[serde(default)]
        status: Status,
    }
    let text = std::fs::read_to_string(path(key)).ok()?;
    serde_json::from_str::<Head>(&text)
        .ok()
        .map(|head| head.status)
}

impl Status {
    /// The status as the picker shows it.
    pub fn name(self) -> &'static str {
        match self {
            Status::Ingested => "INGESTED",
            Status::Prototyping => "PROTOTYPING",
            Status::Prototyped => "PROTOTYPED",
            Status::Reviewed => "REVIEWED",
            Status::Planning => "PLANNING",
            Status::Planned => "PLANNED",
            Status::Approved => "APPROVED",
            Status::Fixing => "FIXING",
            Status::Fixed => "FIXED",
        }
    }
}

/// When the case file for `key` last changed.
pub fn modified(key: &str) -> Option<SystemTime> {
    std::fs::metadata(path(key)).and_then(|m| m.modified()).ok()
}

fn opens(step: &Step) -> Option<(PathBuf, usize)> {
    let excerpt = step.excerpts.first()?;
    let path = expand(&excerpt.repo_root).join(&excerpt.path);
    let line = step
        .open_cmd_line
        .or_else(|| excerpt.hunks.first().map(|h| h.start))
        .unwrap_or(1);
    path.is_file().then_some((path, line))
}

/// The excerpts of `step` as lines: each file's hunks with their text read
/// from disk, the inserted lines after the line they follow, a gap between
/// hunks and a heading before every file but the first.
fn lines(step: &Step) -> Vec<Line> {
    let mut out = Vec::new();
    for (index, excerpt) in step.excerpts.iter().enumerate() {
        if index > 0 {
            out.push((0, excerpt.path.clone(), Mark::File));
        }
        let disk: Vec<String> =
            std::fs::read_to_string(expand(&excerpt.repo_root).join(&excerpt.path))
                .map(|text| text.lines().map(str::to_owned).collect())
                .unwrap_or_default();
        let inserted = |after: usize, out: &mut Vec<Line>| {
            for line in excerpt.inserted.iter().filter(|i| i.after_line == after) {
                out.push((0, line.text.clone(), line.mark));
            }
        };
        inserted(0, &mut out);
        for (h, hunk) in excerpt.hunks.iter().enumerate() {
            if h > 0 {
                out.push((0, "...".to_owned(), Mark::Gap));
            }
            for number in hunk.start..=hunk.end {
                let text = disk
                    .get(number.wrapping_sub(1))
                    .cloned()
                    .unwrap_or_default();
                let mark = hunk
                    .marks
                    .get(&number.to_string())
                    .copied()
                    .unwrap_or(Mark::Ctx);
                out.push((number, text, mark));
                inserted(number, &mut out);
            }
        }
    }
    out
}

/// Once the fix is under way, the step's files as the fix branch changed
/// them: `git diff <base>...<branch>` in each repository that has both,
/// read through the git CLI. None before that, or when the diff is empty.
fn diffed(case: &CaseFile, step: &Step) -> Option<(Vec<Line>, String)> {
    if case.status < Status::Fixing {
        return None;
    }
    let (mut out, mut range) = (Vec::new(), String::new());
    for repo in &case.repos {
        let (Some(branch), Some(base)) = (&repo.branch, &repo.base_branch) else {
            continue;
        };
        let mine = |name: &Option<String>| name.as_deref() == Some(repo.name.as_str());
        let mut paths: Vec<&str> = step
            .spots
            .iter()
            .filter_map(|spot| match spot {
                Spot::File(file) if mine(&file.repo) => Some(file.path.as_str()),
                _ => None,
            })
            .chain(
                step.excerpts
                    .iter()
                    .filter(|e| mine(&e.repo))
                    .map(|e| e.path.as_str()),
            )
            .collect();
        paths.sort_unstable();
        paths.dedup();
        if paths.is_empty() {
            continue;
        }
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(expand(&repo.root))
            .args(["diff", "--no-color", "--no-ext-diff", "-U3"])
            .arg(format!("{base}...{branch}"))
            .arg("--")
            .args(&paths)
            .output()
            .ok()?;
        parse_diff(&String::from_utf8_lossy(&output.stdout), &mut out);
        range = format!("{base}...{branch}");
    }
    (!out.is_empty()).then_some((out, range))
}

/// A unified diff as code-panel lines: a heading before every file but
/// the first, a gap between hunks, new line numbers for context and added
/// lines, old ones for removed lines.
fn parse_diff(diff: &str, out: &mut Vec<Line>) {
    let (mut old, mut new, mut first_hunk) = (0, 0, true);
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            if !out.is_empty() {
                out.push((0, path.trim_start_matches("b/").to_owned(), Mark::File));
            }
            first_hunk = true;
        } else if let Some(rest) = line.strip_prefix("@@ -") {
            let number = |text: &str| -> usize {
                text.split(',')
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(1)
            };
            let mut parts = rest.split_whitespace();
            old = number(parts.next().unwrap_or_default());
            new = number(parts.next().unwrap_or_default().trim_start_matches('+'));
            if !first_hunk {
                out.push((0, "...".to_owned(), Mark::Gap));
            }
            first_hunk = false;
        } else if line.starts_with("diff --git")
            || line.starts_with("--- ")
            || line.starts_with("index ")
            || line.starts_with('\\')
            || line.starts_with("new file")
            || line.starts_with("deleted file")
        {
        } else if let Some(text) = line.strip_prefix('+') {
            out.push((new, text.to_owned(), Mark::Add));
            new += 1;
        } else if let Some(text) = line.strip_prefix('-') {
            out.push((old, text.to_owned(), Mark::Del));
            old += 1;
        } else {
            let text = line.strip_prefix(' ').unwrap_or(line);
            out.push((new, text.to_owned(), Mark::Ctx));
            old += 1;
            new += 1;
        }
    }
}

/// The crumb and the file name the code panel heads a step with.
pub fn heading(step: &Step) -> (String, String) {
    let Some(excerpt) = step.excerpts.first() else {
        return (String::new(), step.place.clone());
    };
    // A root that is no folder here (a stub, like a Vault path) is shown as
    // it is written.
    let root = expand(&excerpt.repo_root);
    let repo = match root.is_dir() {
        true => root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        false => excerpt.repo_root.clone(),
    };
    let path = Path::new(&excerpt.path);
    let mut crumb = vec![repo];
    if let Some(parent) = path.parent() {
        crumb.extend(
            parent
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned()),
        );
    }
    crumb.retain(|c| !c.is_empty());
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| excerpt.path.clone());
    (crumb.join(" / "), file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_case_file_reads_with_spots_lanes_and_marks() {
        let text = r#"{
            "key": "X-1", "written": "2026-10-22",
            "problem": {"headline": "h", "plain": "p", "problems": [],
                        "today_chain": [{"kind": "role", "label": "root", "danger": true}]},
            "systems": [{"id": "r", "kind": "role", "label": "root", "group": "db"}],
            "steps": [{"id": "S1", "title": "t", "summary": "s", "where": "w",
                       "spots": [{"system": "r"}, {"file": {"repo_root": "~/x", "path": "a.sql", "lines": [3, 4]}}],
                       "excerpts": [{"repo_root": "/nowhere", "path": "a.sql",
                                     "hunks": [{"start": 3, "end": 4, "marks": {"4": "hit"}}],
                                     "inserted": [{"after_line": 4, "text": "new"}]}],
                       "risks": [{"level": "warn", "tag": "T", "text": "x"}]}],
            "lanes": [{"from": {"system": "r"}, "to": {"file": {"repo_root": "~/x", "path": "a.sql"}},
                       "when": "after", "kind": "defined_by"}]
        }"#;
        let case: CaseFile = serde_json::from_str(text).expect("parses");
        assert_eq!(case.steps[0].spots[0], Spot::System("r".into()));
        let lines = lines(&case.steps[0]);
        let marks: Vec<Mark> = lines.iter().map(|l| l.2).collect();
        assert_eq!(marks, [Mark::Ctx, Mark::Hit, Mark::Add]);
        assert_eq!(lines[2].1, "new");
    }

    #[test]
    fn the_skill_example_reads_as_a_planned_case() {
        let text = include_str!("../../../../.claude/skills/ticket-case/example-SHOP-42.json");
        let case: CaseFile = serde_json::from_str(text).expect("the example parses");
        assert_eq!(case.status, Status::Planned);
        assert!(!case.approach.is_empty() && !case.steps.is_empty());
    }

    #[test]
    fn a_diff_reads_as_marked_lines() {
        let diff = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -3,2 +3,2 @@\n keep\n-old\n+new\n";
        let mut out = Vec::new();
        parse_diff(diff, &mut out);
        assert_eq!(
            out,
            [
                (3, "keep".to_owned(), Mark::Ctx),
                (4, "old".to_owned(), Mark::Del),
                (4, "new".to_owned(), Mark::Add)
            ]
        );
    }
}
