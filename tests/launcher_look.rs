//! Renders the launcher's states to PNG for comparison against the STORY-083
//! mock (`sdlc/0134-story-083-the-launcher-wears-the-netrunn/`).
//!
//! Ignored by default: it needs a GPU adapter. Run it with
//! `UPDATE_SNAPSHOTS=1 cargo test --release --test launcher_look -- --ignored`;
//! the PNGs land in `tests/snapshots/` and are not committed.
//!
//! The fixture is STORY-083's: cellworks and The-Factory on this Mac,
//! factory-evals on the connected host "wsl", surface-kit on the offline
//! host "devbox", with a fake git listing and a fake host probe.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use cellworks::app::CellworksApp;
use cellworks_code_analysis::{GitListing, GitUnavailable};
use cellworks_workspace::{HostProbe, RecentStore, Sources, Universe, scan};
use egui::Key;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

/// A git listing that answers from the real tree the fixture wrote to disk,
/// with the branch fixed to "main" for every repository — never real git.
struct FakeGit;

impl GitListing for FakeGit {
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        let entries =
            fs::read_dir(repository).map_err(|error| GitUnavailable(error.to_string()))?;
        Ok(entries
            .flatten()
            .map(|entry| entry.file_name())
            .filter(|name| name != ".git")
            .map(PathBuf::from)
            .collect())
    }

    fn branch(&self, repository: &Path) -> Result<String, GitUnavailable> {
        if repository.join(".git").is_dir() {
            Ok("main".to_owned())
        } else {
            Err(GitUnavailable("not a repository".to_owned()))
        }
    }
}

/// A host probe that answers exactly what the fixture says, so no test here
/// runs real ssh.
struct FakeProbe(BTreeMap<String, bool>);

impl HostProbe for FakeProbe {
    fn connected(&self, host: &str) -> bool {
        self.0.get(host).copied().unwrap_or(false)
    }
}

/// The Background of issue #134: `Projects/cellworks` and
/// `Projects/The-Factory`, both git repositories, on this Mac; a settings
/// file naming "factory-evals" on "wsl" and "surface-kit" on "devbox",
/// with or without `[launcher]\ntheme_words = false`.
struct Fixture {
    _home: tempfile::TempDir,
    home: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let home_dir = tempfile::tempdir().expect("a temporary home directory");
        // Canonical from the start, so every path a scan reports compares
        // equal to one built from `home` here, even where a temporary
        // directory is reached through a symlink.
        let home = fs::canonicalize(home_dir.path()).expect("the temporary directory resolves");
        repository(&home, "Projects/cellworks", &["a.rs", "b.rs"]);
        repository(&home, "Projects/The-Factory", &["c.rs"]);
        Fixture {
            _home: home_dir,
            home,
        }
    }

    fn path(&self) -> &Path {
        &self.home
    }

    fn sources(&self) -> Sources {
        Sources {
            projects_folder: self.home.join("Projects"),
            settings_file: self.home.join("settings.toml"),
        }
    }

    /// Writes the settings file naming "wsl" and "devbox"'s one remote
    /// project each, as the engineer would write them, unresolved.
    /// `theme_words_off` writes `[launcher]\ntheme_words = false` first (the
    /// scenario's "the settings switch theme words off"); otherwise no
    /// `[launcher]` table is written at all (the scenario's "the settings do
    /// not mention theme words"), which Requirement 6 leaves on.
    fn settings(&self, theme_words_off: bool) {
        let mut text = if theme_words_off {
            "[launcher]\ntheme_words = false\n".to_owned()
        } else {
            String::new()
        };
        text.push_str("\n[hosts.\"wsl\"]\nprojects = [\"~/src/factory-evals\"]\n");
        text.push_str("\n[hosts.\"devbox\"]\nprojects = [\"/srv/surface-kit\"]\n");
        fs::write(self.home.join("settings.toml"), text).expect("write the settings file");
    }
}

fn repository(root: &Path, relative: &str, files: &[&str]) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.join(".git")).expect("create a repository in the fixture");
    for file in files {
        fs::write(path.join(file), "").expect("write a file in the fixture");
    }
    path
}

/// The app over `fixture`'s tree: the Background's "wsl" connected and
/// "devbox" offline.
fn app(fixture: &Fixture) -> CellworksApp {
    let sources = fixture.sources();
    let universe = Universe::scanning(&sources);
    let (sender, receiver) = mpsc::channel();
    for event in scan(&sources) {
        sender.send(event).expect("the app is listening");
    }
    drop(sender);

    let probe = FakeProbe(BTreeMap::from([
        ("wsl".to_owned(), true),
        ("devbox".to_owned(), false),
    ]));
    let mut app = CellworksApp::over(universe, receiver)
        .with_git(Arc::new(FakeGit))
        .with_probe(Arc::new(probe))
        .with_home(fixture.path().to_path_buf())
        .with_recent(RecentStore::at(fixture.path().join("recent")));
    settle(&mut app);
    app
}

