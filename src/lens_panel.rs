//! The lens panel: the project's own lenses, how each relation is drawn, and
//! a Claude session that creates and refines them from a question and
//! feedback.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::SystemTime;

use cellworks_map::project::{self, Viz};

use crate::opened::OpenedProject;

/// The skill Claude follows, shipped with Cellworks.
const SKILL: &str = include_str!("../.claude/skills/cellworks-lens/SKILL.md");

#[derive(Default)]
pub struct LensPanel {
    pub open: bool,
    pub autostarted: bool,
    pub question: String,
    feedback: String,
    log: Vec<String>,
    /// Lines of Claude's stream, while it runs.
    run: Option<Receiver<String>>,
    session: Option<String>,
    /// The lens Claude works on.
    target: Option<String>,
    names: Vec<String>,
    checked: f64,
    /// When the shown lens's files last changed, as last seen.
    seen: Option<(Option<SystemTime>, Option<SystemTime>)>,
    error: Option<String>,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn slug(question: &str) -> String {
    let words: Vec<String> = question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .take(4)
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        "lens".to_owned()
    } else {
        words.join("-")
    }
}

impl LensPanel {
    /// The lens to show once the project is read.
    pub fn preset(&mut self, name: Option<String>) {
        self.target = name;
    }

    /// Reloads the shown lens when its files change on disk, and picks up a
    /// lens Claude just created. Once a second.
    pub fn watch(&mut self, opened: &mut OpenedProject, now: f64) {
        if now - self.checked < 1.0 {
            return;
        }
        self.checked = now;
        let Some(root) = opened.root().map(Path::to_path_buf) else {
            return;
        };
        self.names = project::list(&root);
        let shown = opened.project_lens().map(|(n, _)| n.clone());
        let name = match (&shown, &self.target) {
            (Some(n), _) => n.clone(),
            (None, Some(t)) if self.names.contains(t) => t.clone(),
            _ => return,
        };
        let stamp = (
            modified(&project::def_path(&root, &name)),
            modified(&project::data_path(&root, &name)),
        );
        if shown.as_deref() == Some(name.as_str()) && self.seen == Some(stamp) {
            return;
        }
        self.seen = Some(stamp);
        self.show(opened, &root, &name, now);
    }

