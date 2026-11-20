//! Helpers shared by the acceptance tests of the binary: real folder trees in
//! a temporary directory, a recorded git, and a frame harness around
//! `cellworks::draw`. No helper runs git, reads `$HOME` or opens a window.

// Each test file compiles this module on its own and uses only part of it.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use cellworks::app::CellworksApp;
use cellworks::opened::ProjectState;
use cellworks_code_analysis::{GitListing, GitUnavailable};
use cellworks_map::ProjectMap;
use cellworks_map::ui::DrawnTower;
use cellworks_workspace::{Project, Sources, Universe, WorkspaceEvent};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

// STORY-022's own fixtures (a fake forge and checkout, and the "shop" home
// and head trees a review is opened over) need a little more of Sources'
// public surface than the rest of this module.
use std::collections::BTreeMap;
use std::sync::Mutex;

use cellworks_sources::pull_requests::forge::{Checkout, Forge, ForgeUnavailable, Issue};
use cellworks_sources::pull_requests::{PullRequest, PullRequestState};

/// What git says, recorded: the files it lists, or that it cannot answer.
pub struct Recorded(pub Result<Vec<PathBuf>, GitUnavailable>);

impl Recorded {
    pub fn listing(files: &[&str]) -> Arc<Recorded> {
        Arc::new(Recorded(Ok(files.iter().map(PathBuf::from).collect())))
    }
}

impl GitListing for Recorded {
    fn visible_files(&self, _: &Path) -> Result<Vec<PathBuf>, GitUnavailable> {
        self.0.clone()
    }
}

/// A folder tree for one project, under a temporary directory.
pub struct Tree {
    _directory: tempfile::TempDir,
    pub root: PathBuf,
}

impl Tree {
    pub fn project(name: &str, files: &[&str]) -> Tree {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let root = directory.path().join(name);
        fs::create_dir_all(&root).expect("create the project folder");
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, "").expect("write the file");
        }
        let root = fs::canonicalize(root).expect("the project folder resolves");
        Tree {
            _directory: directory,
            root,
        }
    }

    /// A project whose files hold the given sources.
    pub fn sources(name: &str, files: &[(&str, &str)]) -> Tree {
        let names: Vec<&str> = files.iter().map(|(file, _)| *file).collect();
        let tree = Tree::project(name, &names);
        for (file, source) in files {
            fs::write(tree.root.join(file), source).expect("write the source");
        }
        tree
    }

    pub fn repository(name: &str, files: &[&str]) -> Tree {
        let tree = Tree::project(name, files);
        fs::create_dir(tree.root.join(".git")).expect("create .git");
        tree
    }

    pub fn as_project(&self, district_prefix: Option<&str>) -> Project {
        Project {
            district_prefix: district_prefix.map(str::to_owned),
            ..Project::at(self.root.clone())
        }
    }
}

/// An app whose universe holds `projects`, asking `git` about repositories.
pub fn app(projects: Vec<Project>, git: Arc<dyn GitListing + Send + Sync>) -> CellworksApp {
    let sources = Sources {
        projects_folder: PathBuf::from("/nowhere/Projects"),
        settings_file: PathBuf::from("/nowhere/settings.toml"),
    };
    let (sender, receiver) = mpsc::channel();
    sender
        .send(WorkspaceEvent::ProjectsDiscovered(projects))
        .expect("the app is listening");
    let mut app = CellworksApp::over(Universe::scanning(&sources), receiver).with_git(git);
    app.apply_pending();
    // Tests read files as text; they never start Neovim.
    app.editors = cellworks_editing::Editors::read_only();
    app
}

/// A frame every 1/60 s, so the two clicks of a double-click fall within
/// egui's double-click delay; the harness's default step is a quarter second.
pub fn harness(app: CellworksApp) -> Harness<'static, CellworksApp> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 900.0))
        .with_step_dt(1.0 / 60.0)
        // A camera gliding to a facade asks for frames until it arrives.
        .with_max_steps(240)
        .build_ui_state(|ui, app: &mut CellworksApp| cellworks::draw(ui, app), app);
    harness.run();
    harness
}

/// The user opens the project: a double-click on its entry in the universe.
pub fn double_click(harness: &mut Harness<'_, CellworksApp>, name: &str) {
    let entry = harness.get_by_label(name);
    entry.click();
    entry.click();
    harness.run();
}

