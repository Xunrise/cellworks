//! STORY-082, the launcher is a keyboard-first palette.
//!
//! One test per scenario of issue #132, named after it, run headless through
//! `cellworks::draw` and `egui_kittest`, the way
//! `tests/story_078_the_launcher_shows_every_project_grouped_by_where_it_lives.rs`
//! drives STORY-078's share of the same view. The fixture is the one the
//! design's Proof section describes: cellworks and The-Factory on this Mac,
//! factory-evals on the connected remote host "wsl", surface-kit on the
//! offline remote host "devbox" — read with a fake git listing (branch
//! always "main") and a fake host probe, so no test here runs real git or
//! real ssh. The harness is 1280 px wide, for the design's 920 px card.

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
use std::collections::BTreeMap;

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

/// The fixture the design's Proof section describes: `Projects/cellworks`
/// and `Projects/The-Factory`, both git repositories; a settings file naming
/// the remote projects of "wsl" and "devbox"; a recent file naming
/// "cellworks".
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

    /// Writes the settings file: one `[hosts."<name>"]` table per host,
    /// naming the one remote project on it as the engineer would write it,
    /// unresolved.
    ///
    /// STORY-083 Decision 10: this suite's own assertions are plain-words
    /// text, so every fixture here switches theme words off — otherwise the
    /// new themed defaults (e.g. "Open fortress" instead of "Open · Enter")
    /// would break them.
    fn settings(&self, hosts: &[(&str, &str)]) {
        let mut text = "[launcher]\ntheme_words = false\n".to_owned();
        for (host, project) in hosts {
            text.push_str(&format!(
                "\n[hosts.\"{host}\"]\nprojects = [\"{project}\"]\n"
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

/// The app over `fixture`'s tree, probed with `connected`: one entry per
/// host named in the settings file, true where it answers.
fn app(fixture: &Fixture, connected: &[(&str, bool)]) -> CellworksApp {
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
        .with_git(Arc::new(FakeGit))
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

/// The design's Proof section: a harness 1280 px wide, for the 920 px card.
fn harness_over(app: CellworksApp) -> Harness<'static, CellworksApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
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

/// Builds the fixture every scenario shares: cellworks and The-Factory on
/// this Mac, factory-evals on the connected host "wsl", surface-kit on the
/// offline host "devbox", and cellworks named in the recent file.
fn fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture.settings(&[
        ("wsl", "~/src/factory-evals"),
        ("devbox", "/srv/surface-kit"),
    ]);
    let cellworks = fixture.path().join("Projects/cellworks");
    fixture.recent(&[&cellworks.display().to_string()]);
    fixture
}

/// The launcher over the shared fixture, wsl connected and devbox offline.
fn launcher_app(fixture: &Fixture) -> CellworksApp {
    app(fixture, &[("wsl", true), ("devbox", false)])
}

/// The launcher lists recent projects first, then each location
#[test]
fn the_launcher_lists_recent_projects_first_then_each_location() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "the search field has the focus on the first frame"
    );

    let shown = labels(&harness);
    let recent_at = shown
        .iter()
        .position(|label| label.contains("cellworks"))
        .expect("cellworks is shown somewhere: {shown:?}");
    for heading in ["This Mac", "wsl", "devbox"] {
        // The chip row draws the same text ("This Mac", a host's bare name)
        // before the list does, so the group heading — what this assertion
        // means — is each label's *last* occurrence, not its first.
        let heading_at = shown
            .iter()
            .rposition(|label| label == heading)
            .unwrap_or_else(|| panic!("the heading {heading:?} is shown: {shown:?}"));
        assert!(
            recent_at < heading_at,
            "cellworks, under Recent, precedes the group {heading:?}: {shown:?}"
        );
    }

    for (name, path) in [
        ("cellworks", "~/Projects/cellworks"),
        ("The-Factory", "~/Projects/The-Factory"),
    ] {
        assert!(shown.iter().any(|label| label.contains(name)), "{shown:?}");
        assert!(shown.iter().any(|label| label.contains(path)), "{shown:?}");
    }
    // "folder" means the project *has* no branch (requirement 5). A branch
    // is read off the frame loop only once the project is selected
    // (unchanged from STORY-078), and nothing is selected yet, so no row
    // calls a repository a folder here.
    assert!(
        !shown.iter().any(|label| label.as_str() == "folder"),
        "no row claims a project has no branch before its branch is read: {shown:?}"
    );

    harness.get_by_label("cellworks").click_accesskit();
    harness.run();
    settle(harness.state_mut());
    harness.run();

    let shown = labels(&harness);
    assert!(
        shown
            .iter()
            .filter(|label| label.as_str() == "main")
            .count()
            >= 2,
        "once read, the branch is shown on the row as well as in the preview \
         pane: {shown:?}"
    );
}

/// Each location shows its project count and remote host state
#[test]
fn each_location_shows_its_project_count_and_remote_host_state() {
    let fixture = fixture();
    let harness = harness_over(launcher_app(&fixture));

    let shown = labels(&harness);
    // The exact split of a chip's text across widgets is the implementer's
    // call; this only pins that the host name, state word and count are
    // shown somewhere, not how they are grouped into labels.
    for term in ["All", "This Mac", "wsl", "connected", "devbox", "offline"] {
        assert!(
            shown.iter().any(|label| label.contains(term)),
            "{term:?} is shown: {shown:?}"
        );
    }
    // ASSUMED (Q-1): a chip's text carries the state's word alone and the
    // glyph is painted beside it, so no label anywhere embeds one.
    assert!(
        !shown.iter().any(|label| label.contains(['●', '◌', '○'])),
        "no label embeds a state glyph: {shown:?}"
    );
    assert!(
        shown.iter().any(|label| label.trim() == "4"),
        "the All chip's count of 4: {shown:?}"
    );
    assert!(
        shown.iter().any(|label| label.trim() == "2"),
        "the This Mac chip's count of 2: {shown:?}"
    );
}

/// The engineer moves through the locations from the keyboard
#[test]
fn the_engineer_moves_through_the_locations_from_the_keyboard() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    harness.key_press(Key::Tab);
    harness.run();
    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "the search field still has the focus after the first Tab"
    );
    harness.key_press(Key::Tab);
    harness.run();
    // egui moves the focus on Tab in its own pass, before the key block can
    // consume the key: the launcher asks for it straight back, or the next
    // letter typed would reach the map's shortcuts instead of the field.
    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "the search field still has the focus after the second Tab"
    );

    let shown = labels(&harness);
    assert!(
        shown.iter().any(|label| label.contains("factory-evals")),
        "{shown:?}"
    );
    for other in ["cellworks", "The-Factory", "surface-kit"] {
        assert!(
            !shown.iter().any(|label| label.contains(other)),
            "{other} is hidden once wsl is picked: {shown:?}"
        );
    }
}

