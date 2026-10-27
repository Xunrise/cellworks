//! STORY-078, the launcher shows every project grouped by where it lives.
//!
//! One test per scenario of issue #111, named after it, run headless through
//! `cellworks::draw` and `egui_kittest`, the way `tests/universe_frame.rs`
//! drives the binary's own share of STORY-002. The fixture is the one the
//! design's Proof section describes: a temporary home holding two git
//! repositories under `Projects`, a plain folder added through the settings
//! file, and one remote host with one project on it — read with a fake git
//! listing (branch always "main") and a fake host probe, so no test here
//! runs real git or real ssh. `CellworksApp::over` starts neither, so the
//! fakes are wired in with `with_git`, `with_probe`, `with_home` and
//! `with_recent` before any frame runs.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use cellworks::app::CellworksApp;
use cellworks_code_analysis::{GitListing, GitUnavailable, LoggedCommit};
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

/// A git listing that records every call, of any kind, with the repository
/// it was made for.
struct CountingGit(std::sync::Mutex<Vec<(&'static str, PathBuf)>>);

impl CountingGit {
    fn record(&self, call: &'static str, repository: &Path) {
        self.0
            .lock()
            .unwrap()
            .push((call, repository.to_path_buf()));
    }
}

impl GitListing for CountingGit {
    fn visible_files(&self, repository: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        self.record("visible_files", repository);
        FakeGit.visible_files(repository)
    }

    fn history(&self, repository: &Path) -> Result<Vec<Vec<PathBuf>>, GitUnavailable> {
        self.record("history", repository);
        FakeGit.history(repository)
    }

    fn branch(&self, repository: &Path) -> Result<String, GitUnavailable> {
        self.record("branch", repository);
        FakeGit.branch(repository)
    }

    fn log(&self, repository: &Path) -> Result<Vec<LoggedCommit>, GitUnavailable> {
        self.record("log", repository);
        FakeGit.log(repository)
    }

    fn blame(
        &self,
        repository: &Path,
        files: &[PathBuf],
    ) -> BTreeMap<PathBuf, Vec<Option<String>>> {
        self.record("blame", repository);
        FakeGit.blame(repository, files)
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

/// The fixture tree the design's Proof section describes: `Projects/cellworks`
/// and `Projects/shop`, both git repositories, and `scratch/tool`, a plain
/// folder. `settings` and `recent` add the settings file and the recent file
/// the engineer would have hand-edited or Cellworks would have written.
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
        repository(&home, "Projects/shop", &["x.rs"]);
        plain_folder(&home, "scratch/tool");
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

    /// Writes the settings file: the added folder `scratch/tool`, and one
    /// `[hosts."<name>"]` table per host, naming the remote projects on it as
    /// the engineer would write them, unresolved.
    ///
    /// STORY-083 Decision 10: this suite's own assertions are plain-words
    /// text, so every fixture here switches theme words off — otherwise the
    /// new themed defaults (e.g. "Open fortress" instead of "Open · Enter")
    /// would break them.
    fn settings(&self, hosts: &[(&str, &[&str])]) {
        let scratch = self.home.join("scratch/tool");
        // `added_folders` must come before the `[launcher]` table: TOML reads
        // every key after a `[table]` header as belonging to it until the
        // next header, so writing `[launcher]` first would silently fold
        // `added_folders` into `[launcher]` instead of leaving it top-level —
        // which is exactly why `scratch/tool` stopped being discovered when
        // this line was first added.
        let mut text = format!(
            "added_folders = [\"{}\"]\n\n[launcher]\ntheme_words = false\n",
            scratch.display()
        );
        for (host, projects) in hosts {
            let list: Vec<String> = projects.iter().map(|path| format!("\"{path}\"")).collect();
            text.push_str(&format!(
                "\n[hosts.\"{host}\"]\nprojects = [{}]\n",
                list.join(", ")
            ));
        }
        fs::write(self.home.join("settings.toml"), text).expect("write the settings file");
    }

    /// Writes the recent file beside the settings file: one project id per
    /// line, most recent first.
    fn recent(&self, ids: &[&str]) {
        fs::write(self.home.join("recent"), ids.join("\n") + "\n").expect("write the recent file");
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

fn plain_folder(root: &Path, relative: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(&path).expect("create a folder in the fixture");
    path
}

/// The app over `fixture`'s tree, probed with `connected`: one entry per host
/// named in the settings file, true where it answers.
fn app(fixture: &Fixture, connected: &[(&str, bool)]) -> CellworksApp {
    app_using(fixture, connected, Arc::new(FakeGit))
}

/// [`app`], asking `git` about repositories.
fn app_using(
    fixture: &Fixture,
    connected: &[(&str, bool)],
    git: Arc<dyn GitListing + Send + Sync>,
) -> CellworksApp {
    let sources = fixture.sources();
    let universe = Universe::scanning(&sources);
    let (sender, receiver) = mpsc::channel();
    for event in scan(&sources) {
        sender.send(event).expect("the app is listening");
    }
    drop(sender);

    let probe = FakeProbe(
        connected
            .iter()
            .map(|(host, state)| ((*host).to_owned(), *state))
            .collect(),
    );
    let mut app = CellworksApp::over(universe, receiver)
        .with_git(git)
        .with_probe(Arc::new(probe))
        .with_home(fixture.path().to_path_buf())
        .with_recent(RecentStore::at(fixture.path().join("recent")));
    settle(&mut app);
    app
}

/// Drains whatever the scan, the host probes and the project details have
/// sent so far: the fakes above answer almost at once, so a handful of short
/// waits is enough, never a blocking wait.
fn settle(app: &mut CellworksApp) {
    for _ in 0..50 {
        app.apply_pending();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn harness_over(app: CellworksApp) -> Harness<'static, CellworksApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
    harness.run();
    harness
}

/// The text of every labelled entry the frame put in the accessibility tree,
/// in tree order.
fn labels(harness: &Harness<'_, CellworksApp>) -> Vec<String> {
    let root = harness.root();
    std::iter::once(root)
        .chain(root.children_recursive())
        .filter_map(|node| {
            let node = node.accesskit_node();
            match node.role() {
                Role::Label => node.value().map(|text| text.to_string()),
                _ => None,
            }
        })
        .collect()
}

/// Clicks into the search box and types `text`, letter by letter, the way a
/// typed-in search narrows the list one keystroke at a time.
fn search(harness: &mut Harness<'_, CellworksApp>, text: &str) {
    harness.get_by_role(Role::TextInput).click();
    harness.run();
    for letter in text.chars() {
        harness
            .input_mut()
            .events
            .push(egui::Event::Text(letter.to_string()));
    }
    harness.run();
}

/// Projects are grouped by where they live: this Mac, or a remote host
///
/// STORY-082 requirement 5 made the group a *location* rather than a parent
/// folder, so the fixture's two local folders (`~/Projects` and `~/scratch`)
/// are one group now; the folder each project lives in is still named, on
/// its own row, by the path asserted below.
#[test]
fn projects_are_grouped_by_location_or_remote_host() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let harness = harness_over(app(&fixture, &[("az.wsl", true)]));

    let shown = labels(&harness);

    for heading in ["This Mac", "az.wsl"] {
        assert!(shown.contains(&heading.to_owned()), "{shown:?}");
    }
    // Twice in the whole frame: the location chip and the one group heading.
    // Three times would mean a heading per folder again.
    assert_eq!(
        shown
            .iter()
            .filter(|label| label.as_str() == "This Mac")
            .count(),
        2,
        "two local folders share one group heading: {shown:?}"
    );
    for (name, path) in [
        ("cellworks", "~/Projects/cellworks"),
        ("shop", "~/Projects/shop"),
        ("tool", "~/scratch/tool"),
        ("api", "~/work/api"),
    ] {
        assert!(shown.contains(&name.to_owned()), "{shown:?}");
        assert!(shown.contains(&path.to_owned()), "{shown:?}");
    }
}

/// The location chips narrow the list
#[test]
fn the_location_chips_narrow_the_list() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"]), ("gcp.vm", &["~/infra/db"])]);
    let mut harness = harness_over(app(&fixture, &[("az.wsl", true), ("gcp.vm", false)]));

    let shown = labels(&harness);
    // A chip's text is the state's word alone (STORY-082 ASSUMED (Q-1)); the
    // glyph beside it is painted, so no label carries one.
    assert!(shown.contains(&"connected".to_owned()), "{shown:?}");
    assert!(shown.contains(&"offline".to_owned()), "{shown:?}");

    // The chip row is drawn before the main area's groups, so the first
    // "az.wsl" found in the tree is its chip.
    harness
        .get_all_by_label("az.wsl")
        .next()
        .expect("az.wsl is shown at least once")
        .click_accesskit();
    harness.run();

    let shown = labels(&harness);
    assert!(shown.contains(&"api".to_owned()), "{shown:?}");
    assert!(!shown.contains(&"db".to_owned()), "{shown:?}");
}

/// Searching narrows the list by name or path
#[test]
fn searching_narrows_the_list_by_name_or_path() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let mut harness = harness_over(app(&fixture, &[("az.wsl", true)]));

    search(&mut harness, "api");

    let shown = labels(&harness);
    assert!(shown.contains(&"api".to_owned()), "{shown:?}");
    for other in ["cellworks", "shop", "tool"] {
        assert!(!shown.contains(&other.to_owned()), "{shown:?}");
    }
}

/// A search that matches nothing says so
#[test]
fn a_search_that_matches_nothing_says_so() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let mut harness = harness_over(app(&fixture, &[("az.wsl", true)]));

    search(&mut harness, "nothing-like-this");

    let shown = labels(&harness);
    assert!(
        shown.contains(&"No project matches \"nothing-like-this\".".to_owned()),
        "{shown:?}"
    );
    // The header always offers to add a project, and the no-match state
    // offers it again right beside the message, so two buttons share this
    // label: `query_all_by_label` tolerates that, where `query_by_label`
    // panics on more than one match.
    assert!(
        harness
            .query_all_by_label("Add project ⌘N")
            .next()
            .is_some(),
        "a search with no match still offers to add a project"
    );
}

/// Recently opened projects come first
#[test]
fn recently_opened_projects_come_first() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let cellworks = fixture.path().join("Projects/cellworks");
    fixture.recent(&[&cellworks.display().to_string(), "az.wsl:~/work/api"]);
    let harness = harness_over(app(&fixture, &[("az.wsl", true)]));

    let shown = labels(&harness);
    let cellworks_at = shown.iter().position(|label| label == "recent: cellworks");
    let api_at = shown.iter().position(|label| label == "recent: api");

    assert!(
        cellworks_at.is_some() && api_at.is_some(),
        "both recent cards are shown: {shown:?}"
    );
    assert!(
        cellworks_at < api_at,
        "cellworks, opened last, precedes api: {shown:?}"
    );
}

/// The selected project shows its details and opens on Enter
#[test]
fn the_selected_project_shows_its_details_and_opens_on_enter() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let mut harness = harness_over(app(&fixture, &[("az.wsl", true)]));

    harness.get_by_label("cellworks").click_accesskit();
    harness.run();
    // The details are read after the selection, off the frame loop.
    settle(harness.state_mut());
    harness.run();

    let shown = labels(&harness);
    // The preview pane now reads the kind and the location together, as
    // "git repository · This Mac", rather than as two separate labels.
    assert!(
        shown.iter().any(|label| label.contains("git repository")),
        "{shown:?}"
    );
    assert!(shown.contains(&"main".to_owned()), "{shown:?}");
    assert!(
        shown
            .iter()
            .any(|label| label.contains('2') && label.to_lowercase().contains("file")),
        "the file count is shown: {shown:?}"
    );

    harness.key_press(Key::Enter);
    harness.run();

    assert!(
        harness
            .state()
            .opened()
            .is_some_and(|opened| opened.project().name == "cellworks"),
        "Enter opens the selected project as a city"
    );
}