/// Lets the map of a new lens, worked out while the towers lift off,
/// arrive.
pub fn await_lens(harness: &mut Harness<'_, CellworksApp>) {
    // A key pressed is read in the next frame.
    harness.step();
    let started = Instant::now();
    while harness
        .state()
        .opened()
        .is_some_and(|opened| opened.is_relensing())
        && started.elapsed() < Duration::from_secs(30)
    {
        std::thread::sleep(Duration::from_millis(1));
        harness.state_mut().apply_pending();
    }
}

/// Polls `apply_pending` until the open project's read (or a re-read) and its
/// lanes/building-features preparation are no longer busy, or `timeout` has
/// passed. Never advances a frame itself; the caller decides how.
fn poll_until_read(harness: &mut Harness<'_, CellworksApp>, timeout: Duration) {
    let started = Instant::now();
    loop {
        harness.state_mut().apply_pending();
        let busy = harness
            .state()
            .opened()
            .is_some_and(|opened| opened.is_reading() || opened.is_preparing());
        if !busy || started.elapsed() > timeout {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Lets the read, or the read again, finish and the map's lanes and
/// building features be worked out, then draws the frame that shows it.
pub fn await_read(harness: &mut Harness<'_, CellworksApp>) {
    poll_until_read(harness, Duration::from_secs(30));
    harness.run();
}

/// Like [`await_read`], but advances only `steps` frames afterward instead of
/// running until the frame goes idle: safe to call while something elsewhere
/// (a review's pulse, STORY-022) may still be requesting continuous
/// repaints, which `Harness::run` would wait forever for.
pub fn await_read_bounded(
    harness: &mut Harness<'_, CellworksApp>,
    timeout: Duration,
    steps: usize,
) {
    poll_until_read(harness, timeout);
    harness.run_steps(steps);
}

/// Opens the project of `tree` and returns the frame that shows its map.
pub fn open(
    tree: &Tree,
    district_prefix: Option<&str>,
    git: Arc<dyn GitListing + Send + Sync>,
) -> Harness<'static, CellworksApp> {
    let project = tree.as_project(district_prefix);
    let name = project.name.clone();
    let mut harness = harness(app(vec![project], git));
    double_click(&mut harness, &name);
    await_read(&mut harness);
    harness
}

/// The breadcrumb above an open project starts with this.
pub const BREADCRUMB: &str = "Universe";

/// The text of every label in the frame, in tree order, except the
/// breadcrumb and the Explorer's own subtree (one "Explorer" accessibility
/// container, `Role::Group`, holding its header, lens chips, filter and
/// rows — not elements of the view `labels()` reports on).
pub fn labels(harness: &Harness<'_, CellworksApp>) -> Vec<String> {
    let root = harness.root();
    let nodes: Vec<_> = std::iter::once(root)
        .chain(root.children_recursive())
        .collect();
    let explorer_subtree: std::collections::HashSet<_> = nodes
        .iter()
        .find(|node| {
            let node = node.accesskit_node();
            node.role() == Role::Group && node.label().as_deref() == Some("Explorer")
        })
        .map(|explorer| {
            std::iter::once(explorer.accesskit_node().id())
                .chain(
                    explorer
                        .children_recursive()
                        .map(|node| node.accesskit_node().id()),
                )
                .collect()
        })
        .unwrap_or_default();
    nodes
        .into_iter()
        .filter(|node| !explorer_subtree.contains(&node.accesskit_node().id()))
        .filter_map(|node| {
            let node = node.accesskit_node();
            match node.role() {
                Role::Label => node.value().map(|text| text.to_string()),
                _ => None,
            }
        })
        .filter(|text| !text.starts_with(BREADCRUMB))
        // The lens caption, the lane legend and the switch banner explain
        // the view; they are not elements of it.
        .filter(|text| !text.starts_with("Lens: ") && !text.starts_with("Lanes: "))
        .filter(|text| {
            !text
                .lines()
                .next()
                .is_some_and(|first| first.ends_with(" LENS"))
        })
        .collect()
}

pub fn map(harness: &Harness<'_, CellworksApp>) -> ProjectMap {
    match harness.state().opened().map(|opened| opened.state()) {
        Some(ProjectState::Mapped(map)) => (**map).clone(),
        other => panic!("expected a mapped project, got {other:?}"),
    }
}

/// Builds the folder tree [`shop_city`] describes, without opening it:
/// "catalog" and "orders". The file "catalog/pricing.rs" holds "discount"
/// (12 lines, its third line indented deeper than its second, its fourth
/// calling "round_price"), "round_price" and "total" (40 lines). The block
/// is a skyline: the taller "total" stands on a row behind the shorter
/// "discount", and is wide enough to reach out behind it; most lines of
/// "total" are short. "catalog/mod.rs" holds only `use` statements.
fn shop_tree_sources(more: &str) -> Tree {
    let pad = |line: &str, width: usize| format!("{line:<width$}// padding", width = width - 10);
    let mut discount = vec![
        "fn discount(price: u32) -> u32 {".to_owned(),
        pad("    let base = price + 0;", 120),
        "        let deeper = base;".to_owned(),
        "    let rounded = round_price(deeper);".to_owned(),
    ];
    discount.extend((1..=7).map(|n| format!("    let a{n} = rounded;")));
    discount.push("}".to_owned());
    let round_price = [
        pad("fn round_price(price: u32) -> u32 {", 60),
        "    price".to_owned(),
        "}".to_owned(),
    ];
    let mut total = vec![
        "fn total(prices: &[u32]) -> u32 {".to_owned(),
        pad("    let sum: u32 = prices.iter().sum();", 160),
    ];
    total.extend((0..37).map(|_| "    0;".to_owned()));
    total.push("}".to_owned());
    let pricing = [discount, round_price.to_vec(), total]
        .iter()
        .map(|lines| lines.join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n");
    Tree::sources(
        "shop",
        &[
            ("catalog/pricing.rs", &format!("{pricing}{more}")),
            ("catalog/mod.rs", "use std::fmt;\nuse std::io;\n"),
            ("orders/order.rs", "fn place() {\n    let order = 1;\n}\n"),
        ],
    )
}

/// The project "shop" as a city: see [`shop_tree_sources`] for its shape.
pub fn shop_city() -> (Tree, Harness<'static, CellworksApp>) {
    shop_city_with("")
}

/// [`shop_city`] with `more` written at the end of "catalog/pricing.rs".
pub fn shop_city_with(more: &str) -> (Tree, Harness<'static, CellworksApp>) {
    let tree = shop_tree_sources(more);
    let harness = open(&tree, None, Recorded::listing(&[]));
    (tree, harness)
}

/// Like [`shop_city`], but "shop" is a real git checkout (a `.git` folder,
/// not just a bare project), so sessions can be placed in its city and a
/// second checkout can be built as one of its worktrees with
/// [`shop_worktree`].
pub fn shop_tree() -> (Tree, Harness<'static, CellworksApp>) {
    let tree = shop_tree_sources("");
    fs::create_dir(tree.root.join(".git")).expect("create .git");
    // A `.git` folder makes `load` trust git over walking the folder (see
    // `cellworks_code_analysis::load`), so the recorded listing must name
    // the same files `shop_tree_sources` writes, or the map comes up empty.
    let git = Recorded::listing(&["catalog/pricing.rs", "catalog/mod.rs", "orders/order.rs"]);
    let harness = open(&tree, None, git);
    (tree, harness)
}

/// A second checkout of the "shop" repository at `tree`, sharing its common
/// dir as a worktree would: a `.git` FILE with `gitdir: <...>` pointing at a
/// private git-dir under `tree.root`'s own `.git/worktrees/<name>`, with a
/// `commondir` file resolving back to `tree.root`'s own `.git`. Returns the
/// new checkout's root folder (not opened as a project - its files are
/// never read by Cellworks, only named by a session's events).
pub fn shop_worktree(tree: &Tree, name: &str, branch: &str) -> PathBuf {
    let private_git_dir = tree.root.join(".git/worktrees").join(name);
    fs::create_dir_all(&private_git_dir).expect("create the worktree's private git dir");
    fs::write(
        private_git_dir.join("HEAD"),
        format!("ref: refs/heads/{branch}\n"),
    )
    .expect("write the worktree's HEAD");
    fs::write(private_git_dir.join("commondir"), "../..\n").expect("write commondir");

    let checkout_root = tree
        .root
        .parent()
        .expect("the tree has a parent folder")
        .join(name);
    fs::create_dir_all(&checkout_root).expect("create the worktree's checkout folder");
    fs::write(
        checkout_root.join(".git"),
        format!("gitdir: {}\n", private_git_dir.display()),
    )
    .expect("write the worktree's .git file");
    checkout_root
}

/// The common dir of `tree`'s "shop" repository, the same value
/// `cellworks_agent_sessions` would compute for it and for any of its
/// worktrees.
pub fn shop_common_dir(tree: &Tree) -> PathBuf {
    fs::canonicalize(tree.root.join(".git")).expect("the .git folder exists")
}

/// The drawn tower at `path`.
pub fn tower<'h>(harness: &'h Harness<'_, CellworksApp>, path: &Path) -> &'h DrawnTower {
    harness
        .state()
        .towers()
        .iter()
        .find(|tower| tower.path == path)
        .unwrap_or_else(|| panic!("no tower {}", path.display()))
}

// --- STORY-022: a pull request opened as a review ----------------------
//
// A second, separate two-crate "shop" (not `shop_city`/`shop_tree_sources`,
// which has no `Cargo.toml` and so no cross-file call resolution): "catalog"
// (`pricing.rs`: "discount", "total") and "orders" (`lib.rs`: "checkout",
// which calls "catalog::discount" and is never itself touched by the pull
// request, so it becomes "impacted" purely by calling a changed tower;
// "ship", touched by nothing, the review's "untouched" control). Modelled on
// `tests/story_015_relationship_lenses.rs`'s two-crate fixture, the only
// pattern in this repo proven to resolve a call across files.

/// The manifest of the "catalog" crate the review fixture's pull request
/// changes.
const REVIEW_CATALOG_MANIFEST: &str = "[package]\nname = \"catalog\"\n";

/// The manifest of the "orders" crate.
const REVIEW_ORDERS_MANIFEST: &str = "[package]\nname = \"orders\"\n";

/// "catalog/src/lib.rs": re-exports "pricing"'s items at the crate root, so
/// "orders"'s `use catalog::discount;` resolves (`CodeAnalysis` indexes a
/// crate's items by their own file's module path — "discount" lives at
/// `catalog::pricing::discount` without this — and follows a `pub use`
/// re-export the same way `cellworks_map`'s own crate root re-exports
/// `graph::ProjectMap` as `cellworks_map::ProjectMap`). Identical at the home
/// and the head: the pull request touches "pricing.rs" only.
const REVIEW_CATALOG_LIB: &str = "\
pub mod pricing;
pub use pricing::discount;
pub use pricing::total;
";

/// `sdlc/critical.yaml` of the review fixture's home project: "Cargo.lock"
/// raises the "dependencies" flag.
const REVIEW_CRITICAL_YAML: &str = "flags:\n  dependencies:\n    - \"cargo.lock\"\n";

/// "catalog/src/pricing.rs" before the pull request: "discount" (lines 1-4)
/// halves a price; "total" (lines 6-14) sums a slice with two placeholder
/// lines (8 and 12) the pull request removes.
const REVIEW_PRICING_HOME: &str = "\
pub fn discount(price: u32) -> u32 {
    let half = price / 2;
    half
}

pub fn total(prices: &[u32]) -> u32 {
    let mut sum = 0;
    let _unused_a = 0;
    for price in prices {
        sum += price;
    }
    let _unused_b = 0;
    sum
}
";

/// "catalog/src/pricing.rs" at the pull request's head: "discount" (now lines
/// 1-6) gained two lines of its own (new lines 3 and 4); "total" (now lines
/// 8-14) lost its two placeholder lines.
const REVIEW_PRICING_HEAD: &str = "\
pub fn discount(price: u32) -> u32 {
    let half = price / 2;
    let doubled = half + half;
    let rounded = doubled / 2;
    half
}

pub fn total(prices: &[u32]) -> u32 {
    let mut sum = 0;
    for price in prices {
        sum += price;
    }
    sum
}
";

/// "orders/src/lib.rs", identical at the home and the head: "checkout" calls
/// "catalog::discount", so it is impacted once the pull request changes
/// "discount"; "ship" calls nothing the pull request touches, so it stays
/// untouched by the review.
const REVIEW_ORDERS_LIB: &str = "\
use catalog::discount;

pub fn checkout(price: u32) -> u32 {
    discount(price)
}

pub fn ship() -> bool {
    true
}
";

/// The pull request's own unified diff (`git diff --no-color --no-ext-diff
/// -U0 <base>...<head>` shaped, see `crates/sources/src/pull_requests/diff.rs`):
/// two lines added inside "discount"'s own range, two lines removed from
/// "total"'s, and one line added to "Cargo.lock", which stands no tower
/// (+3/-2 over 2 files, matching [`FakeForge`]'s own listed totals).
pub const REVIEW_DIFF: &str = "\
diff --git a/catalog/src/pricing.rs b/catalog/src/pricing.rs
index 1111111..2222222 100644
--- a/catalog/src/pricing.rs
+++ b/catalog/src/pricing.rs
@@ -2,0 +3,2 @@
+    let doubled = half + half;
+    let rounded = doubled / 2;
@@ -8 +9,0 @@
-    let _unused_a = 0;
@@ -12 +12,0 @@
-    let _unused_b = 0;
diff --git a/Cargo.lock b/Cargo.lock
index 3333333..4444444 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -0,0 +1 @@
+pinned-dependency = \"1.0.0\"
";

fn write_review_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
    fs::write(path, contents).expect("write the file");
}