/// A letter typed into the search field stays in it: the field is focused
/// from the first frame (requirement 2), and requirement 10's "does not reach
/// the map handling" holds for the letters as well as for Esc. "u", "m" and
/// "z" are the three map shortcuts that act whether a project is open or not,
/// so they are the ones a typed project name ("surface-kit", "minikube") can
/// silently fire behind the launcher.
#[test]
fn letters_typed_into_the_search_field_do_not_reach_the_map_shortcuts() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "the search field has the focus on the first frame"
    );

    let locked = harness.state().navigation().is_locked();
    for key in [Key::U, Key::M, Key::Z] {
        harness.key_press(key);
        harness.run();
    }

    let navigation = harness.state().navigation();
    assert!(
        !navigation.is_underground(),
        "u typed into the search field did not turn the view underground"
    );
    assert!(
        !navigation.shows_cables(),
        "m typed into the search field did not switch the cables on"
    );
    assert_eq!(
        navigation.is_locked(),
        locked,
        "z typed into the search field did not change the navigation lock"
    );
}

/// Search narrows the list and marks what matched
#[test]
fn search_narrows_the_list_and_marks_what_matched() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    search(&mut harness, "fac");

    let shown = labels(&harness);
    for name in ["The-Factory", "factory-evals", "surface-kit"] {
        assert!(shown.iter().any(|label| label.contains(name)), "{shown:?}");
    }
    assert!(
        !shown.iter().any(|label| label.contains("cellworks")),
        "{shown:?}"
    );
    // The underline itself cannot be asserted through the accessibility
    // tree; only the text and the match count are checked here.
    assert!(
        shown
            .iter()
            .any(|label| label.contains('3') && label.to_lowercase().contains("match")),
        "the launcher says it found 3 matches: {shown:?}"
    );
}

/// Clearing the search brings the full list back
#[test]
fn clearing_the_search_brings_the_full_list_back() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    search(&mut harness, "fac");
    harness.key_press(Key::Escape);
    harness.run();

    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some(String::new()),
        "the search field is empty"
    );
    // Requirement 10: Esc clears the text "and nothing else". A single-line
    // field surrenders its focus on Escape, so the launcher asks for it back
    // — otherwise the next letter typed would reach the map's shortcuts.
    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "the search field still has the focus after Esc"
    );

    let shown = labels(&harness);
    for name in ["cellworks", "The-Factory", "factory-evals", "surface-kit"] {
        assert!(
            shown.iter().any(|label| label.contains(name)),
            "{name} is listed again: {shown:?}"
        );
    }
}

/// The selected project shows its details and opens on Enter
#[test]
fn the_selected_project_shows_its_details_and_opens_on_enter() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    harness.get_by_label("cellworks").click_accesskit();
    harness.run();
    // The details are read after the selection, off the frame loop.
    settle(harness.state_mut());
    harness.run();

    let shown = labels(&harness);
    assert!(
        shown.iter().any(|label| label.contains("cellworks")),
        "{shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|label| label.contains("git repository") && label.contains("This Mac")),
        "the kind and the location are shown together: {shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|label| label.contains("~/Projects/cellworks")),
        "the path is shown: {shown:?}"
    );
    assert!(shown.iter().any(|label| label == "main"), "{shown:?}");
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

/// A project on an offline remote host cannot be opened
#[test]
fn a_project_on_an_offline_remote_host_cannot_be_opened() {
    let fixture = fixture();
    let mut harness = harness_over(launcher_app(&fixture));

    harness.get_by_label("surface-kit").click_accesskit();
    harness.run();

    // The pane names where the project lives (requirement 7's
    // "<kind> · <location>"), which for a remote project is its host — not
    // the host's connection state, which its chip carries.
    assert!(
        labels(&harness).contains(&"remote · devbox".to_owned()),
        "the pane names the host the project lives on: {:?}",
        labels(&harness)
    );

    assert!(
        harness
            .get_by_label("Connect the host first")
            .accesskit_node()
            .is_disabled(),
        "Open is disabled for a project on an offline host, and says so"
    );

    harness.key_press(Key::Enter);
    harness.run();

    assert!(
        harness.state().opened().is_none(),
        "Enter on surface-kit opens nothing"
    );
    // The launcher is still the only thing on screen, so it is still the
    // keyboard's owner: a single-line field surrenders its focus on Enter
    // (and the click on the row took it away before that), and the launcher
    // asks for it back both times.
    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "the search field still has the focus after an Enter that opened nothing"
    );
}