    fn show(&mut self, opened: &mut OpenedProject, root: &Path, name: &str, now: f64) {
        match project::load(root, name) {
            Ok(lens) => {
                self.error = None;
                opened.show_project_lens(name.to_owned(), lens, now);
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Demo: adds a line to the conversation.
    pub fn say(&mut self, line: String) {
        self.log.push(line);
    }

    /// Demo: shows lens `name` straight away, as if Claude had just built it.
    pub fn show_now(&mut self, opened: &mut OpenedProject, name: &str, now: f64) {
        let Some(root) = opened.root().map(Path::to_path_buf) else {
            return;
        };
        self.open = true;
        self.target = Some(name.to_owned());
        self.names = project::list(&root);
        self.seen = Some((
            modified(&project::def_path(&root, name)),
            modified(&project::data_path(&root, name)),
        ));
        self.show(opened, &root, name, now);
    }

    /// Drains Claude's stream into the log.
    pub fn drain(&mut self) {
        while let Some(run) = &self.run {
            match run.try_recv() {
                Ok(line) => self.read(&line),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.run = None,
            }
        }
    }

    fn read(&mut self, line: &str) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            self.log.push(line.to_owned());
            return;
        };
        if let Some(id) = event["session_id"].as_str() {
            self.session = Some(id.to_owned());
        }
        match event["type"].as_str() {
            Some("assistant") => {
                for part in event["message"]["content"].as_array().into_iter().flatten() {
                    match part["type"].as_str() {
                        Some("text") => {
                            self.log
                                .push(part["text"].as_str().unwrap_or_default().to_owned());
                        }
                        Some("tool_use") => {
                            let input = &part["input"];
                            let what = input["command"]
                                .as_str()
                                .or(input["file_path"].as_str())
                                .or(input["pattern"].as_str())
                                .unwrap_or_default();
                            let what: String = what.chars().take(140).collect();
                            self.log.push(format!(
                                "› {} {what}",
                                part["name"].as_str().unwrap_or_default()
                            ));
                        }
                        _ => {}
                    }
                }
            }
            Some("result") => {
                let cost = event["total_cost_usd"].as_f64().unwrap_or_default();
                self.log.push(format!("— done (${cost:.2})"));
            }
            _ => {}
        }
    }

    fn ask(&mut self, root: &Path, prompt: String, resume: bool) {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("cellworks"));
        let prompt = format!(
            "{prompt}\n\nThe cellworks binary is `{}`. The repo is `{}`.",
            exe.display(),
            root.display()
        );
        let mut command = Command::new("claude");
        command
            .current_dir(root)
            .arg("-p")
            .arg(&prompt)
            .args(["--output-format", "stream-json", "--verbose"])
            .args(["--append-system-prompt", SKILL])
            // ponytail: broad tool allow-list for a local prototype; scope
            // Bash to the cellworks binary in the real implementation.
            .args(["--allowedTools", "Bash Read Write Edit Glob Grep"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if resume && let Some(session) = &self.session {
            command.args(["--resume", session]);
        }
        self.log
            .push(format!("» {}", prompt.lines().next().unwrap_or_default()));
        let (sender, receiver) = channel();
        match command.spawn() {
            Ok(mut child) => {
                let out = child.stdout.take();
                let err = child.stderr.take();
                let errors = sender.clone();
                std::thread::spawn(move || {
                    if let Some(err) = err {
                        for line in BufReader::new(err).lines().map_while(Result::ok) {
                            let _ = errors.send(format!("stderr: {line}"));
                        }
                    }
                });
                std::thread::spawn(move || {
                    if let Some(out) = out {
                        for line in BufReader::new(out).lines().map_while(Result::ok) {
                            if sender.send(line).is_err() {
                                break;
                            }
                        }
                    }
                    let _ = child.wait();
                });
                self.run = Some(receiver);
            }
            Err(e) => self.log.push(format!("could not start claude: {e}")),
        }
    }

    /// Draws the panel.
    pub fn ui(&mut self, ui: &mut egui::Ui, opened: &mut OpenedProject, now: f64) {
        let Some(root) = opened.root().map(Path::to_path_buf) else {
            ui.label("Reading the project…");
            return;
        };
        let running = self.run.is_some();
        if running {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        crate::demo::mark(ui.ctx(), "lens.panel", ui.max_rect());
        ui.heading("Project lenses");
        let shown = opened.project_lens().map(|(n, l)| (n.clone(), l.clone()));
        for name in self.names.clone() {
            let on = shown.as_ref().is_some_and(|(n, _)| *n == name);
            if ui.selectable_label(on, &name).clicked() {
                self.target = Some(name.clone());
                self.seen = None;
                self.show(opened, &root, &name, now);
            }
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }

        ui.separator();
        ui.label("New lens: what should the map answer?");
        let field = ui.add(
            egui::TextEdit::multiline(&mut self.question)
                .desired_rows(2)
                .hint_text("Which tests cover which class?"),
        );
        crate::demo::mark(ui.ctx(), "lens.question", field.rect);
        let create = ui.add_enabled(
            !running && !self.question.trim().is_empty(),
            egui::Button::new("Create with Claude"),
        );
        crate::demo::mark(ui.ctx(), "lens.create", create.rect);
        // PROTOTYPE (stakeholder demo): the tour answers with a scripted turn.
        if create.clicked() && crate::demo::is_demo() {
            self.question.clear();
        } else if create.clicked() {
            let name = slug(&self.question);
            self.target = Some(name.clone());
            self.session = None;
            let prompt = format!(
                "Create the Cellworks project lens `{name}` that answers: {}",
                self.question.trim()
            );
            self.ask(&root, prompt, false);
        }

        if let Some((name, lens)) = &shown {
            ui.separator();
            ui.strong(name);
            ui.label(&lens.def.question);
            if lens.stale() {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "data is stale: rules changed since the last build",
                );
            }
            let mut def = lens.def.clone();
            let mut changed = ui
                .checkbox(
                    &mut def.cut.split_files,
                    "one tower per function (split files)",
                )
                .changed();
            for relation in &mut def.relations {
                let count = lens
                    .data
                    .relations
                    .iter()
                    .filter(|l| l.kind == relation.kind)
                    .count();
                ui.horizontal(|ui| {
                    ui.label(format!("{} ({count})", relation.kind));
                    egui::ComboBox::from_id_salt(&relation.kind)
                        .selected_text(relation.viz.name())
                        .show_ui(ui, |ui| {
                            for viz in Viz::ALL {
                                changed |= ui
                                    .selectable_value(&mut relation.viz, viz, viz.name())
                                    .changed();
                            }
                        });
                });
            }
            let u = &lens.data.unmatched;
            ui.label(format!(
                "{} files · {} underground · unplaced {} · unhung {}",
                lens.data.entities.len(),
                lens.data.entities.iter().filter(|e| e.underground).count(),
                u.unplaced.len(),
                u.unhung.len()
            ))
            .on_hover_text(u.unhung.join("\n"));
            if changed {
                match project::save_def(&root, name, &def) {
                    Ok(()) => self.checked = 0.0,
                    Err(e) => self.error = Some(e),
                }
            }
            ui.label("Feedback:");
            ui.add(egui::TextEdit::multiline(&mut self.feedback).desired_rows(2));
            if ui
                .add_enabled(
                    !running && !self.feedback.trim().is_empty(),
                    egui::Button::new("Send to Claude"),
                )
                .clicked()
            {
                let resume = self.session.is_some() && self.target.as_deref() == Some(name);
                let prompt = format!(
                    "Feedback on the Cellworks project lens `{name}`: {}",
                    self.feedback.trim()
                );
                self.target = Some(name.clone());
                self.ask(&root, prompt, resume);
                self.feedback.clear();
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label(if running {
                "Claude is working…"
            } else {
                "Claude"
            });
            if running {
                ui.spinner();
            }
        });
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log {
                    ui.add(egui::Label::new(egui::RichText::new(line).small()).wrap());
                }
            });
    }
}