fn write_review_home(root: &Path) {
    write_review_file(root, "catalog/Cargo.toml", REVIEW_CATALOG_MANIFEST);
    write_review_file(root, "catalog/src/lib.rs", REVIEW_CATALOG_LIB);
    write_review_file(root, "catalog/src/pricing.rs", REVIEW_PRICING_HOME);
    write_review_file(root, "orders/Cargo.toml", REVIEW_ORDERS_MANIFEST);
    write_review_file(root, "orders/src/lib.rs", REVIEW_ORDERS_LIB);
    write_review_file(root, "sdlc/critical.yaml", REVIEW_CRITICAL_YAML);
}

fn write_review_head(root: &Path) {
    write_review_file(root, "catalog/Cargo.toml", REVIEW_CATALOG_MANIFEST);
    write_review_file(root, "catalog/src/lib.rs", REVIEW_CATALOG_LIB);
    write_review_file(root, "catalog/src/pricing.rs", REVIEW_PRICING_HEAD);
    write_review_file(root, "orders/Cargo.toml", REVIEW_ORDERS_MANIFEST);
    write_review_file(root, "orders/src/lib.rs", REVIEW_ORDERS_LIB);
}

/// The "shop" project as it stands before pull request "#41": two crates,
/// "catalog" ("discount", "total") and "orders" ("checkout", "ship"), with
/// "sdlc/critical.yaml" raising "dependencies" on "Cargo.lock" (STORY-022).
pub fn review_home_tree() -> Tree {
    let tree = Tree::project("shop", &[]);
    write_review_home(&tree.root);
    tree
}

