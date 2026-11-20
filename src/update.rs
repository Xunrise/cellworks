//! Self-update from GitHub Releases, after Palaver's design.
//!
//! The latest release carries `manifest.json` (version, and the app zip's
//! URL, size and sha256). Running from a `.app` bundle, Cellworks reads it at
//! start and hourly. When it names a newer version the corner offers it; a
//! click downloads the zip to a `.part` file, checks size and sha256, unpacks
//! it with `ditto` into the staging folder and verifies its code signature.
//! "Restart" then hands over to a detached `sh` script that waits for this
//! process to exit, moves the old bundle aside, moves the new one in and
//! opens it. The new app writes `started-ok` with its version once it draws;
//! without that within two minutes the script puts the old bundle back.
//!
//! git, curl, shasum, ditto and codesign run as child processes (ADR-0003);
//! results reach the UI thread over `std::sync::mpsc`.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const MANIFEST: &str =
    "https://github.com/Xunrise/cellworks/releases/latest/download/manifest.json";
/// What a click on the corner badge does.
type Click = fn(&mut Updater, &egui::Context);

const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// Seconds the new app must keep drawing before it reports a good start.
const STARTED_AFTER: Duration = Duration::from_secs(5);

/// `manifest.json`, as the release workflow writes it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    pub schema: u32,
    pub version: String,
    pub app: Asset,
    #[serde(default)]
    pub notes: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Not running from a `.app` bundle (cargo run, tests).
    Unsupported,
    Idle,
    Checking,
    UpToDate,
    Available(Manifest),
    Downloading(Manifest),
    Ready(Manifest),
    Failed(String),
}

enum Event {
    Checked(Result<Option<Manifest>, String>),
    Staged(Result<Manifest, String>),
}

pub struct Updater {
    pub state: State,
    /// Whether the hourly check runs; "check now" works either way.
    pub auto_check: bool,
    bundle: Option<PathBuf>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    last_check: Option<Instant>,
    started: Instant,
    reported: bool,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new(running_bundle())
    }
}

impl Updater {
    pub fn new(bundle: Option<PathBuf>) -> Self {
        let (tx, rx) = channel();
        let state = match &bundle {
            Some(_) => match std::fs::read_to_string(dir().join("failed")) {
                Ok(reason) => {
                    let _ = std::fs::remove_file(dir().join("failed"));
                    State::Failed(reason.trim().to_owned())
                }
                // A version downloaded before the last quit is still ready.
                Err(_) => staged().map_or(State::Idle, State::Ready),
            },
            None => State::Unsupported,
        };
        Updater {
            state,
            auto_check: true,
            bundle,
            tx,
            rx,
            last_check: None,
            started: Instant::now(),
            reported: false,
        }
    }

    /// Once per frame: drains results, reports a good start, checks when due.
    pub fn tick(&mut self, ctx: &egui::Context) {
        if self.bundle.is_none() {
            return;
        }
        // egui draws on demand; wake up for the start report and the checks.
        ctx.request_repaint_after(if self.reported {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(1)
        });
        while let Ok(event) = self.rx.try_recv() {
            self.state = match event {
                Event::Checked(Ok(Some(manifest))) => State::Available(manifest),
                Event::Checked(Ok(None)) => State::UpToDate,
                // Offline or GitHub down: stay quiet, try again next hour.
                Event::Checked(Err(_)) => State::Idle,
                Event::Staged(Ok(manifest)) => State::Ready(manifest),
                Event::Staged(Err(error)) => State::Failed(error),
            };
        }
        if !self.reported && self.started.elapsed() >= STARTED_AFTER {
            self.reported = true;
            let _ = std::fs::create_dir_all(dir());
            let _ = std::fs::write(dir().join("started-ok"), env!("CARGO_PKG_VERSION"));
        }
        let quiet = matches!(self.state, State::Idle | State::UpToDate | State::Failed(_));
        if quiet && self.auto_check && self.last_check.is_none_or(|t| t.elapsed() >= CHECK_EVERY) {
            self.check();
        }
    }

    /// Checks now, whatever `auto_check` says; nothing outside a bundle or
    /// while a check or a download is under way.
    pub fn check_now(&mut self) {
        if self.bundle.is_some()
            && matches!(self.state, State::Idle | State::UpToDate | State::Failed(_))
        {
            self.check();
        }
    }

