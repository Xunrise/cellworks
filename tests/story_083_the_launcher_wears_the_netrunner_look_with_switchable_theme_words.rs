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

    // Esc leaves CMD and a second one clears the search (requirement 8), so
    // "cellworks" is reachable again before it is opened: switching the words
    // changed none of this.
    harness.key_press(Key::Escape);
    harness.run();
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

/// The name of the project the launcher has selected, as the navigation
/// layer sees it: none for nothing selected or for a project on another host.
fn selected(harness: &Harness<'_, CellworksApp>) -> Option<String> {
    harness
        .state()
        .navigation()
        .selection()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
}

/// Types `text` into whatever has the keyboard, as the platform does: a text
/// event per character.
fn type_text(harness: &mut Harness<'_, CellworksApp>, text: &str) {
    for letter in text.chars() {
        harness
            .input_mut()
            .events
            .push(egui::Event::Text(letter.to_string()));
    }
    harness.run();
}

/// Requirement 8: the launcher opens in NAV, with j and k moving the selection
/// like the arrows
#[test]
fn the_launcher_opens_in_nav_and_j_and_k_move_the_selection() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    assert!(
        !harness.get_by_role(Role::TextInput).is_focused(),
        "the search field has no focus at launch"
    );
    assert_eq!(selected(&harness), None, "nothing is selected yet");

    harness.key_press(Key::J);
    harness.run();
    let first = selected(&harness);
    assert!(first.is_some(), "the first j selects the first row");

    harness.key_press(Key::J);
    harness.run();
    let second = selected(&harness);
    assert_ne!(second, first, "the second j moves down");

    harness.key_press(Key::K);
    harness.run();
    assert_eq!(selected(&harness), first, "k moves back up");

    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(selected(&harness), second, "the arrows still move too");
    assert!(
        !harness.get_by_role(Role::TextInput).is_focused(),
        "moving in NAV leaves the field unfocused"
    );
    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some(String::new()),
        "no letter was typed into it"
    );
}

/// Requirement 8: g selects the first row and G the last
#[test]
fn g_selects_the_first_row_and_shift_g_the_last() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    harness.key_press(Key::J);
    harness.run();
    let first = selected(&harness);
    assert!(first.is_some());

    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::G);
    harness.run();
    assert_ne!(selected(&harness), first, "G leaves the first row");
    assert_eq!(
        selected(&harness),
        None,
        "the last row is a project on another host"
    );

    harness.key_press(Key::G);
    harness.run();
    assert_eq!(selected(&harness), first, "g is back on the first row");
    assert!(
        !harness.state().navigation().is_underground(),
        "shift+g does not show the view underground behind the launcher"
    );
}

/// Requirement 8: ⌘K focuses the search field, and ⌘K again leaves it with the
/// filter kept
#[test]
fn cmd_k_enters_cmd_and_cmd_k_again_returns_to_nav_keeping_the_filter() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(
        harness.get_by_role(Role::TextInput).is_focused(),
        "⌘K focuses the search field"
    );

    type_text(&mut harness, "fac");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(
        !harness.get_by_role(Role::TextInput).is_focused(),
        "⌘K again leaves the field"
    );
    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some("fac".to_owned()),
        "the filter text stays"
    );
}

/// Requirement 8: "/" and ":" enter CMD without being typed into the field
#[test]
fn slash_and_colon_enter_cmd_without_being_typed() {
    for key in ["/", ":"] {
        let fixture = Fixture::new();
        fixture.settings(false);
        let mut harness = harness_over(app(&fixture));

        type_text(&mut harness, key);
        harness.run();
        assert!(
            harness.get_by_role(Role::TextInput).is_focused(),
            "{key} focuses the search field"
        );
        assert_eq!(
            harness.get_by_role(Role::TextInput).value(),
            Some(String::new()),
            "{key} is not typed into it"
        );

        type_text(&mut harness, "fac");
        assert_eq!(
            harness.get_by_role(Role::TextInput).value(),
            Some("fac".to_owned()),
            "what follows is typed"
        );
    }
}

/// Requirement 8: Esc in CMD returns to NAV keeping the filter, and Esc in NAV
/// clears it
#[test]
fn esc_leaves_cmd_keeping_the_filter_and_clears_it_in_nav() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    search(&mut harness, "fac");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.get_by_role(Role::TextInput).is_focused());
    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some("fac".to_owned()),
        "Esc in CMD keeps the filter"
    );

    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some(String::new()),
        "Esc in NAV clears it"
    );
    assert!(!harness.get_by_role(Role::TextInput).is_focused());
}

