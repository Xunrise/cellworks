//! PROTOTYPE (proto/iterate-loop): the stack-agnostic iterate loop.
//! Disposable: no tests, hardcoded values, everything in one file.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

const BRIEF: &str = "You are prototyping in this git worktree, driven from Cellworks. \
Start the app's dev server yourself with its own hot reload (use .claude/launch.json if it exists) \
as a background process, and tee ALL of its output to .cellworks/run.log \
(for example `npm run dev 2>&1 | tee -a .cellworks/run.log`). \
Then wait: feedback arrives as messages that start with [round N]. \
Each names a file:line anchor and/or a screenshot path; read the screenshot when given. \
Make the change, keep the dev server running, and reply briefly.";

/// One line of a file as the plan's code panel shows it: its number in the
/// newer version (none for a removed line or a gap), its mark and text.
#[derive(Clone)]
pub struct CodeLine {
    pub no: Option<usize>,
    /// ' ' context, '+' added, '-' removed, '…' a gap between hunks.
    pub mark: char,
    pub text: String,
}

/// What the watcher thread reports once a second.
#[derive(Default, Clone)]
pub struct Status {
    pub port: Option<u16>,
    pub live: bool,
    /// Repo-relative file and line, from the run log.
    pub fires: Vec<(String, u32, String)>,
    /// Files changed since the last round snapshot (the round in progress).
    pub touched: Vec<String>,
}

pub struct Round {
    pub commit: String,
    pub files: Vec<String>,
    pub note: String,
}

pub struct Draft {
    pub anchor: Option<String>,
    pub image: Option<PathBuf>,
    pub note: String,
}

pub struct Iterate {
    pub worktree: PathBuf,
    pub slug: String,
    pub session_name: String,
    brief: String,
    pub spawned: Option<Result<String, String>>,
    spawn_rx: Receiver<Result<String, String>>,
    rx: Receiver<Status>,
    base_tx: Sender<String>,
    pub status: Status,
    /// Round 0 is the base snapshot taken at start.
    pub rounds: Vec<Round>,
    /// The round whose scaffolding is shown; `None` follows the live round.
    pub viewing: Option<usize>,
    pub draft: Option<Draft>,
    capture: Option<(Child, PathBuf)>,
    /// Bytes to type into the pane, and when.
    queue: Vec<(Instant, Vec<u8>)>,
    pub log: Vec<String>,
    /// The last thing sent or done, shown briefly over the feedback bar.
    pub toast: Option<(String, Instant)>,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    git_env(dir, args, None)
}