    fn check(&mut self) {
        self.last_check = Some(Instant::now());
        self.state = State::Checking;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result =
                fetch_manifest().map(|m| newer(&m, env!("CARGO_PKG_VERSION")).then_some(m));
            let _ = tx.send(Event::Checked(result));
        });
    }

    /// Downloads and stages the offered version.
    pub fn download(&mut self) {
        let State::Available(manifest) = self.state.clone() else {
            return;
        };
        self.state = State::Downloading(manifest.clone());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = stage(&manifest).map(|()| manifest);
            if result.is_err() {
                let _ = std::fs::remove_dir_all(dir().join("staging"));
            }
            let _ = tx.send(Event::Staged(result));
        });
    }

    /// Starts the swap script and quits; the script reopens Cellworks.
    pub fn restart(&mut self, ctx: &egui::Context) {
        let (State::Ready(manifest), Some(bundle)) = (&self.state, &self.bundle) else {
            return;
        };
        match swap(bundle, &manifest.version) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(error) => self.state = State::Failed(error),
        }
    }

    /// The corner badge: nothing while idle, an offer, progress or the
    /// restart button otherwise.
    pub fn corner(&mut self, ctx: &egui::Context) {
        use cellworks_workspace::theme;
        let (text, colour, action): (String, egui::Color32, Option<Click>) = match &self.state {
            State::Available(m) => (
                format!("UPDATE {} AVAILABLE", m.version),
                theme::YELLOW,
                Some(|u, _| u.download()),
            ),
            State::Downloading(m) => (format!("DOWNLOADING {}…", m.version), theme::DIM, None),
            State::Ready(m) => (
                format!("RESTART FOR {}", m.version),
                theme::OK,
                Some(|u, ctx| u.restart(ctx)),
            ),
            State::Failed(e) => (format!("UPDATE FAILED: {e}"), theme::DANGER, None),
            _ => return,
        };
        let notes = match &self.state {
            State::Available(m) | State::Ready(m) => m.notes.clone(),
            _ => String::new(),
        };
        egui::Area::new(egui::Id::new("update-corner"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                theme::frame().inner_margin(8).show(ui, |ui| {
                    let label = egui::RichText::new(text)
                        .monospace()
                        .size(11.0)
                        .color(colour);
                    let response = match action {
                        Some(_) => ui.add(egui::Button::new(label).frame(false)),
                        None => ui.label(label),
                    };
                    let response = if notes.is_empty() {
                        response
                    } else {
                        response.on_hover_text(notes)
                    };
                    if response.clicked()
                        && let Some(action) = action
                    {
                        action(self, ctx);
                    }
                });
            });
        if matches!(self.state, State::Downloading(_)) {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

/// True when `manifest` names a version newer than `current`.
fn newer(manifest: &Manifest, current: &str) -> bool {
    match (
        semver::Version::parse(&manifest.version),
        semver::Version::parse(current),
    ) {
        (Ok(offered), Ok(running)) => offered > running,
        _ => false,
    }
}

/// The checks Palaver makes before trusting a manifest.
fn valid(manifest: &Manifest) -> Result<(), String> {
    let name_ok = manifest
        .app
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
        && !manifest.app.name.starts_with('.');
    let hash_ok = manifest.app.sha256.len() == 64
        && manifest
            .app
            .sha256
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c));
    if manifest.schema != 1 {
        Err(format!("unknown manifest schema {}", manifest.schema))
    } else if !name_ok {
        Err("the manifest names an odd file".to_owned())
    } else if !manifest.app.url.starts_with("https://") {
        Err("the manifest's download is not https".to_owned())
    } else if !hash_ok {
        Err("the manifest's sha256 is malformed".to_owned())
    } else {
        Ok(())
    }
}

fn fetch_manifest() -> Result<Manifest, String> {
    let url = std::env::var("CELLWORKS_MANIFEST_URL").unwrap_or_else(|_| MANIFEST.to_owned());
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--max-time",
            "30",
            "-A",
            "Cellworks",
        ])
        .arg(&url)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err("could not read the latest release".to_owned());
    }
    let manifest: Manifest =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("bad manifest: {e}"))?;
    valid(&manifest)?;
    Ok(manifest)
}

/// Downloads, verifies and unpacks the release into `staging/Cellworks.app`.
fn stage(manifest: &Manifest) -> Result<(), String> {
    let staging = dir().join("staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let part = staging.join(format!("{}.part", manifest.app.name));
    let ok = Command::new("curl")
        .args([
            "-fsSL",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "-A",
            "Cellworks",
            "-o",
        ])
        .arg(&part)
        .arg(&manifest.app.url)
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("curl: {e}"))?
        .success();
    if !ok {
        return Err("the download failed; nothing was changed".to_owned());
    }
    let size = std::fs::metadata(&part).map_err(|e| e.to_string())?.len();
    if size != manifest.app.size || sha256(&part)? != manifest.app.sha256 {
        return Err("the download arrived damaged; nothing was changed".to_owned());
    }
    let unpacked = staging.join("app");
    run(Command::new("/usr/bin/ditto")
        .arg("-xk")
        .arg(&part)
        .arg(&unpacked))?;
    let app = unpacked.join("Cellworks.app");
    run(Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&app))
    .map_err(|_| "the new version's signature does not verify; nothing was changed".to_owned())?;
    let version = Command::new(app.join("Contents/MacOS/cellworks"))
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    let said = String::from_utf8_lossy(&version.stdout);
    if said.trim() != format!("cellworks {}", manifest.version) {
        return Err(format!("the download says it is {}", said.trim()));
    }
    let _ = std::fs::remove_file(&part);
    let kept = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    std::fs::write(staging.join("manifest.json"), kept).map_err(|e| e.to_string())?;
    Ok(())
}