/// Requirement 8: no letter in NAV reaches the map shortcuts behind the
/// launcher, nor the search field
#[test]
fn letters_in_nav_do_not_reach_the_map_shortcuts() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    let locked = harness.state().navigation().is_locked();
    for key in [
        Key::U,
        Key::M,
        Key::Z,
        Key::I,
        Key::E,
        Key::F,
        Key::L,
        Key::A,
        Key::Q,
        Key::H,
    ] {
        harness.key_press(key);
        harness.run();
    }
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::U);
    harness.run();

    let navigation = harness.state().navigation();
    assert!(!navigation.is_underground(), "u did nothing");
    assert!(!navigation.shows_cables(), "m did nothing");
    assert_eq!(navigation.is_locked(), locked, "z did nothing");
    assert!(
        !harness.get_by_role(Role::TextInput).is_focused(),
        "no letter enters CMD"
    );
    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some(String::new()),
        "no letter reaches the field"
    );
}

// STORY-083 requirements 14 to 16: paging, the sort chip, folder subheadings.

/// The name of the row the frame draws as selected, if any.
fn selected_row(harness: &Harness<'_, CellworksApp>) -> Option<String> {
    let root = harness.root();
    std::iter::once(root)
        .chain(root.children_recursive())
        .find(|node| node.accesskit_node().is_selected() == Some(true))
        .and_then(|node| node.accesskit_node().value().map(|text| text.to_string()))
}

/// The project names of `names` that the frame shows, in the order it draws
/// them (a row's label is exactly its project's name).
fn row_order(harness: &Harness<'_, CellworksApp>, names: &[&str]) -> Vec<String> {
    labels(harness)
        .into_iter()
        .filter(|label| names.contains(&label.as_str()))
        .collect()
}

fn press_ctrl(harness: &mut Harness<'_, CellworksApp>, key: Key) {
    harness.key_press_modifiers(egui::Modifiers::CTRL, key);
    harness.run();
}

/// Ctrl+D and Ctrl+U move a page, clamped at either end
#[test]
fn ctrl_d_and_ctrl_u_move_a_page_clamped_at_either_end() {
    let fixture = Fixture::new();
    fixture.settings(true);
    for number in 1..=14 {
        repository(fixture.path(), &format!("Projects/p{number:02}"), &["a.rs"]);
    }
    let mut harness = harness_over(app(&fixture));

    press_ctrl(&mut harness, Key::D);
    let first = selected_row(&harness).expect("the first Ctrl+D selects the first row");
    assert_eq!(first, "The-Factory");

    press_ctrl(&mut harness, Key::D);
    let second = selected_row(&harness).expect("a row stays selected");
    assert_ne!(second, "cellworks", "a page is more than one row");
    assert_ne!(second, first);

    press_ctrl(&mut harness, Key::U);
    assert_eq!(selected_row(&harness), Some(first.clone()));
    press_ctrl(&mut harness, Key::U);
    assert_eq!(
        selected_row(&harness),
        Some(first),
        "no wrap above the first row"
    );

    for _ in 0..6 {
        press_ctrl(&mut harness, Key::D);
    }
    assert_eq!(selected_row(&harness).as_deref(), Some("factory-evals"));
    press_ctrl(&mut harness, Key::D);
    assert_eq!(
        selected_row(&harness).as_deref(),
        Some("factory-evals"),
        "no wrap below the last row"
    );
}

/// Ctrl+D pages in CMD too and never reaches the field
#[test]
fn ctrl_d_pages_in_cmd_without_typing_into_the_field() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = harness_over(app(&fixture));

    harness.get_by_role(Role::TextInput).click();
    harness.run();
    press_ctrl(&mut harness, Key::D);

    assert!(selected_row(&harness).is_some(), "the selection moved");
    assert_eq!(
        harness.get_by_role(Role::TextInput).value(),
        Some(String::new()),
        "the field typed nothing"
    );
}

