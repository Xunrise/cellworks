//! Builds or recalculates a project's context cells with Claude Code run
//! headless: Claude reads the code and writes the bounded contexts and their
//! context map under `docs/domain/`, in the form the DomainKnowledge reader
//! takes, and the cells lens is read again once it is done.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// What Claude is asked to do: the layout `cellworks_domain_knowledge::read`
/// takes, and to keep what people already wrote.
const PROMPT: &str = "\
Build or recalculate the bounded contexts (context cells) of this repository for Cellworks.

Read the code and any existing docs first. Then write, under docs/domain/:
- one folder per bounded context, docs/domain/contexts/<Name>/<Name>.md, starting with the heading
  `# Context: <Name>` and, as the first line after it, one sentence of what the context is for;
  below it, its responsibilities, its terms and the folders that implement it;
- docs/domain/context-map.md with a table whose rows read
  `| <n> | <Upstream> | <Downstream> | <Type> | <Clarity> | <Notes> |` under the header
  `| # | Upstream | Downstream | Type | Clarity | Notes |`, one row per relationship.

Name each context after the folder that implements it where there is one, in PascalCase
(the folder `domain-knowledge` implements `DomainKnowledge`), so the map can place its files.
Where docs/domain already exists, keep what people wrote: add contexts the code now has, update
descriptions and relationships that no longer match the code, and remove only contexts whose
code is gone. Write nothing outside docs/domain/. End with one line naming the contexts.";

/// Where a build stands.
#[derive(Clone, Debug, PartialEq)]
pub enum CellsState {
    Idle,
    /// Claude is working: how many steps so far, and the last thing it said
    /// or did.
    Running {
        steps: usize,
        last: String,
    },
    /// Finished at `at`, costing `cost` dollars, with Claude's last words.
    Done {
        cost: f64,
        summary: String,
        at: f64,
    },
    Failed(String),
}

/// One build of the context cells, for one project at a time.
pub struct CellsBuild {
    pub state: CellsState,
    /// The project being built.
    pub root: Option<PathBuf>,
    program: PathBuf,
    run: Option<Receiver<String>>,
    errors: Vec<String>,
}

impl Default for CellsBuild {
    fn default() -> Self {
        Self::with_program("claude")
    }
}

