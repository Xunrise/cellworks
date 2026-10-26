//! STORY-083, the launcher wears the Netrunner look with switchable theme
//! words.
//!
//! One test per scenario of issue #134, named after it, run headless through
//! `cellworks::draw` and `egui_kittest`, following
//! `tests/story_082_the_launcher_is_a_keyboard_first_palette.rs`'s fixture and
//! helpers exactly: cellworks and The-Factory on this Mac, factory-evals on
//! the connected remote host "wsl", surface-kit on the offline remote host
//! "devbox" — read with a fake git listing (branch always "main") and a fake
//! host probe, so no test here runs real git or real ssh.
//!
//! Design requirement 10: the Gherkin's quoted strings are matched
//! case-insensitively against the displayed text, because the themed table
//! renders several of them upper case ("Local net 2" is drawn "LOCAL NET 2").
//! Every assertion below therefore reads through `shown_contains`, a
//! case-insensitive substring check over `labels(&harness)`, rather than an
//! exact `get_by_label` on a themed string — the exact split of text across
//! widgets is the implementer's call, the same as STORY-082's own
//! "ASSUMED (Q-1)" comment already explains for its chip labels. Where a
//! quoted string pairs a word with a count ("Local net 2"), the word and the
//! count are checked separately, since STORY-082's own chip test already
//! finds "This Mac" and its count "2" in two different labels rather than
//! one combined "This Mac 2".

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
///
/// Unlike STORY-078's and STORY-082's own `labels()` (which only ever needed
/// `ui.label()` text, read off AccessKit's `Role::Label` nodes via `value()`),
/// this suite also checks the CTA, "Add" and "Sync" buttons' own wording —
/// an `egui::Button`'s accessible text is AccessKit's `label()`, not
/// `value()` (`value()` is reserved for a `Role::Label` node — see kittest's
/// own `By::label` doc comment and its `matches` implementation, which this
/// mirrors), so a label-only read would silently miss every button.
fn labels(harness: &Harness<'_, CellworksApp>) -> Vec<String> {
    let root = harness.root();
    std::iter::once(root)
        .chain(root.children_recursive())
        .filter_map(|node| {
            let node = node.accesskit_node();
            let text = if node.role() == Role::Label {
                node.value()
            } else {
                node.label()
            };
            text.map(|text| text.to_string())
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

/// Requirement 10: a case-insensitive substring check over every shown
/// label, because the themed table draws several strings upper case.
fn shown_contains(shown: &[String], needle: &str) -> bool {
    let needle = needle.to_lowercase();
    shown
        .iter()
        .any(|label| label.to_lowercase().contains(&needle))
}

/// How many shown labels, once trimmed, read exactly `text` — for a count
/// that STORY-082's own chip row already draws as its own label, separate
/// from the word beside it.
fn count_of_label(shown: &[String], text: &str) -> usize {
    shown.iter().filter(|label| label.trim() == text).count()
}

/// Theme words are shown by default
#[test]
fn theme_words_are_shown_by_default() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    let shown = labels(&harness);
    // "4 fortresses · 2 nets" may or may not be one label: each half is
    // checked on its own.
    for needle in ["4 fortresses", "2 nets"] {
        assert!(shown_contains(&shown, needle), "{needle:?} in {shown:?}");
    }
    // "All 4", "Local net 2", "wsl connected 1", "devbox offline 1": each
    // fragment (the location's word, the host's own name and its state word)
    // is checked on its own label, the way STORY-082's own chip test already
    // does for "This Mac", "wsl", "connected" etc. — a chip's host name and
    // state word are separate labels beside the state's painted glyph
    // (STORY-082 ASSUMED (Q-1)), never one combined string.
    for word in ["All", "Local net", "wsl", "connected", "devbox", "offline"] {
        assert!(shown_contains(&shown, word), "{word:?} in {shown:?}");
    }
    assert!(
        count_of_label(&shown, "4") >= 1,
        "All's count of 4: {shown:?}"
    );
    assert!(
        count_of_label(&shown, "2") >= 1,
        "Local net's count of 2: {shown:?}"
    );
    assert!(
        count_of_label(&shown, "1") >= 2,
        "wsl's and devbox's counts of 1 each: {shown:?}"
    );

    harness.get_by_label("cellworks").click_accesskit();
    harness.run();

    let shown = labels(&harness);
    assert!(
        shown_contains(&shown, "open fortress"),
        "the open action for cellworks reads \"Open fortress\": {shown:?}"
    );
}

/// An offline host is reported as ICE
#[test]
fn an_offline_host_is_reported_as_ice() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    harness.get_by_label("surface-kit").click_accesskit();
    harness.run();

    let shown = labels(&harness);
    // "ICE // NET OFFLINE" may be one label or more than one: each
    // distinctive fragment is checked on its own, rather than the whole
    // phrase, which risks a false split.
    for needle in ["ice //", "net offline"] {
        assert!(shown_contains(&shown, needle), "{needle:?} in {shown:?}");
    }
    assert!(
        shown_contains(&shown, "connect the net first"),
        "the disabled open action reads \"Connect the net first\": {shown:?}"
    );

    harness.key_press(Key::Enter);
    harness.run();

    assert!(
        harness.state().opened().is_none(),
        "Enter on surface-kit opens nothing: it cannot be used while its net is offline"
    );
}

/// Theme words can be switched off
#[test]
fn theme_words_can_be_switched_off() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = harness_over(app(&fixture));

    let shown = labels(&harness);
    for needle in ["4 projects", "2 hosts"] {
        assert!(shown_contains(&shown, needle), "{needle:?} in {shown:?}");
    }
    // "This Mac" is STORY-082's unchanged text: not case-folded, so checked
    // exactly rather than case-insensitively. The host name and its state
    // word stay separate labels beside the state's painted glyph (STORY-082
    // ASSUMED (Q-1)), never one combined string.
    for word in ["All", "This Mac", "wsl", "connected", "devbox", "offline"] {
        assert!(
            shown.iter().any(|label| label.contains(word)),
            "{word:?} in {shown:?}"
        );
    }
    assert!(
        count_of_label(&shown, "4") >= 1,
        "All's count of 4: {shown:?}"
    );
    assert!(
        count_of_label(&shown, "2") >= 1,
        "This Mac's count of 2: {shown:?}"
    );
    assert!(
        count_of_label(&shown, "1") >= 2,
        "wsl's and devbox's counts of 1 each: {shown:?}"
    );

    harness.get_by_label("cellworks").click_accesskit();
    harness.run();

    let shown = labels(&harness);
    assert!(
        shown_contains(&shown, "open project"),
        "the open action for cellworks reads \"Open project\" once theme words are off: {shown:?}"
    );
}