/// Shift+Tab and a click cycle the sort chip
#[test]
fn shift_tab_and_a_click_cycle_the_sort_chip() {
    let fixture = Fixture::new();
    fixture.settings(false);
    let mut harness = harness_over(app(&fixture));

    assert!(shown_contains(&labels(&harness), "Sort: dir"));

    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert!(shown_contains(&labels(&harness), "Sort: opened"));

    harness.get_by_label("Sort: opened").click_accesskit();
    harness.run();
    assert!(shown_contains(&labels(&harness), "Sort: commit"));

    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert!(
        shown_contains(&labels(&harness), "Sort: dir"),
        "the fourth Shift+Tab wraps back to dir"
    );
    assert!(
        !harness.get_by_role(Role::TextInput).is_focused(),
        "Shift+Tab leaves NAV alone"
    );
}

/// Tab keeps cycling the locations, not the sort
#[test]
fn tab_keeps_cycling_the_locations() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let mut harness = harness_over(app(&fixture));

    harness.key_press(Key::Tab);
    harness.run();

    assert!(shown_contains(&labels(&harness), "Sort: dir"));
    assert_eq!(
        count_of_label(&labels(&harness), "factory-evals"),
        0,
        "Tab narrowed the list to This Mac"
    );
}

/// The opened sort lists the last opened first, then the rest by name
#[test]
fn the_opened_sort_lists_the_last_opened_first() {
    let fixture = Fixture::new();
    fixture.settings(true);
    fs::write(
        fixture.path().join("recent"),
        format!("{}\n", fixture.path().join("Projects/cellworks").display()),
    )
    .expect("write the recent file");
    let mut harness = harness_over(app(&fixture));
    let names = ["cellworks", "The-Factory"];

    assert_eq!(row_order(&harness, &names), ["The-Factory", "cellworks"]);

    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();

    assert_eq!(row_order(&harness, &names), ["cellworks", "The-Factory"]);
}

/// The changed sort lists the most recently modified folder first
#[test]
fn the_changed_sort_lists_the_newest_folder_first() {
    let fixture = Fixture::new();
    fixture.settings(true);
    let now = std::time::SystemTime::now();
    for (name, age) in [("cellworks", 10), ("The-Factory", 5000)] {
        fs::File::open(fixture.path().join("Projects").join(name))
            .and_then(|folder| folder.set_modified(now - std::time::Duration::from_secs(age)))
            .expect("set the folder's modification time");
    }
    let mut harness = harness_over(app(&fixture));
    let names = ["cellworks", "The-Factory"];

    for _ in 0..3 {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        harness.run();
    }

    assert!(shown_contains(&labels(&harness), "Sort: changed"));
    assert_eq!(row_order(&harness, &names), ["cellworks", "The-Factory"]);
}

/// A second folder: `Elsewhere/alpha`, added by hand beside `Projects`.
fn with_second_folder(fixture: &Fixture) -> PathBuf {
    let alpha = repository(fixture.path(), "Elsewhere/alpha", &["a.rs"]);
    fixture.settings(true);
    let settings = fixture.path().join("settings.toml");
    let text = fs::read_to_string(&settings).expect("read the settings file");
    fs::write(
        &settings,
        format!("added_folders = [\"{}\"]\n\n{text}", alpha.display()),
    )
    .expect("write the settings file");
    alpha
}

/// In dir sort the local group names each folder once
#[test]
fn dir_sort_names_each_folder_inside_the_local_group() {
    let fixture = Fixture::new();
    with_second_folder(&fixture);
    let mut harness = harness_over(app(&fixture));

    let shown = labels(&harness);
    for folder in ["~/Projects", "~/Elsewhere"] {
        assert_eq!(count_of_label(&shown, folder), 1, "{folder:?} in {shown:?}");
    }
    assert_eq!(
        count_of_label(&shown, "This Mac"),
        2,
        "the chip and one group heading, not one group per folder: {shown:?}"
    );

    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    let shown = labels(&harness);
    assert_eq!(
        count_of_label(&shown, "~/Projects"),
        0,
        "no folder subheadings outside dir sort: {shown:?}"
    );
}

/// A search naming a folder lists the whole folder
#[test]
fn a_folder_search_lists_the_whole_folder() {
    let fixture = Fixture::new();
    with_second_folder(&fixture);
    let mut harness = harness_over(app(&fixture));

    search(&mut harness, "Projects");

    let shown = labels(&harness);
    for name in ["cellworks", "The-Factory"] {
        assert!(count_of_label(&shown, name) >= 1, "{name} in {shown:?}");
    }
    assert_eq!(count_of_label(&shown, "alpha"), 0, "{shown:?}");
}
