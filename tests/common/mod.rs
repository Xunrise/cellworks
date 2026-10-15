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

/// Lets the read, or the read again, finish and the map's lanes and
/// building features be worked out, then draws the frame that shows it.
pub fn await_read(harness: &mut Harness<'_, CellworksApp>) {
    let started = Instant::now();
    loop {
        harness.state_mut().apply_pending();
        let busy = harness
            .state()
            .opened()
            .is_some_and(|opened| opened.is_reading() || opened.is_preparing());
        if !busy || started.elapsed() > Duration::from_secs(30) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    harness.run();
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
/// breadcrumb.
pub fn labels(harness: &Harness<'_, CellworksApp>) -> Vec<String> {
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

/// The project "shop" as a city: the crates "catalog" and "orders". The
/// file "catalog/pricing.rs" holds "discount" (12 lines, its third line
/// indented deeper than its second, its fourth calling "round_price"),
/// "round_price" and "total" (40 lines). The block is a skyline: the taller
/// "total" stands on a row behind the shorter "discount", and is wide enough
/// to reach out behind it; most lines of "total" are short. "catalog/mod.rs"
/// holds only `use` statements.
pub fn shop_city() -> (Tree, Harness<'static, CellworksApp>) {
    shop_city_with("")
}

/// [`shop_city`] with `more` written at the end of "catalog/pricing.rs".
pub fn shop_city_with(more: &str) -> (Tree, Harness<'static, CellworksApp>) {
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
    let tree = Tree::sources(
        "shop",
        &[
            ("catalog/pricing.rs", &format!("{pricing}{more}")),
            ("catalog/mod.rs", "use std::fmt;\nuse std::io;\n"),
            ("orders/order.rs", "fn place() {\n    let order = 1;\n}\n"),
        ],
    );
    let harness = open(&tree, None, Recorded::listing(&[]));
    (tree, harness)
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