/// Drains what the scan and the fakes have sent so far.
fn settle(app: &mut CellworksApp) {
    for _ in 0..50 {
        app.apply_pending();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Renders one launcher state: theme words on or off, an optional search,
/// `downs` presses of ArrowDown, then a snapshot named `name`.
fn shot(theme_off: bool, query: &str, downs: usize, name: &str) {
    let fixture = Fixture::new();
    fixture.settings(theme_off);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(400)
        .build_ui_state(
            |ui, app: &mut CellworksApp| cellworks::draw(ui, app),
            app(&fixture),
        );
    cellworks_workspace::ui::install_fonts(&harness.ctx);
    for _ in 0..90 {
        harness.step();
    }
    if !query.is_empty() {
        harness.get_by_role(Role::TextInput).click();
        harness.step();
        for letter in query.chars() {
            harness
                .input_mut()
                .events
                .push(egui::Event::Text(letter.to_string()));
        }
        harness.step();
    }
    for _ in 0..downs {
        harness.key_press(Key::ArrowDown);
        harness.step();
    }
    for _ in 0..90 {
        harness.step();
    }
    harness.snapshot(name);
}

#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn launcher_main() {
    shot(false, "", 1, "launcher_main");
}

#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn launcher_search() {
    shot(false, "fac", 2, "launcher_search");
}

#[test]
#[ignore = "renders PNGs through wgpu; run on demand"]
fn launcher_plain() {
    shot(true, "", 1, "launcher_plain");
}

/// The list scrolls with the selection: after the arrow keys walk past the
/// bottom of a short window, the selected row is still inside it.
#[test]
fn the_list_scrolls_with_the_arrow_keys() {
    let fixture = Fixture::new();
    for n in 0..20 {
        repository(
            fixture.path(),
            &format!("Projects/project-{n:02}"),
            &["a.rs"],
        );
    }
    fixture.settings(true);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 500.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            |ui, app: &mut CellworksApp| cellworks::draw(ui, app),
            app(&fixture),
        );
    harness.run();
    for _ in 0..22 {
        harness.key_press(Key::ArrowDown);
        harness.run();
    }

    let root = harness.root();
    let selected = std::iter::once(root)
        .chain(root.children_recursive())
        .find(|node| node.accesskit_node().is_selected() == Some(true))
        .expect("a row is selected");
    let bounds = selected
        .accesskit_node()
        .bounding_box()
        .expect("the selected row has bounds");
    assert!(
        bounds.y1 <= 500.0,
        "the selected row ends at {} px, below the 500 px window",
        bounds.y1
    );
}

fn launcher_harness(fixture: &Fixture) -> Harness<'static, CellworksApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            |ui, app: &mut CellworksApp| cellworks::draw(ui, app),
            app(fixture),
        );
    harness.run();
    harness
}

fn pointer_move(harness: &mut Harness<'_, CellworksApp>, pos: egui::Pos2) {
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(pos));
    harness.step();
}

fn pointer_button(harness: &mut Harness<'_, CellworksApp>, pos: egui::Pos2, pressed: bool) {
    harness.input_mut().events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
}

fn pointer_click(harness: &mut Harness<'_, CellworksApp>, pos: egui::Pos2) {
    pointer_move(harness, pos);
    pointer_button(harness, pos, true);
    pointer_button(harness, pos, false);
}

fn search_focused(harness: &Harness<'_, CellworksApp>) -> bool {
    harness
        .ctx
        .memory(|memory| memory.has_focus(egui::Id::new("cellworks-launcher-search")))
}

/// A click anywhere inside the visible search box focuses the field: its top
/// inner edge and the ":" prompt included, not only the one text line.
#[test]
fn a_click_anywhere_in_the_search_box_focuses_the_field() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = launcher_harness(&fixture);
    let field = harness.get_by_role(Role::TextInput).rect();
    let centre_y = field.center().y;

    let top_edge = egui::pos2(field.center().x, centre_y - 24.0);
    pointer_click(&mut harness, top_edge);
    harness.run();
    assert!(search_focused(&harness), "the top inner edge focuses");

    harness.key_press(Key::Escape);
    harness.run();
    assert!(!search_focused(&harness));

    let prompt = egui::pos2(field.left() + 20.0, centre_y);
    pointer_click(&mut harness, prompt);
    harness.run();
    assert!(search_focused(&harness), "the prompt focuses");
}

/// Hovering a project row shows the pointing hand.
#[test]
fn a_hovered_row_shows_the_pointing_hand() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = launcher_harness(&fixture);
    let row = harness.get_by_label("cellworks").rect();
    pointer_move(&mut harness, row.center());
    harness.run();
    assert_eq!(
        harness.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
}