/// Git reads each listed local project once, so every row can show its
/// branch, and never runs for a remote project
#[test]
fn git_reads_each_listed_local_project_once() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let git = Arc::new(CountingGit(std::sync::Mutex::new(Vec::new())));
    let mut harness = harness_over(app_using(&fixture, &[], git.clone()));
    for _ in 0..3 {
        settle(harness.state_mut());
        harness.run();
    }

    let calls = git.0.lock().unwrap();
    let cellworks = fixture.path().join("Projects/cellworks");
    assert!(
        calls
            .iter()
            .any(|(name, repository)| *name == "branch" && *repository == cellworks),
        "a listed local project's branch is read: {calls:?}"
    );
    assert!(
        calls
            .iter()
            .all(|(_, repository)| repository.starts_with(fixture.path())),
        "only local projects are read: {calls:?}"
    );
    for (call, repository) in calls.iter() {
        assert_eq!(
            calls
                .iter()
                .filter(|other| other.0 == *call && other.1 == *repository)
                .count(),
            1,
            "{call} is asked once for {repository:?}: {calls:?}"
        );
    }
}

/// Enter on a remote project opens nothing
#[test]
fn enter_on_a_remote_project_opens_nothing() {
    let fixture = Fixture::new();
    fixture.settings(&[("az.wsl", &["~/work/api"])]);
    let mut harness = harness_over(app(&fixture, &[("az.wsl", true)]));

    harness.get_by_label("api").click_accesskit();
    harness.run();

    assert!(
        harness
            .get_by_label("Open project")
            .accesskit_node()
            .is_disabled(),
        "Open is disabled for a remote project on a connected host, with a \
         disabled-hover explaining why"
    );

    harness.key_press(Key::Enter);
    harness.run();

    assert!(
        harness.state().opened().is_none(),
        "Enter on a remote selection opens nothing"
    );
}