/// Plain words for an offline host
#[test]
fn plain_words_for_an_offline_host() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = harness_over(app(&fixture));

    harness.get_by_label("surface-kit").click_accesskit();
    harness.run();

    let shown = labels(&harness);
    assert!(
        shown.iter().any(|label| label.contains("Host offline")),
        "the preview warns \"Host offline\" exactly, once theme words are off: {shown:?}"
    );
    assert!(
        !shown.iter().any(|label| label.contains("ICE")),
        "no \"ICE //\" prefix survives once theme words are off: {shown:?}"
    );
    assert!(
        shown_contains(&shown, "connect the host first"),
        "the disabled open action reads \"Connect the host first\": {shown:?}"
    );

    harness.key_press(Key::Enter);
    harness.run();

    assert!(
        harness.state().opened().is_none(),
        "Enter on surface-kit opens nothing: it cannot be used while its host is offline"
    );
}

/// Switching words does not change behaviour
#[test]
fn switching_words_does_not_change_behaviour() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = harness_over(app(&fixture));

    search(&mut harness, "fac");

    let shown = labels(&harness);
    for name in ["The-Factory", "factory-evals", "surface-kit"] {
        assert!(shown.iter().any(|label| label.contains(name)), "{shown:?}");
    }
    assert!(
        !shown.iter().any(|label| label.contains("cellworks")),
        "cellworks does not match \"fac\": {shown:?}"
    );

    // The search is cleared the way Esc already does (STORY-082 requirement
    // 10), so "cellworks" is reachable again before it is opened: switching
    // the words changed none of this.
    harness.key_press(Key::Escape);
    harness.run();

    harness.get_by_label("cellworks").click_accesskit();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();

    assert!(
        harness
            .state()
            .opened()
            .is_some_and(|opened| opened.project().name == "cellworks"),
        "Enter on cellworks opens it, exactly as with theme words on"
    );
}

/// Design Decision 7/8: with reduced motion (`animation_time == 0.0`, the
/// same signal `src/app.rs` sets globally when the system asks for it), the
/// launcher's rise-in and breathing glow schedule no repaint at all — motion
/// repaints only use `request_repaint_after_secs`, never an immediate one, so
/// a settled harness's root viewport reports `repaint_delay` of `Duration::MAX`
/// (egui's "nothing asked to be redrawn") once reduced, rather than the
/// roughly-30fps delay the breathing CTA asks for otherwise.
#[test]
fn a_reduced_motion_harness_settles_with_no_repaint_scheduled() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            |ui, app: &mut CellworksApp| {
                ui.ctx().all_styles_mut(|style| style.animation_time = 0.0);
                cellworks::draw(ui, app);
            },
            app(&fixture),
        );
    harness.run();

    // The project is selected and openable, so the breathing CTA is on
    // screen: this is exactly the case Decision 8's risk note describes
    // ("repaints the launcher about 30 times a second while it is open") when
    // motion is not reduced — proving this harness asks for nothing instead.
    harness.get_by_label("cellworks").click_accesskit();
    harness.run();

    let repaint_delay = harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .expect("the root viewport reports its output")
        .repaint_delay;
    assert_eq!(
        repaint_delay,
        std::time::Duration::MAX,
        "reduced motion schedules no repaint for the launcher's own motion"
    );
}