fn git_env(dir: &Path, args: &[&str], index: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// A commit of the whole worktree, untracked files included, without
/// touching the real index or any branch.
fn snapshot(worktree: &Path, message: &str) -> Result<String, String> {
    let index = worktree.join(".cellworks/snapshot.index");
    let _ = std::fs::remove_file(&index);
    git_env(worktree, &["read-tree", "HEAD"], Some(&index))?;
    git_env(worktree, &["add", "-A", "."], Some(&index))?;
    let tree = git_env(worktree, &["write-tree"], Some(&index))?;
    git(
        worktree,
        &["commit-tree", &tree, "-p", "HEAD", "-m", message],
    )
}

/// The ref round `n` of the loop in `worktree` is kept under. Worktrees of
/// one repository share their refs, so each loop has its own folder.
fn round_ref(worktree: &Path, n: usize) -> String {
    let name = worktree
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("refs/cellworks/{name}/rounds/{n}")
}

fn changed(worktree: &Path, from: &str, to: &str) -> Vec<String> {
    git(worktree, &["diff", "--name-only", from, to])
        .map(|s| s.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

impl Iterate {
    /// Prototype this: worktree off the project's HEAD, a background session
    /// in it with the brief, and the watcher.
    pub fn start(project: &Path) -> Result<Iterate, String> {
        let repo = project
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let slug = format!("iter-{}", secs % 100_000);
        let worktree = project
            .parent()
            .ok_or("no parent")?
            .join(format!("{repo}-proto-{slug}"));
        git(
            project,
            &[
                "worktree",
                "add",
                &worktree.to_string_lossy(),
                "-b",
                &format!("proto/{slug}"),
            ],
        )?;
        Iterate::begin(
            &worktree,
            slug.clone(),
            format!("{repo}-{slug}"),
            BRIEF.to_owned(),
        )
    }

    /// The iterate loop for a ticket case's fix: its worktree (made here
    /// unless the plan session already made it), a session with the fix
    /// prompt and the iterate brief.
    pub fn start_at(
        root: &Path,
        worktree: &Path,
        branch: &str,
        base: &str,
        name: &str,
        prompt: &str,
    ) -> Result<Iterate, String> {
        if !worktree.is_dir() {
            git(
                root,
                &[
                    "worktree",
                    "add",
                    &worktree.to_string_lossy(),
                    "-b",
                    branch,
                    base,
                ],
            )
            .or_else(|_| {
                git(
                    root,
                    &["worktree", "add", &worktree.to_string_lossy(), branch],
                )
            })?;
        }
        Iterate::begin(
            worktree,
            branch.rsplit('/').next().unwrap_or(branch).to_owned(),
            name.to_owned(),
            format!(
                "{prompt}\n\nWork in rounds with the engineer, who watches from Cellworks. {BRIEF}"
            ),
        )
    }

    fn begin(
        worktree: &Path,
        slug: String,
        session_name: String,
        brief: String,
    ) -> Result<Iterate, String> {
        let worktree = worktree.canonicalize().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(worktree.join(".cellworks/shots")).map_err(|e| e.to_string())?;
        // Keep our own files out of the round snapshots.
        let exclude = git(&worktree, &["rev-parse", "--git-path", "info/exclude"])?;
        let exclude = worktree.join(exclude);
        let _ = std::fs::create_dir_all(exclude.parent().unwrap());
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&exclude)
            .map_err(|e| e.to_string())?;
        let _ = writeln!(f, ".cellworks/");

        let spawn_rx = spawn(&worktree, &session_name, &brief);

        let base = snapshot(&worktree, "round 0")?;
        let _ = git(&worktree, &["update-ref", &round_ref(&worktree, 0), &base]);
        let (tx, rx) = channel();
        let (base_tx, base_rx) = channel();
        watch(worktree.clone(), base.clone(), base_rx, tx);
        Ok(Iterate {
            worktree,
            slug,
            session_name,
            brief,
            spawned: None,
            spawn_rx,
            rx,
            base_tx,
            status: Status::default(),
            rounds: vec![Round {
                commit: base,
                files: Vec::new(),
                note: String::new(),
            }],
            viewing: None,
            draft: None,
            capture: None,
            queue: Vec::new(),
            log: Vec::new(),
            toast: None,
        })
    }

    /// Picks up a loop already running in `worktree`, after a restart.
    pub fn attach(worktree: &Path, session_name: &str) -> Result<Iterate, String> {
        let worktree = worktree.canonicalize().map_err(|e| e.to_string())?;
        let notes: Vec<String> = std::fs::read_to_string(worktree.join(".cellworks/feedback.log"))
            .unwrap_or_default()
            .lines()
            .map(|l| {
                l.split(" | ")
                    .next()
                    .unwrap_or(l)
                    .split_once("] ")
                    .map(|x| x.1)
                    .unwrap_or(l)
                    .to_owned()
            })
            .collect();
        let mut rounds = Vec::new();
        while let Ok(commit) = git(
            &worktree,
            &["rev-parse", &round_ref(&worktree, rounds.len())],
        ) {
            let k = rounds.len();
            rounds.push(Round {
                commit,
                files: Vec::new(),
                note: if k == 0 {
                    "base".into()
                } else {
                    notes.get(k - 1).cloned().unwrap_or_default()
                },
            });
        }
        for k in 0..rounds.len().saturating_sub(1) {
            rounds[k].files = changed(&worktree, &rounds[k].commit, &rounds[k + 1].commit);
        }
        let base = rounds.last().ok_or("no rounds")?.commit.clone();
        let slug = worktree
            .file_name()
            .map(|n| {
                n.to_string_lossy()
                    .rsplit("-proto-")
                    .next()
                    .unwrap_or("")
                    .to_owned()
            })
            .unwrap_or_default();
        let (tx, rx) = channel();
        let (base_tx, base_rx) = channel();
        watch(worktree.clone(), base, base_rx, tx);
        let (_, spawn_rx) = channel();
        Ok(Iterate {
            worktree,
            slug,
            session_name: session_name.to_owned(),
            brief: BRIEF.to_owned(),
            spawned: Some(Ok("attached".into())),
            spawn_rx,
            rx,
            base_tx,
            status: Status::default(),
            rounds,
            viewing: None,
            draft: None,
            capture: None,
            queue: Vec::new(),
            log: Vec::new(),
            toast: None,
        })
    }

    /// Starts the session again, after a failed start.
    pub fn respawn(&mut self) {
        self.spawned = None;
        self.spawn_rx = spawn(&self.worktree, &self.session_name, &self.brief);
    }

    pub fn drain(&mut self) {
        while let Ok(status) = self.rx.try_recv() {
            self.status = status;
        }
        if let Ok(spawned) = self.spawn_rx.try_recv() {
            self.log.push(match &spawned {
                Ok(line) => format!("session started: {line}"),
                Err(e) => format!("session failed: {e}"),
            });
            self.spawned = Some(spawned);
        }
        if let Some((child, path)) = &mut self.capture
            && let Ok(Some(_)) = child.try_wait()
        {
            let image = path.exists().then(|| path.clone());
            self.capture = None;
            if image.is_some() {
                let draft = self.draft.get_or_insert(Draft {
                    anchor: None,
                    image: None,
                    note: String::new(),
                });
                draft.image = image;
            }
        }
    }

    /// Bytes due for the pane now.
    pub fn due(&mut self) -> Vec<Vec<u8>> {
        let now = Instant::now();
        let (due, later): (Vec<_>, Vec<_>) = self.queue.drain(..).partition(|(at, _)| *at <= now);
        self.queue = later;
        due.into_iter().map(|(_, bytes)| bytes).collect()
    }

    pub fn url(&self) -> Option<String> {
        self.status
            .port
            .map(|port| format!("http://localhost:{port}"))
    }

    pub fn pin(&mut self, anchor: String) {
        let draft = self.draft.get_or_insert(Draft {
            anchor: None,
            image: None,
            note: String::new(),
        });
        draft.anchor = Some(anchor);
    }

    /// Region capture over any window, via macOS `screencapture -i`.
    pub fn capture(&mut self) {
        // PROTOTYPE: the headless tour attaches its own image.
        if self.capture.is_some() || std::env::var_os("CELLWORKS_DRY_LAUNCH").is_some() {
            return;
        }
        let n = self.rounds.len();
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = self
            .worktree
            .join(format!(".cellworks/shots/round-{n}-{secs}.png"));
        match Command::new("screencapture").arg("-i").arg(&path).spawn() {
            Ok(child) => self.capture = Some((child, path)),
            Err(e) => self.log.push(format!("screencapture failed: {e}")),
        }
    }

    /// Sends the draft: closes the round, types it into the pane, appends
    /// it to the feedback log. `delay` waits for a freshly attached pane.
    pub fn send(&mut self, delay: Duration) {
        let Some(draft) = self.draft.take() else {
            return;
        };
        let n = self.rounds.len() - 1;
        let mut message = format!("[round {}] {}", n + 1, draft.note.trim().replace('\n', " "));
        if self.rounds[n].note.is_empty() {
            self.rounds[n].note = draft.note.trim().to_owned();
        }
        if let Some(anchor) = &draft.anchor {
            message.push_str(&format!(" | at: {anchor}"));
        }
        if let Some(image) = &draft.image {
            message.push_str(&format!(" | screenshot: {}", image.display()));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.worktree.join(".cellworks/feedback.log"))
        {
            let _ = writeln!(f, "{message}");
        }
        let at = Instant::now() + delay;
        self.toast = Some((
            format!("Sent to {}: {message}", self.session_name),
            Instant::now(),
        ));
        self.queue.push((at, message.into_bytes()));
        // Enter on its own, so a paste-detecting TUI submits it.
        self.queue
            .push((at + Duration::from_millis(400), b"\r".to_vec()));
        self.viewing = None;
    }

    /// How many notes went into the open round, from the feedback log.
    pub fn notes_in_round(&self) -> usize {
        let log = std::fs::read_to_string(self.worktree.join(".cellworks/feedback.log"))
            .unwrap_or_default();
        let tag = format!("[round {}]", self.rounds.len());
        log.lines().filter(|l| l.starts_with(&tag)).count()
    }

    /// Closes the round in progress: snapshots the worktree and opens the
    /// next one.
    pub fn close_round(&mut self) {
        let n = self.rounds.len();
        match snapshot(&self.worktree, &format!("round {n}")) {
            Ok(commit) => {
                let prev = &self.rounds[n - 1].commit;
                let files = changed(&self.worktree, prev, &commit);
                let _ = git(
                    &self.worktree,
                    &["update-ref", &round_ref(&self.worktree, n), &commit],
                );
                let _ = self.base_tx.send(commit.clone());
                // What the session did since the last feedback belongs to
                // the round that feedback opened.
                self.rounds[n - 1].files = files;
                self.log.push(format!("round {n} closed"));
                self.toast = Some((
                    format!(
                        "Round {n} closed. Snapshot {}; round {} is open.",
                        round_ref(&self.worktree, n),
                        n + 1
                    ),
                    Instant::now(),
                ));
                self.rounds.push(Round {
                    commit,
                    files: Vec::new(),
                    note: String::new(),
                });
            }
            Err(e) => self.log.push(format!("snapshot failed: {e}")),
        }
        self.append(&format!("--- round {n} closed"));
        self.viewing = None;
    }

    fn append(&self, line: &str) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(self.worktree.join(".cellworks/feedback.log"))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    /// `rel` as the round on show changed it: the viewed round's diff, else
    /// the open round's against its last snapshot, with three lines of
    /// context; the whole file when the round did not touch it. The label
    /// says which.
    pub fn file_view(&self, rel: &str) -> (String, Vec<CodeLine>) {
        let (from, to, label) = match self.viewing {
            Some(k) if k + 1 < self.rounds.len() => (
                self.rounds[k].commit.clone(),
                Some(self.rounds[k + 1].commit.clone()),
                format!("ROUND {}", k + 1),
            ),
            _ => (
                self.rounds
                    .last()
                    .map(|r| r.commit.clone())
                    .unwrap_or_default(),
                snapshot(&self.worktree, "view").ok(),
                format!("ROUND {} · LIVE", self.rounds.len()),
            ),
        };
        let Some(to) = to else {
            return (label, Vec::new());
        };
        self.file_diff(&from, &to, label, rel)
    }

    /// What changed since `base` (committed and not): the files and the
    /// commits.
    pub fn since(&self, base: &str) -> (Vec<String>, Vec<String>) {
        let from =
            git(&self.worktree, &["merge-base", base, "HEAD"]).unwrap_or_else(|_| base.to_owned());
        let files = snapshot(&self.worktree, "review")
            .map(|now| changed(&self.worktree, &from, &now))
            .unwrap_or_default();
        let commits = git(
            &self.worktree,
            &["log", "--oneline", &format!("{from}..HEAD")],
        )
        .map(|t| t.lines().map(str::to_owned).collect())
        .unwrap_or_default();
        (files, commits)
    }

    /// `rel` against `base`, for the review.
    pub fn file_view_since(&self, base: &str, rel: &str) -> (String, Vec<CodeLine>) {
        let from =
            git(&self.worktree, &["merge-base", base, "HEAD"]).unwrap_or_else(|_| base.to_owned());
        match snapshot(&self.worktree, "review") {
            Ok(to) => self.file_diff(&from, &to, format!("SINCE {}", base.to_uppercase()), rel),
            Err(_) => (String::new(), Vec::new()),
        }
    }

    fn file_diff(&self, from: &str, to: &str, label: String, rel: &str) -> (String, Vec<CodeLine>) {
        let diff = git(&self.worktree, &["diff", "-U3", from, to, "--", rel]).unwrap_or_default();
        if diff.is_empty() {
            let text = git(&self.worktree, &["show", &format!("{to}:{rel}")]).unwrap_or_default();
            let lines = text
                .lines()
                .take(600)
                .enumerate()
                .map(|(i, l)| CodeLine {
                    no: Some(i + 1),
                    mark: ' ',
                    text: l.to_owned(),
                })
                .collect();
            return (format!("{label} · UNCHANGED"), lines);
        }
        let mut lines = Vec::new();
        let mut no = 0;
        for line in diff.lines() {
            if let Some(rest) = line.strip_prefix("@@") {
                // @@ -a,b +c,d @@
                let new = rest.split('+').nth(1).unwrap_or("1");
                no = new
                    .split([',', ' '])
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(1);
                if !lines.is_empty() {
                    lines.push(CodeLine {
                        no: None,
                        mark: '…',
                        text: "...".into(),
                    });
                }
            } else if line.starts_with("+++")
                || line.starts_with("---")
                || line.starts_with("diff ")
                || line.starts_with("index ")
                || line.starts_with("new file")
                || line.starts_with("\\")
            {
            } else if let Some(t) = line.strip_prefix('+') {
                lines.push(CodeLine {
                    no: Some(no),
                    mark: '+',
                    text: t.to_owned(),
                });
                no += 1;
            } else if let Some(t) = line.strip_prefix('-') {
                lines.push(CodeLine {
                    no: None,
                    mark: '-',
                    text: t.to_owned(),
                });
            } else if !lines.is_empty() || no > 0 {
                lines.push(CodeLine {
                    no: Some(no),
                    mark: ' ',
                    text: line.strip_prefix(' ').unwrap_or(line).to_owned(),
                });
                no += 1;
            }
        }
        (label, lines)
    }

    /// Types `line` into the session's pane and submits it.
    pub fn type_line(&mut self, line: String) {
        let at = Instant::now();
        self.toast = Some((format!("Sent to {}: {line}", self.session_name), at));
        self.queue.push((at, line.into_bytes()));
        self.queue
            .push((at + Duration::from_millis(400), b"\r".to_vec()));
    }

    /// Puts the worktree back to round `k`'s snapshot.
    pub fn revert(&mut self, k: usize) {
        let Some(target) = self.rounds.get(k).map(|r| r.commit.clone()) else {
            return;
        };
        let now = match snapshot(&self.worktree, "before revert") {
            Ok(now) => now,
            Err(e) => {
                self.log.push(format!("revert failed: {e}"));
                return;
            }
        };
        let added = git(
            &self.worktree,
            &["diff", "--name-only", "--diff-filter=A", &target, &now],
        )
        .unwrap_or_default();
        for file in added.lines() {
            let _ = std::fs::remove_file(self.worktree.join(file));
        }
        match git(
            &self.worktree,
            &["restore", "--source", &target, "--worktree", "--", ":/"],
        ) {
            Ok(_) => {
                self.log.push(format!("undid round {} and later", k + 1));
                self.toast = Some((
                    format!("Undid round {} and later. The session was told.", k + 1),
                    Instant::now(),
                ));
                self.append(&format!("--- undid round {} and later", k + 1));
                let notice = format!(
                    "[undo] Cellworks reset the worktree to how it was before round {r}. \
                     Your changes from round {r} on are gone; re-read files before editing.",
                    r = k + 1
                );
                let at = Instant::now();
                self.queue.push((at, notice.into_bytes()));
                self.queue
                    .push((at + Duration::from_millis(400), b"\r".to_vec()));
            }
            Err(e) => self.log.push(format!("revert failed: {e}")),
        }
    }

    /// The files whose scaffolding shows: the viewed round's, or the live
    /// round's.
    pub fn scaffolding(&self) -> &[String] {
        match self.viewing {
            Some(k) => self.rounds.get(k).map_or(&[], |r| r.files.as_slice()),
            None => &self.status.touched,
        }
    }

    /// Steps through the closed rounds; past the last one is the live round.
    pub fn step(&mut self, by: isize) {
        let live = self.rounds.len() as isize - 1;
        let at = self.viewing.map_or(live, |k| k as isize) + by;
        self.viewing = if at >= live {
            None
        } else {
            Some(at.max(0) as usize)
        };
    }
}

fn spawn(worktree: &Path, name: &str, brief: &str) -> Receiver<Result<String, String>> {
    let brief = brief.to_owned();
    let (spawn_tx, spawn_rx) = channel();
    // PROTOTYPE (stakeholder demo): no real session; the tour plays the agent.
    if std::env::var_os("CELLWORKS_DEMO").is_some() {
        let _ = spawn_tx.send(Ok("demo".to_owned()));
        return spawn_rx;
    }
    let worktree = worktree.to_owned();
    let name = name.to_owned();
    std::thread::spawn(move || {
        let out = Command::new("claude")
            .args(["--bg", "-n", &name, &brief])
            .current_dir(&worktree)
            .stdin(std::process::Stdio::null())
            .output();
        let result = match out {
            Ok(out) => {
                let text = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
                if out.status.success() {
                    Ok(text.lines().next().unwrap_or("").to_owned())
                } else {
                    Err(text.trim().to_owned())
                }
            }
            Err(e) => Err(e.to_string()),
        };
        let _ = spawn_tx.send(result);
    });
    spawn_rx
}

fn watch(worktree: PathBuf, base: String, base_rx: Receiver<String>, tx: Sender<Status>) {
    std::thread::spawn(move || {
        let mut base = base;
        loop {
            while let Ok(next) = base_rx.try_recv() {
                base = next;
            }
            let port = launch_port(&worktree);
            let live = port.is_some_and(probe);
            let fires = std::fs::read_to_string(worktree.join(".cellworks/run.log"))
                .map(|log| fires(&worktree, &log))
                .unwrap_or_default();
            let touched = snapshot(&worktree, "live")
                .map(|now| changed(&worktree, &base, &now))
                .unwrap_or_default();
            if tx
                .send(Status {
                    port,
                    live,
                    fires,
                    touched,
                })
                .is_err()
            {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn launch_port(worktree: &Path) -> Option<u16> {
    let text = std::fs::read_to_string(worktree.join(".claude/launch.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json["configurations"]
        .as_array()?
        .iter()
        .find_map(|c| c["port"].as_u64())
        .map(|p| p as u16)
}

fn probe(port: u16) -> bool {
    ["127.0.0.1", "[::1]"].iter().any(|host| {
        format!("{host}:{port}").parse().is_ok_and(|addr| {
            std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(150)).is_ok()
        })
    })
}

/// `path:line(:col)` and `path(line,col)` mentions of files in the worktree,
/// since the last line that reads like a clean build.
// ponytail: one generic heuristic, a per-tool matcher if this misfires.
fn fires(worktree: &Path, log: &str) -> Vec<(String, u32, String)> {
    const CLEAN: [&str; 6] = [
        "found 0 errors",
        "hmr update",
        "page reload",
        "ready in",
        "finished `",
        "compiled successfully",
    ];
    let lines: Vec<&str> = log.lines().collect();
    let start = lines
        .iter()
        .rposition(|l| {
            let l = l.to_lowercase();
            CLEAN.iter().any(|c| l.contains(c))
        })
        .map_or(0, |i| i + 1);
    let mut out: Vec<(String, u32, String)> = Vec::new();
    for line in &lines[start..] {
        for token in line.split_whitespace() {
            let token = token.trim_matches(|c: char| "\"'`,;[]".contains(c));
            let Some((file, n)) = locate(token) else {
                continue;
            };
            let rel = Path::new(file)
                .strip_prefix(worktree)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| PathBuf::from(file));
            if rel.is_relative()
                && worktree.join(&rel).is_file()
                && !out
                    .iter()
                    .any(|(f, l, _)| *f == rel.to_string_lossy() && *l == n)
            {
                out.push((
                    rel.to_string_lossy().into_owned(),
                    n,
                    line.trim().to_owned(),
                ));
            }
        }
    }
    out
}

fn locate(token: &str) -> Option<(&str, u32)> {
    // path(line,col)
    if let Some(open) = token.find('(') {
        let file = &token[..open];
        let n = token[open + 1..].split([',', ')']).next()?.parse().ok()?;
        return Some((file, n));
    }
    // path:line(:col)
    let mut parts = token.split(':');
    let file = parts.next()?;
    let n = parts.next()?.parse().ok()?;
    file.contains('.').then_some((file, n))
}