/// The staged version, when one newer than this build waits in `staging`
/// and its app still says it is that version.
fn staged() -> Option<Manifest> {
    let staging = dir().join("staging");
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(staging.join("manifest.json")).ok()?).ok()?;
    if !newer(&manifest, env!("CARGO_PKG_VERSION")) {
        let _ = std::fs::remove_dir_all(&staging);
        return None;
    }
    let out = Command::new(staging.join("app/Cellworks.app/Contents/MacOS/cellworks"))
        .arg("--version")
        .output()
        .ok()?;
    (String::from_utf8_lossy(&out.stdout).trim() == format!("cellworks {}", manifest.version))
        .then_some(manifest)
}

fn sha256(path: &Path) -> Result<String, String> {
    let out = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .map_err(|e| format!("shasum: {e}"))?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| "shasum printed nothing".to_owned())
}

fn run(command: &mut Command) -> Result<(), String> {
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{command:?} failed"))
}

/// Writes and starts the swap script, detached so it outlives this process.
fn swap(bundle: &Path, version: &str) -> Result<(), String> {
    let dir = dir();
    let script = dir.join("swap.sh");
    let _ = std::fs::remove_file(dir.join("started-ok"));
    std::fs::write(&script, SWAP).map_err(|e| e.to_string())?;
    Command::new("/bin/sh")
        .arg(&script)
        .arg(std::process::id().to_string())
        .arg(bundle)
        .arg(dir.join("staging/app/Cellworks.app"))
        .arg(&dir)
        .arg(version)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start the swap: {e}"))
}

/// `swap.sh <pid> <target.app> <staged.app> <dir> <version>`.
const SWAP: &str = r#"#!/bin/sh
pid="$1"; target="$2"; staged="$3"; dir="$4"; version="$5"
previous="$dir/previous/Cellworks.app"
fail() { printf '%s\n' "$1" > "$dir/failed"; }
while kill -0 "$pid" 2>/dev/null; do sleep 0.2; done
rm -rf "$dir/previous"; mkdir -p "$dir/previous"
if ! mv "$target" "$previous"; then fail "could not move the old version aside"; open "$target"; exit 1; fi
if ! mv "$staged" "$target"; then mv "$previous" "$target"; fail "could not move the new version in"; open "$target"; exit 1; fi
open "$target"
i=0
while [ $i -lt 120 ]; do
  if [ "$(cat "$dir/started-ok" 2>/dev/null)" = "$version" ]; then
    rm -rf "$dir/previous" "$dir/staging"; exit 0
  fi
  sleep 1; i=$((i + 1))
done
pkill -f "$target/Contents/MacOS/" 2>/dev/null; sleep 1
rm -rf "$dir/failed.app"; mv "$target" "$dir/failed.app"
mv "$previous" "$target"
fail "$version did not start; kept the version you had"
open "$target"
"#;

/// `~/Library/Application Support/cellworks/updates`.
fn dir() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join("Library/Application Support/cellworks/updates")
}

/// The `.app` this process runs from: `<x>.app/Contents/MacOS/cellworks`.
fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> Manifest {
        Manifest {
            schema: 1,
            version: version.to_owned(),
            app: Asset {
                name: "cellworks-app-0.2.0.zip".to_owned(),
                url: "https://github.com/x/y/releases/download/v0.2.0/cellworks-app-0.2.0.zip"
                    .to_owned(),
                size: 1,
                sha256: "a".repeat(64),
            },
            notes: String::new(),
        }
    }

    #[test]
    fn only_a_newer_version_is_offered() {
        assert!(newer(&manifest("0.2.0"), "0.1.0"));
        assert!(!newer(&manifest("0.1.0"), "0.1.0"));
        assert!(!newer(&manifest("0.1.0-rc1"), "0.1.0"));
        assert!(!newer(&manifest("junk"), "0.1.0"));
    }

    #[test]
    fn a_manifest_must_be_https_with_a_clean_name_and_hash() {
        assert!(valid(&manifest("0.2.0")).is_ok());
        let mut m = manifest("0.2.0");
        m.app.url = "http://example.com/a.zip".to_owned();
        assert!(valid(&m).is_err());
        let mut m = manifest("0.2.0");
        m.app.name = "../evil.zip".to_owned();
        assert!(valid(&m).is_err());
        let mut m = manifest("0.2.0");
        m.app.sha256 = "A".repeat(64);
        assert!(valid(&m).is_err());
        let mut m = manifest("0.2.0");
        m.schema = 2;
        assert!(valid(&m).is_err());
    }

    #[test]
    fn auto_check_off_means_no_hourly_check() {
        let ctx = egui::Context::default();
        let mut updater = Updater::new(Some(PathBuf::from("/nowhere/Cellworks.app")));
        updater.auto_check = false;
        updater.tick(&ctx);
        assert_eq!(updater.state, State::Idle);
    }

    #[test]
    fn outside_a_bundle_nothing_happens() {
        let mut u = Updater::new(None);
        u.tick(&egui::Context::default());
        assert_eq!(u.state, State::Unsupported);
        u.check_now();
        assert_eq!(u.state, State::Unsupported);
    }
}