/// The "shop" project at pull request "#41"'s head (STORY-022): "discount"
/// gained two lines, "total" lost two; "orders/src/lib.rs" is unchanged. Used
/// directly by a test that wants the head on its own, and by [`FakeCheckout`]
/// to build the worktree it reports.
pub fn review_head_tree() -> Tree {
    let tree = Tree::project("shop", &[]);
    write_review_head(&tree.root);
    tree
}

/// A [`Forge`] double for STORY-022: one open pull request ("#41",
/// "STORY-012", changing "catalog/src/pricing.rs" and "Cargo.lock"), one
/// merged pull request with no story, and the GitHub issue the review's story
/// card reads through `story_issues`. Touches no network, runs no `gh`.
///
/// `Forge::story_issues` does not exist yet - STORY-022 (design.md
/// requirement 9) adds it, to find the issue whose title starts with the
/// story's id. This fake pins the shape this test needs it to have: `fn
/// story_issues(&self, repo: &Path, search: &str) -> Result<Vec<forge::Issue>,
/// ForgeUnavailable>`, with `Issue { number, title, body }` the minimum a
/// story's issue is read back from (`gh issue list --json number,title,body`).
pub struct FakeForge {
    open: Vec<PullRequest>,
    merged: Vec<PullRequest>,
}