impl CellsBuild {
    /// A build that runs `program` in place of `claude`, for tests.
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        CellsBuild {
            state: CellsState::Idle,
            root: None,
            program: program.into(),
            run: None,
            errors: Vec::new(),
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, CellsState::Running { .. })
    }

    /// Starts Claude on the project at `root`, unless a build already runs.
    pub fn start(&mut self, root: &Path) {
        if self.is_running() {
            return;
        }
        self.errors.clear();
        self.root = Some(root.to_path_buf());
        let mut command = Command::new(&self.program);
        command
            .current_dir(root)
            .arg("-p")
            .arg(PROMPT)
            .args(["--output-format", "stream-json", "--verbose"])
            // It reads the code and writes only the domain docs.
            .args(["--allowedTools", "Read Glob Grep Write Edit"])
            .args(["--permission-mode", "acceptEdits"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.state = CellsState::Failed(format!(
                    "could not start {}: {error}",
                    self.program.display()
                ));
                return;
            }
        };
        let (sender, receiver) = channel();
        let (out, err) = (child.stdout.take(), child.stderr.take());
        let errors = sender.clone();
        std::thread::spawn(move || {
            for line in err
                .into_iter()
                .flat_map(|e| BufReader::new(e).lines().map_while(Result::ok))
            {
                let _ = errors.send(format!("\u{0}stderr {line}"));
            }
        });
        std::thread::spawn(move || {
            for line in out
                .into_iter()
                .flat_map(|o| BufReader::new(o).lines().map_while(Result::ok))
            {
                if sender.send(line).is_err() {
                    return;
                }
            }
            let status = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            let _ = sender.send(format!("\u{0}exit {status}"));
        });
        self.run = Some(receiver);
        self.state = CellsState::Running {
            steps: 0,
            last: "starting Claude".to_owned(),
        };
    }

    /// Takes what Claude wrote since the last frame; true once the build
    /// has just succeeded, so the cells are read again.
    pub fn drain(&mut self, now: f64) -> bool {
        let mut finished = false;
        while let Some(run) = &self.run {
            match run.try_recv() {
                Ok(line) => finished |= self.read(&line, now),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.run = None;
                    if self.is_running() {
                        self.state = CellsState::Failed("Claude stopped without a result".into());
                    }
                }
            }
        }
        finished
    }

    fn read(&mut self, line: &str, now: f64) -> bool {
        if let Some(error) = line.strip_prefix("\u{0}stderr ") {
            self.errors.push(error.to_owned());
            return false;
        }
        if let Some(code) = line.strip_prefix("\u{0}exit ") {
            if self.is_running() {
                let why = self.errors.last().cloned().unwrap_or_default();
                self.state = CellsState::Failed(format!("Claude exited with {code}. {why}"));
            }
            return false;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        match event["type"].as_str() {
            Some("assistant") => {
                if let CellsState::Running { steps, last } = &mut self.state {
                    for part in event["message"]["content"].as_array().into_iter().flatten() {
                        *steps += 1;
                        *last = match part["type"].as_str() {
                            Some("tool_use") => {
                                let input = &part["input"];
                                let what = input["file_path"]
                                    .as_str()
                                    .or(input["pattern"].as_str())
                                    .unwrap_or_default();
                                format!("{} {what}", part["name"].as_str().unwrap_or_default())
                            }
                            _ => part["text"].as_str().unwrap_or_default().trim().to_owned(),
                        };
                    }
                }
                false
            }
            Some("result") => {
                let summary = event["result"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                if event["is_error"].as_bool().unwrap_or(false) {
                    self.state = CellsState::Failed(summary);
                    return false;
                }
                self.state = CellsState::Done {
                    cost: event["total_cost_usd"].as_f64().unwrap_or_default(),
                    summary,
                    at: now,
                };
                true
            }
            _ => false,
        }
    }

    /// A line on where the build stands, for the map and the settings;
    /// `None` when idle or long finished at `now`.
    pub fn status(&self, now: f64) -> Option<(String, bool)> {
        match &self.state {
            CellsState::Idle => None,
            CellsState::Running { steps, last } => {
                let last: String = last
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .chars()
                    .take(90)
                    .collect();
                Some((
                    format!("Claude is building the context cells · {steps} steps · {last}"),
                    false,
                ))
            }
            CellsState::Done { cost, summary, at } if now - at < 12.0 => {
                let summary: String = summary
                    .lines()
                    .last()
                    .unwrap_or_default()
                    .chars()
                    .take(90)
                    .collect();
                Some((
                    format!("Context cells rebuilt (${cost:.2}) · {summary}"),
                    false,
                ))
            }
            CellsState::Done { .. } => None,
            CellsState::Failed(error) => {
                let error: String = error.chars().take(160).collect();
                Some((format!("Building the context cells failed: {error}"), true))
            }
        }
    }

    /// Forgets a failure once it has been read.
    pub fn dismiss(&mut self) {
        if matches!(self.state, CellsState::Failed(_)) {
            self.state = CellsState::Idle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for `claude` that prints `stream` and exits with `code`.
    fn fake(dir: &Path, stream: &str, code: i32) -> PathBuf {
        let path = dir.join("claude");
        std::fs::write(
            &path,
            format!("#!/bin/sh\ncat <<'EOF'\n{stream}\nEOF\nexit {code}\n"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn finish(build: &mut CellsBuild) -> bool {
        let started = std::time::Instant::now();
        let mut done = false;
        while build.run.is_some() && started.elapsed().as_secs() < 10 {
            done |= build.drain(1.0);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        done
    }

    #[test]
    fn a_build_reports_its_steps_then_its_cost() {
        let dir = tempfile::tempdir().unwrap();
        let stream = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"docs/domain/contexts/Map/Map.md"}}]}}
{"type":"result","is_error":false,"total_cost_usd":0.42,"result":"Contexts: Map, Editing"}"#;
        let mut build = CellsBuild::with_program(fake(dir.path(), stream, 0));
        build.start(dir.path());
        assert!(build.is_running());
        assert!(finish(&mut build), "the build reports success once");
        let (line, failed) = build.status(2.0).expect("a status");
        assert!(!failed);
        assert!(
            line.contains("$0.42") && line.contains("Map, Editing"),
            "{line}"
        );
    }

    #[test]
    fn a_build_that_fails_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let mut build = CellsBuild::with_program(fake(dir.path(), "not json", 3));
        build.start(dir.path());
        assert!(!finish(&mut build));
        let (line, failed) = build.status(2.0).expect("a status");
        assert!(failed && line.contains("exited with 3"), "{line}");
    }

    #[test]
    fn a_missing_claude_is_a_failure_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let mut build = CellsBuild::with_program(dir.path().join("no-such-claude"));
        build.start(dir.path());
        assert!(matches!(build.state, CellsState::Failed(_)));
    }
}