impl FakeForge {
    /// Pull request "#41" (open, "STORY-012", +3/-2 over "catalog/src/pricing.rs"
    /// and "Cargo.lock") and "#9" (merged, no story).
    pub fn new() -> FakeForge {
        FakeForge {
            open: vec![PullRequest {
                number: 41,
                state: PullRequestState::Open,
                title: "STORY-012: Give engineers a price breakdown".to_owned(),
                branch: "feature/44-story-012-price-breakdown".to_owned(),
                head: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b".to_owned(),
                base: "9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2a1f0e".to_owned(),
                author: "xunrise".to_owned(),
                story: None,
                additions: 3,
                deletions: 2,
                changed_files: 2,
                files: vec!["catalog/src/pricing.rs".to_owned(), "Cargo.lock".to_owned()],
                flags: BTreeMap::new(),
            }],
            merged: vec![PullRequest {
                number: 9,
                state: PullRequestState::Merged,
                title: "Promote test to main".to_owned(),
                branch: "chore/promote-test-to-main".to_owned(),
                head: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                base: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                author: "xunrise".to_owned(),
                story: None,
                additions: 1,
                deletions: 1,
                changed_files: 1,
                files: vec!["README.md".to_owned()],
                flags: BTreeMap::new(),
            }],
        }
    }
}

impl Default for FakeForge {
    fn default() -> FakeForge {
        FakeForge::new()
    }
}

impl Forge for FakeForge {
    fn open_pull_requests(&self, _repo: &Path) -> Result<Vec<PullRequest>, ForgeUnavailable> {
        Ok(self.open.clone())
    }

    fn merged_pull_requests(
        &self,
        _repo: &Path,
        _limit: usize,
    ) -> Result<Vec<PullRequest>, ForgeUnavailable> {
        Ok(self.merged.clone())
    }

    fn story_issues(&self, _repo: &Path, _search: &str) -> Result<Vec<Issue>, ForgeUnavailable> {
        Ok(vec![Issue {
            number: 83,
            title: "STORY-012: Give engineers a price breakdown".to_owned(),
            body: "\
Feature: Give engineers a price breakdown
  As a shopper comparing prices
  I want to see the discount and the total together
  So that I know what I am really paying
"
            .to_owned(),
        }])
    }
}

/// A [`Checkout`] double for STORY-022: [`Checkout::diff`] always answers the
/// diff it was built with; [`Checkout::add_worktree`] writes
/// [`review_head_tree`]'s files at `into` directly (no real git worktree, no
/// `.git` folder, so the binary's own mapping of the checkout never runs real
/// git either); [`Checkout::remove_worktree`] records `into` so a test can
/// tell the head was cleaned up. Touches no network, runs no `git`.
pub struct FakeCheckout {
    diff: String,
    removed: Mutex<Vec<PathBuf>>,
}

impl FakeCheckout {
    pub fn new(diff: impl Into<String>) -> FakeCheckout {
        FakeCheckout {
            diff: diff.into(),
            removed: Mutex::new(Vec::new()),
        }
    }

    /// Every worktree folder [`Checkout::remove_worktree`] was asked to
    /// remove, in the order it was asked.
    pub fn removed_worktrees(&self) -> Vec<PathBuf> {
        self.removed
            .lock()
            .expect("the lock is not poisoned")
            .clone()
    }
}

impl Checkout for FakeCheckout {
    fn diff(&self, _repo: &Path, _pull_request: &PullRequest) -> Result<String, ForgeUnavailable> {
        Ok(self.diff.clone())
    }

    fn add_worktree(
        &self,
        _repo: &Path,
        _commit: &str,
        into: &Path,
    ) -> Result<(), ForgeUnavailable> {
        fs::create_dir_all(into).map_err(|error| {
            ForgeUnavailable(format!("could not create {}: {error}", into.display()))
        })?;
        write_review_head(into);
        Ok(())
    }

    fn remove_worktree(&self, _repo: &Path, into: &Path) -> Result<(), ForgeUnavailable> {
        self.removed
            .lock()
            .expect("the lock is not poisoned")
            .push(into.to_path_buf());
        Ok(())
    }
}
